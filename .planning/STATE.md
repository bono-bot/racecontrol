---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 176-02-PLAN.md
last_updated: "2026-03-24T04:26:25.669Z"
progress:
  total_phases: 137
  completed_phases: 103
  total_plans: 247
  completed_plans: 244
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 176-02-PLAN.md
last_updated: "2026-03-24T04:19:08.086Z"
progress:
  total_phases: 137
  completed_phases: 102
  total_plans: 247
  completed_plans: 243
  percent: 98
---

---
gsd_state_version: 1.0
milestone: v22.0
milestone_name: Feature Management & OTA Pipeline
status: in_progress
stopped_at: "Completed 177-03-PLAN.md"
last_updated: "2026-03-24T10:43:00+05:30"
current_phase: 177
current_phase_name: Server-Side Registry Config Foundation
current_plan: 03
progress:
  [███░░░░░░░] 30%
  completed_phases: 1
  total_plans: 4
  completed_plans: 6
  percent: 30
decisions:
  - "Single-binary-tier policy adopted: per-pod behavioral differences expressed via runtime flag registry, not separate Cargo builds"
  - "Phase ordering: 176 foundation -> 177 server and 178 agent (parallel) -> 179 OTA -> 180 admin UI (parallel with 179 after 177) -> 181 gates"
  - "OTA binary identity uses SHA256 content hash not git commit hash per research pitfall 3"
  - "Config push must NEVER route through fleet exec endpoint -- WebSocket typed ConfigPush only"
  - "Telemetry excluded from Cargo feature gates -- too entangled with billing/game state (SimAdapter trait, event loop 100ms tick, AC billing bypass). Runtime flag only."
  - "Serde #[serde(other)] catch-all added to AgentMessage + CoreToAgentMessage in Phase 176 -- must deploy updated binaries to all pods BEFORE adding new message variants (two-step deploy)"
  - "rc-sentry added to scope -- feature gates: watchdog, tier1-fixes, ai-diagnosis. Gets flags via local config from rc-agent (no WS to server)."
  - "176-01: serde adjacently-tagged + #[serde(other)] only discards content when data is null; non-null map data with unknown type requires custom deserializer (deferred)"
  - "176-03: single-binary-tier policy documented as CLAUDE.md standing rule; --no-default-features is CI-only, never deployed to pods"
  - "177-01: FeatureFlagRow declared in flags.rs imported into state.rs; circular module dependency within same Rust crate is valid"
  - "177-01: FlagSync version = max(row.version) across all flags in cache; update_flag reads old state from RwLock cache for audit old_value"
  - "177-03: FeatureFlag.overrides uses Record<string,boolean> matching Rust HashMap<String,bool> -- no nested objects"
  - "177-03: ValidationErrors schema added for CP-06 validation error response shape"
  - "177-03: ConfigPush.acked_at is optional (?) in TypeScript and nullable in OpenAPI -- absent for pending/delivered entries"
blockers: []
---

---
gsd_state_version: 1.0
milestone: v21.0
milestone_name: Health Monitoring & Unified Deploy
status: in_progress
stopped_at: Completed 174-05-PLAN.md (REPO-04/REPO-05 deferred — server offline)
last_updated: "2026-03-23T22:19:00+05:30"
current_phase: 174
current_phase_name: health-monitoring-unified-deploy
current_plan: 05
progress:
  completed_phases: 100
  total_plans: 242
  completed_plans: 240
  percent: 99
decisions:
  - "174-02: /health route added before /relay/health so standard path matches first; /relay/health preserved for failover-orchestrator backward compat"
  - "174-02: version hardcoded as 1.0.0 in /health handler (comms-link does not inject package.json version)"
  - "174-03: gitignored ALL *.json in deploy-staging with !package.json, !package-lock.json, !chains.json exceptions — covers 548+ one-off relay payloads"
  - "174-03: added extra gitignore patterns beyond plan (*.log, *.jpg, *.png, screenshots/, kiosk-deploy/, kiosk-stage/, web-deploy/, *.spec, sshd_config, restore-db.js) discovered from actual inventory"
  - "174-04: comms-link health check targets localhost:8766 (relay runs on James .27); deploy.sh uses schtasks for racecontrol restart (survives SSH disconnect); rc-agent excluded (pod-specific)"
  - "174-05: rc-sentry added as 6th service in runbook (check-health.sh checks it); REPO-04/REPO-05 deferred as human_needed — server offline"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 174-01-PLAN.md
last_updated: "2026-03-22T22:15:49.068Z"
progress:
  total_phases: 130
  completed_phases: 100
  total_plans: 242
  completed_plans: 236
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 173-04-PLAN.md
last_updated: "2026-03-22T22:01:48.992Z"
progress:
  total_phases: 130
  completed_phases: 100
  total_plans: 237
  completed_plans: 234
  percent: 99
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 173-03-PLAN.md
last_updated: "2026-03-22T21:56:30.660Z"
progress:
  [██████████] 99%
  completed_phases: 99
  total_plans: 237
  completed_plans: 233
  percent: 98
---

---
gsd_state_version: 1.0
milestone: v21.0
milestone_name: API Contracts & Stabilization
status: in_progress
stopped_at: Completed 173-04-PLAN.md
last_updated: "2026-03-23T09:31:00+05:30"
current_phase: 173
current_phase_name: api-contracts
current_plan: 04
progress:
  [██████████] 98%
  completed_phases: 99
  total_plans: 234
  completed_plans: 234
  percent: 99
decisions:
  - "173-01: TypeScript-style type notation used throughout API-BOUNDARIES.md (not Rust) since all consumers are TypeScript"
  - "173-01: comms-link relay :8766 documented as a full boundary section"
  - "173-01: rc-agent kiosk-allowlist auth bug documented inline in API-BOUNDARIES.md"
  - "173-02: PricingTier shared type includes is_trial/is_active/sort_order (Rust fields missing from plan spec)"
  - "173-02: Driver.created_at made optional (kiosk creates partial frontend Driver objects)"
  - "173-02: Driver.has_used_trial added to shared type (kiosk API computed field used in SetupWizard)"
  - "173-03: ActiveSession.pod_number kept as admin-specific extra field (not in shared BillingSession)"
  - "173-03: openapi.yaml placed in docs/ (canonical) and copied to web/public/api-docs/ (static serving at :3200/api-docs/)"
  - "173-04: Vitest chosen for contract tests (native ESM, no transform config, fast cold start)"
  - "173-04: assertX(data: unknown): asserts data is T pattern used for runtime + compile-time contract enforcement, zero any types"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 172-02-PLAN.md
last_updated: "2026-03-22T20:57:53.582Z"
progress:
  total_phases: 130
  completed_phases: 99
  total_plans: 233
  completed_plans: 230
---

---
gsd_state_version: 1.0
milestone: v21.0
milestone_name: Cross-Project Sync & Stabilization
status: in_progress
stopped_at: Completed 172-03-PLAN.md
last_updated: "2026-03-23T02:21:00+05:30"
current_phase: 172
current_phase_name: standing-rules-sync
current_plan: 03
progress:
  total_phases: 130
  completed_phases: 98
  total_plans: 233
  completed_plans: 232
  percent: 99
decisions:
  - "172-01: people-tracker has no git remote — CLAUDE.md committed locally only, push skipped"
  - "172-01: people-tracker uses Python rules subset (no TypeScript rules)"
  - "172-01: racingpoint-admin gets extra rules: Next.js hydration + UI must reflect config truth"
  - "172-03: shell_relay not available on relay — used SSH fallback for racecontrol git pull on Bono VPS"
  - "172-03: Compliance script passed 'All repos compliant' before user manual checkpoint verification"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 170-03-PLAN.md
last_updated: "2026-03-23T12:30:00+05:30"
current_phase: 170
current_phase_name: repo-hygiene-dependency-audit
current_plan: 03
progress:
  total_phases: 130
  completed_phases: 96
  total_plans: 229
  completed_plans: 227
  percent: 99
---

---
gsd_state_version: 1.0
milestone: v21.0
milestone_name: Cross-Project Sync & Stabilization
status: in_progress
stopped_at: Defining requirements
last_updated: "2026-03-23T12:00:00+05:30"
current_phase: null
current_phase_name: Not started
progress:
  [██████████] 98%
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 156-02-PLAN.md
last_updated: "2026-03-22T17:38:31.405Z"
progress:
  [██████████] 99%
  completed_phases: 94
  total_plans: 222
  completed_plans: 219
  percent: 99
---

---
gsd_state_version: 1.0
milestone: v17.0
milestone_name: Promotions Engine
status: in_progress
stopped_at: "Completed 157-01-PLAN.md"
last_updated: "2026-03-22T23:30:00+05:30"
current_phase: 157
current_phase_name: Promotions Integration
current_plan: 02
progress:
  [██████████] 99%
  total_phases: 125
  completed_phases: 94
  total_plans: 222
  completed_plans: 220
  percent: 99
decisions:
  - "156-01: stacking_group column on cafe_promos table (not a separate table) — sufficient for Phase 157 mutual-exclusivity logic"
  - "156-01: promo_type validated at application layer + SQLite CHECK constraint for defense-in-depth"
  - "156-01: config stored as TEXT (JSON string) — flexible schema per promo_type without separate tables"
  - "157-01: happy_hour discount applies to total_paise (not per-item) — avoids needing unit prices in evaluate_promos"
  - "157-01: gaming_bundle display only in v1 — auto-apply needs billing session lookup in order hot path"
  - "157-01: single largest discount wins across stacking groups in v1 — avoids unexpected over-discounting"
  - "157-01: promo fetch failure non-fatal via unwrap_or_default — promo errors never block orders"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: "Completed 155-02-PLAN.md (checkpoint: awaiting human verify)"
