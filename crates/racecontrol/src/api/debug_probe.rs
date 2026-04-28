//! Debug probe endpoints — read-only diagnostic checks for staff kiosk debug page.
//!
//! Two surfaces:
//!   - GET /debug/probe/{pod_id}  — aggregate pod state probe (rc-agent /health,
//!     /debug/ws-state, sentinel-file checks via /exec, Session 0/1 detection).
//!     Used by the kiosk debug page for: (a) auto-context-snapshot on incident
//!     submit, (b) post-fix verification, (c) standalone diagnostics.
//!   - GET /debug/halo-catalog    — registry of HALO probes parsed from
//!     comms-link/HALO-CATALOG-*.md. Hardcoded snapshot for v1; future versions
//!     read the catalog files at startup. Most probes are status=PROPOSED with
//!     no live runner — Mesh Intelligence is intended replacement.
//!
//! Auth: both endpoints registered under staff_routes — staff JWT required.
//! No auto-fix surface here; pure observability.

#![allow(unused_imports)]

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

use crate::state::AppState;

// ─── Pod State Probe ───────────────────────────────────────────────────────

/// Aggregated read-only probe of pod runtime state.
/// Each sub-probe is independently fallible; partial failures populate the
/// `errors` array but do not fail the whole response.
#[derive(Serialize, Default)]
pub struct PodStateProbe {
    pub pod_id: String,
    pub pod_number: u32,
    pub pod_ip: Option<String>,
    /// rc-agent build_id (from /health). None if rc-agent unreachable.
    pub build_id: Option<String>,
    /// rc-agent uptime in seconds. None if rc-agent unreachable.
    pub uptime_secs: Option<u64>,
    /// Lock screen state from native debug server (Hidden/PinEntry/ActiveSession/ScreenBlanked/...)
    pub lock_screen_state: Option<String>,
    /// Native lock window alive flag.
    pub native_window_alive: Option<bool>,
    /// Sentinel-file presence map. Keys: maintenance_mode, ota_deploying,
    /// graceful_relaunch, deploy_in_progress, restart_sentinel.
    pub sentinels: serde_json::Map<String, Value>,
    /// rc-agent process Session column (Console=Session 1, Services=Session 0,
    /// or null if probe failed).
    pub rc_agent_session: Option<String>,
    /// ConspitLink process count (1 expected; 0 = down, >1 = multiplication bug).
    pub conspitlink_count: Option<u32>,
    /// Probe-side errors per sub-probe.
    pub errors: Vec<String>,
    /// IST timestamp when probe ran.
    pub probed_at: String,
}

