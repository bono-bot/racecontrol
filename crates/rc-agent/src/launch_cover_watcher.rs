//! Phase 1.5 — Option B watcher (DORMANT, draft pending PACT-008 AMEND-1 ratify).
//!
//! Per PACT-008 AMEND-1 (PACT-20260429-023, FILED 2026-04-29 14:55 IST commit `af76fc0`
//! comms-link main): bono Option A (HWND_TOPMOST + SetWindowPos polling) conflicts with
//! LAUNCH-FIX-2/3 (`ws_handler.rs:792-799` + `ws_handler.rs:1114-1120`) — keeping cover
//! topmost during launch causes AC to hang at 41MB RAM 0 CPU on exclusive-fullscreen
//! acquisition. Option B-with-timeout uses an EnumWindows watcher that polls for the
//! game's visible top-level window without re-asserting cover topmost — z-order naturally
//! relinquishes when the game's window claims foreground.
//!
//! This module is DORMANT — no integration with `ws_handler.rs:798/1118` yet. Phase 1.6
//! wire-in commit (post-AMEND-1-ratify) replaces the immediate `close_browser()` calls
//! with a `tokio::task::spawn_blocking(watch_for_game_window(...))` that emits
//! `LaunchCoverEvent::ForegroundWindowSeen` (or TimeoutFired) into the
//! `LaunchCoverContext` from `crate::launch_cover`.
//!
//! Reusable contract:
//! - `expected_game_exe(sim)` mirrors `steam_checks::game_exe_for_sim` (kept in sync per
//!   GAME-02 cross-process update rule, see `game_process.rs:207`).
//! - `watch_for_game_window` is synchronous + blocking — call from spawn_blocking.
//! - 60s default deadline matches `crate::launch_cover::COVER_MAX_WAIT_MS`.
//!
//! NOT TESTED: real Win32 polling on multi-monitor pods (Phase 3 venue verify).

use rc_common::types::SimType;
use std::time::{Duration, Instant};

/// Map SimType → executable names that own the game's main top-level window.
/// Mirrors `crate::steam_checks::game_exe_for_sim` — kept in sync per
/// `crate::game_process::all_game_process_names` GAME-02 rule.
/// Watcher matches ANY of the listed exes (game-specific exe takes priority over
/// launcher exes; launcher exes pre-game-window can be filtered post-match).
pub fn expected_game_exe(sim_type: SimType) -> &'static [&'static str] {
    match sim_type {
        SimType::AssettoCorsa => &["acs.exe", "AssettoCorsa.exe"],
        SimType::AssettoCorsaEvo => &["AssettoCorsaEVO.exe", "AssettoCorsa2.exe", "AC2-Win64-Shipping.exe"],
        SimType::AssettoCorsaRally => &["acr.exe"],
        SimType::IRacing => &["iRacingSim64DX11.exe"],
        SimType::F125 => &["F1_25.exe", "F1_2025.exe"],
        SimType::LeMansUltimate => &["LMU.exe", "Le Mans Ultimate.exe"],
        SimType::Forza => &["ForzaMotorsport.exe"],
        SimType::ForzaHorizon5 => &["ForzaHorizon5.exe"],
    }
}

/// Outcome of one watcher invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchOutcome {
    /// Game's visible top-level window detected. `class` is the Win32 class name from
    /// GetClassNameW; `pid` is the process owning that HWND.
    GameWindowVisible {
        class: String,
        pid: u32,
    },
    /// Deadline elapsed without finding a matching visible window.
    Timeout {
        observed_classes: Vec<String>,
    },
    /// Non-Windows build — watcher is a no-op.
    NotApplicable,
}