last_updated: "2026-03-22T17:09:22.431Z"
progress:
  total_phases: 124
  completed_phases: 93
  total_plans: 220
  completed_plans: 217
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: "Completed 155-02-PLAN.md"
last_updated: "2026-03-22T17:30:00+05:30"
progress:
  total_phases: 124
  completed_phases: 93
  total_plans: 220
  completed_plans: 218
  percent: 99
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: "Completed 154-03-PLAN.md"
last_updated: "2026-03-22T17:48:00+05:30"
current_phase: 154
current_phase_name: Ordering Core
current_plan: 04
progress:
  [██████████] 99%
  total_phases: 124
  completed_phases: 91
  total_plans: 218
  completed_plans: 214
  percent: 98
decisions:
  - "154-03: Cart state React-only (no localStorage) — simple and sufficient for staff POS flow"
  - "154-03: Out-of-stock overlay on card (not disabled button) for unambiguous visual status"
  - "154-03: Driver autocomplete filters preloaded list client-side — no per-keystroke API calls"
  - "154-03: Order preserved on API error so staff can adjust and retry without rebuilding cart"
---

---
gsd_state_version: 1.0
milestone: v17.1
milestone_name: Watchdog-to-AI Migration
status: in_progress
stopped_at: "Completed 162-02-PLAN.md"
last_updated: "2026-03-22T22:00:00+05:30"
current_phase: 162
current_phase_name: James Watchdog Migration
current_plan: 03
progress:
  completed_phases: 91
  total_plans: 215
  completed_plans: 212
  percent: 99
decisions:
  - "162-02: HKLM Run key requires admin elevation — Task Scheduler (SYSTEM) is sufficient primary persistence"
  - "162-02: watchdog-state.json empty counts at first run confirms all 5 services healthy on James"
---

---
gsd_state_version: 1.0
milestone: v17.1
milestone_name: Watchdog-to-AI Migration
status: in_progress
stopped_at: "Completed 161-01-PLAN.md"
last_updated: "2026-03-22T21:00:00+05:30"
current_phase: 161
current_phase_name: Pod Monitor Merge
current_plan: 02
progress:
  [██████████] 99%
  completed_phases: 89
  total_plans: 211
  completed_plans: 209
  percent: 99
decisions:
  - "161-01: PodRecoveryTracker held in local HashMap in heal_all_pods loop — not in AppState, no shared state complexity"
  - "161-01: Step 1 logs SkipCascadeGuardActive with reason graduated_step1_wait_30s (no dedicated Wait action in RecoveryAction)"
  - "161-01: AlertStaff step stays at AlertStaff and re-alerts each cycle until pod comes back online"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 160-02-PLAN.md
last_updated: "2026-03-22T14:59:55.004Z"
progress:
  total_phases: 124
  completed_phases: 89
  total_plans: 211
  completed_plans: 208
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 153-02-PLAN.md
last_updated: "2026-03-22T15:05:00+05:30"
progress:
  total_phases: 124
  completed_phases: 88
  total_plans: 211
  completed_plans: 208
  percent: 98
decisions:
  - "153-02: Separate useEffect for low-stock polling (not merged into loadData) for clean separation of concerns"
  - "153-02: Banner is best-effort — fetch failures swallowed silently, banner absent rather than error state"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: "Completed 152-01-PLAN.md"
last_updated: "2026-03-22T19:52:00+05:30"
current_phase: 152
current_phase_name: Inventory Tracking
current_plan: 02
progress:
  [██████████] 98%
  completed_phases: 85
  total_plans: 207
  completed_plans: 202
  percent: 98
decisions:
  - "152-01: is_countable defaults to false — inventory tracking is opt-in per item"
  - "152-01: Restock returns 200+JSON error for non-countable items (not 400) to distinguish item-found-but-not-trackable"
  - "152-01: stock_quantity = stock_quantity + ? uses atomic SQL to avoid concurrent restock race conditions"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: not_started
stopped_at: Completed 151-02-PLAN.md
last_updated: "2026-03-22T13:41:33.911Z"
progress:
  total_phases: 124
  completed_phases: 85
  total_plans: 205
  completed_plans: 200
---

---
gsd_state_version: 1.0
milestone: v17.1
milestone_name: Watchdog-to-AI Migration
status: in_progress
stopped_at: "Completed 159-01-PLAN.md"
last_updated: "2026-03-22T19:15:00+05:30"
current_phase: 159
current_phase_name: Recovery Consolidation Foundation
current_plan: 02
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 1
  completed_plans: 1
  percent: 5
decisions:
  - "159-01: OwnershipConflict uses plain enum + impl Display/Error (thiserror in workspace but not in rc-common deps; tracing added instead)"
  - "159-01: RecoveryLogger.log() always returns Ok(()) — I/O errors emit tracing::warn, callers not burdened with log write failures"
  - "159-01: ProcessOwnership::register() idempotent for same authority, Err only on different-owner conflict"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: "Completed 150-02-PLAN.md"
last_updated: "2026-03-22T18:30:00+05:30"
progress:
  total_phases: 120
  completed_phases: 85
  total_plans: 201
  completed_plans: 199
  percent: 99
decisions:
  - "150-02: importCafePreview and uploadCafeItemImage use raw fetch (not fetchApi) — fetchApi sets Content-Type: application/json which breaks multipart boundary"
  - "150-02: Column mapping bar is read-only in v1 — fuzzy matching handles 95% of cases, dropdown override deferred"
  - "150-02: Preview table limited to first 100 rows via .slice(0, 100)"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: "Completed 148-01-PLAN.md (awaiting checkpoint:human-verify at Task 2)"
last_updated: "2026-03-22T08:39:42.571Z"
progress:
  [██████████] 98%
  completed_phases: 82
  total_plans: 197
  completed_plans: 194
---

---
gsd_state_version: 1.0
milestone: v16.1
milestone_name: Camera Dashboard Pro
status: in_progress
stopped_at: "Completed 148-01-PLAN.md"
last_updated: "2026-03-22T13:55:00+05:30"
progress:
  total_phases: 110
  completed_phases: 83
  total_plans: 197
  completed_plans: 195
  percent: 98
decisions:
  - "148-01: Native HTML5 DnD used (no @dnd-kit) per plan instruction"
  - "148-01: camerasRef kept in sync with cameras state so polling interval callbacks see current order"
  - "148-01: Pre-warm connection promoted to fullscreen if channel matches, including already-arrived tracks via getReceivers()"
  - "148-01: -m-6 negative margin applied to outer wrapper to cancel DashboardLayout p-6 for edge-to-edge grid"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 145-01-PLAN.md
last_updated: "2026-03-22T07:25:17.172Z"
progress:
  [██████████] 97%
  completed_phases: 80
  total_plans: 193
  completed_plans: 190
---

---
gsd_state_version: 1.0
milestone: v18.3
milestone_name: Camera Dashboard
status: in_progress
stopped_at: Completed 147-01-PLAN.md (awaiting checkpoint:human-verify)
last_updated: "2026-03-22T13:18:00+05:30"
progress:
  total_phases: 1
  completed_phases: 0
  total_plans: 3
  completed_plans: 3
decisions:
  - "146-01: CameraConfig uses Option<String>/Option<u32> for display_name/display_order so None signals use-default to callers"
  - "146-01: zone field uses serde default_zone() fn returning 'other' so JSON always has a string (no null in API response)"
  - "146-01: CORS Method::PUT added preemptively for layout PUT endpoint in plan 02"
  - "146-02: LayoutState uses Mutex<CameraLayout> — PUT updates are rare so simple lock is fine"
  - "146-02: Layout file path derived from config_path parent so camera-layout.json lives beside rc-sentry-ai.toml"
  - "146-02: Atomic write uses write-to-.json.tmp then tokio::fs::rename to prevent partial-write corruption"
  - "147-01: applyMode(mode, save=false) on fetchLayout so initial restore does not double-PUT to server"
  - "147-01: All DOM via createElement — no innerHTML per CONTEXT.md security hook requirement"
  - "147-01: openFullscreen stub shows snapshot; plan 03 replaces with go2rtc WebRTC"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 145-01-PLAN.md
last_updated: "2026-03-22T07:01:39.680Z"
progress:
  total_phases: 110
  completed_phases: 79
  total_plans: 191
  completed_plans: 188
---

---
gsd_state_version: 1.0
milestone: v17.0
milestone_name: AI Debugger Autonomy & Self-Healing
status: in_progress
stopped_at: Completed 140-02-PLAN.md
last_updated: "2026-03-22T11:15:00+05:30"
progress:
  total_phases: 106
  completed_phases: 76
  total_plans: 189
  completed_plans: 185
decisions:
  - "140-02: execute_ai_action uses matches!(action, KillEdge|KillGame|RestartRcAgent) for destructive detection"
  - "140-02: RestartRcAgent writes sentinel file before delayed exit so watchdog distinguishes intentional restart from crash"
  - "140-02: parse_ai_action_server in pod_healer.rs returns &str to avoid cross-crate rc-agent dependency"
  - "140-02: debug_suggestion.clone() added before broadcast so pod_id and model fields accessible post-send"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 138-03-PLAN.md
last_updated: "2026-03-22T05:12:59.373Z"
progress:
  total_phases: 106
  completed_phases: 75
  total_plans: 186
  completed_plans: 182
---

