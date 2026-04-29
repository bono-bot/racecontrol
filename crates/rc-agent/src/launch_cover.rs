//! Launch-cover state machine — types and pure state transitions.
//!
//! Substrate for PACT-20260429-008 Phase 1
//! (RATIFIED-PROCEED 2026-04-29 11:30 IST, bono DUAL-AGREE-A-CAVEATS Option A polling).
//!
//! Bono caveats encoded:
//!   C1 cadence-tunable: COVER_TOPMOST_REASSERT_INTERVAL_MS as a const, not a callsite literal
//!   C2 max-wait timeout: COVER_MAX_WAIT_MS gates DismissReason::TimeoutFired
//!   C3 mechanism-agnostic: events fire regardless of whether polling thread or external
//!      dispatcher caused the transition; consumers (PACT-006 W.x detector, PACT-019
//!      launch_cover_timeout_dismiss telemetry) read state transitions, not impl details.
//!
//! This commit ships TYPES + TRANSITIONS only — no Win32 calls, no thread spawn, no
//! integration with the live launch path in ws_handler.rs. Behavior is unchanged in
//! production. A follow-up commit (Phase 1.5) wires this into ws_handler.rs:1118 and
//! adds the Win32 polling thread (SetWindowPos HWND_TOPMOST + GetForegroundWindow +
//! GetWindowRect debounce).

use std::time::{Duration, Instant};

/// Cadence for HWND_TOPMOST re-assertion via SetWindowPos. Bono caveat C1 — tunable;
/// baseline measurement on Pod 8 (per bono C1 prerequisite) will lock the value.
/// Start conservative at 250ms; consider 100ms if venue probe shows shorter race window.
pub const COVER_TOPMOST_REASSERT_INTERVAL_MS: u64 = 250;

/// Maximum time to keep the cover up waiting for the game window to confirm fullscreen.
/// Bono caveat C2 — kiosk-safety hard-stop. 60s mirrors typical AC cold-launch P95.
pub const COVER_MAX_WAIT_MS: u64 = 60_000;

