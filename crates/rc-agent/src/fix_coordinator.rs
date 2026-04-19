//! GAP-8 Option 8α — process-wide coordination lock for rc-agent auto-fix
//! dispatch.
//!
//! The rc-agent hosts two independent autonomous debugging subsystems that can
//! both decide to apply a destructive fix (taskkill, Stop-Process, service
//! restart) to the same pod at the same time:
//!
//!   - **System A** — `ai_debugger::try_auto_fix()` synchronous crash-event
//!     handler. Fires on crash events in `event_loop.rs`.
//!   - **System B** — `tier_engine::tier1_deterministic()` background poller.
//!     Fires every 5 seconds during anomaly scans.
//!
//! Before this module, a crash during a tier-1 kill-orphans sweep could result
//! in System A killing processes that System B was mid-way through managing
//! (and vice versa), with no coordination and no log trail of the conflict.
//!
//! This module provides a single global `FixCoordinator` with an exclusive
//! token. Both systems must call `try_acquire(who)` before executing any
//! destructive action. The guard is released on drop. A 60-second stale-lock
//! timeout prevents a panic or hang in one system from permanently blocking
//! the other.
//!
//! Autonomous-safe per CGP v4.3: no behaviour change when only one system
//! fires; serialises concurrent fires into sequential execution (losing side
//! skips with a structured WARN log instead of racing).

use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const LOG_TARGET: &str = "rc_agent::fix_coordinator";
const STALE_LOCK_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
struct FixHeld {
    who: String,
    started_at: Instant,
}

#[derive(Debug)]
struct Inner {
    held: Mutex<Option<FixHeld>>,
    stale_timeout: Duration,
}

/// Fix coordinator — cheaply cloneable handle around an `Arc<Inner>`. A single
/// process-wide instance lives in `global()`; tests construct their own with
/// `FixCoordinator::new_with_timeout()` so they don't share state.
#[derive(Debug, Clone)]
pub struct FixCoordinator(Arc<Inner>);

/// RAII guard — releases the lock on drop.
#[must_use = "FixGuard releases on drop; binding to _ drops immediately"]
pub struct FixGuard {
    who: String,
    inner: Arc<Inner>,
}

impl Drop for FixGuard {
    fn drop(&mut self) {
        if let Ok(mut held) = self.inner.held.lock() {
            if let Some(h) = held.as_ref() {
                if h.who == self.who {
                    *held = None;
                }
            }
        }
    }
}

impl FixCoordinator {
    pub fn new_with_timeout(stale_timeout: Duration) -> Self {
        Self(Arc::new(Inner {
            held: Mutex::new(None),
            stale_timeout,
        }))
    }

    /// Try to acquire the fix lock. Returns `Some(guard)` on success.
    /// Returns `None` if another system is actively holding — the caller
    /// should skip its fix and log.
    pub fn try_acquire(&self, who: &str) -> Option<FixGuard> {
        let mut held = match self.0.held.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        match held.as_ref() {
            Some(h) if h.started_at.elapsed() < self.0.stale_timeout => {
                tracing::warn!(
                    target: LOG_TARGET,
                    held_by = %h.who,
                    requester = %who,
                    held_for_ms = h.started_at.elapsed().as_millis() as u64,
                    "FIX-COORD: acquire denied — another fix in progress"
                );
                None
            }
            Some(h) => {
                // Stale: previous holder did not release within timeout.
                // Steal the lock so one crashed system can't permanently block the other.
                tracing::warn!(
                    target: LOG_TARGET,
                    prev_holder = %h.who,
                    requester = %who,
                    held_for_s = h.started_at.elapsed().as_secs(),
                    "FIX-COORD: stealing stale lock"
                );
                *held = Some(FixHeld {
                    who: who.to_string(),
                    started_at: Instant::now(),
                });
                Some(FixGuard {
                    who: who.to_string(),
                    inner: self.0.clone(),
                })
            }
            None => {
                *held = Some(FixHeld {
                    who: who.to_string(),
                    started_at: Instant::now(),
                });
                Some(FixGuard {
                    who: who.to_string(),
                    inner: self.0.clone(),
                })
            }
        }
    }