---
gsd_state_version: 1.0
milestone: v17.0
milestone_name: AI Debugger Autonomy & Self-Healing
status: in_progress
stopped_at: Completed 139-02-PLAN.md
last_updated: "2026-03-22T05:10:00.000Z"
progress:
  total_phases: 106
  completed_phases: 74
  total_plans: 184
  completed_plans: 181
decisions:
  - "139-02: ForceRelaunchBrowser arm uses direct state.lock_screen access matching all other lock_screen arms"
  - "139-02: Billing guard uses Relaxed ordering matching existing BlankScreen arm pattern"
  - "139-02: pod_id destructured as pod_id: _ — agent acts on its own lock screen unconditionally"
---

---
gsd_state_version: 1.0
milestone: v18.2
milestone_name: Debugging & Quality Gates
status: in_progress
stopped_at: Completed 144-02-PLAN.md
last_updated: "2026-03-22T10:52:00.000Z"
progress:
  total_phases: 106
  completed_phases: 74
  total_plans: 184
  completed_plans: 182
decisions:
  - "143-02: Bono-side full syntax check excluded from automated test — shell_relay requires APPROVE tier; relay liveness probe only"
  - "143-02: INTEG-02 appended to existing integration.test.js inside PSK else branch"
  - "143-01: Used node:http over fetch for compatibility with existing test pattern; chain status assertion case-insensitive after daemon returns 'OK'"
  - "144-01: Integration suite skip (no COMMS_PSK) is pass — skip is not failure; INTEG_EXIT only gates when PSK set"
  - "144-01: set -euo pipefail omitted from run-all.sh — all suites must run even if early one fails; per-suite exit codes captured manually"
  - "144-02: Pre-Ship Gate placed before [James Only] in CLAUDE.md to apply to both AIs equally"
  - "144-02: SKILL.md not in any git repo — content saved to disk, no commit possible for Task 2"
---

---
gsd_state_version: 1.0
milestone: v17.0
milestone_name: AI Debugger Autonomy & Self-Healing
status: in_progress
stopped_at: Completed 139-01-PLAN.md
last_updated: "2026-03-22T10:05:00.000Z"
progress:
  total_phases: 106
  completed_phases: 74
  total_plans: 183
  completed_plans: 178
decisions:
  - "139-01: ForceRelaunchBrowser added to CoreToAgentMessage for soft WS recovery before restart"
  - "139-01: execute_heal_action relaunch_lock_screen arm returns early (no shell exec) — WS dispatch only"
  - "139-01: Billing guard on relaunch: has_active_billing -> warn only, never dispatch during session"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: defining_requirements
stopped_at: Completed 138-03-PLAN.md
last_updated: "2026-03-22T04:50:35.945Z"
progress:
  total_phases: 106
  completed_phases: 73
  total_plans: 182
  completed_plans: 177
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: defining_requirements
stopped_at: Completed 142-01-PLAN.md
last_updated: "2026-03-22T10:30:00.000Z"
progress:
  total_phases: 106
  completed_phases: 72
  total_plans: 181
  completed_plans: 177
decisions:
  - "142-01: Standing rules merged into 6 categories with justifications — Deploy, Comms, Code Quality, Process, Debugging, Security"
  - "142-01: Deploy rules 4+5 pruned (absorbed into verification sequence); Standing rule 8 merged into auto-push"
---

---
gsd_state_version: 1.0
milestone: v18.2
milestone_name: Debugging & Quality Gates
status: defining_requirements
stopped_at: null
last_updated: "2026-03-22T04:30:00.000Z"
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: roadmap_ready
stopped_at: Completed 138-02-PLAN.md
last_updated: "2026-03-22T04:24:39.902Z"
progress:
  total_phases: 103
  completed_phases: 71
  total_plans: 179
  completed_plans: 175
  percent: 98
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: roadmap_ready
stopped_at: Completed 137-02-PLAN.md
last_updated: "2026-03-22T04:08:41.187Z"
progress:
  [██████████] 98%
  completed_phases: 71
  total_plans: 176
  completed_plans: 173
  percent: 98
---

---
gsd_state_version: 1.0
milestone: v17.0
milestone_name: AI Debugger Autonomy & Self-Healing
status: roadmap_ready
stopped_at: "Roadmap created -- Phase 137 ready to plan"
last_updated: "2026-03-22T09:00:00.000Z"
current_phase: 137
current_phase_name: Browser Watchdog
progress:
  [██████████] 98%
  completed_phases: 0
  total_plans: 10
  completed_plans: 0
decisions:
  - "137: Browser watchdog lives in LockScreenManager (rc-agent), not a separate binary"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: roadmap_ready
stopped_at: Completed 133-01-PLAN.md
last_updated: "2026-03-22T03:43:22.676Z"
progress:
  total_phases: 103
  completed_phases: 70
  total_plans: 174
  completed_plans: 171
  - "139: RelaunchLockScreen must check billing_active before dispatching (standing rule #10)"
  - "140: Whitelist-only safe actions -- no arbitrary shell execution from AI responses"
  - "141: WARN scanner runs inside healer cycle, not a separate tokio task"
---

---
gsd_state_version: 1.0
milestone: v17.0
milestone_name: AI Debugger Autonomy & Self-Healing
status: not_started
stopped_at: "Defining requirements"
last_updated: "2026-03-22T08:45:00.000Z"
current_phase: null
current_phase_name: null
progress:
  total_phases: 0
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 133-01-PLAN.md
last_updated: "2026-03-22T03:31:19.102Z"
progress:
  total_phases: 98
  completed_phases: 69
  total_plans: 172
  completed_plans: 169
---

---
gsd_state_version: 1.0
milestone: v18.1
milestone_name: Seamless Execution Hardening
status: in_progress
stopped_at: "Completed 136-02-PLAN.md"
last_updated: "2026-03-22T04:25:00.000Z"
current_phase: 136
current_phase_name: Chain Endpoint + Visibility
progress:
  total_phases: 2
  completed_phases: 1
  total_plans: 4
  completed_plans: 4
decisions:
  - "135-01: /sc MINUTE /mo 2 chosen over ONLOGON+repeat — simpler and achieves same 2-min detection window"
  - "135-01: Restart via start-comms-link.bat (not node directly) to preserve all env vars and start supervisor"
  - "135-01: No -Wait on bat start since start /min backgrounds immediately"
  - "135-02: Used PowerShell schtasks fallback (no /rl HIGHEST) when Node registration failed with Access Denied"
  - "135-02: HKCU Run key was already correct from Plan 01 — no changes needed"
  - "136-01: chain_result handler placed after delegate_result, before catch-all; execId mapped from msg.payload?.chainId"
  - "136-02: lastBonoMessageAt set at top of message handler before type checks to capture all WS messages including control"
  - "136-02: REALTIME guard placed after execId assignment so 503 includes execId for tracing"
  - "136-02: Existing if (!sent) guard preserved as secondary defense in depth"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 133-01-PLAN.md
last_updated: "2026-03-21T23:22:05.398Z"
progress:
  total_phases: 96
  completed_phases: 68
  total_plans: 170
  completed_plans: 167
---

---
gsd_state_version: 1.0
milestone: v18.0
milestone_name: Advanced Chain Features Integration Hardening
status: in_progress
stopped_at: Completed 134-02-PLAN.md
last_updated: "2026-03-22T23:11:00.000Z"
progress:
  total_phases: 96
  completed_phases: 68
  total_plans: 170
  completed_plans: 168
decisions:
  - "133-02: AuditLogger instantiated before execHandler/shellRelay (closures capture at construction time)"
  - "133-02: Executor-side audit fires before sending delegate_result to guarantee log entry survives network failures"
  - "133-02: Requester-side audit uses chainId_step_i as execId for cross-machine correlation"
  - "133-02: bonoAuditLogger returned from wireBono() for testability"
  - "134-01: templatesFn defaults to () => ({}) for backward-compatible constructor"
  - "134-01: Inline steps take precedence over template name when both provided"
  - "134-01: Retry does not re-attempt on broker timeout, only on non-zero exitCode"
  - "134-01: Fresh execId per retry to avoid ExecResultBroker dedup blocking retries"
  - "134-02: pause() returns deep copy -- caller can JSON.stringify without affecting #activeState"
  - "134-02: resume() takes full steps array in savedState so audit indices are absolute positions"
  - "134-02: bono chain state hooks in wss connection/close events (symmetric to james ConnectionMode)"
  - "134-02: buildIntrospectionResponse exposes only name/description/tier/timeoutMs -- never binary/args"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 133-01-PLAN.md
last_updated: "2026-03-21T22:52:35.092Z"
progress:
  total_phases: 96
  completed_phases: 67
  total_plans: 168
  completed_plans: 165
---

---
gsd_state_version: 1.0
milestone: v18.0
milestone_name: Task Delegation Audit Trail
status: in_progress
stopped_at: Completed 133-02-PLAN.md
last_updated: "2026-03-21T22:47:30.000Z"
progress:
  total_phases: 96
  completed_phases: 67
  total_plans: 168
  completed_plans: 165
decisions:
  - "133-02: AuditLogger instantiated before execHandler/shellRelay (closures capture at construction time)"
  - "133-02: Executor-side audit fires before sending delegate_result to guarantee log entry survives network failures"
  - "133-02: Requester-side audit uses chainId_step_i as execId for cross-machine correlation"
  - "133-02: bonoAuditLogger returned from wireBono() for testability"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 133-01-PLAN.md