/// Block until the game's visible top-level window is detected or the deadline fires.
///
/// Polls EnumWindows every `poll_interval_ms` (default 250ms — matches bono caveat C1
/// cadence-tunable for `crate::launch_cover::COVER_TOPMOST_REASSERT_INTERVAL_MS`).
///
/// Match criteria: `IsWindowVisible(hwnd) == TRUE` AND
///   `GetWindowThreadProcessId(hwnd, &mut pid) == launched_pid`
///   AND class name does NOT match our own splash (`RacingPointLockScreen`).
///
/// IMPORTANT: this is blocking. Call from `tokio::task::spawn_blocking`.
#[cfg(windows)]
pub fn watch_for_game_window(
    expected_pid: u32,
    _sim_type: SimType,
    timeout_ms: u64,
    poll_interval_ms: u64,
) -> WatchOutcome {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex;
    use winapi::shared::minwindef::{BOOL, LPARAM, TRUE, FALSE};
    use winapi::shared::windef::HWND;
    use winapi::um::winuser::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible,
    };

    static EXPECTED_PID: AtomicU32 = AtomicU32::new(0);
    static FOUND_PID: AtomicU32 = AtomicU32::new(0);
    static FOUND_CLASS: Mutex<String> = Mutex::new(String::new());
    static OBSERVED: Mutex<Vec<String>> = Mutex::new(Vec::new());

    EXPECTED_PID.store(expected_pid, Ordering::SeqCst);

    unsafe extern "system" fn enum_callback(hwnd: HWND, _lparam: LPARAM) -> BOOL {
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return TRUE;
        }
        let mut class_buf = [0u16; 256];
        let len = unsafe { GetClassNameW(hwnd, class_buf.as_mut_ptr(), 256) };
        if len <= 0 {
            return TRUE;
        }
        let class = String::from_utf16_lossy(&class_buf[..len as usize]);
        // Never match our own splash window
        if class == "RacingPointLockScreen" {
            return TRUE;
        }
        if let Ok(mut g) = OBSERVED.lock() {
            if !g.iter().any(|c| c == &class) {
                g.push(class.clone());
            }
        }
        let mut pid: u32 = 0;
        let _ = unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let expected = EXPECTED_PID.load(Ordering::SeqCst);
        if pid == expected && expected != 0 {
            FOUND_PID.store(pid, Ordering::SeqCst);
            if let Ok(mut g) = FOUND_CLASS.lock() {
                *g = class;
            }
            return FALSE; // stop enumeration
        }
        TRUE
    }

    if let Ok(mut g) = OBSERVED.lock() {
        g.clear();
    }
    if let Ok(mut g) = FOUND_CLASS.lock() {
        g.clear();
    }
    FOUND_PID.store(0, Ordering::SeqCst);

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        unsafe {
            EnumWindows(Some(enum_callback), 0);
        }
        let pid = FOUND_PID.load(Ordering::SeqCst);
        if pid != 0 {
            let class = FOUND_CLASS.lock().map(|g| g.clone()).unwrap_or_default();
            return WatchOutcome::GameWindowVisible { class, pid };
        }
        std::thread::sleep(Duration::from_millis(poll_interval_ms));
    }

    let observed = OBSERVED.lock().map(|g| g.clone()).unwrap_or_default();
    WatchOutcome::Timeout {
        observed_classes: observed,
    }
}

#[cfg(not(windows))]
pub fn watch_for_game_window(
    _expected_pid: u32,
    _sim_type: SimType,
    _timeout_ms: u64,
    _poll_interval_ms: u64,
) -> WatchOutcome {
    WatchOutcome::NotApplicable
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expected_game_exe_returns_acs_for_assetto_corsa() {
        let exes = expected_game_exe(SimType::AssettoCorsa);
        assert!(exes.contains(&"acs.exe"));
    }

    #[test]
    fn expected_game_exe_returns_f125_for_f1_25() {
        let exes = expected_game_exe(SimType::F125);
        assert!(exes.contains(&"F1_25.exe"));
    }

    #[test]
    fn expected_game_exe_returns_evo_for_assetto_corsa_evo() {
        let exes = expected_game_exe(SimType::AssettoCorsaEvo);
        assert!(exes.contains(&"AssettoCorsaEVO.exe"));
    }

    #[test]
    fn expected_game_exe_returns_iracing_for_iracing() {
        let exes = expected_game_exe(SimType::IRacing);
        assert!(exes.contains(&"iRacingSim64DX11.exe"));
    }

    #[test]
    fn expected_game_exe_returns_lmu_for_lemansultimate() {
        let exes = expected_game_exe(SimType::LeMansUltimate);
        assert!(exes.iter().any(|e| e.starts_with("LMU") || e.contains("Le Mans")));
    }

    #[cfg(not(windows))]
    #[test]
    fn watcher_is_noop_on_non_windows() {
        let outcome = watch_for_game_window(1234, SimType::AssettoCorsa, 100, 50);
        assert_eq!(outcome, WatchOutcome::NotApplicable);
    }

    #[test]
    fn watch_outcome_equality() {
        let a = WatchOutcome::Timeout { observed_classes: vec!["a".into()] };
        let b = WatchOutcome::Timeout { observed_classes: vec!["a".into()] };
        assert_eq!(a, b);
        let c = WatchOutcome::GameWindowVisible { class: "x".into(), pid: 1 };
        assert_ne!(a, c);
    }

    #[cfg(windows)]
    #[test]
    fn watcher_returns_timeout_when_pid_never_appears() {
        // Use a pid that's vanishingly unlikely to be running
        let bogus_pid: u32 = 4_294_967_290; // u32::MAX - 5
        let outcome = watch_for_game_window(bogus_pid, SimType::AssettoCorsa, 600, 100);
        match outcome {
            WatchOutcome::Timeout { .. } => {}
            other => panic!("expected Timeout, got {:?}", other),
        }
    }
}
