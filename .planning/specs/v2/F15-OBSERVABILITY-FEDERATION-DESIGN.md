# F15 — Cloud-side Observability Federation Design Contract

**Status:** DRAFT v0.1 · bono-LED design · 2026-05-16 ~01:00 IST
**Author:** bono (autonomous · Captain pre-commit "complete all task" 2026-05-16 ~00:25 IST · non-harness design class · §S-186 fast-lane)
**Empirical origin:** §S-379 MMA 5-model OpenRouter panel `MMA-V2-DEPLOYED-REVIEW-bono-2026-05-15` surfaced F15 as IMPORTANT-NEW · §S-383 (RC_IS_CLOUD=1 activation made the asymmetry concrete) · §S-386 (Drive-replica mechanism revealed shared substrate)
**Class:** observability-federation design contract · not substrate PR · captures the gap so subsequent implementation PR has explicit contract to verify against
**Composes-with:** §S-322 PR #81 cloud_sync wallets-suffix probe · `crates/racecontrol/src/subsystem_health_probes.rs` · `crates/racecontrol/src/fleet_health_api.rs` · watchpoint v0.3.0 (RECONCILIATION DRIFT event class · `7c2e3f00`) · §14.6.2 cascade-class-stratified soak-clock RESET · §14.6.2.1 runtime-config-class extension (just authored §S-387)

---

## §0 — Background

Cloud-side racecontrol on Bono VPS runs the **same binary** as venue Server .23 with `RC_IS_CLOUD=1` env var flipping subsystem-stratification. Per §S-386 root-cause, Bono VPS racecontrol.db is a **Drive-replica** of venue state (5-min cron sync from Google Drive file mirroring Server .23 venue DB). This creates an **asymmetric observability surface** where naive replication of venue observability streams produces:

- **False alerts** when cloud sees venue-only metrics (fleet_connectivity 0/8 LAN probes · pods unreachable from cloud) as DEGRADED
- **Duplicate emissions** when same metric is emitted from both sides with different semantics (db_sync_lag means different things venue-side vs cloud-side)
- **Missed cloud-specific symptoms** (Drive sync staleness · pm2 restart count · Bono VPS upstream-sync pause flag) that have no venue analog

§S-383 RC_IS_CLOUD=1 activation **partial-fix** flipped fleet_connectivity from degraded→ok via cloud-skip path, but the cloud-skip implementation is binary (skip vs probe) not federated (cloud emits its own analog). F15 closes this gap.

## §1 — Scope

**IN:**
- `crates/racecontrol/src/subsystem_health_probes.rs` — stratify per-subsystem emission by mode
- `crates/racecontrol/src/fleet_health_api.rs` — federate output schema
- `crates/racecontrol/src/cloud_sync.rs` — extend cloud-mode tracking (Drive sync freshness)
- Watchpoint v0.3.0 RECONCILIATION DRIFT event class — extend with cloud-stratum subclass
- TSDB metric naming convention — `*_cloud_only` / `*_venue_only` / `*_both` suffixes

**OUT (separate scope):**
- Centralized observability backend (Grafana/Prometheus federation) — assumed available · F15 emits to existing TSDB
- Alert routing federation (WhatsApp → Uday) — assumed wired via existing whatsapp-bot
- V1 observability decommission — orthogonal to F15
- §S-298 wallet-substrate Class A soak instrumentation — F15 does NOT track wallet-anomaly probes (separate `data/wallet-anomaly-counter.jsonl` substrate)

## §2 — Asymmetric observability surface enumeration