last_updated: "2026-03-21T22:41:36.780Z"
progress:
  total_phases: 96
  completed_phases: 66
  total_plans: 168
  completed_plans: 164
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 132-01-PLAN.md
last_updated: "2026-03-21T22:21:04.791Z"
progress:
  total_phases: 96
  completed_phases: 65
  total_plans: 166
  completed_plans: 162
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 130-02-PLAN.md
last_updated: "2026-03-21T21:54:11.720Z"
progress:
  total_phases: 96
  completed_phases: 64
  total_plans: 163
  completed_plans: 160
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 130-02-PLAN.md
last_updated: "2026-03-21T21:44:28.233Z"
progress:
  total_phases: 96
  completed_phases: 64
  total_plans: 163
  completed_plans: 160
  percent: 98
---

---
gsd_state_version: 1.0
milestone: v18.0
milestone_name: Seamless Execution
status: in_progress
stopped_at: Completed 132-02-PLAN.md
last_updated: "2026-03-22T04:28:00.000Z"
progress:
  completed_phases: 2
  total_plans: 4
  completed_plans: 4
  percent: 30
current_phase: 132
current_phase_name: Chain Orchestration
current_plan: 2
decisions:
  - "130-01: Object.freeze(new Set()) for ALLOWED_BINARIES — test uses Object.isFrozen() since freeze doesn't block Set.add()"
  - "130-01: DynamicCommandRegistry uses private class fields (#commands, #safeEnv) for true encapsulation"
  - "130-01: Constructor DI pattern — safeEnv injected, not imported inside dynamic-registry.js"
  - "130-02: wireBono() is sync -- used fire-and-forget async IIFE for persistence loading to avoid breaking call site"
  - "130-02: ExecHandler dual-registry lookup: dynamicRegistry?.get(command) ?? staticRegistry[command] (DREG-04)"
  - "130-02: #trackCompleted() private method centralizes LRU eviction check + add for completedExecs"
  - "131-01: ShellRelayHandler completely separate from ExecHandler -- no tier routing switch, SHELL_RELAY_TIER constant = 'approve'"
  - "131-01: Binary allowlist check fires before notifyFn -- disallowed binary rejected silently (no notification leak)"
  - "131-01: bono/index.js exec_approval handler added (was missing) -- placed before exec_request handler"
  - "131-01: Dedup covers both pendingApprovals (in-flight) and completedExecs (finished) -- prevents replay"
  - "132-01: ExecResultBroker is pure standalone utility with zero imports -- no dependency on protocol.js or any other comms-link module"
  - "132-01: ChainOrchestrator uses Promise.race between stepLoopPromise and chainTimeout timer -- avoids AbortController complexity"
  - "132-02: FailoverOrchestrator #pending Map fully removed -- broker is single source of truth for exec_result resolution"
  - "132-02: exec_result on Bono side calls BOTH bonoExecResultBroker.handleResult AND wss.emit -- backward compat preserved"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: roadmap_created
stopped_at: Completed 119-03-PLAN.md
last_updated: "2026-03-21T19:11:51.902Z"
progress:
  total_phases: 81
  completed_phases: 63
  total_plans: 161
  completed_plans: 158
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: roadmap_created
stopped_at: Completed 114-03-PLAN.md
last_updated: "2026-03-21T17:06:00Z"
progress:
  total_phases: 81
  completed_phases: 57
  total_plans: 148
  completed_plans: 144
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: roadmap_created
stopped_at: Completed 112-03-PLAN.md
last_updated: "2026-03-21T15:22:29.074Z"
progress:
  total_phases: 81
  completed_phases: 54
  total_plans: 138
  completed_plans: 135
  percent: 96
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: roadmap_created
stopped_at: Completed 112-01-PLAN.md
last_updated: "2026-03-21T15:06:38.548Z"
progress:
  [██████████] 96%
  completed_phases: 53
  total_plans: 138
  completed_plans: 131
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: roadmap_created
stopped_at: Completed 105-02-PLAN.md
last_updated: "2026-03-21T14:20:16.896Z"
progress:
  total_phases: 73
  completed_phases: 52
  total_plans: 131
  completed_plans: 129
---

---
gsd_state_version: 1.0
milestone: v11.2
milestone_name: RC Sentry AI Debugger
status: roadmap_created
stopped_at: Roadmap written — awaiting plan-phase 101
last_updated: "2026-03-21T20:00:00.000Z"
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
current_phase: 101
current_phase_name: rc-common Types Foundation
phases:
  - "101: rc-common Types Foundation (FLEET-01)"
  - "102: Watchdog Core (DETECT-01, DETECT-02, DETECT-05)"
  - "103: Tier 1 Fixes and Escalation FSM (FIX-01 to FIX-06, ESC-01, ESC-02)"
  - "104: Pattern Memory and Ollama Integration (MEM-01 to MEM-03, LLM-01 to LLM-03)"
  - "105: Server Endpoint, bat File, and Fleet Rollout (FLEET-02, FLEET-03, ESC-03, DETECT-03, DETECT-04)"
decisions:
  - "Phases 101–105 start numbering — continues from Phase 100 (last shipped)"
  - "DETECT-03 and DETECT-04 (bat file + self_heal update) placed in Phase 105 — they are deploy prerequisites verified in the canary integration test, not standalone watchdog code"
  - "ESC-03 (escalation fleet report + email) placed in Phase 105 — requires FLEET-02 server endpoint to exist before end-to-end test can verify it"
  - "Ollama is fire-and-forget on separate std::thread — restart latency target under 10s regardless of Ollama state"
  - "Anti-cheat constraint is Phase 102 success criterion — confirmed in binary by absence of OpenProcess/CreateToolhelp32Snapshot"
---

---
gsd_state_version: 1.0
milestone: v15.0
milestone_name: AntiCheat Compatibility
status: in_progress
stopped_at: Completed 109-02-PLAN.md
last_updated: "2026-03-21T15:41:00.000Z"
progress:
  total_phases: 5
  completed_phases: 0
  total_plans: 4
  completed_plans: 5
  percent: 50
current_phase: 109
current_phase_name: Safe Mode State Machine
phases:
  - "107: Behavior Audit + Certificate Procurement (AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04)"
  - "108: Keyboard Hook Replacement (HARD-01, VALID-03)"
  - "109: Safe Mode State Machine (SAFE-01 through SAFE-07)"
  - "110: Telemetry Gating (HARD-03, HARD-04, HARD-05)"
  - "111: Code Signing + Per-Game Canary Validation (HARD-02, VALID-01, VALID-02)"
decisions:
  - "Phases 107-111 start numbering -- continues from Phase 106 (last shipped)"
  - "Hook replacement (Phase 108) must complete BEFORE safe mode state machine (Phase 109) -- safe mode has no hook state to manage after replacement"
  - "Certificate procurement begins Phase 107 -- OV cert delivery (1-5 days) is on the critical path to Phase 111 signing"
  - "Safe mode is a positive-enable allowlist, not a negative-disable blocklist -- only approved operations proceed during a protected session"
  - "AC EVO telemetry feature-flagged off by default (HARD-05) -- reassess at Kunos v1.0 release"
  - "WMI Win32_ProcessStartTrace subscription chosen over polling to close the EAC 2-5s initialization window"
  - "v13.0 Multi-Game Launcher MUST NOT deploy to customer pods until Phase 111 canary validation complete"
  - "ConspitLink audit deferred -- template created and ready; ProcMon capture requires physical access to Pod 8"
  - "SetWindowsHookEx is UNSAFE (permanent removal, Phase 108) for all kernel-level AC games -- not just suspended"
  - "Ollama queries must be SUSPENDED during protected game sessions due to GPU/VRAM contention visible to EAAC"
  - "107-01: No ReadProcessMemory or WriteProcessMemory found in rc-agent -- critical CRITICAL-level API is absent from codebase"
  - "107-01: All sim adapters use OpenFileMappingW + MapViewOfFile (correct safe pattern, not ReadProcessMemory)"
  - "107-01: OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION) in game_process.rs:321 is HIGH risk -- even query-only handles to game PID are detectable by EAAC/EOS/EAC"
  - "107-01: Phase 108 MUST use GPO registry keys (NoWinKeys=1 + DisableTaskMgr=1) -- pods are Windows 11 Pro, Keyboard Filter requires IoT Enterprise LTSC"
  - "108-01: GPO via reg.exe (no winreg crate) chosen -- matches lock_screen.rs pattern, zero new dependencies"
  - "108-01: keyboard-hook Cargo feature preserves rollback -- cargo build --features keyboard-hook restores SetWindowsHookEx behavior"
  - "108-01: imports (AtomicPtr, Ordering, LPARAM, LRESULT, WPARAM) also gated behind cfg(feature) to prevent unused import warnings"
  - "109-01: WRC.exe in PROTECTED_EXE_NAMES even without SimType variant -- exe_to_sim_type returns None gracefully, detection is future-proof"
  - "109-01: detect_running_protected_game() uses #[cfg(not(test))] stub to keep unit tests hermetic (no sysinfo scans in test builds)"
  - "109-02: Ollama suppression via call-site guard (Option B) -- state.safe_mode.active checked in event_loop before tokio::spawn, no signature change to analyze_crash"
  - "109-02: KioskManager and LockScreenManager use wire_safe_mode() post-construction wiring -- avoids changing new() signature while keeping kiosk startup call unaffected"
  - "109-02: self_heal::repair_registry_key is startup-only (main.rs:239) -- no gate needed, runs before event loop and before any game"
  - "109-02: WRC.exe safe mode activation uses manual field assignment (safe_mode.active=true, game=None) since no SimType::EaWrc variant exists"
  - "109-01: SafeMode::enter() clears cooldown_until -- game start takes priority over pending cooldown window"
  - "109-01: process_guard::spawn() extended with _safe_mode_active stub (5th arg) -- Plan 02 wires into scan loop"
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 93-02-PLAN.md
last_updated: "2026-03-21T09:35:00.648Z"
progress:
  [█████████░] 92%
  completed_phases: 48
  total_plans: 117
  completed_plans: 115
  percent: 98
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 93-02-PLAN.md
last_updated: "2026-03-21T09:07:11.301Z"
progress:
  [██████████] 98%
  completed_phases: 84
  total_plans: 209
  completed_plans: 206
  percent: 96
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 100-02-PLAN.md
last_updated: "2026-03-21T06:24:44Z"
last_activity: "2026-03-21 -- 100-02 complete: Fleet page Racing Red Maintenance badge, PIN-gated modal with failure list, Clear Maintenance button calling POST /pods/{id}/clear-maintenance (STAFF-01, STAFF-02)"
progress:
  [██████████] 96%
  total_phases: 65
  completed_phases: 41
  total_plans: 108
  completed_plans: 104
  percent: 96
