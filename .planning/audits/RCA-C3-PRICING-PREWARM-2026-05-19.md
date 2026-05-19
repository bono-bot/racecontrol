# §S-146 5-section RCA — C.3 Pricing Prewarm

**Authored:** 2026-05-19 ~14:55 IST by James
**Scope:** Coordinator bundle `rp-v2-apps/coordinator/JAMES-SETUP/pricing-prewarm/` applied to `racecontrol/crates/racecontrol/src/background_tasks.rs`
**Trigger:** §S-146 V1↔V2 RCA gate fires — modifying V1-era code (background_tasks.rs) to support V2-customer-flow reliability (C2 gap closure per JAMES-1 caveat 2)
**Gate authority:** CD-2 scoped flip RATIFIED per `rp-v2-apps/coordinator/CAPTAIN-RATIFY-PHASE-A-2026-05-19.md` lines 254-265 (Captain answers recorded 2026-05-19 13:45 IST)
**Eligibility for §S-186 fast-lane:** NO — PR created today (post-2026-05-09); change is gap-closure (feature class), not bug fix. Full 5-section RCA required.
**Foundational-boundary escalation:** NO — pricing-cache cold-start is reliability/availability, not billing/wallet/auth/pod-state-channel/WhatsApp-identity/DB-schema. MMA Step 1 DIAGNOSE not required.

---

## §1 — Boundary map

Where V2 crosses into V1 at this change surface.

| Element | Path : line | V1/V2 | Role |
|---|---|---|---|
| Modified file | `crates/racecontrol/src/background_tasks.rs:23-29` (spawn_all body) | V1 | Adding one spawn-call line + helper fn at end of file |
| Modified file | `crates/racecontrol/src/background_tasks.rs:1-18` (module imports) | V1 | NO new imports added — uses already-imported `Arc`, `Duration`, `cloud_sync`, and the workspace `rand` already used at line 361 (`spawn_staff_pin_rotation`) |
| V1 function reused | `crates/racecontrol/src/cloud_sync_pull.rs:257` — `pub(crate) async fn pull_tables_now(state: &Arc<AppState>, tables: &[&str]) -> anyhow::Result<()>` | V1 | Existing function already called from `api/staff_pin_sync.rs:132, 219` with same signature — proven call-shape |
| V1 function reused | `crates/racecontrol/src/cloud_sync.rs:30` — `pub(crate) use crate::cloud_sync_pull::pull_tables_now;` | V1 | Re-export gives `cloud_sync::pull_tables_now` access from `background_tasks.rs` via the existing `racecontrol_crate::cloud_sync` wildcard import |
| Workspace dependency | `crates/racecontrol/Cargo.toml:68` — `rand = { workspace = true }` | V1 | Already a workspace dep — no Cargo.toml change |
| Data path touched | DB table `pricing_tier` (per `cloud_sync_pull.rs:333` upsert) | V1 schema, V2 read-path consumer | Prewarm causes earlier population; semantics of the table unchanged |
| V2-customer-flow tie-in | JAMES-1 caveat C2 — first-access pricing-cache gap during Internet outage breaks pod-launch for any pricing-tier not yet cached | V2 customer flow | C2 is the failure mode this change closes |

**No DB migration. No protocol change. No new API endpoint. No schema field added. No new feature flag. No new env var.**

---

## §2 — Inherited-issue catalogue

V1 bugs / footguns / races touching this same boundary, sourced from `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` candidate categories A-J + §S-61 PART 41 + LOGBOOK grep for `background_tasks.rs|cloud_sync|pricing_tier|pull_tables_now`.

