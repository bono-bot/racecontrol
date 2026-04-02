//! Venue State — ping-based venue open/closed detection.
//!
//! Replaces all hardcoded `is_peak_hours()` / `hour >= 16 && hour < 22` checks
//! with a single reachability-based signal: venue is open if the server (.23)
//! or James (.27) is reachable via HTTP.
//!
//! Background task pings both every 60s and updates an AtomicBool.
//! If both are unreachable for 5+ consecutive checks (5 min), venue is closed.
//!
//! Rule from Uday: "If James or .23 is on and pinging, venue is open."

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

const LOG_TARGET: &str = "venue-state";

/// Probe interval: check every 60 seconds.
const PROBE_INTERVAL_SECS: u64 = 60;

/// Consecutive unreachable count before declaring venue closed.
/// 5 checks × 60s = 5 minutes of both hosts unreachable.
const CLOSED_THRESHOLD: u32 = 5;

/// HTTP timeout for each probe (fast fail).
const PROBE_TIMEOUT_SECS: u64 = 3;

/// Server health endpoint (racecontrol on .23).
/// Since this module runs ON the server, we probe localhost.
const SERVER_PROBE_URL: &str = "http://127.0.0.1:8080/api/v1/health";

/// James machine probe (on-site workstation .27).
const JAMES_PROBE_URL: &str = "http://192.168.31.27:8090/health";

/// Current venue state: true = open, false = closed.
static VENUE_OPEN: AtomicBool = AtomicBool::new(true); // Default open (server is starting = open)

/// Consecutive unreachable count (both hosts down).
static UNREACHABLE_COUNT: AtomicU32 = AtomicU32::new(0);

/// Is the venue currently open?
/// Safe to call from any context (sync or async) — reads an atomic.
pub fn venue_is_open() -> bool {
    VENUE_OPEN.load(Ordering::Relaxed)
}

/// Spawn the venue state background probe task.
pub fn spawn() {
    tokio::spawn(async move {
        tracing::info!(
            target: LOG_TARGET,
            "Venue state monitor started ({}s interval, closed after {} consecutive failures)",
            PROBE_INTERVAL_SECS, CLOSED_THRESHOLD
        );

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(PROBE_TIMEOUT_SECS))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "Failed to build HTTP client: {}", e);
                return;
            }
        };

        let mut interval = tokio::time::interval(Duration::from_secs(PROBE_INTERVAL_SECS));

        loop {
            interval.tick().await;

            // Probe server (localhost — we ARE the server)
            let server_ok = client.get(SERVER_PROBE_URL).send().await
                .map(|r| r.status().is_success())
                .unwrap_or(false);

            // Probe James (.27) — only if server probe failed (optimization)
            let james_ok = if server_ok {
                true // Server is up = venue is open, skip James probe
            } else {
                client.get(JAMES_PROBE_URL).send().await
                    .map(|r| r.status().is_success())
                    .unwrap_or(false)
            };

            let either_reachable = server_ok || james_ok;

            if either_reachable {
                let prev_count = UNREACHABLE_COUNT.swap(0, Ordering::Relaxed);
                let was_closed = !VENUE_OPEN.swap(true, Ordering::Relaxed);
                if was_closed {
                    tracing::info!(
                        target: LOG_TARGET,
                        "Venue OPENED — {} reachable (was closed for {} checks)",
                        if server_ok { "server" } else { "james" },
                        prev_count
                    );
                }
            } else {
                let count = UNREACHABLE_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
                if count >= CLOSED_THRESHOLD {
                    let was_open = VENUE_OPEN.swap(false, Ordering::Relaxed);
                    if was_open {
                        tracing::warn!(
                            target: LOG_TARGET,
                            "Venue CLOSED — both server and james unreachable for {} consecutive checks ({}s)",
                            count, count as u64 * PROBE_INTERVAL_SECS
                        );
                    }
                } else {
                    tracing::debug!(
                        target: LOG_TARGET,
                        "Both hosts unreachable ({}/{} before closed)",
                        count, CLOSED_THRESHOLD
                    );
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_open() {
        // On startup, venue defaults to open (server is starting)
        assert!(venue_is_open());
    }
}