decisions:

  - "PIN gate accepts any 4-digit input for maintenance modal — casual venue TV protection; actual security is JWT-protected clear-maintenance endpoint"
  - "maintenance check runs first in statusBorder/statusLabel/statusLabelColor so in_maintenance=true always overrides WS/HTTP status visuals"

---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 85-01-PLAN.md
last_updated: "2026-03-21T07:00:00.000Z"
last_activity: "2026-03-21 -- 85-01 complete: LmuAdapter with rF2 shared memory (Scoring + Telemetry), torn-read guard, sector splits (cumulative derivation), first-packet safety, session transition reset, 6 unit tests (TEL-LMU-01, TEL-LMU-02, TEL-LMU-03)"
progress:
  [██████████] 96%
  completed_phases: 40
  total_plans: 107
  completed_plans: 103
  percent: 96
decisions:

  - "clear_on_disconnect() clears in_maintenance=false because offline pods are not in maintenance from the server's perspective"
  - "Optimistic server-side clear on clear_maintenance_pod() for instant staff visual feedback without waiting for PreFlightPassed roundtrip"
  - "sector_times_ms() uses .round() not truncation — (42.3-20.1)*1000 = 22199.99 would truncate to 22199 instead of 22200"
  - "pub mod lmu registered in Plan 01 (not Plan 02) — required for cargo test sims::lmu compilation"

---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 84-01-PLAN.md
last_updated: "2026-03-21T05:27:15.878Z"
last_activity: "2026-03-21 -- 84-01 complete: IracingAdapter with shared memory, dynamic variable lookup, double-buffer tick-lock, session transition detection, pre-flight app.ini check, 8 unit tests (TEL-IR-01, TEL-IR-02, TEL-IR-03, TEL-IR-04)"
progress:
  total_phases: 65
  completed_phases: 39
  total_plans: 104
  completed_plans: 100
  percent: 96
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 99-02-PLAN.md
last_updated: "2026-03-21T05:25:55.665Z"
last_activity: "2026-03-21 -- 83-01 complete: 6 F1 25 unit tests added (lap completion, sector splits, invalid lap flag, session type mapping, first-packet safety, take semantics) — TEL-F1-01, TEL-F1-02, TEL-F1-03 verified"
progress:
  [██████████] 96%
  completed_phases: 39
  total_plans: 104
  completed_plans: 99
  percent: 95
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 99-01-PLAN.md
last_updated: "2026-03-21T05:20:01.351Z"
last_activity: "2026-03-21 -- 83-01 complete: 6 F1 25 unit tests added (lap completion, sector splits, invalid lap flag, session type mapping, first-packet safety, take semantics) — TEL-F1-01, TEL-F1-02, TEL-F1-03 verified"
progress:
  [██████████] 95%
  completed_phases: 38
  total_plans: 104
  completed_plans: 98
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 98-02-PLAN.md
last_updated: "2026-03-21T04:56:25.218Z"
last_activity: "2026-03-21 -- 98-02 complete: DISP-01 HTTP probe (127.0.0.1:18923) + DISP-02 GetWindowRect (Chrome_WidgetWin_1) in pre_flight.rs (5 concurrent checks) + 30s maintenance retry select! arm in event_loop.rs (PF-06, DISP-01, DISP-02)"
progress:
  total_phases: 65
  completed_phases: 38
  total_plans: 102
  completed_plans: 97
  percent: 95
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 98-01-PLAN.md
last_updated: "2026-03-21T04:47:06.000Z"
last_activity: "2026-03-21 -- 98-01 complete: MaintenanceRequired LockScreenState variant + in_maintenance AtomicBool on AppState + ClearMaintenance ws_handler (PF-04, PF-05)"
progress:
  [██████████] 95%
  completed_phases: 37
  total_plans: 100
  completed_plans: 96
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 83-01-PLAN.md
last_updated: "2026-03-21T04:32:00.084Z"
last_activity: "2026-03-21 -- 97-02 complete: pre_flight.rs concurrent check runner (HID, ConspitLink, orphan game) + ws_handler pre-flight gate with billing_active.store(true) inside Pass branch (PF-01, PF-02, PF-03, HW-01, HW-02, HW-03, SYS-01)"
progress:
  total_phases: 65
  completed_phases: 37
  total_plans: 98
  completed_plans: 95
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 97-02-PLAN.md
last_updated: "2026-03-21T04:20:27.168Z"
last_activity: "2026-03-21 -- 97-02 complete: pre_flight.rs concurrent check runner (HID, ConspitLink, orphan game) + ws_handler pre-flight gate with billing_active.store(true) inside Pass branch (PF-01, PF-02, PF-03, HW-01, HW-02, HW-03, SYS-01)"
progress:
  total_phases: 65
  completed_phases: 36
  total_plans: 97
  completed_plans: 94
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 97-02-PLAN.md
last_updated: "2026-03-21T06:33:11.251Z"
last_activity: "2026-03-21 -- 97-02 complete: pre_flight.rs concurrent check runner (HID, ConspitLink, orphan game) + ws_handler pre-flight gate with billing_active.store(true) inside Pass branch (PF-01, PF-02, PF-03, HW-01, HW-02, HW-03, SYS-01)"
progress:
  total_phases: 75
  completed_phases: 73
  total_plans: 186
  completed_plans: 185
  percent: 97
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 82-03-PLAN.md
last_updated: "2026-03-21T04:11:01.247Z"
last_activity: "2026-03-21 -- 97-01 complete: PreFlightPassed + PreFlightFailed AgentMessage variants + ClearMaintenance CoreToAgentMessage variant + PreflightConfig struct wired into AgentConfig (PF-07)"
progress:
  [██████████] 97%
  completed_phases: 29
  total_plans: 82
  completed_plans: 81
  percent: 99
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 97-01-PLAN.md
last_updated: "2026-03-21T04:06:13.920Z"
last_activity: "2026-03-21 -- 80-02 complete: PIN rotation alerting (system_settings + 24h WhatsApp check) + HMAC-SHA256 cloud sync signing/verification in permissive mode (ADMIN-06, AUTH-07)"
progress:
  [██████████] 99%
  completed_phases: 29
  total_plans: 82
  completed_plans: 80
  percent: 95
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Phase 97 context gathered
last_updated: "2026-03-21T03:40:37.614Z"
last_activity: "2026-03-21 -- 79-02 complete: PII encryption migration, 9 phone queries use phone_hash, 7 log statements redacted, cloud sync encrypts (DATA-01, DATA-02, DATA-03)"
progress:
  [██████████] 95%
  completed_phases: 28
  total_plans: 80
  completed_plans: 77
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 79-02-PLAN.md
last_updated: "2026-03-21T03:03:57.288Z"
last_activity: 2026-03-21 — Milestone v11.1 Pre-Flight Session Checks started
progress:
  total_phases: 57
  completed_phases: 28
  total_plans: 75
  completed_plans: 76
  percent: 100
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Phase 82 context gathered
last_updated: "2026-03-21T02:59:19.931Z"
progress:
  [██████████] 100%
  completed_phases: 27
  total_plans: 75
  completed_plans: 75
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 79-03-PLAN.md
last_updated: "2026-03-21T02:56:24Z"
last_activity: "2026-03-21 -- 79-03 complete: DPDP data export (decrypted PII JSON) + cascade delete (21 child tables in transaction) with 8 unit tests (DATA-04, DATA-05)"
progress:
  total_phases: 57
  completed_phases: 27
  total_plans: 75
  completed_plans: 75
  percent: 100
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 89-03-PLAN.md
last_updated: "2026-03-21T02:29:20.428Z"
progress:
  [██████████] 100%
  completed_phases: 65
  total_plans: 177
  completed_plans: 170
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Phase 81 UI-SPEC approved
last_updated: "2026-03-21T01:22:08.465Z"
last_activity: "2026-03-21 -- 78-03 complete: BillingStarted session_token + KioskLockdown auto-pause billing + debounced WhatsApp alert (SESS-04, SESS-05)"
progress:
  total_phases: 53
  completed_phases: 28
  total_plans: 80
  completed_plans: 72
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 74-01-PLAN.md
last_updated: "2026-03-21T01:06:56.526Z"
last_activity: "2026-03-21 -- 78-01 complete: Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown USB/accessibility/TaskMgr lockdown (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)"
progress:
  total_phases: 53
  completed_phases: 27
  total_plans: 76
  completed_plans: 70
  percent: 92
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 69-03-PLAN.md
last_updated: "2026-03-21T01:01:53.983Z"
last_activity: "2026-03-21 -- 78-01 complete: Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown USB/accessibility/TaskMgr lockdown (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)"
progress:
  [█████████░] 92%
  completed_phases: 27
  total_plans: 76
  completed_plans: 69
  percent: 91
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 74-01-PLAN.md
last_updated: "2026-03-21T01:01:17.895Z"
last_activity: "2026-03-21 -- 78-01 complete: Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown USB/accessibility/TaskMgr lockdown (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)"
progress:
  [█████████░] 91%
  completed_phases: 26
  total_plans: 76
  completed_plans: 68
  percent: 89
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 69-02-PLAN.md
last_updated: "2026-03-21T01:00:37.615Z"
last_activity: "2026-03-21 -- 69-02 complete: failover_broadcast endpoint + split-brain guard in rc-agent SwitchController (ORCH-02, ORCH-03)"
progress:
  [█████████░] 89%
  completed_phases: 26
  total_plans: 76
  completed_plans: 67
  percent: 88
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Phase 81 context gathered
last_updated: "2026-03-21T00:56:28.952Z"
last_activity: "2026-03-20 -- 77-02 complete: dual-port HTTPS 8443 + tower-helmet security headers + protocol-aware kiosk API_BASE (TLS-01, TLS-03, TLS-04, KIOSK-06)"
progress:
  [█████████░] 88%
  completed_phases: 26
  total_plans: 76
  completed_plans: 66
  percent: 87
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 69-01-PLAN.md
last_updated: "2026-03-21T00:56:00.000Z"
last_activity: "2026-03-21 -- 69-01 complete: HealthMonitor FSM (12-tick/60s hysteresis) + FailoverOrchestrator (activate_failover -> exec_result -> broadcast -> notify) wired into james/index.js (HLTH-01, HLTH-02, HLTH-03, ORCH-01, ORCH-04)"
progress:
  [█████████░] 87%
  completed_phases: 26
  total_plans: 76
  completed_plans: 65
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 77-01-PLAN.md
last_updated: "2026-03-20T14:22:16.463Z"
last_activity: "2026-03-20 -- 77-01 complete: TLS foundation with rcgen cert gen, RustlsConfig loader, ServerConfig extension (TLS-02, TLS-04)"
progress:
  total_phases: 45
  completed_phases: 26
  total_plans: 70
  completed_plans: 64
  percent: 91
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 77-01-PLAN.md
last_updated: "2026-03-20T14:18:04.350Z"
last_activity: "2026-03-20 -- 77-01 complete: TLS foundation with rcgen cert gen, RustlsConfig loader, ServerConfig extension (TLS-02, TLS-04)"
progress:
  [█████████░] 91%
  completed_phases: 25
  total_plans: 70
  completed_plans: 63
  percent: 90
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 68-01-PLAN.md
last_updated: "2026-03-20T14:02:05.654Z"
last_activity: "2026-03-20 -- 76-06 complete: strict JWT enforcement on 172 staff routes (AUTH-01, AUTH-02, AUTH-03, SESS-01)"
progress:
  [█████████░] 90%
  completed_phases: 24
  total_plans: 67
  completed_plans: 61
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 76-06-PLAN.md
last_updated: "2026-03-20T13:38:47.349Z"
last_activity: "2026-03-20 -- 76-06 complete: strict JWT enforcement on 172 staff routes (AUTH-01, AUTH-02, AUTH-03, SESS-01)"
progress:
  total_phases: 45
  completed_phases: 24
  total_plans: 62
  completed_plans: 60
  percent: 97
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-21)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** Phase 93 — Community & Tribal Identity