| # | V1 issue | Source | Boundary touched? |
|---|---|---|---|
| 1 | `cloud_sync::spawn` runs as detached `tokio::spawn` with no lifecycle log on entry — silent task death class (Standing Rule #LongLivedTasksMustLogLifecycle) | racecontrol/CLAUDE.md → Standing Rules → Debugging | Adjacent — same spawn pattern. New code follows the rule (lifecycle log on task entry: `"pricing-prewarm task started (24h ± 15min cadence)"`) |
| 2 | `cloud_sync` upsert paths can fail silently with `tracing::warn!` and continue (e.g., `cloud_sync_pull.rs:313, 323, 333`) | direct read of cloud_sync_pull.rs | Same — by design (best-effort sync). New code inherits this property explicitly and is documented as best-effort |
| 3 | Cloud-authoritative pricing has been a recurring V1 footgun: pricing-tier reads against stale local cache for unfamiliar SKUs returned `None`/empty, causing customer-launch failures | JAMES-1 Q-J1 + caveat C2 | Direct — this change is the closure of that footgun |
| 4 | DB migration order issue: `pricing_tier` table CREATE pre-dates V2 planning (2026-05-01) | racecontrol/CLAUDE.md → Standing Rules → DB migrations must cover ALL consumers | Adjacent — schema unchanged by this PR; just adding earlier-populator |
| 5 | `tokio::time::interval` vs `tokio::time::sleep` race conditions when system clock jumps — interval drifts on system suspend/resume, sleep doesn't | racecontrol/CLAUDE.md → Code Quality general principle | Adjacent — new code uses `sleep` not `interval`, deliberate per cadence requirements ("24h ± 15min jitter centered" requires recomputing per-tick interval, which `interval` doesn't natively support) |
| 6 | `cloud_sync::pull_tables_now` per `api/staff_pin_sync.rs:132` is called from the API handler in a synchronous-await pattern (blocking the handler) | direct read of staff_pin_sync.rs | Different boundary — handler call. New code is detached spawn (correct pattern for background prewarm) |
| 7 | Server .23 racecontrol cold-start time 30-120s — observable per `feedback_capability_claim_without_probe_20260514.md` Anchor #2 (.23 racecontrol crash false-DOWN claim during cold-start window) | feedback_capability_claim_without_probe_20260514.md | Adjacent — new code's 60s startup-delay aligns the first prewarm hit AFTER cold-start completes. No additional cold-start path created |
| 8 | Workspace dep `rand` — historically `rand::random::<u64>()` blocking on entropy starvation on cold-boot Windows server | racecontrol/CLAUDE.md → not documented as past bug here; defensive note | Adjacent — `rand::random::<u64>() % window_ms` runs after 60s startup delay, after first tick — entropy pool warm. Same pattern as `spawn_staff_pin_rotation` at line 360 which is already in production using `rand::Rng` |

---

## §3 — Past-bug disposition

For each catalogued issue, classify per §S-146 Section 3 schema.

| # | Disposition | Justification |
|---|---|---|
| 1 | `ROOT-CAUSED-AND-FIXED` | This PR inherits the lifecycle-log discipline (line 1 of task body emits `tracing::info!`). Same fix posture as recent `launch-state-prune` task (line 98 of `background_tasks.rs`) |
| 2 | `NOT-APPLICABLE-TO-V2` | By-design property. Best-effort prewarm is the kaizen-correct choice: a failed prewarm degrades to "cache stays at previous state" — no V2 customer-flow regression (worst case is identical to no-prewarm) |
| 3 | `ROOT-CAUSED-AND-FIXED` | This PR is the fix. JAMES-1 caveat C2 identified the root cause as "first access happens at customer launch during outage"; this change moves first access to a scheduled cron, decoupled from customer flow |
| 4 | `NOT-APPLICABLE-TO-V2` | Schema unchanged. The table existed pre-2026-05-01 and continues to be consumed by V2 reads via cloud_sync. New code adds another producer to an existing consumer-pattern |
| 5 | `NOT-APPLICABLE-TO-V2` | Deliberate choice: `sleep`-with-recomputed-next is correct for the ± jitter requirement. `interval` would not give us a centered jitter window |
| 6 | `NOT-APPLICABLE-TO-V2` | Different call-site (handler vs. detached). New code does NOT introduce blocking-handler pattern |
| 7 | `ROOT-CAUSED-AND-FIXED` | Cold-start delay (60s) explicitly addresses this. RCA aligns with `feedback_capability_claim_without_probe_20260514.md` Anchor #2 root cause analysis |
| 8 | `NOT-APPLICABLE-TO-V2` | Same crypto-rng pattern proven in production at `spawn_staff_pin_rotation`. If `rand::random::<u64>()` ever blocks, both prewarm and staff-PIN rotation would be affected — but they aren't (no past incident logged) |

---

## §4 — V2-alignment delta

What this boundary SHOULD look like under V2 doctrine, and how the change moves it there.

