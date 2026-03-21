//! Kiosk mode security for gaming PCs.
//!
//! Prevents customers from accessing system files, desktop, taskbar,
//! and other unauthorized applications while using the sim rig.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use sysinfo::System;
use tracing;
use serde_json;

/// How often (seconds) rc-agent polls the server for the dynamic allowlist.
pub const ALLOWLIST_REFRESH_SECS: u64 = 300; // 5 minutes

/// Number of times an unknown process must be seen before rc-bot acts.
/// First N-1 sightings are logged as WARN only.
const WARN_BEFORE_ACTION_COUNT: u32 = 3;

/// How long (seconds) a temporarily-allowed process stays allowed
/// before auto-rejecting if no server response.
const TEMP_ALLOW_TTL_SECS: u64 = 600; // 10 minutes

/// Path to the learned allowlist file (persists across restarts).
const LEARNED_ALLOWLIST_PATH: &str = "C:\\RacingPoint\\learned-allowlist.json";

/// Tracks how many scan cycles each unknown process name has been seen.
fn unknown_sightings() -> &'static Mutex<HashMap<String, u32>> {
    static SIGHTINGS: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();
    SIGHTINGS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Temporarily allowed processes awaiting server approval.
/// Key: lowercase process name, Value: (exe_path, when_added, sighting_count).
fn temp_allowlist() -> &'static Mutex<HashMap<String, TempAllowEntry>> {
    static TEMP: OnceLock<Mutex<HashMap<String, TempAllowEntry>>> = OnceLock::new();
    TEMP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Learned allowlist — processes approved by the server, persisted to disk.
fn learned_allowlist() -> &'static Mutex<HashSet<String>> {
    static LEARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    LEARNED.get_or_init(|| {
        let set = load_learned_allowlist().unwrap_or_default();
        Mutex::new(set)
    })
}

/// Server-fetched allowlist — processes staff added via admin panel, refreshed every 5 minutes.
/// Starts empty; populated by `set_server_allowlist()` called from `allowlist_poll_loop`.
fn server_allowlist() -> &'static Mutex<HashSet<String>> {
    static SERVER: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SERVER.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Update the in-memory server allowlist from names fetched from the racecontrol API.
/// Called by `allowlist_poll_loop` in main.rs every 5 minutes.
pub fn set_server_allowlist(names: Vec<String>) {
    if let Ok(mut set) = server_allowlist().lock() {
        *set = names.into_iter().map(|s| s.to_lowercase()).collect();
    }
}

#[derive(Clone, Debug)]
pub struct TempAllowEntry {
    pub exe_path: String,
    pub added_at: Instant,
    pub sighting_count: u32,
    pub notified: bool,
}

/// Result of a kiosk enforcement scan — tells the caller what actions to take.
#[derive(Default)]
pub struct EnforceResult {
    /// Processes that were temporarily allowed and need server approval.
    pub pending_approvals: Vec<PendingApproval>,
    /// Processes whose TTL expired without approval — kiosk should lock down.
    pub expired_processes: Vec<String>,
    /// Newly-seen unknown processes that need LLM classification before kill action.
    pub pending_classifications: Vec<PendingClassification>,
}

#[derive(Clone, Debug)]
pub struct PendingApproval {
    pub process_name: String,
    pub exe_path: String,
    pub sighting_count: u32,
}

/// A newly-seen unknown process waiting for LLM verdict (ALLOW/BLOCK/ASK).
#[derive(Clone, Debug)]
pub struct PendingClassification {
    pub process_name: String,
    pub exe_path: String,
}

/// LLM verdict for an unknown process.
#[derive(Debug, Clone, PartialEq)]
pub enum ProcessVerdict {
    Allow,
    Block,
    Ask,
}

fn load_learned_allowlist() -> Option<HashSet<String>> {
    let data = std::fs::read_to_string(LEARNED_ALLOWLIST_PATH).ok()?;
    let list: Vec<String> = serde_json::from_str(&data).ok()?;
    Some(list.into_iter().map(|s| s.to_lowercase()).collect())
}

fn save_learned_allowlist(set: &HashSet<String>) {
    let list: Vec<&String> = set.iter().collect();
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(LEARNED_ALLOWLIST_PATH, json);
    }
}