/// Debounce window for game-window-fullscreen confirmation. Foreground window must
/// match the expected class AND the monitor rect for this duration before we trust it.
pub const COVER_FULLSCREEN_CONFIRM_DEBOUNCE_MS: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchCoverState {
    NotShown,
    Shown {
        since: Instant,
        observed_fullscreen_since: Option<Instant>,
    },
    DismissReady {
        reason: DismissReason,
        at: Instant,
    },
    Hidden {
        reason: DismissReason,
        at: Instant,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DismissReason {
    GameWindowConfirmed { window_class: String },
    TimeoutFired,
    Cancelled,
}

#[derive(Debug, Clone)]
pub enum LaunchCoverEvent {
    Show,
    /// Topmost re-assert pulse from polling thread; pure observability for telemetry,
    /// does not transition state.
    TopmostAsserted,
    /// Foreground window observation. If `matches_expected` AND `monitor_rect_match`
    /// AND persists for COVER_FULLSCREEN_CONFIRM_DEBOUNCE_MS, transitions to DismissReady.
    ForegroundWindowSeen {
        class: String,
        matches_expected: bool,
        monitor_rect_match: bool,
    },
    TimeoutFired,
    Cancel,
    DismissCompleted,
}

#[derive(Debug, Clone)]
pub struct LaunchCoverContext {
    pub state: LaunchCoverState,
    pub expected_window_class: Option<String>,
    pub launched_at: Option<Instant>,
}

impl LaunchCoverContext {
    pub fn new(expected_window_class: Option<String>) -> Self {
        Self {
            state: LaunchCoverState::NotShown,
            expected_window_class,
            launched_at: None,
        }
    }

    /// Apply an event. Returns Some(reason) when the event causes a transition into
    /// DismissReady (the layer that emits PACT-019 launch_cover_timeout_dismiss
    /// telemetry hooks here). Pure — no I/O, no Win32, no clock injection.
    pub fn on_event(&mut self, event: LaunchCoverEvent) -> Option<DismissReason> {
        let now = Instant::now();
        match event {
            LaunchCoverEvent::Show => {
                if matches!(self.state, LaunchCoverState::NotShown) {
                    self.state = LaunchCoverState::Shown {
                        since: now,
                        observed_fullscreen_since: None,
                    };
                    self.launched_at = Some(now);
                }
                None
            }
            LaunchCoverEvent::TopmostAsserted => None,
            LaunchCoverEvent::ForegroundWindowSeen {
                class,
                matches_expected,
                monitor_rect_match,
            } => self.handle_foreground_seen(class, matches_expected, monitor_rect_match, now),
            LaunchCoverEvent::TimeoutFired => {
                if matches!(self.state, LaunchCoverState::Shown { .. }) {
                    let reason = DismissReason::TimeoutFired;
                    self.state = LaunchCoverState::DismissReady {
                        reason: reason.clone(),
                        at: now,
                    };
                    Some(reason)
                } else {
                    None
                }
            }
            LaunchCoverEvent::Cancel => {
                if !matches!(
                    self.state,
                    LaunchCoverState::Hidden { .. } | LaunchCoverState::NotShown
                ) {
                    let reason = DismissReason::Cancelled;
                    self.state = LaunchCoverState::DismissReady {
                        reason: reason.clone(),
                        at: now,
                    };
                    Some(reason)
                } else {
                    None
                }
            }
            LaunchCoverEvent::DismissCompleted => {
                if let LaunchCoverState::DismissReady { reason, .. } = &self.state {
                    let r = reason.clone();
                    self.state = LaunchCoverState::Hidden { reason: r, at: now };
                }
                None
            }
        }
    }

    fn handle_foreground_seen(
        &mut self,
        class: String,
        matches_expected: bool,
        monitor_rect_match: bool,
        now: Instant,
    ) -> Option<DismissReason> {
        let LaunchCoverState::Shown {
            since,
            observed_fullscreen_since,
        } = self.state.clone()
        else {
            return None;
        };

        let class_match = self
            .expected_window_class
            .as_deref()
            .map(|e| e == class)
            .unwrap_or(matches_expected);

        if class_match && monitor_rect_match {
            let confirmed_since = observed_fullscreen_since.unwrap_or(now);
            let elapsed = now.duration_since(confirmed_since);
            if elapsed >= Duration::from_millis(COVER_FULLSCREEN_CONFIRM_DEBOUNCE_MS) {
                let reason = DismissReason::GameWindowConfirmed {
                    window_class: class,
                };
                self.state = LaunchCoverState::DismissReady {
                    reason: reason.clone(),
                    at: now,
                };
                Some(reason)
            } else {
                self.state = LaunchCoverState::Shown {
                    since,
                    observed_fullscreen_since: Some(confirmed_since),
                };
                None
            }
        } else {
            self.state = LaunchCoverState::Shown {
                since,
                observed_fullscreen_since: None,
            };
            None
        }
    }

    pub fn should_timeout_at(&self, now: Instant) -> bool {
        match self.state {
            LaunchCoverState::Shown { since, .. } => {
                now.duration_since(since) >= Duration::from_millis(COVER_MAX_WAIT_MS)
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_transitions_from_notshown_to_shown() {
        let mut ctx = LaunchCoverContext::new(Some("acs.exe".into()));
        assert!(matches!(ctx.state, LaunchCoverState::NotShown));
        let r = ctx.on_event(LaunchCoverEvent::Show);
        assert!(r.is_none());
        assert!(matches!(ctx.state, LaunchCoverState::Shown { .. }));
        assert!(ctx.launched_at.is_some());
    }

    #[test]
    fn topmost_asserted_does_not_transition_state() {
        let mut ctx = LaunchCoverContext::new(None);
        ctx.on_event(LaunchCoverEvent::Show);
        let prior = ctx.state.clone();
        let r = ctx.on_event(LaunchCoverEvent::TopmostAsserted);
        assert!(r.is_none());
        assert_eq!(prior, ctx.state);
    }

    #[test]
    fn timeout_fires_dismissready_with_timeout_reason() {
        let mut ctx = LaunchCoverContext::new(None);
        ctx.on_event(LaunchCoverEvent::Show);
        let r = ctx.on_event(LaunchCoverEvent::TimeoutFired);
        assert!(matches!(r, Some(DismissReason::TimeoutFired)));
        assert!(matches!(
            ctx.state,
            LaunchCoverState::DismissReady {
                reason: DismissReason::TimeoutFired,
                ..
            }
        ));
    }

    #[test]
    fn cancel_from_shown_transitions_to_dismissready_cancelled() {
        let mut ctx = LaunchCoverContext::new(None);
        ctx.on_event(LaunchCoverEvent::Show);
        let r = ctx.on_event(LaunchCoverEvent::Cancel);
        assert!(matches!(r, Some(DismissReason::Cancelled)));
    }

    #[test]
    fn cancel_from_notshown_is_noop() {
        let mut ctx = LaunchCoverContext::new(None);
        let r = ctx.on_event(LaunchCoverEvent::Cancel);
        assert!(r.is_none());
        assert!(matches!(ctx.state, LaunchCoverState::NotShown));
    }

    #[test]
    fn first_window_match_starts_debounce_does_not_dismiss() {
        let mut ctx = LaunchCoverContext::new(Some("acs.exe".into()));
        ctx.on_event(LaunchCoverEvent::Show);
        let r = ctx.on_event(LaunchCoverEvent::ForegroundWindowSeen {
            class: "acs.exe".into(),
            matches_expected: true,
            monitor_rect_match: true,
        });
        assert!(r.is_none());
        if let LaunchCoverState::Shown {
            observed_fullscreen_since,
            ..
        } = &ctx.state
        {
            assert!(observed_fullscreen_since.is_some());
        } else {
            panic!("expected Shown");
        }
    }

    #[test]
    fn window_class_mismatch_resets_debounce() {
        let mut ctx = LaunchCoverContext::new(Some("acs.exe".into()));
        ctx.on_event(LaunchCoverEvent::Show);
        ctx.on_event(LaunchCoverEvent::ForegroundWindowSeen {
            class: "acs.exe".into(),
            matches_expected: true,
            monitor_rect_match: true,
        });
        ctx.on_event(LaunchCoverEvent::ForegroundWindowSeen {
            class: "explorer.exe".into(),
            matches_expected: false,
            monitor_rect_match: false,
        });
        if let LaunchCoverState::Shown {
            observed_fullscreen_since,
            ..
        } = &ctx.state
        {
            assert!(observed_fullscreen_since.is_none());
        } else {
            panic!("expected Shown");
        }
    }

    #[test]
    fn dismiss_completed_moves_dismissready_to_hidden() {
        let mut ctx = LaunchCoverContext::new(None);
        ctx.on_event(LaunchCoverEvent::Show);
        ctx.on_event(LaunchCoverEvent::TimeoutFired);
        ctx.on_event(LaunchCoverEvent::DismissCompleted);
        assert!(matches!(
            ctx.state,
            LaunchCoverState::Hidden {
                reason: DismissReason::TimeoutFired,
                ..
            }
        ));
    }

    #[test]
    fn timeout_only_fires_from_shown() {
        let mut ctx = LaunchCoverContext::new(None);
        let r = ctx.on_event(LaunchCoverEvent::TimeoutFired);
        assert!(r.is_none(), "TimeoutFired from NotShown is a no-op");
        assert!(matches!(ctx.state, LaunchCoverState::NotShown));
    }

    #[test]
    fn should_timeout_at_respects_max_wait() {
        let mut ctx = LaunchCoverContext::new(None);
        ctx.on_event(LaunchCoverEvent::Show);
        let now = Instant::now();
        let later = now + Duration::from_millis(COVER_MAX_WAIT_MS + 1);
        assert!(ctx.should_timeout_at(later));
        assert!(!ctx.should_timeout_at(now));
    }
}