**V2 doctrine reference:** `project_v2_customer_workflows_consolidated_20260503.md` + `project_v2_core_product_definition.md` + `feedback_v2_lbac_v0.1_active.md` + OFFLINE-RESILIENCE doctrine at `rp-v2-apps/coordinator/DOCTRINES/2026-05-19-OFFLINE-RESILIENCE.md`.

**Tier A (offline-resilience floor) requirement:** the customer-launch write path on James (.23) must succeed end-to-end on LAN with zero Internet. Current pricing-cache "first-access cold cache" violates Tier A for any pricing-tier not yet read locally.

**Pre-change boundary state:**
- Pricing tier cached on first access (V1-era behavior)
- Internet drop BEFORE first-access to any SKU = customer-launch write fails for that SKU
- Failure mode: visible to customer as "no local price for SKU X" — pod-launch refuses to proceed
- Frequency: low (requires a new pricing tier rolled out cloud-side that hasn't been read locally)
- Severity: high when triggered — blocks customer launches for that SKU until Internet returns

**Post-change boundary state:**
- Pricing tier prewarmed nightly via scheduled cron (24h ± 15min jitter)
- + Initial prewarm 60s after racecontrol startup (catches the case where racecontrol just restarted and prewarm hasn't run yet)
- Internet drop after prewarm succeeded = customer-launch reads the prewarmed cache = no failure
- Worst case: Internet was already down when racecontrol started AND a new pricing tier was rolled out during the outage AND a customer requests exactly that SKU — same failure mode as pre-change, scope narrowed from "any SKU ever" to "new SKUs rolled out during outage"

**Delta:** moves Tier A pricing resilience from "untested cold-cache + customer-flow blocking on miss" to "warm-cache by default + customer-flow blocking only on the narrow new-SKU-during-outage edge case". Aligns with OFFLINE-RESILIENCE doctrine Tier A1 ("racecontrol heart self-sufficient on LAN").

---

## §5 — V2-framed proposal

**The change is V2-doctrine-positive.** It tightens Tier A floor without introducing V1-shaped antipatterns (no point-to-point hardcoded sync, no manual op replacing automated, no new silos). It uses existing V1 infrastructure (`cloud_sync::pull_tables_now`) at a new cadence, in service of V2 customer-flow reliability.

**Scope as authored:**
- 1 spawn-call line added in `spawn_all` (near existing `cloud_sync::spawn` call at line 193)
- 1 helper `fn spawn_pricing_prewarm(state: Arc<AppState>)` added at end of file (~50 LOC)
- Zero schema changes, zero protocol changes, zero migrations, zero new dependencies
- Patches by intent (not by line) since reference patch's `spawn_all` signature didn't match actual 5-arg shape

**V2-alignment statement:** This change moves the boundary toward V2 OFFLINE-RESILIENCE doctrine Tier A1. Anchor: doctrine Architectural Lock #4 (`Each pod ships with a static maintenance HTML`) sits adjacent — this PR addresses a Tier A racecontrol-heart resilience gap that complements Lock #4's pod-side resilience gap.

**Test plan (for the cargo build + deploy + verify cycle):**
- `cargo build --release -p racecontrol` — proves the patched module compiles + links
- After `deploy-server.sh`: `tail -100` racecontrol log at .23, look for `pricing_prewarm` target — first line `"pricing-prewarm task started"` should appear immediately after startup; second line `"pricing table prewarmed from cloud_sync"` should appear ~60s after startup
- 24h soak: log search for any `WARN pricing_prewarm` after the first cycle would indicate Internet-egress issues (not a code bug — expected on Internet-down)
- The bundle's `smoke-pricing-cache.sh` confirms `pricing_tier` table has ≥1 local row with `updated_at ≤ 25h old` post-apply

**Rollback:** revert the commit + redeploy via `deploy-server.sh`. Per the bundle README: "Hard-cut; no rollback ladder needed" because the task is idempotent and lifecycle-bounded (panic in the spawn-task does not affect any other tokio task; the prewarm just stops happening, reverting to current pre-PR cache behavior).

**Permanence:** the change is in source code (this branch → commit → merge to main → next `deploy-server.sh` invocation). Will survive redeploy.

**Universal-sync targets:** racecontrol-only PR. No comms-link, no bono memory, no harness changes. JAMES-INBOX-RESPONSE `coordinator/JAMES-INBOX-RESPONSE.md` updated separately (JAMES-4 already pushed at 14:42 IST commit `42397c6`).