/// Processes that are always allowed to run (case-insensitive basenames).
const ALLOWED_PROCESSES: &[&str] = &[
    // System essentials
    "system",
    "system idle process",
    "svchost.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "lsass.exe",
    "services.exe",
    "smss.exe",
    "dwm.exe",
    "fontdrvhost.exe",
    "sihost.exe",
    "taskhostw.exe",
    "runtimebroker.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "textinputhost.exe",
    "shellexperiencehost.exe",
    "ctfmon.exe",
    "conhost.exe",
    "dllhost.exe",
    "wudfhost.exe",
    "audiodg.exe",
    "searchindexer.exe",
    "securityhealthservice.exe",
    "securityhealthsystray.exe",
    "sgrmbroker.exe",
    "spoolsv.exe",
    "msiexec.exe",
    "registry",
    "memory compression",
    "dashost.exe",
    "wmiprvse.exe",
    "lsaiso.exe",
    "wlms.exe",
    "unsecapp.exe",
    "settingsynchost.exe",
    "backgroundtaskhost.exe",

    // Windows shell
    "explorer.exe",

    // GPU / Display
    "nvcontainer.exe",
    "nvdisplay.container.exe",
    "nvspcaps64.exe",
    "nvidia share.exe",
    "nvidia web helper.exe",
    "nvidia overlay.exe",

    // RaceControl services
    "rc-agent.exe",
    "rc-agent",
    "rc-sentry.exe",
    "rc-sentry",
    "pod-agent.exe",
    "pod-agent",

    // Windows shell tools (used by watchdog, minimize, etc.)
    "cmd.exe",
    "powershell.exe",
    "curl.exe",
    "wscript.exe",
    "cscript.exe",
    "schtasks.exe",
    "tasklist.exe",
    "taskkill.exe",
    "findstr.exe",
    "find.exe",

    // Wheelbase telemetry
    "conspitlink2.0.exe",

    // Ollama (local LLM)
    "ollama.exe",
    "ollama_llama_server.exe",

    // Pod watchdog — hybrid connectivity agent (fallback :9091 + CLOSE_WAIT auto-restart)
    "pod_watchdog.exe",
    "pod_watchdog",

    // Node.js services
    "node.exe",

    // Edge WebView2 (for lock screen UI)
    "msedge.exe",
    "msedgewebview2.exe",

    // Steam and game launchers
    "steam.exe",
    "steamservice.exe",
    "steamwebhelper.exe",
    "steamclient.exe",
    "gameoverlayui.exe",
    "gameoverlayrenderer.exe",

    // Sim racing games
    "acs.exe",                     // Assetto Corsa
    "acserver.exe",                // AC dedicated server
    // NOTE: "content manager.exe" is NOT here — only allowed in employee debug mode
    "assettocorsa2.exe",           // Assetto Corsa Evo
    "ac2-win64-shipping.exe",      // AC Evo (Unreal Engine shipping build)
    "iracing.exe",                 // iRacing
    "iracingservice.exe",
    "iracingsim64dx11.exe",
    "f1_25.exe",                   // F1 25
    "lemansultimate.exe",          // LMU
    "forzahorizon5.exe",           // Forza
    "forzamotorsport.exe",

    // Audio
    "realtek audio console.exe",

    // Networking & remote access
    "networkmanager.exe",
    "tailscaled.exe",              // Tailscale mesh VPN (Phase 27)
    "tailscale-ipn.exe",           // Tailscale GUI
    "ipconfig.exe",                // Used by Tailscale internally
    "netstat.exe",                 // Used by self-monitor

    // Remote desktop (Phase 27)
    "rustdesk.exe",                // RustDesk — remote access when rc-agent is down
    "rustdesk_service.exe",

    // Installer
    "rc-installer.exe",            // Pod installer (Rust)

    // Audio services (Realtek — respawns endlessly if killed)
    "rtkauduservice64.exe",
    "rtkbtmanserv.exe",            // Realtek Bluetooth manager
    "soundkeeper64.exe",           // Prevents audio device sleep

    // Peripherals — respawn endlessly via Windows Service Manager
    "corsairdevicecontrolservice.exe",  // Corsair iCUE (keyboards/mice)

    // GPU drivers (AMD — some pods have AMD GPUs)
    "atieclxx.exe",                // AMD display driver
    "atiesrxx.exe",                // AMD display driver
    "amdfendrsr.exe",              // AMD FidelityFX Super Resolution
    "amdrsserv.exe",               // AMD Radeon Software
    "amdow.exe",                   // AMD overlay
    "amdppkgsvc.exe",              // AMD package service

    // Motherboard services (Gigabyte — all pods)
    "aoruslcdservice.exe",         // AORUS LCD panel
    "easytuneengineservice.exe",   // Gigabyte EasyTune
    "gigabyteupdateservice.exe",   // Gigabyte updater
    "gbt_dl_lib.exe",              // Gigabyte download lib
    "gcc.exe",                     // Gigabyte control center

    // NVIDIA extras
    "nvsphelper64.exe",            // NVIDIA ShadowPlay helper
    "nvrla.exe",                   // NVIDIA telemetry
    "nvfvsdksvc_x64.exe",         // NVIDIA FrameView SDK

    // Performance monitoring
    "fvcontainer.exe",             // FrameView container
    "fvcontainer.system.exe",      // FrameView system
    "presentmon_x64.exe",          // PresentMon frame timing

    // Windows services that respawn if killed
    "gamingservices.exe",
    "gamingservicesnet.exe",
    "shellhost.exe",

    // Windows Defender / Security
    "msmpeng.exe",                 // Antimalware Service Executable
    "nissrv.exe",                  // Network Inspection Service
    "mpdefendercoreservice.exe",   // Defender Core Service
    "smartscreen.exe",             // SmartScreen filter
    "securityhealthhost.exe",      // Security Health Host

    // Windows Update & Maintenance
    "trustedinstaller.exe",        // Windows Module Installer
    "tiworker.exe",                // Windows Update worker
    "mousocoreworker.exe",         // Update orchestrator
    "sppsvc.exe",                  // Software Protection Platform
    "wuauclt.exe",                 // Windows Update client

    // Windows System Services
    "searchprotocolhost.exe",      // Windows Search
    "searchfilterhost.exe",        // Windows Search filter
    "midisrv.exe",                 // MIDI service
    "useroobebroker.exe",          // Windows OOBE broker
    "phoneexperiencehost.exe",     // Phone Link
    "secure system",               // Windows kernel process

    // Windows Shell & UX
    "openconsole.exe",             // Windows Terminal console
    "windowsterminal.exe",         // Windows Terminal
    "widgets.exe",                 // Windows Widgets
    "widgetservice.exe",           // Widgets service
    "applicationframehost.exe",    // UWP app frame host

    // Microsoft Office
    "officeclicktorun.exe",        // Office Click-to-Run
    "sdxhelper.exe",               // Office SDX helper
    "m365copilot.exe",             // Microsoft 365 Copilot

    // Updaters
    "updater.exe",                 // Chrome/app updater
    "onedrivestandaloneupdater.exe", // OneDrive updater
    "microsoftedgeupdate.exe",     // Edge updater

    // Logitech peripherals
    "logi_lamparray_service.exe",  // Logitech LED/lighting

    // Windows Start menu
    "microsoftstartfeedprovider.exe", // Start menu feed

    // Tailscale GUI (tailscaled.exe already listed above)
    "tailscale.exe",

    // Other Windows system processes seen in logs
    "onedrive.exe",                // OneDrive sync
    "copilot.exe",                 // Windows Copilot
    "storedesktopextension.exe",   // Microsoft Store
    "systemsettings.exe",          // Windows Settings (Win+I)

    // Browsers
    "firefox.exe",                 // Mozilla Firefox

    // Racing Point internal
    "rc-sentry.exe",              // Backup remote exec service

    // AMD Radeon Software (all AMD pods)
    "amdrssrcext.exe",             // AMD Radeon Settings extension

    // Adobe Creative Cloud (Pod 6 has CC installed)
    "creative cloud helper.exe",   // Adobe CC helper
    "coresync.exe",                // Adobe CoreSync
    "adobe desktop service.exe",   // Adobe Desktop Service
    "armsvc.exe",                  // Adobe ARM update service

    // Windows system processes (seen on multiple pods)
    "aggregatorhost.exe",          // Windows Aggregator Host
    "crossdeviceresume.exe",       // Windows cross-device resume
    "appactions.exe",              // Windows app actions
    "wmiapsrv.exe",                // WMI Performance Adapter
    "notepad.exe",                 // Notepad (Windows Store version)
    "mqsvc.exe",                   // Message Queuing service
    "widgetboard.exe",             // Windows Widgets board
    "finddevice.exe",              // Find My Device
    "gameinputredistservice.exe",  // Microsoft GameInput

    // Ollama (Pod 8 has local LLM)
    "ollama app.exe",              // Ollama desktop app

    // VSD Craft (sim rig diagnostics — Pods 3,7,8)
    "vsd craft.exe",               // VSD Craft

    // Garage61 (sim management — Pods 3,7)
    "garage61-agent.exe",          // Garage61 agent
    "garage61-launcher.exe",       // Garage61 launcher

    // Pico Connect / Streaming Service (Pod 7 — VR)
    "capture_server.exe",          // Pico Connect capture
    "ps_service_launcher.exe",     // Pico streaming launcher
    "ps_server.exe",               // Pico streaming server

    // NVIDIA extras (Pod 7)
    "nvsmartmaxapp64.exe",         // NVIDIA Smart Max Audio
    "nvsmartmaxapp.exe",           // NVIDIA Smart Max Audio (32-bit)

    // Bluetooth (Pod 7)
    "bluetooth audio keepalive.exe", // BT Audio Keepalive

    // Gigabyte (Pod 3)
    "rpmdaemon.exe",               // Gigabyte Smart Backup

    // VNM Config (Pod 3 — startup item)
    "vnmconfig.exe",               // VNM network config

    // OneDrive sync service (all pods — triggers lockdown if missing)
    "onedrive.sync.service.exe",   // OneDrive background sync

    // Windows Cross-Device (Pod 1)
    "crossdeviceservice.exe",      // Windows cross-device service

    // SSH Agent (Pod 1 — Tailscale/OpenSSH)
    "ssh-agent.exe",               // OpenSSH authentication agent

    // GoPro (Pod 1)
    "gopro webcam.exe",            // GoPro Webcam utility

    // Adobe Creative Cloud (Pod 6 — additional procs)
    "adobeipcbroker.exe",          // Adobe IPC Broker
    "adobeupdateservice.exe",      // Adobe Update Service
    "creative cloud.exe",          // Adobe Creative Cloud main
    "ccxprocess.exe",              // Adobe CC Experience

    // Xbox / Edge Game Assist (Pod 8)
    "edgegameassist.exe",          // Edge Game Assist overlay
    "xboxpcappft.exe",             // Xbox PC app
];

