# GAPS-VENUE-20260424 — James-hemisphere ecosystem gap catalogue

**STATUS: FINAL — PACT-010 PROCEED(A) Uday-arbitrated 2026-04-24 17:43 IST**

Per PACT-20260424-010 Option A (symmetric hemisphere split). Uday arbitrated via sync-engaged Decision Tree (G33) — bypassed 4h async Bono-vote window. Outcome: PROCEEDED-STRUCTURE. Bono commitment: `GAPS-CLOUD-20260424.md` (cloud hemisphere) within 24h of next Bono session. James fill-PACTs (top 1-3) due 2026-04-25.

**Hemisphere:** James (venue .27 + Server .23 + Pods 1-8 + POS .130 + on-site comms-link daemon + racecontrol binary on server + 10 racecontrol worktrees)

**Discovery surfaces used:** graphify-racecontrol MCP (14934 nodes), local grep + Glob, memory (MEMORY.md + 75 topic files), PACTS.md rows 001-009, INBOX entries, git log across worktrees, PACT-009 F1-F8 friction triage.

**Schema per PACT-010:** `{id, class, evidence-pointer, proposed-fix, blast-radius, PACT-worthy?}`

**Cadence:** Weekly refresh Fridays (per PACT-010 recommendation), as long as catalogue mode is useful.

---

## Class A — Deploy backlog (CGP v4.3 backlog-gate items)

| id | class | evidence | proposed-fix | blast-radius | PACT? |
|----|-------|----------|--------------|--------------|-------|
| V-A1 | deploy-backlog | `project_fh5_haptic_xinput_skip_20260423.md` — PR on `fix/fh5-haptic-no-ffb-preload-20260423` @ `0f3cb05e`; XInput allowlist early-return; NOT DEPLOYED | Pod 8 canary → fleet roll after visual sign-off | rc-agent all pods | below-threshold (in-domain deploy) |
| V-A2 | deploy-backlog | `project_freedom_mode_focus_contract_20260423.md` — PR #33 @ `b9b37cd7` 3 commits MMA-3/3-MERGE; NOT DEPLOYED | Pod 8 canary → game-launch-in-freedom verify → fleet roll | rc-agent all pods | below-threshold |
| V-A3 | deploy-backlog | `project_racing_hud_v1_and_ac_ephemeral_shm_20260423.md` — Pod 8 deployed `3c2a1b48-dirty` per earlier memory; fleet roll gated on physical visual sign-off | Physical visual sign-off Pod 8 → Pods 1-7 + POS .130 | rc-overlay + AC ephemeral SHM | below-threshold |
| V-A4 | deploy-in-flight | PACT-009 rc-agent `ff3894af` 5/8 pods rolled; Pods 5-7 blocked by permission rail | Pod 5-7 complete after Bono vote on PACT-009 + Uday confirm | rc-agent 3 pods | covered by PACT-009 |

## Class B — Process ownership (S8 crate scope)

| id | class | evidence | proposed-fix | blast-radius | PACT? |
|----|-------|----------|--------------|--------------|-------|
| V-B1 | process-tree | Pod 1 WindowsTerminal 488-proc tree (`session_handoff_20260423_process_ownership_and_deploy_poe_gates.md`) | Root-cause WindowsTerminal spawn loop or allowlist | Pod 1 process stability | **PACT-worthy** (allowlist is cross-boundary shared state) |
| V-B2 | process-duplication | Pod 6 ConspitLink ×2 — same handoff memory | Determine single-owner spawn rule; allowlist tightening | Pod 6 FFB coherence | **PACT-worthy** |
| V-B3 | allowlist-coverage | Q5 drift — D:\racecontrol.toml missing ~19 process-guard allowlist entries vs git (`project_q5_racecontrol_toml_drift_20260423.md`) | S8 Wave 8 daily-reset schtask + config-layer single-source-of-truth | Server .23 process guard | **PACT-worthy** (Q5 shared-state) |
| V-B4 | allowlist-owner | S8 crate allowlist owner is `<TBD — Uday sign-off>` (`plan_process_manager_crate_20260423.md`) | Uday sign-off on allowlist governance before S8 Wave 5 CI gate lands | S8 rollout timing | **PACT-worthy** (Uday-escalation per Pacific Rim doctrine) |

## Class C — Config drift