## Current Position

Phase: 93 (Community & Tribal Identity) — EXECUTING
Plan: 1 of 2

## Phase Map -- v11.0 Agent & Sentry Hardening

| Phase | Name | Requirements | Status |
|-------|------|--------------|--------|
| 71 | rc-common Foundation + rc-sentry Core Hardening | SHARED-01..03, SHARD-01..05 | Complete (2/2 plans done) |
| 72 | rc-sentry Endpoint Expansion + Integration Tests | SEXP-01..04, SHARD-06, TEST-04 | Complete (2/2 plans done) |
| 73 | Critical Business Tests | TEST-01, TEST-02, TEST-03 | Complete (2/2 plans done) |
| 74 | rc-agent Decomposition | DECOMP-01..04 | In Progress (3/4 plans done) |

**Phase 71:** rc-common exec.rs with feature gate (SHARED) + rc-sentry timeout, truncation, concurrency cap, partial read fix, structured logging (SHARD). No rc-agent changes. Verify `cargo tree -p rc-sentry` shows no tokio after every rc-common change.
**Phase 72:** rc-sentry endpoint expansion (/health, /version, /files, /processes, graceful shutdown) + TcpStream-based integration tests on ephemeral port.
**Phase 73:** billing_guard + failure_monitor unit tests using watch channel injection / mockall; FfbBackend trait seam. MUST complete before Phase 74 (Refactor Second rule).
**Phase 74:** config.rs -> app_state.rs -> ws_handler.rs -> event_loop.rs extraction in strict dependency order. select! dispatch body (event_loop) deferred to v12.0 if risk is too high.

## Key Constraints for This Milestone

- rc-sentry MUST stay stdlib-only -- never add tokio. Run `cargo build --bin rc-sentry` after every rc-common change
- rc-common feature gate: default = sync (rc-sentry uses this), "async-exec" = tokio (rc-agent uses this)
- wait-timeout 0.2 is the only correct stdlib-compatible child process timeout on Windows
- mockall 0.13 goes in rc-agent dev-dependencies only (MSRV 1.77, project at 1.93.1)
- event_loop.rs extraction is the highest regression risk -- protect with Phase 73 tests first
- select! variable extraction uses ConnectionState struct pattern, never Arc<Mutex<T>> for local variables

## Performance Metrics

**Velocity (recent):**

- Phase 56 P01: 494 min | Phase 56 P02: 3 min | Phase 57 P01-03: ~35 min total
- Average recent plan: ~15 min

**Updated after each plan completion**

## Accumulated Context

### Roadmap Evolution

- Phase 106 added: Structured Log Labels — Add [build_id][module] prefix to all rc-agent tracing output

### Decisions (v11.0)