/// Kiosk mode manager.
pub struct KioskManager {
    active: Arc<AtomicBool>,
    debug_mode: bool,
    pub lockdown: Arc<AtomicBool>,
    pub lockdown_reason: Arc<Mutex<String>>,
    allowed_extra: HashSet<String>,
}

impl KioskManager {
    pub fn new() -> Self {
        // Load learned allowlist on startup
        let learned_count = learned_allowlist().lock()
            .map(|l| l.len()).unwrap_or(0);
        if learned_count > 0 {
            tracing::info!("Kiosk: loaded {} learned-allowlist entries", learned_count);
        }

        Self {
            active: Arc::new(AtomicBool::new(false)),
            debug_mode: false,
            lockdown: Arc::new(AtomicBool::new(false)),
            lockdown_reason: Arc::new(Mutex::new(String::new())),
            allowed_extra: HashSet::new(),
        }
    }

    /// Enable kiosk mode — hides taskbar, starts process watchdog.
    pub fn activate(&self) {
        if self.active.load(Ordering::SeqCst) {
            return;
        }
        self.active.store(true, Ordering::SeqCst);
        tracing::info!("Kiosk mode ACTIVATED — blocking unauthorized access");

        #[cfg(windows)]
        {
            hide_taskbar(true);
            install_keyboard_hook();
        }
    }

