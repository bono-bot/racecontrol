use std::sync::mpsc;
use std::time::{Duration, Instant};

use rc_common::types::SimType;

const LOG_TARGET: &str = "safe-mode";

/// Protected exe names that trigger safe mode when they start.
/// WRC.exe is included even though there is no SimType::EaWrc variant yet —
/// detection still fires, but exe_to_sim_type returns None for WRC.
pub const PROTECTED_EXE_NAMES: &[&str] = &[
    "F1_25.exe",
    "iRacingSim64DX11.exe",
    "Le Mans Ultimate.exe",
    "AssettoCorsaEVO.exe",
    "AC2-Win64-Shipping.exe",
    "WRC.exe",
];

/// Cooldown duration after a protected game exits before safe mode is lifted.
const COOLDOWN_SECS: u64 = 30;

// ─── SafeMode state machine ───────────────────────────────────────────────────

/// Safe mode state machine.
///
/// Gates risky subsystems (process_guard scan, content_scanner,
/// kiosk keyboard hooks) while a protected game is running or
/// during the 30-second post-exit cooldown window.
///
/// Lives in AppState (not ConnectionState) so it survives WebSocket reconnections.
pub struct SafeMode {
    /// Whether safe mode is currently active (game running OR in cooldown).
    pub active: bool,
    /// Which protected game is currently running. None during cooldown.
    pub game: Option<SimType>,
    /// Instant until which the cooldown lasts. None when not in cooldown.
    pub cooldown_until: Option<Instant>,
}

impl SafeMode {
    /// Create a new SafeMode in the inactive state.
    pub fn new() -> Self {
        Self {
            active: false,
            game: None,
            cooldown_until: None,
        }
    }

    /// Transition to active state for the given game.
    ///
    /// - Sets active=true, game=Some(game)
    /// - Clears any pending cooldown (game takes priority over cooldown)
    pub fn enter(&mut self, game: SimType) {
        tracing::info!(
            target: LOG_TARGET,
            game = %game,
            "Safe mode ENTER — protected game started"
        );
        self.active = true;
        self.game = Some(game);
        self.cooldown_until = None;
    }

    /// Begin the post-exit cooldown window (30s).
    ///
    /// - Keeps active=true
    /// - Clears game to None
    /// - Sets cooldown_until = now + 30s
    ///
    /// Returns the cooldown expiry Instant for the caller to arm a timer.
    pub fn start_cooldown(&mut self) -> Instant {
        let expires = Instant::now() + Duration::from_secs(COOLDOWN_SECS);
        tracing::info!(
            target: LOG_TARGET,
            cooldown_secs = COOLDOWN_SECS,
            "Safe mode COOLDOWN — protected game exited, holding for {}s", COOLDOWN_SECS
        );
        self.active = true;
        self.game = None;
        self.cooldown_until = Some(expires);
        expires
    }

    /// Exit safe mode entirely (cooldown expired or manual override).
    ///
    /// - Sets active=false, game=None, cooldown_until=None
    pub fn exit(&mut self) {
        tracing::info!(target: LOG_TARGET, "Safe mode EXIT — resuming normal operations");
        self.active = false;
        self.game = None;
        self.cooldown_until = None;
    }
}

// ─── Game classification ──────────────────────────────────────────────────────

/// Returns true if this sim has anti-cheat that is incompatible with
/// rc-agent's process scan / keyboard hook techniques.
pub fn is_protected_game(sim: SimType) -> bool {
    matches!(
        sim,
        SimType::F125 | SimType::IRacing | SimType::LeMansUltimate | SimType::AssettoCorsaEvo
    )
}

/// Map a protected exe file name (case-insensitive) to a SimType.
/// Returns None for exe names that are in PROTECTED_EXE_NAMES but have
/// no SimType variant yet (e.g., WRC.exe).
pub fn exe_to_sim_type(exe_name: &str) -> Option<SimType> {
    // Normalise to lower-case for comparison
    let lower = exe_name.to_lowercase();
    match lower.as_str() {
        "f1_25.exe" => Some(SimType::F125),
        "iracsim64dx11.exe" | "iracingsim64dx11.exe" => Some(SimType::IRacing),
        "le mans ultimate.exe" => Some(SimType::LeMansUltimate),
        "assettocorsaevo.exe" | "ac2-win64-shipping.exe" => Some(SimType::AssettoCorsaEvo),
        "wrc.exe" => None, // No SimType::EaWrc variant yet
        _ => None,
    }
}

// ─── WMI process watcher ─────────────────────────────────────────────────────

