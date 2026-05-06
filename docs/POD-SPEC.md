# Pod Specification — Declarative Source of Truth

**Purpose.** Single source of truth for what a Racing Point pod must look like. Drift detection compares against this spec; remediation scripts bring pods into compliance with this spec. When this file disagrees with reality, **either the pod is wrong or this file is wrong** — pick one and update the other in the same change.

**Authored.** 2026-05-06 from `fleet-uniformity-audit.ps1` results across pods 1-8.
**Anti-pattern blocked by this doc.** "Manual SSH fix on one pod" — every pod-side change goes through this spec + a remediation script.

---

## Section 1 — Identity (per-pod)

These are the only fields a pod is allowed to differ on:

| Field | Source | Per-pod values |
|---|---|---|
| `pod.number` | rc-agent.toml `[pod] number` | 1–8 |
| `pod.name` | rc-agent.toml `[pod] name` | "Pod 1" … "Pod 8" |
| Static IPv4 (Ethernet) | NIC config | 192.168.31.{89, 33, 28, 88, 86, 87, 38, 91} |
| MAC | hardware | per Network Map (CLAUDE.md) |
| Tailscale IP | Tailscale auto-assigned | per Network Map |
| Hostname | OS | SIM1 … SIM8 |

Everything else MUST be uniform.

---

## Section 2 — Binaries (sha256 + size)

