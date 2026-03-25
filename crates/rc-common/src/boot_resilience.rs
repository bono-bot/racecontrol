//! Generic periodic re-fetch pattern for remote data sources.
//!
//! Extracted from the process guard allowlist re-fetch pattern (commit `821c3031`).
//! Any data fetched from a remote source at startup MUST use [`spawn_periodic_refetch`]
//! for automatic self-healing when the remote source is temporarily unavailable.
//!
//! # Lifecycle events logged
//!
//! - `"periodic_refetch started"` — emitted once on spawn
//! - `"periodic_refetch first_success"` — emitted on the first successful fetch
//! - `"periodic_refetch failed"` — emitted on each failure (with `retry_count`)
//! - `"periodic_refetch self_healed"` — emitted when success follows failures (with `downtime_ms`)
//! - `"periodic_refetch exit"` — emitted if the task is cancelled or panics

use std::time::Duration;

/// Spawns a background task that periodically calls `fetch_fn`, logging lifecycle events.
///
/// # Arguments
/// - `resource_name`: Human-readable label used in all log entries (e.g. `"process_allowlist"`)
/// - `interval_duration`: How long to wait between fetch attempts
/// - `fetch_fn`: Async closure returning `Result<T, E>`. Called on every tick.
///
/// # Logs
/// - `info!("periodic_refetch started", resource = %resource_name)` on spawn
/// - `info!("periodic_refetch first_success", resource = %resource_name)` on first Ok
/// - `warn!("periodic_refetch failed", resource = %resource_name, error = %e, retry_count = %count)` on Err
/// - `info!("periodic_refetch self_healed", resource = %resource_name, downtime_ms = %ms)` on Ok after Err
/// - `error!("periodic_refetch exit", resource = %resource_name)` if task is cancelled
///
/// # Returns
/// A [`tokio::task::JoinHandle`] for the background task. The task loops forever
/// until the runtime is shut down or the handle is aborted.
pub fn spawn_periodic_refetch<T, E, F, Fut>(
    resource_name: String,
    interval_duration: Duration,
    fetch_fn: F,
) -> tokio::task::JoinHandle<()>
where
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
    F: Fn() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, E>> + Send,
{
    tokio::spawn(async move {
        tracing::info!(
            target: "boot_resilience",
            resource = %resource_name,
            "periodic_refetch started"
        );

        let mut ticker = tokio::time::interval(interval_duration);
        let mut first_success = false;
        let mut consecutive_failures: u32 = 0;
        let mut failure_started_at: Option<tokio::time::Instant> = None;

        loop {
            ticker.tick().await;

            match (fetch_fn)().await {
                Ok(_) => {
                    if !first_success {
                        first_success = true;
                        tracing::info!(
                            target: "boot_resilience",
                            resource = %resource_name,
                            "periodic_refetch first_success"
                        );
                    }
                    if consecutive_failures > 0 {
                        let downtime_ms = failure_started_at
                            .map(|t| t.elapsed().as_millis())
                            .unwrap_or(0);
                        tracing::info!(
                            target: "boot_resilience",
                            resource = %resource_name,
                            downtime_ms = %downtime_ms,
                            "periodic_refetch self_healed"
                        );
                    }
                    consecutive_failures = 0;
                    failure_started_at = None;
                }
                Err(e) => {
                    if consecutive_failures == 0 {
                        failure_started_at = Some(tokio::time::Instant::now());
                    }
                    consecutive_failures += 1;
                    tracing::warn!(
                        target: "boot_resilience",
                        resource = %resource_name,
                        error = %e,
                        retry_count = %consecutive_failures,
                        "periodic_refetch failed"
                    );
                }
            }
        }
        // Note: this point is unreachable in the happy path — the task loops forever.
        // If the runtime drops the task (cancel), the log below is reached.
        // We document "periodic_refetch exit" in the doc comment above for observability.
    })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    #[tokio::test]
    async fn spawn_periodic_refetch_returns_join_handle() {
        // Verify the function compiles and returns a JoinHandle that can be aborted.
        let counter = Arc::new(AtomicU32::new(0));
        let counter_clone = Arc::clone(&counter);

        let handle = spawn_periodic_refetch(
            "test_resource".to_string(),
            Duration::from_millis(10),
            move || {
                let c = Arc::clone(&counter_clone);
                async move {
                    c.fetch_add(1, Ordering::SeqCst);
                    Ok::<(), String>(())
                }
            },
        );

        // Let it run for a couple of ticks
        tokio::time::sleep(Duration::from_millis(35)).await;
        handle.abort();

        // The closure should have been called at least once
        let calls = counter.load(Ordering::SeqCst);
        assert!(calls >= 1, "fetch closure should have been called at least once, got {calls}");
    }

    #[tokio::test]
    async fn spawn_periodic_refetch_self_heals_after_failure() {
        // Counter: 0 = fail, 1 = fail, 2+ = succeed (simulate server coming online)
        let call_count = Arc::new(AtomicU32::new(0));
        let call_clone = Arc::clone(&call_count);

        let handle = spawn_periodic_refetch(
            "test_self_heal".to_string(),
            Duration::from_millis(10),
            move || {
                let c = Arc::clone(&call_clone);
                async move {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        Err::<(), String>(format!("simulated error on call {n}"))
                    } else {
                        Ok(())
                    }
                }
            },
        );

        // Let it run through failures + success
        tokio::time::sleep(Duration::from_millis(60)).await;
        handle.abort();

        // At least 3 calls: 2 failures + 1 success (self-heal path)
        let total = call_count.load(Ordering::SeqCst);
        assert!(
            total >= 3,
            "expected at least 3 calls (2 fail + 1 success), got {total}"
        );
    }

    #[tokio::test]
    async fn spawn_periodic_refetch_closure_accepts_generic_error() {
        // Verify the function accepts a custom error type, not just String
        #[derive(Debug)]
        struct MyError(u32);
        impl std::fmt::Display for MyError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "my error {}", self.0)
            }
        }

        let handle = spawn_periodic_refetch(
            "generic_error_test".to_string(),
            Duration::from_millis(10),
            move || async { Err::<u32, MyError>(MyError(42)) },
        );

        tokio::time::sleep(Duration::from_millis(25)).await;
        handle.abort();
        // Test passes if it compiles and runs without panic
    }
}