    /// Disable kiosk mode — restores taskbar and normal access.
    pub fn deactivate(&self) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }
        self.active.store(false, Ordering::SeqCst);
        tracing::info!("Kiosk mode DEACTIVATED — restoring normal access");

        #[cfg(windows)]
        {
            hide_taskbar(false);
            remove_keyboard_hook();
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Add a process name to the allow list (e.g., the current game).
    pub fn allow_process(&mut self, name: &str) {
        self.allowed_extra.insert(name.to_lowercase());
    }

    /// Remove a process from the extra allow list.
    pub fn disallow_process(&mut self, name: &str) {
        self.allowed_extra.remove(&name.to_lowercase());
    }

    /// Enter employee debug mode — allows Content Manager and deactivates kiosk restrictions.
    pub fn enter_debug_mode(&mut self) {
        self.debug_mode = true;
        self.deactivate();
        tracing::info!("Kiosk: EMPLOYEE DEBUG MODE — Content Manager and all apps allowed");
    }

    /// Exit employee debug mode — re-engages kiosk restrictions.
    pub fn exit_debug_mode(&mut self) {
        self.debug_mode = false;
        tracing::info!("Kiosk: exiting debug mode");
    }

    /// Check if in debug mode
    pub fn is_debug_mode(&self) -> bool {
        self.debug_mode
    }

    /// Enter lockdown mode — shows "contact staff" message on lock screen.
    pub fn enter_lockdown(&self, reason: &str) {
        self.lockdown.store(true, Ordering::SeqCst);
        if let Ok(mut r) = self.lockdown_reason.lock() {
            *r = reason.to_string();
        }
        tracing::warn!("Kiosk: LOCKDOWN — {}", reason);
    }

    /// Exit lockdown mode (e.g., after employee PIN entry).
    pub fn exit_lockdown(&self) {
        self.lockdown.store(false, Ordering::SeqCst);
        if let Ok(mut r) = self.lockdown_reason.lock() {
            r.clear();
        }
        tracing::info!("Kiosk: lockdown cleared");
    }

    /// Check if kiosk is in lockdown mode.
    pub fn is_locked_down(&self) -> bool {
        self.lockdown.load(Ordering::SeqCst)
    }

    /// Get the lockdown reason message.
    pub fn lockdown_reason(&self) -> String {
        self.lockdown_reason.lock()
            .map(|r| r.clone())
            .unwrap_or_default()
    }

    /// Server approved a process — move from temp to permanent learned allowlist.
    pub fn approve_process(name: &str) {
        let name_lower = name.to_lowercase();

        // Remove from temp allowlist
        if let Ok(mut temp) = temp_allowlist().lock() {
            temp.remove(&name_lower);
        }

        // Add to learned allowlist and persist
        if let Ok(mut learned) = learned_allowlist().lock() {
            learned.insert(name_lower.clone());
            save_learned_allowlist(&learned);
        }

        tracing::info!("Kiosk: process '{}' APPROVED — added to learned allowlist", name);
    }

    /// Server rejected a process — remove from temp, kill it, trigger lockdown.
    pub fn reject_process(name: &str) -> bool {
        let name_lower = name.to_lowercase();

        // Remove from temp allowlist
        if let Ok(mut temp) = temp_allowlist().lock() {
            temp.remove(&name_lower);
        }

        // Remove from sightings so it gets killed immediately
        if let Ok(mut sightings) = unknown_sightings().lock() {
            sightings.remove(&name_lower);
        }

        tracing::warn!("Kiosk: process '{}' REJECTED — will be killed on next scan", name);
        true // caller should trigger lockdown
    }

    /// Build the full allowed process set (static + dynamic + learned + server + temp).
    pub fn allowed_set_snapshot(&self) -> HashSet<String> {
        let mut set: HashSet<String> = ALLOWED_PROCESSES
            .iter()
            .map(|s| s.to_lowercase())
            .chain(self.allowed_extra.iter().cloned())
            .collect();

        // Add learned allowlist
        if let Ok(learned) = learned_allowlist().lock() {
            set.extend(learned.iter().cloned());
        }

        // Add server-fetched allowlist (staff-managed via admin panel)
        if let Ok(server) = server_allowlist().lock() {
            set.extend(server.iter().cloned());
        }

        // Add temporarily allowed processes (within TTL)
        if let Ok(temp) = temp_allowlist().lock() {
            for (name, entry) in temp.iter() {
                if entry.added_at.elapsed().as_secs() < TEMP_ALLOW_TTL_SECS {
                    set.insert(name.clone());
                }
            }
        }

        set
    }

    /// Check if kiosk enforcement should run (active and not in debug mode).
    pub fn should_enforce(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Scan running processes and enforce the allow list.
    ///
    /// Uses warn-then-allow: unknown processes are logged for the first
    /// WARN_BEFORE_ACTION_COUNT sightings. On reaching the threshold, instead
    /// of killing, the process is temporarily allowed and a notification is
    /// sent to racecontrol for approval. If rejected or TTL expires, the
    /// process is killed and kiosk enters lockdown.
    ///
    /// Returns an `EnforceResult` with pending approvals and expired processes
    /// so the caller can send WebSocket notifications and trigger lockdown.
    ///
    /// WARNING: This calls `sysinfo::refresh_processes()` which blocks for 100-300ms
    /// on Windows. Always call from `tokio::task::spawn_blocking`, never from the
    /// async event loop directly.
    pub fn enforce_process_whitelist_blocking(allowed_set: HashSet<String>) -> EnforceResult {
        let mut result = EnforceResult::default();
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        // Collect names seen this cycle to prune stale entries later
        let mut seen_this_cycle: HashSet<String> = HashSet::new();

        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy().to_lowercase();
            if name.is_empty() {
                continue;
            }
            // Skip if allowed (includes static + dynamic + learned + temp)
            if allowed_set.contains(&name) {
                continue;
            }
            // Skip system PIDs (0, 4)
            if pid.as_u32() <= 4 {
                continue;
            }
            // Allow any process running from Steam folder (games launched by customers)
            let exe_path = process.exe().map(|p| p.to_string_lossy().to_lowercase()).unwrap_or_default();
            if exe_path.contains("\\steam\\") || exe_path.contains("\\steamapps\\") || exe_path.contains("/steam/") {
                continue;
            }

            seen_this_cycle.insert(name.clone());

            // Track sightings
            let count = {
                let mut sightings = unknown_sightings().lock().unwrap_or_else(|e| e.into_inner());
                let count = sightings.entry(name.clone()).or_insert(0);
                *count += 1;
                *count
            };

            if count < WARN_BEFORE_ACTION_COUNT {
                // Watching phase — log but don't act
                tracing::warn!(
                    "Kiosk: unknown process '{}' (PID {}) — seen {}/{} times, watching",
                    name, pid, count, WARN_BEFORE_ACTION_COUNT
                );
            } else if count == WARN_BEFORE_ACTION_COUNT {
                // Threshold reached — temporarily allow and request approval
                let mut temp = temp_allowlist().lock().unwrap_or_else(|e| e.into_inner());
                if !temp.contains_key(&name) {
                    tracing::warn!(
                        "Kiosk: temporarily allowing '{}' (PID {}) — requesting server approval",
                        name, pid
                    );
                    temp.insert(name.clone(), TempAllowEntry {
                        exe_path: exe_path.clone(),
                        added_at: Instant::now(),
                        sighting_count: count,
                        notified: false,
                    });
                    result.pending_approvals.push(PendingApproval {
                        process_name: name.clone(),
                        exe_path: exe_path.clone(),
                        sighting_count: count,
                    });
                    // Queue for LLM classification — async caller fires classify_process
                    result.pending_classifications.push(PendingClassification {
                        process_name: name.clone(),
                        exe_path,
                    });
                }
            }
            // count > threshold: process is in temp_allowlist, so allowed_set contains it
            // (unless TTL expired — handled below)
        }

        // Prune sightings for processes that exited on their own
        if let Ok(mut sightings) = unknown_sightings().lock() {
            sightings.retain(|name, _| seen_this_cycle.contains(name));
        }

        // Check for expired temp allowlist entries (TTL exceeded, no server response)
        if let Ok(mut temp) = temp_allowlist().lock() {
            let expired: Vec<String> = temp.iter()
                .filter(|(_, entry)| entry.added_at.elapsed().as_secs() >= TEMP_ALLOW_TTL_SECS)
                .map(|(name, _)| name.clone())
                .collect();

            for name in &expired {
                tracing::warn!(
                    "Kiosk: temp allowlist for '{}' EXPIRED ({}s) — triggering lockdown",
                    name, TEMP_ALLOW_TTL_SECS
                );
                temp.remove(name);
            }
            result.expired_processes = expired;
        }

        result
    }
}