- Build order: rc-common extraction -> rc-sentry hardening -> rc-agent decomposition (rc-common unblocks both sentry and agent)
- Phase 71 combines rc-common extraction + rc-sentry core hardening (they must be co-developed to validate the feature gate)
- Phase 73 (tests) precedes Phase 74 (decomposition) -- Refactor Second is a non-negotiable standing rule
- FfbBackend trait is the correct approach over #[cfg(test)] stubs -- cleaner seam, decided before writing any FFB tests
- SHARD-04 (partial TCP read fix) added to Phase 71 -- correctness issue distinct from timeout/truncation, flagged by research
- 71-01: wait-timeout = 0.2 for stdlib-only child process timeout; tokio optional dep with feature gate prevents rc-sentry contamination (SHARED-01..03 complete)
- 71-01: Truncation on Vec<u8> before String::from_utf8_lossy to prevent char boundary panics in exec.rs
- 75-01: 269 racecontrol routes classified into 5 tiers; 172 staff/admin routes have zero auth (CRITICAL); OTP plaintext in logs elevated to CRITICAL
- 75-01: rc-agent /exec and rc-sentry TCP flagged CRITICAL -- arbitrary command execution with zero auth on LAN
- 71-02: SlotGuard Drop impl ensures EXEC_SLOTS decremented even on panic -- prevents 429 lockout (SHARD-01..05 complete)
- 71-02: THREAD_COUNTER separate from EXEC_SLOTS -- EXEC_SLOTS=live connections, THREAD_COUNTER=monotonic spawn IDs
- 75-02: rand 0.8 thread_rng().r#gen() for JWT key gen (gen is Rust 2024 reserved keyword); RACECONTROL_* env var naming for all secrets
- 75-02: default_jwt_secret() kept for serde backward compat; resolve_jwt_secret() catches dangerous default at runtime
- 76-03: subtle crate for constant-time service key comparison on rc-agent; permissive mode when RCAGENT_SERVICE_KEY unset; /ping and /health remain public
- 66-05: INFRA-01 complete via static IP alone -- TP-Link EX220 firmware bug (Error 5024) persists ARP entries in NVRAM across reboots, permanently blocking DHCP reservation for server .23; reservation is "won't fix" for this router model, add if factory-reset or replaced
- 72-01: build.rs copied from rc-agent for GIT_HASH embedding; winapi 0.3 consoleapi-only for SetConsoleCtrlHandler; non-blocking accept loop polls SHUTDOWN_REQUESTED every 10ms for graceful drain (SEXP-01..04, SHARD-06 complete)
- 72-02: inline #[cfg(test)] module with incoming().take(N) for clean thread exit; ephemeral ports via 127.0.0.1:0; all 7 tests pass with zero tokio contamination (TEST-04 complete)
- 66-05: Bono deployment (exec round-trip) deferred async via INBOX.md; INFRA-03 code complete on both sides, live verification pending Bono pm2 restart
- 67-01: Allowlist approach for sanitizer (only venue/pods/branding) -- never denylist; httpPost used for relay/sync POST for consistency; RACECONTROL_TOML_PATH env var for configurable path (SYNC-01, SYNC-02 complete)
- 67-02: parse_config_snapshot extracted as pub(crate) fn for testability -- sync_push calls it rather than inlining (SYNC-03 complete)
- 67-02: config_snapshot uses total += 1 (single record semantics, not per-field) -- consistent with other upserts
- 67-02: Structured tracing on config_snapshot receipt: venue name, pod count, hash prefix (first 8 chars)
- 76-02: argon2 0.5 with Argon2id default params for admin PIN hashing; spawn_blocking for CPU-heavy verification; 503 when no hash configured; 12h JWT expiry (shift-length limit)
- 76-04: tower_governor 0.8 with PeerIpKeyExtractor for per-IP rate limiting; into_make_service_with_connect_info for ConnectInfo; SQLx transaction wraps validate_pin token lifecycle
- 76-04: Bot wallet check (AUTH-05) already existed; billing is deferred (in-memory), not DB -- TOCTOU mitigated by optimistic locking
- 73-01: FfbBackend trait uses FfbController::method(self) fully-qualified delegation to avoid infinite recursion when trait and inherent method names match; mockall mock tests added inside existing test module; tokio test-util added to dev-deps to fix pre-existing billing_guard compilation (TEST-03 complete)
- 73-02: tokio::time::Instant required (not std::time::Instant) for billing_guard debounce timers -- mock clock only controls tokio::time::* functions; yield_now x5 before first advance() lets spawned task start and register interval before clock moves (TEST-01, TEST-02 complete)
- 76-05: JWT in localStorage with client-side expiry check; AuthGate skips /login pathname to avoid redirect loop; fetchApi auto-clears token + redirects on 401; useIdleTimeout listens to 5 event types with passive listeners (ADMIN-01, ADMIN-03 complete)
- 76-06: One-line swap from require_staff_jwt_permissive to require_staff_jwt on staff sub-router; contract step of expand-migrate-contract; kept permissive variant for rollback (AUTH-01, AUTH-02, AUTH-03, SESS-01 complete)
- 68-01: SwitchController placed after RunSelfTest — additive variant, no enum reorder; failover_url: Option<String> with serde(default) for zero-friction backward compat; last_switch_ms: AtomicU64 on HeartbeatStatus for Plan 02 runtime wiring (FAIL-01, FAIL-03, FAIL-04 data contracts complete)
- 68-02: active_url Arc<RwLock<String>> read inside outer reconnect loop on each iteration — picks up new URL from SwitchController without restart; strict URL allowlist (primary+failover only); log_event made pub for cross-module SWITCH event recording; switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000 (FAIL-02, FAIL-03, FAIL-04 runtime wiring complete)
- 77-01: rcgen 0.14 generate_simple_self_signed takes Vec<String> with auto IP detection (not SanType enum); CertifiedKey has signing_key (not key_pair); backward-compat ServerConfig with Option fields (TLS-02, TLS-04)
- 77-02: HelmetLayer::blank() with selective headers (not with_defaults) -- avoids COEP/COOP/upgrade-insecure-requests that break kiosk proxy; HSTS max-age=300 for testing safety; racingpoint.cloud CORS exact match (security fix from .contains()); HTTPS listener via tokio::spawn with .into_make_service() (no ConnectInfo/rate-limiting on HTTPS port) (TLS-01, TLS-03, TLS-04, KIOSK-06)
- 69-02: failover_broadcast uses simple != for terminal_secret comparison (consistent with all existing service routes -- no subtle crate); split_brain_probe reqwest::Client created once before outer reconnect loop; guard probes :8090/ping with 2s timeout before honoring SwitchController (ORCH-02, ORCH-03)
- 69-01: ONE cycleOk boolean per 5s tick in HealthMonitor -- consecutiveFailures increments by exactly 1 per cycle, not per probe attempt; guarantees DOWN_THRESHOLD=12 = 60s sustained outage (HLTH-01, HLTH-02, HLTH-03 complete)
- 69-01: notify_failover via exec_request to Bono -- server .23 is down so James cannot use .23 email_alerts; FailoverOrchestrator delegates notification to Bono (ORCH-01, ORCH-04 complete)
- 69-03: Secondary watchdog timer: 255s after james_down (45s+255s=5min total) probes 100.71.226.83:8090/ping via Tailscale; skips if .23 reachable (not a venue outage); pm2 via execFileSync fallback restart->start; polls /health 6x before broadcast; AlertCooldown 10-min prevents repeat activations (HLTH-04 complete)
- 69-04: notify_failover tier AUTO (command itself delivers WhatsApp via Evolution API); EXEC_REASON injected as env var by ExecHandler#execute so notify_failover gets the failover reason string; buildSafeEnv() extended with Evolution API vars conditionally; notifyFn fixed to call sendEvolutionText directly; send-email.js stdlib-only with sendmail+SMTP fallback; email on both failover paths (ORCH-04 complete)
- 78-01: Defense-in-depth for DevTools: both --disable-dev-tools browser flag AND F12/Ctrl+Shift+I/J keyboard hook blocks; USBSTOR Start=4 disables mass storage only (HID unaffected); accessibility Flags 506/122/58 disable hotkeys not features (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)
- 74-01: AgentConfig fields all pub (not pub(crate)) for cross-module access in later extractions; load_config pub; validate_config + detect_installed_games pub(crate); billing_guard.rs required crate::config:: path fix after root extraction (DECOMP-01)
- 74-02: AppState fields all pub(crate) not pub -- crate-internal (matches config.rs pattern); crash_recovery bool renamed crash_recovery_startup to avoid collision with CrashRecoveryState inner-loop local; SelfHealResult (not HealResult) -- self_heal.rs uses that name; AiDebugSuggestion from rc_common::types (already a shared type); ws_tx/ws_rx stay loop-local (borrow conflict per RESEARCH.md Pitfall); DECOMP-02 complete
- 74-03: HandleResult::Break/Continue enum (not bool) for self-documenting loop control; anyhow::Result<HandleResult> for serde_json ? propagation; SwitchController params (primary_url/failover_url/active_url/split_brain_probe) passed separately to handle_ws_message -- outer-loop locals not in AppState; LaunchState + CrashRecoveryState made pub(crate) for ws_handler.rs cross-module access; Python file truncation deleted 972-line dead code block (lines 1699-2670); DECOMP-03 complete
- 78-03: Option<String> with #[serde(default)] for session_token -- backward compat with older agents; direct SQL UPDATE for emergency billing pause avoids circular HTTP dependency; LazyLock<Mutex<HashMap>> for per-pod security alert debounce (5min cooldown) (SESS-04, SESS-05)
- 89-02: format_wa_phone promoted to pub(crate) in billing.rs -- single phone formatting source for both billing and psychology modules; STREAK_GRACE_DAYS+7=14d total window for weekly visit streaks; send_pwa_notification uses DB-record pattern (not WebSocket), deferred to Phase 3 (FOUND-01, FOUND-02, FOUND-04)
- 89-03: psychology routes in staff_routes (JWT-protected) -- customer badge display deferred to Phase 90; evaluate_badges + update_streak called sequentially at end of post_session_hooks (already inside tokio::spawn); 5 seed badges use INSERT OR IGNORE -- idempotent across DB migrations; count extracted before into_iter().map() to avoid use-after-move (FOUND-02, FOUND-03, FOUND-04, FOUND-05 complete)
- 81-01: Non-AC crash recovery else branch: match last_sim_type to config.games field (7 variants), clone base_config, override args from last_launch_args, call GameProcess::launch() -- mirrors LaunchGame handler exactly (LAUNCH-02 complete)
- 81-01: DashboardEvent::GameLaunchRequested added at end of enum using existing SimType -- no new imports needed (LAUNCH-04 complete)
- 81-01: pwa_game_request uses extract_driver_id() in-handler (customer JWT); validates pod in state.pods + installed_games; fire-and-forget broadcast; no AppState mutation (LAUNCH-05 complete)
- 70-02: server_recovery uses prev === 'down' guard -- prevents spurious failback on degraded->healthy; only full outage recovery triggers failback sequence (BACK-01, BACK-03, BACK-04 complete)
- 70-02: sync failure does NOT block pod switchback -- sessions missed during export/import logged as syncError in Uday notify message; initiateFailback reuses same alertCooldown as initiateFailover
- 80-02: SHA-256 of admin_pin_hash stored in system_settings for change detection without duplicating sensitive hash; 24h check in alerter loop sends WhatsApp if >30 days (ADMIN-06)
- 80-02: HMAC verification in permissive mode initially -- warns but allows mismatches for deployment transition; GET signing uses reconstructed query string as body (AUTH-07)
- 97-01: pod_id: String (not u32) for PreFlightPassed/PreFlightFailed -- CONTEXT.md had u32 but RESEARCH.md identified deserialization-breaking mismatch; all existing AgentMessage variants use String
- 97-01: ClearMaintenance is a unit variant (no fields) -- CoreToAgentMessage is always routed to a specific pod via its WS connection, pod_id redundant
- 97-01: PreflightConfig follows KioskConfig serde(default) pattern exactly -- reuses existing default_true() fn (PF-07)
- 97-02: MockHidBackend defined locally in pre_flight::tests -- MockTestBackend from ffb_controller is inside private mod tests{}; local mock! avoids cross-module visibility issues
- 97-02: Orphan game state captured before AppState borrow in tokio::join! -- game_pid and has_game_process extracted as plain values to avoid lifetime issues with &AppState across await points
- 97-02: billing_active.store(true) at line 167 in ws_handler.rs -- confirmed AFTER pre_flight gate block (lines 141-165); customers on failed pod never billed (PF-01, HW-01, HW-02, HW-03, SYS-01 complete)
- 82-03: GameState union must include 'loading' for TypeScript to accept game_state === 'loading' comparisons in kiosk KioskPodCard; SIM_TYPE_LABELS + SIM_TYPE_OPTIONS module-level pattern for consistent sim_type display (BILL-03, BILL-05)
- 83-01: No production code changes needed — existing F1 25 adapter already satisfies TEL-F1-01/02/03; 6 unit tests added to prove it. adapter.connected=true set directly in session_type_mapping test to avoid binding port 20777 in unit test environment
- 98-01: failure_strings.clone() before AgentMessage send — keeps original for show_maintenance_required() in ws_handler; debug_server.rs exhaustive match needed MaintenanceRequired arm (Rule 1 auto-fix, caught immediately on first compile)
- 98-02: check_lock_screen_http_on(addr) helper for port-param testability (option b — cleaner); PreFlightPassed has only pod_id field (no timestamp) — corrected from plan snippet at compile time (Rule 1 auto-fix); Window not found returns Warn (advisory, not a blocker)
- 99-01: ws_connect_elapsed_secs passed as u64 parameter to run() — decouples pre_flight module from ConnectionState; Disk check Warn (not Fail) if C: not found — graceful on non-standard disk layouts; WS stability is Warn not Fail per NET-01 spec — advisory only (SYS-02, SYS-03, SYS-04, NET-01 complete)
- 99-02: Option<Instant> on AppState for PreFlightFailed cooldown — safe in single-threaded select! loop, no Arc/Mutex needed; retry loop does NOT send alerts (correct by design from 98-02, only logs + refreshes lock screen); reset to None on Pass ensures first failure after recovery always alerts (STAFF-04 complete)

