//! Watches C:\RacingPoint\ for sentinel file changes using notify 8.2.0.
//! Emits AgentMessage::SentinelChange over the agent's WS sender channel.
//!
//! Phase 206: Implements OBS-04 — every sentinel file create/delete produces
//! an observable WS message within 1 second. Critical sentinels (MAINTENANCE_MODE)
//! also emit eprintln! for immediate local visibility before tracing is initialized.

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use tokio::sync::mpsc;

use rc_common::protocol::AgentMessage;

/// The directory to watch for sentinel files.
const WATCH_DIR: &str = r"C:\RacingPoint";

/// Known sentinel file names. Changes to other files in WATCH_DIR are ignored.
const KNOWN_SENTINELS: &[&str] = &[
    "MAINTENANCE_MODE",
    "GRACEFUL_RELAUNCH",
    "OTA_DEPLOYING",
    "rcagent-restart-sentinel.txt",
];

const LOG_TARGET: &str = "sentinel_watcher";

/// Spawn a background OS thread that watches WATCH_DIR for sentinel file changes.
///
/// Sends `AgentMessage::SentinelChange` to `tx` on every create/delete event for
/// a known sentinel file. The channel is the same mpsc channel that drains into
/// the WebSocket sender in main.rs, so the message is forwarded to racecontrol
/// within the next WS flush cycle (typically < 100ms).
///
/// Uses `std::thread::spawn` (not tokio::spawn) because notify's RecommendedWatcher
/// internally uses a sync mpsc and ReadDirectoryChangesW callback — it cannot live
/// in an async context without wrapping.
///
/// # Shutdown
///
/// The thread exits if `tx` is closed (WS channel dropped). This happens when
/// the agent shuts down. No explicit shutdown signal is needed.
pub fn spawn(tx: mpsc::Sender<AgentMessage>, pod_id: String) {
    std::thread::spawn(move || {
        tracing::info!(
            target: LOG_TARGET,
            dir = WATCH_DIR,
            pod = %pod_id,
            "sentinel watcher started"
        );

        // Sync channel for notify events → our processing loop
        let (notify_tx, notify_rx) = std::sync::mpsc::channel();

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => { let _ = notify_tx.send(event); }
                    // Bug #16: Log notify errors instead of silently dropping them
                    Err(e) => { eprintln!("[sentinel_watcher] notify error: {}", e); }
                }
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    error = %e,
                    "failed to create file watcher — sentinel changes will NOT be observable"
                );
                return;
            }
        };

        if let Err(e) = watcher.watch(Path::new(WATCH_DIR), RecursiveMode::NonRecursive) {
            tracing::error!(
                target: LOG_TARGET,
                error = %e,
                dir = WATCH_DIR,
                "failed to watch directory — sentinel changes will NOT be observable"
            );
            return;
        }

        tracing::info!(target: LOG_TARGET, "watching {} for sentinel files", WATCH_DIR);

        // Bug #16: Use recv_timeout so thread doesn't hang forever if watcher dies silently
        const RECV_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

        loop {
            match notify_rx.recv_timeout(RECV_TIMEOUT) {
                Ok(event) => {
                    let action = match event.kind {
                        EventKind::Create(_) => "created",
                        EventKind::Remove(_) => "deleted",
                        _ => continue, // ignore Modify, Access, Other — only care about existence changes
                    };

                    for path in &event.paths {
                        let file_name = match path.file_name().and_then(|n| n.to_str()) {
                            Some(name) => name,
                            None => continue,
                        };

                        if !KNOWN_SENTINELS.contains(&file_name) {
                            continue; // not a sentinel we care about
                        }

                        tracing::warn!(
                            target: "state",
                            sentinel = file_name,
                            action = action,
                            pod = %pod_id,
                            "sentinel file change detected"
                        );

                        // OBS-01: MAINTENANCE_MODE creation emits eprintln! immediately
                        // (pre-tracing visibility + local alert before WS reaches racecontrol).
                        if file_name == "MAINTENANCE_MODE" && action == "created" {
                            eprintln!(
                                "[ALERT] MAINTENANCE_MODE created on pod {} — WhatsApp alert queued via racecontrol. All restarts are now blocked.",
                                pod_id
                            );
                        }

                        // Timestamp in IST (UTC+5:30) per project timezone convention.
                        // IST = UTC+19800s. east_opt accepts i32 seconds in ±86400 range.
                        // Constant value 19800 is always valid — no runtime failure possible.
                        const IST_OFFSET_SECS: i32 = 5 * 3600 + 30 * 60; // 19800
                        let timestamp = match chrono::FixedOffset::east_opt(IST_OFFSET_SECS) {
                            Some(ist) => chrono::Utc::now().with_timezone(&ist).to_rfc3339(),
                            None => chrono::Utc::now().to_rfc3339(), // unreachable in practice
                        };

                        let msg = AgentMessage::SentinelChange {
                            pod_id: pod_id.clone(),
                            file: file_name.to_string(),
                            action: action.to_string(),
                            timestamp,
                        };

                        if tx.blocking_send(msg).is_err() {
                            tracing::error!(
                                target: LOG_TARGET,
                                "WS sender channel closed — stopping sentinel watcher"
                            );
                            return;
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    continue; // Normal — no sentinel changes in the timeout window
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // notify_rx closed — watcher dropped or thread shutting down
                    tracing::error!(target: LOG_TARGET, "notify channel closed — stopping sentinel watcher");
                    return;
                }
            }
        }
    });
}