    /// Test-only helper to inspect current holder.
    #[cfg(test)]
    fn current_holder(&self) -> Option<String> {
        self.0.held.lock().ok().and_then(|h| h.as_ref().map(|v| v.who.clone()))
    }

    /// Test-only helper to plant a held state with a custom `started_at`.
    /// Used to simulate stale-lock conditions without sleeping.
    #[cfg(test)]
    fn plant_held(&self, who: &str, started_at: Instant) {
        if let Ok(mut g) = self.0.held.lock() {
            *g = Some(FixHeld {
                who: who.to_string(),
                started_at,
            });
        }
    }
}

static GLOBAL: OnceLock<FixCoordinator> = OnceLock::new();

/// The process-wide fix coordinator.
pub fn global() -> &'static FixCoordinator {
    GLOBAL.get_or_init(|| FixCoordinator::new_with_timeout(STALE_LOCK_TIMEOUT))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each test constructs its own instance — no shared state between tests,
    // so they can run in parallel safely.
    fn fresh() -> FixCoordinator {
        FixCoordinator::new_with_timeout(Duration::from_secs(60))
    }

    #[test]
    fn test_acquire_when_free() {
        let c = fresh();
        let g = c.try_acquire("system_a");
        assert!(g.is_some(), "acquire should succeed when coordinator is free");
        assert_eq!(c.current_holder().as_deref(), Some("system_a"));
    }

    #[test]
    fn test_second_acquire_blocked_while_held() {
        let c = fresh();
        let _g1 = c.try_acquire("system_a").expect("first acquire");
        let g2 = c.try_acquire("system_b");
        assert!(g2.is_none(), "second acquire should be denied while held");
        // Holder still system_a
        assert_eq!(c.current_holder().as_deref(), Some("system_a"));
    }

    #[test]
    fn test_reacquire_after_drop() {
        let c = fresh();
        {
            let _g1 = c.try_acquire("system_a").expect("first acquire");
        } // g1 drops here
        let g2 = c.try_acquire("system_b");
        assert!(g2.is_some(), "second acquire should succeed after first drops");
        assert_eq!(c.current_holder().as_deref(), Some("system_b"));
    }

    #[test]
    fn test_stale_lock_stealing() {
        let c = fresh();
        // Plant a held state started 120s ago (2x STALE_LOCK_TIMEOUT).
        let fake_past = Instant::now()
            .checked_sub(Duration::from_secs(120))
            .expect("process has been running long enough for Instant math");
        c.plant_held("system_a", fake_past);
        assert_eq!(c.current_holder().as_deref(), Some("system_a"));
        // System B should be allowed to steal.
        let g = c.try_acquire("system_b");
        assert!(g.is_some(), "system_b should steal stale lock");
        assert_eq!(c.current_holder().as_deref(), Some("system_b"));
    }

    #[test]
    fn test_guard_only_releases_own_lock() {
        let c = fresh();
        // Corner case: if A drops after B has stolen the lock, A's drop
        // must NOT release B's held state.
        let g_a = c.try_acquire("system_a").expect("A acquires");
        // Simulate steal by B (without waiting 60s)
        c.plant_held("system_b", Instant::now());
        drop(g_a); // should NOT clear the lock because holder != system_a
        assert_eq!(
            c.current_holder().as_deref(),
            Some("system_b"),
            "A's drop must not release B's stolen lock"
        );
    }

    #[test]
    fn test_global_instance_is_singleton() {
        // Two calls to global() must return the same underlying Arc<Inner>
        // (so callers across the binary coordinate through one lock).
        let a = global().clone();
        let b = global().clone();
        // Acquire through a; try_acquire through b should observe the held state.
        let _g = a.try_acquire("system_a").expect("a acquires on global");
        assert!(
            b.try_acquire("system_b").is_none(),
            "global() must return one shared coordinator"
        );
    }
}