Stored at `C:\RacingPoint\`. Canonical reference: latest fleet deploy in `LOGBOOK.md`.

| File | Required | Canonical sha256 (16-char prefix) | Size (bytes) |
|---|---|---|---|
| `rc-agent.exe` | YES | `793267B003367C6F` | 27,033,600 |
| `rc-sentry.exe` | YES | `8AA1E943D6728422` | 10,974,208 |
| `start-rcagent.bat` | YES | `D59EA5C4DBCF8753` | 2,232 |
| `start-rcsentry.bat` | YES | _drifted: 3 variants on fleet — TBD canonical_ | _TBD_ |
| `rc-agent.toml` | YES | per-pod (games differ) | 1,500–1,900 |
| `rc-sentry.toml` | NO | (none observed on any pod 2026-05-06) | — |

**Forbidden: any `*-prev.exe`, `rc-agent-<hash>.exe`, `rc-sentry-<hash>.exe` older than 7 days.**

Hashes update with every fleet deploy. Update this section AND the LOGBOOK row in the same commit.

---

## Section 3 — rc-agent.toml schema

Sections expected on every pod:

```
[core]
[ai_debugger]
[pod]
[preflight]
[process_guard]
[lock_screen]
[telemetry_ports]
[wheelbase]
```

Game sections (`[games.<game_key>]`) are present **only when the game is installed on that pod**.

### Uniform keys (must match across all pods)

| Section | Key | Canonical value |
|---|---|---|
| `[core]` | `mdns_enabled` | `true` |
| `[core]` | `url` | `"ws://192.168.31.23:8080/ws/agent"` |
| `[ai_debugger]` | `enabled` | `true` |
| `[ai_debugger]` | `ollama_model` | `"qwen2.5:3b"` |
| `[ai_debugger]` | `ollama_url` | `"http://192.168.31.27:11434"` |
| `[ai_debugger]` | `openrouter_api_key` | `<sk-or-v1-…>` (single shared key) |
| `[ai_debugger]` | `openrouter_model` | `"openrouter/auto"` |
| `[pod]` | `node_type` | `"pod"` |
| `[pod]` | `sim` | `"assetto_corsa"` |
| `[pod]` | `sim_ip` | `"127.0.0.1"` |
| `[pod]` | `sim_port` | `9996` |
| `[preflight]` | `enabled` | `false` |
| `[process_guard]` | `enabled` | `false` |
| `[lock_screen]` | `blanking_url` | `"http://192.168.31.23:3300/kiosk/blank"` |
| `[telemetry_ports]` | `ac` | `9996` |
| `[telemetry_ports]` | `f1` | `20777` _(NOTE: CLAUDE.md says 20778 for F1 25 — verify which is canonical)_ |
| `[telemetry_ports]` | `forza` | `5300` |
| `[telemetry_ports]` | `iracing` | `6789` |
| `[telemetry_ports]` | `lemu` | `5555` |
| `[wheelbase]` | `vid` | `4617` (0x1209) |
| `[wheelbase]` | `pid` | `65456` (0xFFB0) |
| `[games.assetto_corsa]` | full block | (see template) |
| `[games.f1_25]` | full block | (see template) |
| `[games.iracing]` | full block | (see template) |

### Per-pod-installed game sections

These sections appear **only when the game is on that pod**:
- `[games.assetto_corsa_evo]` — Steam app `3058630`
- `[games.assetto_corsa_rally]` — Steam app `3917090`
- `[games.le_mans_ultimate]` — Steam app `2399420`
- `[games.forza_horizon_5]` — Steam app `1551360`

### Current per-pod game-install state (audit 2026-05-06 10:21 IST)

| Pod | AC | F1 25 | iRacing | AC Evo | AC Rally | LMU | Forza H5 |
|-----|----|-------|---------|--------|----------|-----|----------|
| 1   | ✓  | ✓     | ✓       | ✓      | ✓        | —   | ✓        |
| 2   | ✓  | ✓     | ✓       | ✓      | —        | ✓   | —        |
| 3   | ✓  | ✓     | ✓       | ✓      | ✓        | —   | —        |
| 4   | ✓  | ✓     | ✓       | ✓      | —        | ✓   | —        |
| 5   | ✓  | ✓     | ✓       | ✓      | ✓        | —   | —        |
| 6   | ✓  | ✓     | ✓       | —      | —        | ✓   | —        |
| 7   | ✓  | ✓     | ✓       | ✓      | ✓        | ✓   | —        |
| 8   | ✓  | ✓     | ✓       | ✓      | ✓        | —   | —        |

**Captain decision (2026-05-06):** per-pod game allocation is INTENTIONAL. Pods do NOT all need all games. The game-install delta is NOT drift; it is by design. The table above is the current allocation, not a target for uniformity. Future game additions/removals are per-pod operations, not fleet-wide deploys.

---

## Section 4 — Services

### Kaizen-disabled (must be Stopped/Disabled on every pod, post broadcast-hygiene-fleet-arc 2026-05-06)
- `Bonjour Service` (allowed: NOT_FOUND if Bonjour was never installed)
- `SSDPSRV`
- `FDResPub`
- `fdPHost`
- `lltdsvc`

### Canonical-running (must be Running/Auto on every pod)
- `RCWatchdog` (Windows service spawning rc-agent in Session 1)
- `Tailscale`
- `Themes`
- `Audiosrv`
- `Schedule`

---

## Section 5 — Scheduled tasks

### Canonical task list (allowed)
- `StartRCAgent` — boot-time launch of rc-agent via start-rcagent.bat (HKLM Run key handles this; the schtask is fallback)
- `StartRCSentry` — boot-time launch of rc-sentry
- `RacingPointSwap` — atomic binary swap during deploy (created by deploy-server.sh / deploy-pod.sh)
- `RC-Update` — periodic update check (verify whether this is canonical or legacy)

### Forbidden / cleanup (delete on remediation)
Names accumulated over time, no longer canonical:
- `StartRCAgentNow`, `StartRCAgentOnLogon`, `TempStartRCAgent` (pod1)
- `RCAgentNow` (pods 3, 5)
- `RC-Test` (pod5 — debug-only)
- `RC-Agent Watchdog`, `RCAgentWatchdog` (pod8 — duplicate watchdog tasks; RCWatchdog Windows service is the canonical mechanism)
- `RacingPoint-PodAgent`, `RacingPoint-RcAgent` (pod8 — pre-RCWatchdog naming)

Plus Windows-system tasks (uniform, not RC-namespace) are out of scope:
- `DXGIAdapterCache`, `ForceSynchronizeTime`, `NcsiIdentifyUserProxies`, `PrinterCleanupTask`, `ReconcileLanguageResources`, `UIEOrchestrator`, `UpdateUserPictureTask`, `UpdateUserPictureTaskContained`, `VerifiedPublisherCertStoreCheck`

---

## Section 6 — Autostart Run keys

### Canonical HKLM Run (every pod)
- `RCAgent` → `C:\RacingPoint\start-rcagent.bat`
- `RCSentry` → `C:\RacingPoint\start-rcsentry.bat`
- `SecurityHealth` → `C:\WINDOWS\system32\SecurityHealthSystray.exe` (Windows default)
- `RtkAudUService` → Realtek audio (driver-version-dependent — variance acceptable)

### Canonical HKCU Run (every pod, for the kiosk user)
- `Steam` → `"C:\Program Files (x86)\Steam\steam.exe" -silent` (Steam needed for game launch)
- `VSD Craft` → wheelbase software (needed for FFB)

### Forbidden HKCU Run (delete on remediation)
- `OneDrive` (pod8) — consumes RAM, conflicts with kiosk
- `Teams` (pod8) — consumes RAM, conflicts with kiosk
- `SGP Sync App` (pods 6, 7) — unknown app, not part of canonical kiosk
- `RPMKickstart` (pods 3, 5, 6) — GIGABYTE Smart Backup, not needed

---

## Section 7 — Network

### Per-pod (Section 1)
- Static IPv4 on Ethernet adapter
- Tailscale interface auto-assigned
- Default route via `192.168.31.1` (ACT venue gateway)

### Required state (every pod)
- Ethernet status: `Up/Up`
- Wi-Fi: ideally **NO ADAPTER** (uninstalled). Acceptable fallback: `Disconnected/Up` IF the pod is not multi-homed (pod-side Wi-Fi never auto-connects). Forbidden: `Up/Up` (multi-homed risk).
- Wintun (Tailscale): exactly **1 active** instance per adapter; **all other Wintun device-instances are ghosts and must be removed**.

### Current Wintun count drift (audit 2026-05-06)
| Pod | Wintun count |
|---|---|
| pod1 | 4 |
| pod2 | 9 |
| pod3 | 9 |
| pod4 | 8 |
| pod5 | 21 |
| pod6 | 3 |
| pod7 | 5 |
| pod8 | 20 |

Target: 1 per pod. Cleanup: `pnputil /enum-devices` + `pnputil /remove-device <id>` for inactive instances.

---

## Section 8 — Sentinel files

The following files at `C:\RacingPoint\` MUST be absent during normal operation:
- `MAINTENANCE_MODE` (set on 3-restart-in-10-min crash storm; blocks rc-agent start)
- `GRACEFUL_RELAUNCH` (transient during rc-agent self-restart)
- `rcagent-restart-sentinel.txt` (transient)
- `DEPLOY_IN_PROGRESS` (transient during deploy)
- `OTA_DEPLOYING` (transient during OTA)

Audit currently shows: all 5 absent on all 8 pods ✓.

---

## Section 9 — Process state

- `rc-agent.exe` running, **Session ID = 1** (Console session — REQUIRED, not Session 0/Services), working set ~95 MB
- `rc-sentry.exe` running, Session ID = 1
- No duplicate processes (audit shows: 1 rc-agent + 1 rc-sentry per pod ✓)

---

## Section 10 — AC install canonical path

- Steam-installed Assetto Corsa at `C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\`
- Content Manager: NOT REQUIRED (audit shows: not installed on any pod ✓)
- Standalone AC: NOT REQUIRED (audit shows: not installed on any pod ✓)

---

## Drift detection

Run `scripts/fleet-uniformity-audit.ps1` (read-only, no admin) on each pod. Aggregate JSON outputs and diff against this spec. Target: zero drift items.

## Remediation

`scripts/pod-uniformity-cleanup.ps1` brings a pod into spec compliance. Use `-DryRun` (default) to preview, `-Apply` to execute. Always test on one pod (pod 8 canary) before fleet-wide.

## Future state — image-based pods

Once all 8 pods comply with this spec, capture a golden disk image. New pod = restore image + override Section 1 identity. Eliminates the entire class of imperative-state drift.