/// Spawn a background std::thread that runs a PowerShell WMI event subscription,
/// watching for Win32_ProcessStartTrace events matching PROTECTED_EXE_NAMES.
///
/// Returns the receiver end of an mpsc channel. Each message is the exe name
/// (e.g., "F1_25.exe") of a newly-started protected process.
///
/// If PowerShell fails to start, the sender is dropped and the receiver will
/// return Err on try_recv — callers should handle this as a no-op.
pub fn spawn_wmi_watcher() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>();

    std::thread::spawn(move || {
        let ps_script = r#"
$names = @('F1_25.exe','iRacingSim64DX11.exe','Le Mans Ultimate.exe','AssettoCorsaEVO.exe','AC2-Win64-Shipping.exe','WRC.exe')
$query = "SELECT * FROM Win32_ProcessStartTrace"
Register-WmiEvent -Query $query -SourceIdentifier 'RCSafeModeWatch' | Out-Null
while ($true) {
    $event = Wait-Event -SourceIdentifier 'RCSafeModeWatch' -Timeout 5
    if ($event -ne $null) {
        $exe = $event.SourceEventArgs.NewEvent.ProcessName
        if ($names -contains $exe) { Write-Output $exe; [Console]::Out.Flush() }
        Remove-Event -SourceIdentifier 'RCSafeModeWatch'
    }
}
"#;

        let mut cmd = std::process::Command::new("powershell");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let child = cmd
            .args(["-NoProfile", "-NonInteractive", "-Command", ps_script])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();

        let mut child = match child {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(
                    target: LOG_TARGET,
                    "Failed to spawn PowerShell WMI watcher: {} — safe mode will rely on startup scan only",
                    e
                );
                return; // tx dropped here — receiver will get Err
            }
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => {
                tracing::error!(target: LOG_TARGET, "WMI watcher: no stdout pipe");
                return;
            }
        };

        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(exe_name) => {
                    let exe_name = exe_name.trim().to_string();
                    if exe_name.is_empty() {
                        continue;
                    }
                    // Only forward names that are in PROTECTED_EXE_NAMES
                    let is_protected = PROTECTED_EXE_NAMES
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&exe_name));
                    if is_protected {
                        tracing::info!(
                            target: LOG_TARGET,
                            exe = %exe_name,
                            "WMI watcher: protected process started"
                        );
                        if tx.send(exe_name).is_err() {
                            // Receiver dropped — agent shutting down
                            break;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, "WMI watcher: read error: {}", e);
                    break;
                }
            }
        }

        tracing::warn!(target: LOG_TARGET, "WMI watcher exited");
    });

    rx
}

// ─── Startup detection ────────────────────────────────────────────────────────

/// One-time sysinfo scan to detect any already-running protected game at startup.
///
/// Returns the first matching SimType found, or None if no protected game
/// is currently running.
///
/// Guarded with #[cfg(not(test))] — in tests this always returns None so that
/// unit tests do not perform real sysinfo scans.
#[cfg(not(test))]
pub fn detect_running_protected_game() -> Option<SimType> {
    use sysinfo::System;

    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    for (_pid, process) in sys.processes() {
        let pname = process.name().to_string_lossy().to_string();
        for &protected in PROTECTED_EXE_NAMES {
            if pname.eq_ignore_ascii_case(protected) {
                if let Some(sim) = exe_to_sim_type(&pname) {
                    tracing::info!(
                        target: LOG_TARGET,
                        exe = %pname,
                        sim = %sim,
                        "Startup scan: detected running protected game"
                    );
                    return Some(sim);
                }
            }
        }
    }

    None
}