/// GET /api/v1/debug/probe/{pod_id} — aggregate pod state probe.
pub(crate) async fn debug_probe_handler(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
) -> Json<PodStateProbe> {
    let mut out = PodStateProbe {
        pod_id: pod_id.clone(),
        probed_at: chrono::Utc::now().to_rfc3339(),
        ..Default::default()
    };

    // 1. Look up pod from in-memory registry. Drop guard before any await.
    let (pod_number, pod_ip) = {
        let pods = state.pods.read().await;
        match pods.get(&pod_id) {
            Some(p) => (p.number, p.ip_address.clone()),
            None => {
                out.errors.push(format!("Pod {} not registered in fleet", pod_id));
                return Json(out);
            }
        }
    };
    out.pod_number = pod_number;
    out.pod_ip = Some(pod_ip.clone());

    // 2. rc-agent /health (parallel-friendly; no auth needed for /health).
    let health_url = format!("http://{}:8090/health", pod_ip);
    match state.http_client.get(&health_url).timeout(Duration::from_secs(3)).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(v) = resp.json::<Value>().await {
                out.build_id = v.get("build_id").and_then(|x| x.as_str()).map(String::from);
                out.uptime_secs = v.get("uptime_secs").and_then(|x| x.as_u64());
            }
        }
        Ok(resp) => out.errors.push(format!("rc-agent /health returned {}", resp.status())),
        Err(e) => out.errors.push(format!("rc-agent /health unreachable: {}", e)),
    }

    // 3. Native lock screen debug server (port 18924, no auth — internal).
    //    Provides lock_screen_state + native_window_alive.
    let dbg_url = format!("http://{}:18924/", pod_ip);
    match state.http_client.get(&dbg_url).timeout(Duration::from_secs(2)).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(v) = resp.json::<Value>().await {
                out.lock_screen_state = v.get("lock_screen_state").and_then(|x| x.as_str()).map(String::from);
                out.native_window_alive = v.get("native_window_alive").and_then(|x| x.as_u64()).map(|n| n > 0);
            }
        }
        Ok(_) | Err(_) => {
            // Native debug server is best-effort — only running in kiosk session.
            // No error appended; missing fields communicate the gap.
        }
    }

    // 4. Sentinel-file checks via rc-agent /exec.
    //    Single chained `if exist` per sentinel — no parens (CLAUDE.md rule).
    //    Tokens parsed from stdout: `M=1` / `M=0` etc.
    let sentinel_cmd = "if exist C:\\RacingPoint\\MAINTENANCE_MODE echo M=1\
                       & if not exist C:\\RacingPoint\\MAINTENANCE_MODE echo M=0\
                       & if exist C:\\RacingPoint\\OTA_DEPLOYING echo O=1\
                       & if not exist C:\\RacingPoint\\OTA_DEPLOYING echo O=0\
                       & if exist C:\\RacingPoint\\GRACEFUL_RELAUNCH echo G=1\
                       & if not exist C:\\RacingPoint\\GRACEFUL_RELAUNCH echo G=0\
                       & if exist C:\\RacingPoint\\DEPLOY_IN_PROGRESS echo D=1\
                       & if not exist C:\\RacingPoint\\DEPLOY_IN_PROGRESS echo D=0\
                       & if exist C:\\RacingPoint\\rcagent-restart-sentinel.txt echo R=1\
                       & if not exist C:\\RacingPoint\\rcagent-restart-sentinel.txt echo R=0";
    let exec_url = format!("http://{}:8090/exec", pod_ip);
    let exec_body = json!({ "cmd": sentinel_cmd, "timeout_ms": 3000 });
    let mut sent_map = serde_json::Map::new();
    let mut req = state.http_client.post(&exec_url).timeout(Duration::from_secs(5)).json(&exec_body);
    if let Some(key) = &state.config.pods.sentry_service_key {
        req = req.header("X-Service-Key", key.as_str());
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(v) = resp.json::<Value>().await {
                let stdout = v.get("stdout").and_then(|x| x.as_str()).unwrap_or("");
                for (token, key) in &[
                    ("M=", "maintenance_mode"),
                    ("O=", "ota_deploying"),
                    ("G=", "graceful_relaunch"),
                    ("D=", "deploy_in_progress"),
                    ("R=", "restart_sentinel"),
                ] {
                    let present = stdout.contains(&format!("{}1", token));
                    let absent = stdout.contains(&format!("{}0", token));
                    let val = if present { Value::Bool(true) }
                              else if absent { Value::Bool(false) }
                              else { Value::Null };
                    sent_map.insert(key.to_string(), val);
                }
            }
        }
        Ok(resp) => out.errors.push(format!("sentinel exec returned {}", resp.status())),
        Err(e) => out.errors.push(format!("sentinel exec failed: {}", e)),
    }
    out.sentinels = sent_map;

    // 5. rc-agent Session 0/1 detection via tasklist /V.
    //    Output format: "rc-agent.exe","PID","Session","SessionNum","Mem","Status","User","CPU","Title"
    let tasklist_cmd = "tasklist /V /FO CSV /NH /FI \"IMAGENAME eq rc-agent.exe\"";
    let tlist_body = json!({ "cmd": tasklist_cmd, "timeout_ms": 3000 });
    let mut req = state.http_client.post(&exec_url).timeout(Duration::from_secs(5)).json(&tlist_body);
    if let Some(key) = &state.config.pods.sentry_service_key {
        req = req.header("X-Service-Key", key.as_str());
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(v) = resp.json::<Value>().await {
                let stdout = v.get("stdout").and_then(|x| x.as_str()).unwrap_or("");
                if stdout.contains("Console") {
                    out.rc_agent_session = Some("Console".to_string());
                } else if stdout.contains("Services") {
                    out.rc_agent_session = Some("Services".to_string());
                }
            }
        }
        Ok(_) | Err(_) => { /* best-effort */ }
    }

    // 6. ConspitLink process count.
    let conspit_cmd = "tasklist /FO CSV /NH /FI \"IMAGENAME eq ConspitLink.exe\"";
    let csp_body = json!({ "cmd": conspit_cmd, "timeout_ms": 3000 });
    let mut req = state.http_client.post(&exec_url).timeout(Duration::from_secs(5)).json(&csp_body);
    if let Some(key) = &state.config.pods.sentry_service_key {
        req = req.header("X-Service-Key", key.as_str());
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(v) = resp.json::<Value>().await {
                let stdout = v.get("stdout").and_then(|x| x.as_str()).unwrap_or("");
                let count = stdout.matches("ConspitLink.exe").count() as u32;
                out.conspitlink_count = Some(count);
            }
        }
        Ok(_) | Err(_) => { /* best-effort */ }
    }

    Json(out)
}