| id | class | evidence | proposed-fix | blast-radius | PACT? |
|----|-------|----------|--------------|--------------|-------|
| V-C1 | toml-drift | Q5 5-category drift D:\racecontrol.toml vs git (missing config_dir, jwt/relay secret mismatch, process_guard.enabled flip, ~19 allowlist entries, ~10 schtask allowlist entries) | TOML single-source-of-truth (analogous to deploy-server.sh Option 1 class) | Server .23 config surface | **PACT-worthy** (same class as PR #29 structural fix) |
| V-C2 | toml-single-source | D:\racecontrol.toml as unversioned key source — drift accumulates silently between deploys | Move canonical TOML into git repo; D:\ becomes generated-at-deploy | Server .23 + deploy pipeline | **PACT-worthy** |
| V-C3 | swaplog-manual | 14 manual SWAPLOG appends observed (deploy-server.sh Class B merged but class not fully closed — `project_deploy_server_sh_dual_architecture_20260423.md`) | Option 2 server-side SHA256 watcher (Class C not planned yet) | deploy audit trail | **PACT-worthy** (audit integrity is cross-hemisphere) |

## Class D — PACT-009 friction triage (F-series, secondary per 17:18 IST INBOX)

| id | class | evidence | proposed-fix | blast-radius | PACT? |
|----|-------|----------|--------------|--------------|-------|
| V-D1 (F1) | build-hygiene | `-dirty` suffix from uncommitted .planning/ in worktree during rc-agent build | Pre-build git-clean check or allowlist `.planning/` | rc-agent build_id | below-threshold |
| V-D2 (F2) | rail-vs-override | Permission rail fired 3× mid-PACT-009 roll; user explicit override needed | Rail-invoke-aware policy: rail blocks *new* actions only, not user-authorized in-flight | rc-sentry exec policy | **PACT-worthy** (cross-boundary safety policy) |
| V-D3 (F3) | exec-ergonomics | Inline-only `/exec` forcing 3× tool calls per rc-sentry command | Support file-argument or stdin-piped commands | James-side tooling | below-threshold |
| V-D4 (F4) | ledger-drift | SWAPLOG manual append (= V-C3 duplicate) | See V-C3 | — | merged with V-C3 |
| V-D5 (F5) | memory-lifecycle | Memory author-set gate lifecycle unclear | Gate memory reads behind source/freshness tags (already partial in handoff-v2) | Memory integrity | **PACT-worthy** (cross-AI shared memory) |
| V-D6 (F6) | config-source | D:\racecontrol.toml key source (= V-C1 duplicate) | See V-C1 | — | merged with V-C1 |
| V-D7 (F7) | pod-reattach | Pod 2 30s reattach — ConspitLink suspected | ConspitLink Phase 59 investigation (auto-detect may be misfiring) | Pod 2 FFB | below-threshold |
| V-D8 (F8) | POS-exclusion | POS .130 excluded from rc-agent fleet rolls; convention unclear | Doctrine doc: POS .130 is non-gaming, rc-agent does not install | POS deploy doctrine | **PACT-worthy** (fleet-scope definition) |

## Class E — Comms / infra

| id | class | evidence | proposed-fix | blast-radius | PACT? |
|----|-------|----------|--------------|--------------|-------|
| V-E1 | ws-relay-down | Comms-link WS relay disconnected 5h this session (multiple `tool-verified/session` observations) | Watchdog auto-restart + WS-identity-thrash diagnosis (parked this session) | Bono↔James sync latency | **PACT-worthy** (shared comms infrastructure) |
| V-E2 | ssh-22-filtered | Tailscale SSH:22 to Bono VPS filtered (meta-corpus C1 pull blocked) | Confirm Tailscale ACL + fallback to git-channel for binary artifacts | Cross-hemisphere artifact transfer | **PACT-worthy** |
| V-E3 | pact-vote-latency | 30min vote-detection gap (PACT-006 Bono vote 16:59 → James saw 17:15) | BW-1 WS-push on PROPOSE/VOTE write (PACT-006 Delta 2 gap #3 scope) | PACT cadence | defer to Bono first-initiator 2026-04-25 18:00 IST (Delta 2 binding) |

## Class F — Workflow-layer gaps (below-threshold, PACT-008 continuation)

| id | class | evidence | proposed-fix | blast-radius | PACT? |
|----|-------|----------|--------------|--------------|-------|
| V-F1 | workflow-D4 | D4 DPDP E2E not yet shipped; D1-D3 live but E2E gap | PACT-008 Phase 2 continuation | `run-all-checkers.sh` | below-threshold |
| V-F2 | workflow-D8-P2+ | D8 Phase 2+ — more schema-types pairs beyond AcLaunchParams | Extend `schema-types-pairs.toml` | build gate | below-threshold |
| V-F3 | workflow-D11 | Session-FSM scope PENDING (PACT-008 Option B chose D8 over D11) | Re-propose D11 after D8 stabilizes | FSM consistency | defer |
| V-F4 | workflow-D12 | Config-drift checker — scope PENDING | Subsumed by V-C1 / V-C2 TOML single-source | — | merged with V-C1 |
| V-F5 | workflow-D13 | status.json refresh cadence — scope PENDING | Below-threshold scope-out | state/status.json | below-threshold |

---

## Summary counts

- **Total gaps:** 23 (across 6 classes)
- **PACT-worthy:** 11 (V-B1, V-B2, V-B3, V-B4, V-C1, V-C2, V-C3, V-D2, V-D5, V-D8, V-E1, V-E2 = 12 → dedupe merged F4/D4/D6 = **11 unique cross-boundary gaps**)
- **Below-threshold (in-domain):** 9
- **Already covered by open PACT:** 3 (V-A4 ↔ PACT-009; V-E3 ↔ PACT-006 Delta 2; V-F3 ↔ PACT-008)
- **Deploy-backlog (CGP gate):** 3 (V-A1/A2/A3)

## Cross-hemisphere gaps

Per PACT-010 convention "cross-hemisphere gaps go into the initiator's hemisphere" — these are listed here but fill-PACT will request Bono vote:
- V-C1/V-C2 TOML single-source — touches deploy pipeline + cloud parity
- V-E1 WS relay — shared comms-link infrastructure
- V-E2 SSH:22 — Tailscale topology spans both

## Top-3 fill-PACT candidates (per PACT-010 "initiate 1-3 top fills" convention)

If Bono AGREEs(A) on PACT-010, James would propose PACT-20260425-NNN on these in order:

1. **V-B4 allowlist governance** (Uday-escalation; blocks S8 Wave 5)
2. **V-C1 Q5 TOML drift** (Server .23 config integrity — 5 drift categories concrete)
3. **V-D2 rail-vs-override** (cross-boundary safety policy — PACT-009 F2 surfaced acutely)

Bono-side equivalent (awaiting `GAPS-CLOUD-20260424.md`) would pick top 1-3 from cloud hemisphere symmetrically.

---

## Source/freshness

- `[tool-verified/now]` PACTS.md rows 001-010 read this turn
- `[tool-verified/now]` INBOX read this turn (5h WS-relay down confirmed)
- `[memory/recent]` Class A backlog items — MEMORY.md top entries `[source/freshness]` tagged
- `[memory/recent]` Class B process gaps — `session_handoff_20260423_process_ownership_and_deploy_poe_gates.md`
- `[memory/recent]` Class C Q5 drift — `project_q5_racecontrol_toml_drift_20260423.md`
- `[tool-verified/now]` Class D — PACT-009 proposal text read this turn
- `[mirror/not-re-verified]` V-A3 Pod 8 HUD deployed state `3c2a1b48-dirty` — memory-only; not re-probed this turn

## Deliberately NOT done

- Did NOT initiate PACT-20260425-NNN fill-PACTs yet — per Option A convention, fills land 2026-04-25 (James top-3: V-B4 / V-C1 / V-D2)
- Did NOT enumerate Bono hemisphere gaps — vector #1/#2 violation if James frames Bono's catalogue; Bono owns `GAPS-CLOUD-20260424.md`
- Did NOT merge with `project_workflow_plan_day1.md` gap list — keep catalogue separate from plan tracker; link only
- Did NOT retroactively edit PACT-009 into Class A — PACT-009 lifecycle (PROCEEDED-ROLLED) owns its own outcome
- Did NOT re-verify V-A3 Pod 8 HUD state — still memory-only per Source/freshness; live-probe on next fleet-health poll

## FINAL promotion history

- 2026-04-24 17:25 IST — DRAFT filed pending PACT-010 vote
- 2026-04-24 17:43 IST — Uday arbitrated A sync-engaged per G33 (feedback_pact_protocol.md)
- 2026-04-24 ~19:00 IST — DRAFT suffix removed, STATUS flipped FINAL, NOT-done section updated (this commit)

— James FINAL 2026-04-24 17:43 IST (filed 17:25 DRAFT, FINAL per Uday arbitration)