/// Test stub — always returns None so unit tests don't touch sysinfo.
#[cfg(test)]
pub fn detect_running_protected_game() -> Option<SimType> {
    None
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── SafeMode state transitions ────────────────────────────────────────────

    #[test]
    fn new_is_inactive() {
        let sm = SafeMode::new();
        assert!(!sm.active);
        assert!(sm.game.is_none());
        assert!(sm.cooldown_until.is_none());
    }

    #[test]
    fn enter_f125_sets_active() {
        let mut sm = SafeMode::new();
        sm.enter(SimType::F125);
        assert!(sm.active);
        assert_eq!(sm.game, Some(SimType::F125));
        assert!(sm.cooldown_until.is_none());
    }

    #[test]
    fn start_cooldown_keeps_active_clears_game() {
        let mut sm = SafeMode::new();
        sm.enter(SimType::IRacing);
        let expires = sm.start_cooldown();
        assert!(sm.active);
        assert!(sm.game.is_none());
        assert!(sm.cooldown_until.is_some());
        // cooldown_until should be roughly COOLDOWN_SECS from now
        let remaining = expires.saturating_duration_since(Instant::now());
        assert!(remaining.as_secs() <= COOLDOWN_SECS);
        assert!(remaining.as_secs() >= COOLDOWN_SECS - 2);
    }

    #[test]
    fn exit_clears_everything() {
        let mut sm = SafeMode::new();
        sm.enter(SimType::F125);
        sm.exit();
        assert!(!sm.active);
        assert!(sm.game.is_none());
        assert!(sm.cooldown_until.is_none());
    }

    #[test]
    fn enter_during_cooldown_clears_cooldown() {
        let mut sm = SafeMode::new();
        sm.enter(SimType::LeMansUltimate);
        let _ = sm.start_cooldown();
        // While in cooldown, a new protected game starts
        sm.enter(SimType::F125);
        assert!(sm.active);
        assert_eq!(sm.game, Some(SimType::F125));
        // Cooldown should be cleared — game takes priority
        assert!(sm.cooldown_until.is_none());
    }

    // ── is_protected_game ─────────────────────────────────────────────────────

    #[test]
    fn f125_is_protected() {
        assert!(is_protected_game(SimType::F125));
    }

    #[test]
    fn iracing_is_protected() {
        assert!(is_protected_game(SimType::IRacing));
    }

    #[test]
    fn le_mans_ultimate_is_protected() {
        assert!(is_protected_game(SimType::LeMansUltimate));
    }

    #[test]
    fn assetto_corsa_evo_is_protected() {
        assert!(is_protected_game(SimType::AssettoCorsaEvo));
    }

    #[test]
    fn assetto_corsa_is_not_protected() {
        assert!(!is_protected_game(SimType::AssettoCorsa));
    }

    #[test]
    fn forza_is_not_protected() {
        assert!(!is_protected_game(SimType::Forza));
    }

    // ── exe_to_sim_type ───────────────────────────────────────────────────────

    #[test]
    fn f1_25_exe_maps_to_f125() {
        assert_eq!(exe_to_sim_type("F1_25.exe"), Some(SimType::F125));
    }

    #[test]
    fn iracing_exe_maps_to_iracing() {
        assert_eq!(exe_to_sim_type("iRacingSim64DX11.exe"), Some(SimType::IRacing));
    }

    #[test]
    fn le_mans_exe_maps_to_le_mans() {
        assert_eq!(exe_to_sim_type("Le Mans Ultimate.exe"), Some(SimType::LeMansUltimate));
    }

    #[test]
    fn assetto_corsa_evo_exe_maps() {
        assert_eq!(exe_to_sim_type("AssettoCorsaEVO.exe"), Some(SimType::AssettoCorsaEvo));
    }

    #[test]
    fn ac2_win64_maps_to_evo() {
        assert_eq!(exe_to_sim_type("AC2-Win64-Shipping.exe"), Some(SimType::AssettoCorsaEvo));
    }

    #[test]
    fn wrc_exe_returns_none() {
        assert_eq!(exe_to_sim_type("WRC.exe"), None);
    }

    #[test]
    fn unknown_exe_returns_none() {
        assert_eq!(exe_to_sim_type("unknown.exe"), None);
    }

    // ── PROTECTED_EXE_NAMES constant ─────────────────────────────────────────

    #[test]
    fn protected_exe_names_contains_all_expected() {
        assert!(PROTECTED_EXE_NAMES.contains(&"F1_25.exe"));
        assert!(PROTECTED_EXE_NAMES.contains(&"iRacingSim64DX11.exe"));
        assert!(PROTECTED_EXE_NAMES.contains(&"Le Mans Ultimate.exe"));
        assert!(PROTECTED_EXE_NAMES.contains(&"AssettoCorsaEVO.exe"));
        assert!(PROTECTED_EXE_NAMES.contains(&"AC2-Win64-Shipping.exe"));
        assert!(PROTECTED_EXE_NAMES.contains(&"WRC.exe"));
    }

    #[test]
    fn protected_exe_names_has_6_entries() {
        assert_eq!(PROTECTED_EXE_NAMES.len(), 6);
    }

    // ── detect_running_protected_game (test stub) ─────────────────────────────

    #[test]
    fn detect_running_returns_none_in_tests() {
        // In test builds the function always returns None — no real sysinfo scan.
        assert!(detect_running_protected_game().is_none());
    }
}