// ─── LLM Process Classification ────────────────────────────────────────────

/// Classify an unknown process using the local Ollama LLM.
///
/// Returns `ProcessVerdict::Allow` if the process is safe (e.g., Windows system, GPU driver),
/// `ProcessVerdict::Block` if it is clearly malicious or unauthorized,
/// or `ProcessVerdict::Ask` (default on failure/ambiguity) which continues the
/// existing temp-allow + server approval flow.
///
/// This function must be called from an async context (tokio::spawn) — never blocks.
pub async fn classify_process(
    ollama_url: &str,
    ollama_model: &str,
    process_name: &str,
    exe_path: &str,
) -> ProcessVerdict {
    let prompt = format!(
        "You are a Windows process security classifier for a sim racing venue kiosk. \
        Classify this process: name='{}', path='{}'. \
        Rules: Windows system processes (svchost, csrss, lsass, services, smss, wininit, winlogon, dwm, explorer, taskhostw, RuntimeBroker, fontdrvhost, SearchHost, StartMenuExperienceHost, ShellExperienceHost, TextInputHost), \
        GPU drivers (nvidia, amd, radeon, intel), audio services (audiodg, RtkAuduService), \
        Realtek, Gigabyte/AORUS, NVIDIA (nvcontainer, nvidia-smi), AMD, Tailscale, RustDesk, Steam, \
        game launchers, Content Manager = ALLOW. \
        Unknown browser helpers, random updaters, unknown installers = ASK. \
        Keyloggers, screen capture tools, remote access not from allowlist = BLOCK. \
        Reply with exactly one word: ALLOW, BLOCK, or ASK.",
        process_name, exe_path
    );

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("[kiosk-llm] Failed to build HTTP client for '{}': {}", process_name, e);
            return ProcessVerdict::Ask;
        }
    };

    let result = client
        .post(&format!("{}/api/generate", ollama_url))
        .json(&serde_json::json!({
            "model": ollama_model,
            "prompt": prompt,
            "stream": false
        }))
        .send()
        .await;

    match result {
        Ok(resp) => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                let response_text = body["response"]
                    .as_str()
                    .unwrap_or("")
                    .to_uppercase();
                // Extract verdict: BLOCK checked first (most critical), then ALLOW, then ASK
                if response_text.contains("BLOCK") {
                    ProcessVerdict::Block
                } else if response_text.contains("ALLOW") {
                    ProcessVerdict::Allow
                } else if response_text.contains("ASK") {
                    ProcessVerdict::Ask
                } else {
                    tracing::warn!(
                        "[kiosk-llm] Unparseable LLM response for '{}': {}",
                        process_name, response_text
                    );
                    ProcessVerdict::Ask // Default to ASK if unclear — never auto-kill
                }
            } else {
                ProcessVerdict::Ask
            }
        }
        Err(e) => {
            tracing::warn!("[kiosk-llm] Ollama query failed for '{}': {}", process_name, e);
            ProcessVerdict::Ask // Default to ASK on failure — never auto-kill
        }
    }
}