// ─── HALO Catalog ──────────────────────────────────────────────────────────

/// HALO probe metadata, snapshotted from comms-link/HALO-CATALOG-*.md.
/// v1: hardcoded registry. v2: parse catalog files at startup.
/// Most probes are status=PROPOSED — Mesh Intelligence is intended replacement
/// before this surface goes LIVE.
#[derive(Serialize, Clone)]
pub struct HaloProbe {
    pub id: &'static str,
    pub name: &'static str,
    pub layer: &'static str,
    pub bug_class: &'static str,
    pub target: &'static str,
    pub invariant: &'static str,
    pub mechanism: &'static str,
    pub status: &'static str,
    pub reference_incident: &'static str,
    pub escalation: &'static str,
    pub sub_pact: &'static str,
}

const HALO_CATALOG: &[HaloProbe] = &[
    // ─── HALO-V (Venue) ────────────────────────────────────────────────────
    HaloProbe {
        id: "HALO-V.1",
        name: "racecontrol restart-storm detector",
        layer: "V",
        bug_class: "silent-rot",
        target: "Server .23 racecontrol Rust binary",
        invariant: "pm2 restart_time should not increase by ≥10 within any 5-min window",
        mechanism: "B",
        status: "PROPOSED",
        reference_incident: "Cloud rc 577 silent restart cycles 2026-04-25 (PACT-002 / PACT-016)",
        escalation: "auto-PACT-file (class restart-storm-mitigation)",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-V.2",
        name: "pod-alive-but-rc-agent-dead",
        layer: "V",
        bug_class: "spawn-not-alive",
        target: "Pod rc-agent (port 8090)",
        invariant: "Pod LAN-ping success → rc-agent /health on :8090 reachable within 5s",
        mechanism: "B",
        status: "PROPOSED",
        reference_incident: "Pod 1 dead 2026-04-25 (PACT-025 PoE H3)",
        escalation: "WhatsApp Uday + auto-PACT-file (known-bug-hotfix)",
        sub_pact: "PACT-20260426-058",
    },
    HaloProbe {
        id: "HALO-V.3",
        name: "MAINTENANCE_MODE sentinel detector",
        layer: "V",
        bug_class: "sentinel-state",
        target: "C:\\RacingPoint\\MAINTENANCE_MODE on each pod + POS",
        invariant: "MAINTENANCE_MODE should never persist >2h without active operator",
        mechanism: "A",
        status: "PROPOSED",
        reference_incident: "MAINTENANCE_MODE silent pod killer ×2",
        escalation: "INBOX (medium), WhatsApp Uday if >4h",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-V.4",
        name: "SERVER-MONITOR threshold drift",
        layer: "V",
        bug_class: "paired-counter",
        target: "comms-link SERVER-MONITOR alerts vs racecontrol /api/v1/health",
        invariant: "SERVER-MONITOR DOWN must correspond to /health unreachable or status=down",
        mechanism: "A",
        status: "PROPOSED",
        reference_incident: "PACT-025 .23 SERVER-MONITOR DOWN ×6 while health=200 status=degraded",
        escalation: "INBOX",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-V.5",
        name: "port TIME_WAIT cycle",
        layer: "V",
        bug_class: "race-window",
        target: "racecontrol-bound ports (8080, 8090, 8099) on Server .23",
        invariant: "Port-bind retry count over 30s window <5 retries",
        mechanism: "B",
        status: "PROPOSED",
        reference_incident: "Cloud rc 577 cycles, PACT-016 R2 orphan-pipeline-binary",
        escalation: "auto-PACT (restart-storm-mitigation)",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-V.6",
        name: "kiosk frontend staleness",
        layer: "V",
        bug_class: "manifest-drift",
        target: "Pod kiosk Next.js app (per pod build_id)",
        invariant: "Each pod's kiosk build_id matches deploy-manifest's current hash",
        mechanism: "A",
        status: "PROPOSED",
        reference_incident: "Kiosk 14 days stale 72 bug fixes undeployed (2026-03-28)",
        escalation: "INBOX + WhatsApp Uday if drift >3 days",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-V.7",
        name: "F1 25 launch-timeout pattern",
        layer: "V",
        bug_class: "silent-rot",
        target: "game_launch_events.event_type='timeout' for F1 25",
        invariant: "No more than 3 launch-timeouts per pod per 24h",
        mechanism: "A",
        status: "PROPOSED",
        reference_incident: "PACT-006 G8 — pod_4 3/66 F1 25 timeouts at ~8s",
        escalation: "INBOX",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-V.8",
        name: "Conspit wheelbase USB HID enumeration",
        layer: "V",
        bug_class: "spawn-not-alive",
        target: "USB HID VID:0x1209 PID:0xFFB0 on each pod",
        invariant: "Device enumerated → billing system reads wheelbase position within 10s",
        mechanism: "B",
        status: "PROPOSED",
        reference_incident: "Conspit dropouts cause silent billing-pause",
        escalation: "WhatsApp Uday (active customer impact)",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-V.9",
        name: "pod ↔ Server .23 WS thrash (24h rolling)",
        layer: "V",
        bug_class: "silent-rot + customer-impact-cascade",
        target: "alert_incidents subsystem='fleet_connectivity' rows",
        invariant: "No pod accumulates >3 ws_disconnect→ws_reconnect cycles per rolling 1h window",
        mechanism: "A",
        status: "PROPOSED",
        reference_incident: "PACT-086 — 4 mass-end batches in 30 min on 2026-04-26",
        escalation: "INBOX + WhatsApp Uday on second occurrence within 24h",
        sub_pact: "PACT-20260426-086",
    },
    // ─── HALO-R (Racecontrol) ──────────────────────────────────────────────
    HaloProbe {
        id: "HALO-R.1",
        name: "rc-agent Session 0/1 drift",
        layer: "R",
        bug_class: "layer-conflation",
        target: "rc-agent.exe Windows session column",
        invariant: "rc-agent must run in Session 1 (Console); Session 0 (Services) breaks GUI",
        mechanism: "A",
        status: "PROPOSED",
        reference_incident: "2026-03-26 schtasks-restart put rc-agent in Session 0; blanking broke fleet-wide",
        escalation: "INBOX + auto-PACT (known-bug-hotfix)",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-R.2",
        name: "edge_process_count vs lock_screen_state mismatch",
        layer: "R",
        bug_class: "paired-counter",
        target: "rc-agent :18924 native debug — edge_process_count vs lock_screen_state",
        invariant: "lock_screen_state=screen_blanked → edge_process_count > 0 (or native_window_alive=true)",
        mechanism: "A",
        status: "PROPOSED",
        reference_incident: "2026-03-26 ALL 8 pods blanking broken; debug endpoint had answer all along",
        escalation: "INBOX",
        sub_pact: "TBD",
    },
    HaloProbe {
        id: "HALO-R.3",
        name: "ConspitLink process multiplication",
        layer: "R",
        bug_class: "silent-rot",
        target: "ConspitLink.exe process count per pod",
        invariant: "exactly 1 ConspitLink instance per pod (singleton via start-rcagent.bat)",
        mechanism: "C",
        status: "PROPOSED",
        reference_incident: "Pod 1 had 11 ConspitLink instances — wheel flickering",
        escalation: "INBOX (auto-fix=taskkill all + restart one)",
        sub_pact: "TBD",
    },
    // ─── HALO-X (Comms) ────────────────────────────────────────────────────
    HaloProbe {
        id: "HALO-X.2",
        name: "exec-relay health",
        layer: "X",
        bug_class: "spawn-not-alive",
        target: "comms-link relay HTTP :8766 + WS bridge",
        invariant: "Relay /health returns ok AND comms.db→WS bridge delivers within 5s",
        mechanism: "B",
        status: "CALIBRATING",
        reference_incident: "10-day message blackout Mar 21-31, 26 messages lost",
        escalation: "WhatsApp Uday",
        sub_pact: "PACT-20260426-059",
    },
    HaloProbe {
        id: "HALO-X.6",
        name: "INBOX handoff rot",
        layer: "X",
        bug_class: "silent-rot",
        target: "comms-link/INBOX.md HANDOFF markers",
        invariant: "HANDOFF without HANDOFF_ACK within 30 min triggers STALE_HANDOFF",
        mechanism: "A",
        status: "CALIBRATING",
        reference_incident: "Cross-machine handoffs lost when partner offline",
        escalation: "INBOX self-flag",
        sub_pact: "PACT-20260426-060",
    },
    // ─── HALO-M (Memory) ───────────────────────────────────────────────────
    HaloProbe {
        id: "HALO-M.1",
        name: "instruction-manifest drift",
        layer: "M",
        bug_class: "manifest-drift",
        target: "James + Bono CLAUDE.md SHA256",
        invariant: "Both manifests match after sync-instructions chain",
        mechanism: "A",
        status: "PROPOSED",
        reference_incident: "5-day comms blackout Mar 22-27 from instruction drift",
        escalation: "INBOX",
        sub_pact: "TBD",
    },
];

#[derive(Serialize)]
pub struct HaloCatalogResponse {
    pub probes: Vec<HaloProbe>,
    pub total: usize,
    pub by_status: serde_json::Map<String, Value>,
    pub note: &'static str,
}

/// GET /api/v1/debug/halo-catalog — list HALO probes by metadata.
/// No execution — pure registry view.
pub(crate) async fn halo_catalog_handler() -> Json<HaloCatalogResponse> {
    let mut by_status = serde_json::Map::new();
    for status in &["PROPOSED", "CALIBRATING", "LIVE", "RETIRED"] {
        let count = HALO_CATALOG.iter().filter(|p| p.status == *status).count();
        by_status.insert(status.to_string(), Value::from(count));
    }

    Json(HaloCatalogResponse {
        probes: HALO_CATALOG.to_vec(),
        total: HALO_CATALOG.len(),
        by_status,
        note: "Catalog snapshot; runners not yet wired. Mesh Intelligence is the intended replacement once venue is stable.",
    })
}