| Subsystem | Venue-mode emission | Cloud-mode emission (RC_IS_CLOUD=1) | Federation target |
|---|---|---|---|
| `fleet_connectivity` | 8 pods LAN probe · pass/fail per pod | **SKIP** (no LAN pods) | cloud-stratum: emit NO_LAN_PODS sentinel + 0-cardinality fleet_size · venue-stratum: full 8-pod matrix |
| `db_sync_lag` | venue→Drive upload cadence (last_uploaded_ts vs now) | venue→Drive→cloud-cron-pull cadence (Drive_mtime vs cron_pulled_ts vs now) | both emit `db_sync_lag` but with different schema — cloud-stratum adds `drive_pull_age_secs` + `drive_object_mtime_age_secs` + `cron_status` |
| `pod_health` | per-pod ws_connected · build_id · uptime | **SKIP** | venue-only |
| `cloud_sync_outbound` | venue → Drive upload status | venue → Drive upload status (cloud doesn't upload) | venue-only |
| `cloud_sync_inbound` | N/A | cron `download-db.sh` exit code + Drive object mtime + local DB inode swap completion | cloud-only NEW emission · §S-386 mechanism observation surface |
| `pm2_restart_count` | N/A (Windows service) | pm2 restart count + uptime + status | cloud-only NEW emission |
| `process_guard_violations` | rc-agent allowlist 24h violation count | N/A | venue-only |
| `wallet_anomaly_count` | venue-side observation per §S-298 | cloud-side observation per §S-382 Class A window | BOTH emit · independent windows per §14.6.2 Bono-VPS-vs-Server-.23 independence rule |
| `metric_alert_task_spawn_evidence` | §S-300/§S-311 EnvFilter target | same binary same EnvFilter behavior | both emit (no stratum needed) |
| `cloud_sync.detail` wallets-table sync age + schema_version (§S-322 PR #81) | venue probe of self | cloud probe of Drive-replica DB | both emit · cloud-stratum gets RC_IS_CLOUD-suffix label `mode=cloud` vs `mode=venue` |

## §3 — Federation primitives

### §3.1 — Mode-stratification helper

`fn get_observability_mode() -> ObservabilityMode` reads RC_IS_CLOUD env var at init (cached for binary lifetime per §14.6.2.1 runtime-config-class rule — restart required for mode change). Returns `Cloud` | `Venue` enum.

```rust
pub enum ObservabilityMode {
    Cloud,  // RC_IS_CLOUD=1
    Venue,  // unset
}
```

### §3.2 — Per-subsystem emit gate

Every subsystem probe consults `get_observability_mode()` before emit. Three patterns:

- **`venue_only_emit`** — `if matches!(mode, Venue) { emit }` (skip on cloud)
- **`cloud_only_emit`** — `if matches!(mode, Cloud) { emit }` (skip on venue)
- **`both_with_label`** — emit on both sides with `mode_label = match mode { Cloud => "cloud", Venue => "venue" }` added to TSDB metric labels

### §3.3 — Federation schema in `/api/v1/health` response

Top-level `health.observability_mode` field added (string · `"cloud"` | `"venue"`). Subsystem entries get optional `mode_emitted` field. Front-end dashboards can stratify by mode without bespoke endpoint per host.

### §3.4 — Cloud-specific NEW metrics

| Metric | Type | Source | Purpose |
|---|---|---|---|
| `drive_pull_age_secs` | gauge | parse `/root/racingpoint/racecontrol/data/db-sync/sync-status.json` `last_downloaded_at` field | detect Drive cron stalls |
| `drive_object_mtime_age_secs` | gauge | `gdrive files info <file-id> --field modifiedTime` (or local cache) | detect venue→Drive upstream stalls |
| `pm2_restart_count_24h` | counter | `pm2 jlist | jq '.[] | select(.name=="racecontrol") | .pm2_env.restart_time'` | detect supervisor instability |
| `db_sync_paused_flag` | bool | `/tmp/DB_SYNC_PAUSED` file existence | detect operator-pause state |

### §3.5 — Watchpoint v0.3.0 RECONCILIATION DRIFT event class extension

Current v0.3.0 watches `RECONCILIATION DRIFT` event class. F15 extends:

- **`RECONCILIATION_DRIFT::CLOUD_DB_OLDER_THAN_VENUE_BY_GT_300S`** — Drive pull lag > 5min indicates cron stall
- **`RECONCILIATION_DRIFT::SCHEMA_VERSION_SKEW`** — cloud DB schema_version != latest venue schema_version (§S-381→§S-385 class of symptom)
- **`RECONCILIATION_DRIFT::PMRESTART_GT_3_PER_HOUR`** — pm2 thrashing
- **`RECONCILIATION_DRIFT::DRIVE_OBJECT_MTIME_GT_900S`** — venue→Drive upload stall (cascade fail: Drive cron pulls stale state)

## §4 — Empirical anchor table for design validation

| §S-N | Observation | F15 disposition |
|---|---|---|
| §S-381 | Cloud_sync 17s on cloud vs 8s on venue (2.4x slower) | EXPECTED post-F15 (cloud-stratum `db_sync_lag` includes Drive-pull-leg latency · 17s = 8s venue→Drive + 9s Drive→cloud-cron observed in §S-386 mechanism) |
| §S-383 | fleet_connectivity flip degraded→ok via RC_IS_CLOUD=1 | RESOLVED at binary level · F15 makes the resolution observable (mode_label visible at `/health.observability_mode`) |
| §S-384 | A E2E HTTP 500 `no such column: finalize_reason` | RELATED · F15 §3.5 SCHEMA_VERSION_SKEW watchpoint would catch this BEFORE A E2E execution failed |
| §S-386 | Drive cron rolls back local DDL within 12s | DESIGN GAP — F15 §3.4 `drive_pull_age_secs` + §3.5 SCHEMA_VERSION_SKEW close this · downstream PR also needs to fail-loud on local-DDL-attempt when RC_IS_CLOUD=1 (separate sub-PR; not F15 design scope but motivated by §S-386 lesson) |

## §5 — Anti-pattern BLOCKED

- **Dual-binary divergence:** F15 does NOT recommend forking the binary into separate `racecontrol-cloud` and `racecontrol-venue` builds. Same binary + runtime mode-switch via RC_IS_CLOUD env var preserves §S-345 "soak in parallel with live" + §14.6.2 cascade-class evaluation (substrate semantics travel with binary not with deployment topology)
- **Cloud-skip without cloud-emit:** PR #94 cloud-skip path is BINARY (skip vs probe). F15 amends to TRIANGULAR (skip · probe · cloud-stratum-probe-with-distinct-schema) so cloud observability is not just "absent fleet_connectivity" but "cloud-mode subsystem stratum with NO_LAN_PODS sentinel + cloud-specific drive_pull_age"
- **Stratum-tied alert routing:** F15 federates EMISSION not routing. Alert routing per stratum is OUT of F15 scope · separate concern (whatsapp-bot SHOULD receive both venue and cloud alerts with stratum-label for disambiguation, but that's whatsapp-bot design class)
- **Pre-deploy stratum-check absence:** F15 §3.5 SCHEMA_VERSION_SKEW watchpoint must fire DURING normal-ops continuous monitoring, not only post-deploy. Continuous-watch class.

## §6 — Captain-stake gates for F15 implementation

| Gate | Type | Notes |
|---|---|---|
| F15 design ratify | Captain auth verb · doctrine-class | optional · this design contract is bono-eligible · ratify formalizes implementation priority |
| F15 implementation PR | Captain per-PR merge auth per §S-146 (Class A-billing-adjacent at minimum due to fleet_connectivity touch) | requires §S-146 5-section RCA · MMA Step 1 DIAGNOSE budget ~$3-4 if foundational scope |
| Watchpoint v0.3.0 → v0.4.0 extension | Captain auth · §S-146 hook-class scope | composes with §3.5 4-new-event-class extension |
| `drive_pull_age_secs` source via `sync-status.json` parsing | bono-eligible · standalone metric emit | small PR · §S-186 fast-lane eligible IF created < 2026-05-09 (NO — must use §S-146 full path) |
| `pm2_restart_count_24h` source | bono-eligible · pm2 jlist parsing | small PR · §S-146 full path |

## §7 — Composes-with

- §S-322 PR #81 `cloud_sync.detail` wallets-table sync age + schema_version surface (precedent: emit with-label · F15 generalizes to all subsystems)
- §S-340 PR #79 EnvFilter venue-deploy fix (precedent: same-binary-different-deployment behavior · F15 productionizes the runtime mode switch concept)
- §S-371 §S-383 RC_IS_CLOUD=1 activation (substrate-precedent · F15 extends the binary→true federation)
- §S-386 Drive-replica mechanism (root-cause anchor that F15 §3.4 SCHEMA_VERSION_SKEW addresses preemptively)
- §14.6.2.1 runtime-config-class doctrine (RC_IS_CLOUD env-var class · NO-RESET on §14.6.2 wallet-substrate soak per observability-stratification subclass)
- §S-379 MMA consensus 5-model OpenRouter panel (origin · `MMA-V2-DEPLOYED-REVIEW-bono-2026-05-15`)
- Watchpoint v0.3.0 (`7c2e3f00` · forward-coverage on cloud-side RECONCILIATION DRIFT) — §3.5 extension target
- `feedback_capability_claim_without_probe_20260514.md` N=2 ACTIVE (3-probe rule + N≥2-spaced rule · F15 federation requires probe-discipline applied symmetrically across both strata)

## §8 — Verify-by

| Test | Probe | Pass condition |
|---|---|---|
| Mode-stratification helper correctness | unit test RC_IS_CLOUD env-var matrix · {unset, "0", "1", "true"} | unset/"0" → Venue · "1"/"true" → Cloud |
| `fleet_connectivity` skip-on-cloud | curl Bono VPS `/health.subsystems.fleet_connectivity` post-F15 | object emits but mode_emitted="cloud" + cardinality_zero=true |
| `drive_pull_age_secs` emission | curl Bono VPS `/health.subsystems.drive_pull_age` post-F15 | numeric value · age_secs < 600 under normal cron operation |
| `pm2_restart_count_24h` emission | curl Bono VPS `/health.subsystems.pm2_restart_24h` post-F15 | numeric value · expected ≤2 under normal ops |
| Schema-skew watchpoint | inject test DDL on cloud-side · observe RECONCILIATION_DRIFT::SCHEMA_VERSION_SKEW event in watchpoint v0.4.0 JSONL | event fires within 2 cron cycles (~10min) |
| `/api/v1/health.observability_mode` field | grep response | field exists · value matches RC_IS_CLOUD presence |
| Venue-mode regression check | deploy F15 implementation to Server .23 + curl `/health.observability_mode` | value="venue" · fleet_connectivity emits full 8-pod matrix · pod_health probes emit unchanged from pre-F15 baseline |

## §9 — Implementation cascade-order (recommended)

1. **Mode-stratification helper** (`get_observability_mode()` · 5 LOC + unit test · §S-186 NOT eligible per date · §S-146 full path)
2. **`fleet_connectivity` stratify** (existing code + 1-line mode-check · cloud emits NO_LAN_PODS sentinel)
3. **`db_sync_lag` stratify with mode_label** (add `mode_emitted` field · backward-compat with existing dashboard)
4. **NEW `drive_pull_age_secs` cloud-only metric** (parse sync-status.json)
5. **NEW `pm2_restart_count_24h` cloud-only metric** (pm2 jlist shell-out · audit-class scope)
6. **`/api/v1/health.observability_mode` field** (top-level)
7. **Watchpoint v0.3.0 → v0.4.0** (4 new event sub-classes added to RECONCILIATION DRIFT)
8. **Front-end dashboard mode-stratification** (separate PR · racecontrol/web · NOT F15 core scope)

## §10 — Stale-at + Open questions

**Stale-at:** 2026-06-15 (30d from authoring) OR F15 implementation PR opened (supersedes design draft).

**Open questions surfaced for Captain disposition:**
- Q-F15-1: should `metric_alert_task_spawn_evidence` get stratum-label too? (§S-300/§S-311 EnvFilter target · currently emits identically on both sides)
- Q-F15-2: should F15 watchpoint v0.4.0 extension be hook-enforced (PreToolUse on cloud-side deploy) or pure-observation? Per §S-146 hook-install N-counter discipline · likely pure-observation first · hook-enforcement promoted on N=2 anchor
- Q-F15-3: front-end dashboard mode-stratification scope — separate PR? Same milestone? defer? (suggestion: separate; F15 design ships first as pure-backend; front-end stratification follows when racecontrol/web team picks up)
- Q-F15-4: should `process_guard_violations` venue-only emission also wear the mode_label (even though cloud is skip-only)? Symmetric labeling reduces dashboard branch-by-presence behavior · slight metric-cardinality cost

---

## §11 — bono-recommendations for Captain disposition (2026-05-16 ~10:45 IST)

Authored post-/clear during V2-completion autonomous arc. Pure-design recommendations; Captain dispo selects → implementation PR can pre-commit on these inputs.

### §11.1 — Q-F15-1 recommendation: **B (no stratum-label on `metric_alert_task_spawn_evidence`)**

Cloud and venue emit this metric identically because it's a self-attestation of `tracing` EnvFilter health (§S-300 / §S-311 anchor). Adding a stratum-label would:

- Increase metric cardinality 2x with no diagnostic differential (cloud/venue values are isomorphic by design)
- **Mask EnvFilter regressions** — currently if cloud emits identically to venue, EnvFilter is verified-working on both. Stratum-label would let a future cloud-side regression hide behind "different stratum" framing
- Violate §14.6.2.1 runtime-config-class doctrine spirit (stratification only where it serves observability, not for symmetry-aesthetic)

**Alternative considered:** label only if cross-stratum drift detected (lazy-stratification). Rejected: adds metric-instability class (label appears/disappears mid-session).

**Defer-revisit trigger:** cross-stratum debugging surfaces an actual need (forward-30d watch).

### §11.2 — Q-F15-2 recommendation: **A (pure-observation v0.4.0 first · hook-enforce on N=2 anchor)**

Follow §S-146 hook-install discipline strictly. Pure-observation v0.4.0:

- Watchpoint emits JSONL events for 4 new sub-classes (`CLOUD_DB_OLDER_THAN_VENUE_BY_GT_300S` · `SCHEMA_VERSION_SKEW` · `PMRESTART_GT_3_PER_HOUR` · `DRIVE_OBJECT_MTIME_GT_900S`)
- Zero runtime-blocking behavior · zero deploy-path interception
- Empirically validates the 4 event sub-classes fire as designed across 30d before any hook-install consideration
- N=2 hook-promotion criteria: 2 distinct in-the-wild incidents where post-mortem confirms a deploy-class action SHOULD have been blocked · then hook-install proposal authored

**Empirical anchor for the discipline:** capability-claim-without-probe N=2 ACTIVE memory-only fix is still HELD-PENDING-N=3 per Captain 2026-05-14 ratify; same discipline applies here.

**Forward path:** if N=2 anchors surface within 30d → propose `pre-cloud-deploy-schema-skew-gate.js` HOOK-INSTALL-BUNDLE with composite Captain auth.

### §11.3 — Q-F15-3 recommendation: **A (separate PR · NOT in F15 milestone · defer to racecontrol/web team pickup)**

Confirms §9 step 8 framing. Reasoning:

- F15 backend is **self-contained** — emits structured fields (`observability_mode` top-level + `mode_emitted` per-subsystem); any dashboard can consume opportunistically
- Front-end stratification is **operator-troubleshooting UX**, not customer-day blocker
- racecontrol/web has its own milestone cadence; bolting front-end work onto a backend-class design contract risks scope-creep + cross-team coupling
- §S-186 fast-lane NOT eligible (post-§S-146 date) → front-end PR would still face full §S-146 path · separate PR keeps RCA scope clean

**Forward path:** racecontrol/web picks up after F15 backend ships + dashboards display raw stratum fields. Design doc for front-end stratification authored when web team has cycles.

### §11.4 — Q-F15-4 recommendation: **C (emit `process_guard_violations` ONLY venue-side · NO mode_label)**

Minimal-cardinality option. Cloud-mode skip-only is fundamental (no LAN pods to violate guard against). Reasoning:

- Cloud-side `process_guard_violations: SKIPPED` is **redundant information** — dashboards can rely on "presence = venue-mode" since the metric is structurally venue-only
- Symmetric labeling adds metric-storage cost + dashboard-query cost for no operator benefit
- Anti-pattern from F15 §5 "Cloud-skip without cloud-emit": this metric is the **legitimate exception** because the underlying behavior (LAN pod allowlist enforcement) doesn't exist on cloud at all — not just "stratified differently"
- Composes-with venue-only `cloud_sync_outbound` precedent (same structural rationale)

**Future-proof note:** if cloud ever runs its own process-guard-class (against Bono VPS shell allowlist), THAT would be a NEW metric (`vps_shell_guard_violations`), not a stratum of the venue metric.

### §11.5 — Cascade-order recommendation (RATIFY-PACK item U)

**Tier 1 (mandatory first · single-PR · gates everything):**
- **Step 1** — `get_observability_mode()` helper + unit test (5 LOC · ~30min · §S-146 full path · Class A-foundational-schema-billing-adjacent NO — pure observability scaffold, **NOT** billing-adjacent → reclassify to Class U audit-only OR Class Docs-equivalent · NO RESET per §14.6.2.1)

**Tier 2 (parallel after Tier 1 · 3-4 independent PRs):**
- **Step 6** — `/api/v1/health.observability_mode` top-level field (1-line · 5min · zero-risk · highest dashboard-convenience-per-LOC ratio)
- **Step 2** — `fleet_connectivity` stratify with NO_LAN_PODS sentinel (resolves §S-383 visibility gap · highest current-state-confusion-reduction · ~2h · binary still emits cloud-skip path)
- **Step 3** — `db_sync_lag` stratify with mode_label (~2h · informs schema-skew-watchpoint emit pattern downstream)
- **Step 5** — `pm2_restart_count_24h` NEW cloud-only metric (~3h · audit-class scope · pm2 jlist shell-out)

**Tier 3 (sequential after Tier 2):**
- **Step 4** — `drive_pull_age_secs` NEW cloud-only metric (~3h · sync-status.json parsing · depends on Tier 2 emit-pattern conventions for cloud-only metric class)

**Tier 4 (final · sequential after Tier 3):**
- **Step 7** — Watchpoint v0.3.0 → v0.4.0 (~4h · 4 new RECONCILIATION DRIFT event sub-classes · depends on Step 4 source + Step 3 schema_version field landing)

**Out of F15 scope (deferred):**
- Step 8 — Front-end dashboard mode-stratification (separate PR · racecontrol/web team pickup · per §11.3 above)

**Customer-priority ranking** (if forced to ship a subset):
1. Step 1 (gates all) — foundation
2. Step 2 (fleet_connectivity) — current-state confusion #1
3. Step 6 (mode field) — 1-line tax · big dashboard value
4. Step 7 (watchpoint v0.4.0) — forward-prevention §S-381→§S-385 class
5. Step 3 (db_sync_lag stratify) — sustained dashboard utility
6. Step 4 (drive_pull_age_secs) — cron-stall detection
7. Step 5 (pm2_restart_count_24h) — audit-class · low-frequency-but-load-bearing-when-fires

**Foundational-boundary §S-146 escalation note:** Steps 2 (fleet_connectivity touch) and Step 3 (db_sync_lag schema · billing-adjacent class indirectly via wallet-sync-status visibility) trip foundational-boundary on the existing classifier · each requires 5-section RCA + MMA Step 1 DIAGNOSE per §S-146. Step 1 / 4 / 5 / 6 / 7 are non-foundational (observability-scaffold class · cloud-only metric class · pm2 audit class · trivial field add · watchpoint emit-only extension).

**MMA budget estimate (full implementation):** ~$3-4 if foundational scope DIAGNOSE applied to Tier 2 Steps 2+3 (OpenRouter 5-model · §14.1 MAOR Tier-1 batch per-cascade additional · ~$0.20-0.30 per cascade-class commit). Total F15 implementation MMA spend forecast: ~$5-6.

**Time-to-ship estimate (sequential Tier 1→4 by one pilot):** ~16 working hours net code. Parallel-pilot Tier 2 execution can compress to ~10h wall-clock.

---

End of F15-OBSERVABILITY-FEDERATION-DESIGN v0.1.