// ─── Windows-specific implementations ──────────────────────────────────────

#[cfg(windows)]
mod windows_impl {
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::ptr;
    use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
    use winapi::shared::windef::HWND;
    use winapi::um::winuser;

    static HOOK_HANDLE: AtomicPtr<winapi::shared::windef::HHOOK__> =
        AtomicPtr::new(ptr::null_mut());

    /// Low-level keyboard hook callback.
    /// Blocks: Win key, Alt+Tab, Alt+F4, Ctrl+Esc, Alt+Esc, F12, Ctrl+Shift+I, Ctrl+Shift+J, Ctrl+L.
    unsafe extern "system" fn keyboard_hook_proc(
        code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let kb = unsafe { &*(l_param as *const winuser::KBDLLHOOKSTRUCT) };
            let vk = kb.vkCode;
            let flags = kb.flags;
            let alt_down = flags & winuser::LLKHF_ALTDOWN != 0;

            // Block Win key (left and right)
            if vk == winuser::VK_LWIN as u32 || vk == winuser::VK_RWIN as u32 {
                return 1;
            }
            // Block Alt+Tab
            if alt_down && vk == winuser::VK_TAB as u32 {
                return 1;
            }
            // Block Alt+F4
            if alt_down && vk == winuser::VK_F4 as u32 {
                return 1;
            }
            // Block Alt+Esc
            if alt_down && vk == winuser::VK_ESCAPE as u32 {
                return 1;
            }
            // Block Ctrl+Esc (start menu)
            if vk == winuser::VK_ESCAPE as u32 {
                let ctrl_down =
                    unsafe { winuser::GetAsyncKeyState(winuser::VK_CONTROL) } < 0;
                if ctrl_down {
                    return 1;
                }
            }
            // Block F12 (DevTools -- defense in depth with browser flag)
            if vk == winuser::VK_F12 as u32 {
                return 1;
            }
            // Block Ctrl+Shift+I (DevTools alternate)
            if vk == 0x49 /* I */ {
                let ctrl = unsafe { winuser::GetAsyncKeyState(winuser::VK_CONTROL) } < 0;
                let shift = unsafe { winuser::GetAsyncKeyState(winuser::VK_SHIFT) } < 0;
                if ctrl && shift {
                    return 1;
                }
            }
            // Block Ctrl+Shift+J (Console)
            if vk == 0x4A /* J */ {
                let ctrl = unsafe { winuser::GetAsyncKeyState(winuser::VK_CONTROL) } < 0;
                let shift = unsafe { winuser::GetAsyncKeyState(winuser::VK_SHIFT) } < 0;
                if ctrl && shift {
                    return 1;
                }
            }
            // Block Ctrl+L (URL bar -- defense in depth)
            if vk == 0x4C /* L */ {
                if unsafe { winuser::GetAsyncKeyState(winuser::VK_CONTROL) } < 0 {
                    return 1;
                }
            }
        }
        unsafe { winuser::CallNextHookEx(ptr::null_mut(), code, w_param, l_param) }
    }

    pub fn install_keyboard_hook() {
        unsafe {
            let hook = winuser::SetWindowsHookExW(
                winuser::WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                ptr::null_mut(),
                0,
            );
            if !hook.is_null() {
                HOOK_HANDLE.store(hook, Ordering::SeqCst);
                tracing::info!("Kiosk: keyboard hook installed (Win/Alt+Tab/Alt+F4 blocked)");
            } else {
                tracing::error!("Kiosk: failed to install keyboard hook");
            }
        }
    }

    pub fn remove_keyboard_hook() {
        let hook = HOOK_HANDLE.swap(ptr::null_mut(), Ordering::SeqCst);
        if !hook.is_null() {
            unsafe {
                winuser::UnhookWindowsHookEx(hook);
            }
            tracing::info!("Kiosk: keyboard hook removed");
        }
    }

    pub fn hide_taskbar(hide: bool) {
        unsafe {
            let taskbar_class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
            let hwnd: HWND = winuser::FindWindowW(taskbar_class.as_ptr(), ptr::null());
            if !hwnd.is_null() {
                let cmd = if hide {
                    winuser::SW_HIDE
                } else {
                    winuser::SW_SHOW
                };
                winuser::ShowWindow(hwnd, cmd);
                tracing::info!(
                    "Kiosk: taskbar {}",
                    if hide { "hidden" } else { "restored" }
                );
            }
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{hide_taskbar, install_keyboard_hook, remove_keyboard_hook};

// ─── Non-Windows stubs ─────────────────────────────────────────────────────

#[cfg(not(windows))]
pub fn hide_taskbar(_hide: bool) {}

#[cfg(not(windows))]
pub fn install_keyboard_hook() {}

#[cfg(not(windows))]
pub fn remove_keyboard_hook() {}