### Blockers/Concerns

- Phase 73 billing_guard: `attempt_orphan_end` calls reqwest directly -- need to decide on trait-wrap vs callback param before writing tests. Callback param (option b) is simpler, avoid trait boilerplate.
- Phase 74 select! decomposition: enumerate all 14 mutable shared variables before first extraction step -- assign each to ConnectionState (inner loop) or ReconnectState (outer loop)
- v6.0 (Phases 36-40) still blocked on BIOS AMD-V -- does not affect v11.0
- 66-05: exec round-trip (INFRA-03) pending Bono deployment -- Bono notified via INBOX.md commits 3e4091a + 35cea4f, will self-verify once Bono pulls + restarts pm2
- 76-01: Permissive mode for initial staff JWT deploy -- logs unauthenticated requests without rejecting (expand-migrate-contract pattern)
- 76-01: StaffClaims uses role="staff" field -- customer JWTs lacking role field are auto-rejected by deserialization
- 76-01: api_routes() split into 4 tiers (public/customer/staff/service) with state parameter for middleware

- 78-02: kiosk_routes separated from staff_routes -- pods need JWT-protected kiosk endpoints (experiences GET, settings GET, pod-launch, book-multiplayer) but must not access admin routes; layer order JWT first (401) then pod source check (403) (KIOSK-07, KIOSK-05)

### Quick Tasks Completed

| # | Description | Date | Commit | Directory |
|---|-------------|------|--------|-----------|
| 260322-pa9 | Add UI consistency standing rule and camera dashboard keyboard navigation | 2026-03-22 | 84637515 | [260322-pa9-add-ui-consistency-standing-rule-and-cam](./quick/260322-pa9-add-ui-consistency-standing-rule-and-cam/) |
| Phase 151-menu-display P01 | 15 | 2 tasks | 4 files |
| Phase 151-menu-display P02 | 15 | 2 tasks | 4 files |
| Phase 159 P02 | 38m | 2 tasks | 4 files |
| Phase 152 P02 | 45 | 3 tasks | 2 files |
| Phase 160-rc-sentry-ai-migration P01 | 15 | 2 tasks | 2 files |
| Phase 153-inventory-alerts P01 | 9 | 2 tasks | 5 files |
| Phase 160 P02 | 20 | 2 tasks | 1 files |
| Phase 161-pod-monitor-merge P02 | 8 | 2 tasks | 2 files |
| Phase 162 P01 | 32 | 2 tasks | 4 files |
| Phase 154 P01 | 11 | 2 tasks | 3 files |
| Phase 154-ordering-core P02 | 167 | 2 tasks | 2 files |
| Phase 155-receipts-order-history P01 | 35 | 2 tasks | 3 files |
| Phase 156 P02 | 574 | 3 tasks | 2 files |
| Phase 157 P02 | 12 | 2 tasks | 5 files |
| Phase 158-marketing-content P01 | 25 | 2 tasks | 5 files |
| Phase 158-marketing-content P02 | 2 | 1 tasks | 2 files |
| Phase 170-repo-hygiene-dependency-audit P02 | 18 | 2 tasks | 16 files |
| Phase 170 P01 | 15 | 2 tasks | 4 files |
| Phase 170 P03 | 8 | 2 tasks | 6 files |
| Phase 172 P02 | 133 | 2 tasks | 3 files |
| Phase 173 P03 | 480 | 2 tasks | 7 files |
| Phase 173 P04 | 2 | 2 tasks | 10 files |
| Phase 174 P01 | 5 | 2 tasks | 2 files |
| Phase 176 P02 | 95 | 2 tasks | 12 files |

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat -- needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Clean up .planning/update_roadmap_v9.py after v9.0 (can delete)
- Clean up .planning/update_roadmap_v11.py after v11.0 roadmap creation (can delete)

## Session Continuity

Last session: 2026-03-24T04:19:08.074Z
Stopped at: Completed 176-02-PLAN.md
Resume file: None

---
gsd_state_version: 1.0
milestone: v16.0
milestone_name: Security Camera AI & Attendance
status: roadmap_created
stopped_at: Roadmap written -- awaiting plan-phase 112
last_updated: "2026-03-21T22:00:00.000Z"
progress:
  total_phases: 8
  completed_phases: 0
  total_plans: 27
  completed_plans: 0
  percent: 0
current_phase: 112
current_phase_name: RTSP Infrastructure & Camera Pipeline
phases:
  - "112: RTSP Infrastructure & Camera Pipeline (CAM-01, CAM-02, CAM-03, CAM-04)"
  - "113: Face Detection & Privacy Foundation (FACE-01, PRIV-01)"
  - "114: Face Recognition & Quality Gates (FACE-02, FACE-03, FACE-04)"
  - "115: Face Enrollment System (ENRL-01, ENRL-02)"
  - "116: Attendance Engine (ATTN-01, ATTN-02)"
  - "117: Alerts & Notifications (ALRT-01, ALRT-02, ALRT-03)"
  - "118: Live Camera Feeds (MNTR-01)"
  - "119: NVR Playback Proxy (MNTR-02)"
decisions:
  - "Phases 112-119 start numbering -- continues from Phase 111 (v15.0 last phase)"
  - "Local face recognition on RTX 4070 (SCRFD + ArcFace via ort) -- NO cloud API"
  - "NVR at .18 handles all recording -- no separate recording pipeline"
  - "Separate service (rc-sentry-ai) feeding into racecontrol dashboard"
  - "Face-only staff auth (no PIN) -- cameras already at entry points"
  - "DPDP consent framework in Phase 113 -- before any face data collection"
---

---
gsd_state_version: 1.0
milestone: v13.0
milestone_name: Idle Health Monitor
status: in_progress
stopped_at: Completed 138-01-PLAN.md
last_updated: "2026-03-22T04:20:00.000Z"
current_phase: 138
current_phase_name: idle-health-monitor
progress:
  total_phases: 1
  completed_phases: 0
  total_plans: 3
  completed_plans: 1
decisions:
  - "138-01: IdleHealthFailed placed after PreFlightFailed for logical locality with health-check variants"
  - "138-01: consecutive_count uses u32 (not usize) for cross-arch serde stability"
---

---
gsd_state_version: 1.0
milestone: v16.1
milestone_name: Camera Dashboard Pro
status: in_progress
stopped_at: Roadmap created — no plans executed yet
last_updated: "2026-03-22T00:00:00+05:30"
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
decisions:
  - "145: NVR coexistence strategy (snapshot-through-go2rtc vs pause-during-WebRTC) to be decided on live hardware in Phase 145"
  - "146: All mutable user preferences go to camera-layout.json; rc-sentry-ai.toml is read-only at runtime"
  - "147: teardownRtc() written before any connection-opening code — singleton activePeerConnection pattern enforced"
  - "147: CSS grid class swap for layout switching — tiles never destroyed/recreated across mode changes"
  - "148: Next.js localStorage reads only inside useEffect with hydrated flag — never in useState initializer"
---

---
gsd_state_version: 1.0
milestone: v19.0
milestone_name: Cafe Inventory, Ordering & Marketing
status: in_progress
stopped_at: Roadmap created — no plans executed yet
last_updated: "2026-03-22T00:00:00+05:30"
progress:
  total_phases: 10
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
decisions: []
blockers:
  - "Printer model confirmation needed before Phase 155"
  - "WhatsApp marketing channel decision (separate number) needed before Phase 158"
  - "Countable vs uncountable item list from Uday before Phase 152 data entry"
---


---
gsd_state_version: 1.0
milestone: v21.0
milestone_name: Cross-Project Sync & Stabilization
status: in_progress
stopped_at: Roadmap created - ready to plan Phase 170
last_updated: "2026-03-23T12:00:00+05:30"
current_phase: 170
current_phase_name: Repo Hygiene & Dependency Audit
progress:
  total_phases: 6
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
  percent: 0
decisions: []
blockers:
  - "Phase 171 (Bug Fixes) requires pods to be online for BUG-02/BUG-04 verification"
  - "Phase 175 (E2E) requires both POS (:3200) and Kiosk (:8000/:3300) running"
---
