# 🧭 INDEX — Ecosystem V2 (read-first status + first-INR gap map)

> **Purpose:** one place to see "what's done / what's left / who owns each gap" for V2, so a session doesn't re-discover it across ~20 memory files, ~50 `.bono-staging/` handoffs, the §S-N ledger, the progress map, and two repos' PR queues.
> **Compiled:** 2026-06-04 (bono). **Method:** 3 parallel Explore agents (memory+handoffs · ledger+progress-map+roadmap · live git/PR) + direct read-only probes (`gh pr list`, §S-N grep). All facts are file-/probe-backed, not memory-projected.
> **Companion (doctrine-ledger lens):** [`V2-PROGRESS-MAP.md`](./V2-PROGRESS-MAP.md). **Canonical ledger:** `comms-link/V2-MASTER-STATE.md` (§S-N). This index is a *navigation + gap* layer, not a replacement.
> **Freshness:** numbers verified 2026-06-04 ~05:30 IST. Re-run the §Verification probes before trusting on a later date.

---

## PART 0 — Definition of Done (Captain-RATIFIED 2026-05-30, scope-freeze)

> **V2 is COMPLETE when BOTH surfaces are bug-free:**
> 1. **RacingPoint Ecosystem V2** (`rp-v2-apps` — PWA/launch-portal, POS, Kiosk, Pod-display, staff-tablet, admin proxies, contracts, billing-engine, SSE)
> 2. **RaceControl** (Rust `racecontrol` heart, `rc-agent`, `rc-installer`)
>
> **"Bug-free" bar:**
> - First-INR money path passes **e2e on a REAL pod**: register(OTP) → topup → launch(HOLD) → tick-debit → end → bill — **₹ debited + reconciled**
> - **Zero open CRITICAL/blocker bugs** across both surfaces
> - Gate-clean: contract parity ✓ · `heart_v2` + billing tests ✓ · no money-leak / double-spend / double-spawn ✓
>
> **Scope freeze:** NO V2.1+ scope until BOTH pass.

Source: `memory/project_v2_scope_freeze_definition_of_done_20260530.md` · ratified at coordinator `0ea33e7`.

**The single test for any proposed work:** *does it close the first-INR bug-free bar, or is it V2.1+ (frozen → defer)?*

---

## PART 1 — FIRST-INR GAP MAP (process of elimination)

**Legend:** ✅ done+merged · 🟡 in-flight (open PR / local) · 🔴 gap (not built) · ⛔ gated (built, blocked on owner action) · ❄️ frozen V2.1+

### 1A. The money/launch critical path — eliminated to what remains

| Step in the rupee path | State | Where |
|---|---|---|
| Register via OTP | ✅ contract+wiring merged; provider migrating Evolution→MSG91 | rp-v2-apps #18/#19 merged; #27/#28 open; rc #115 merged |
| Topup → **durable** wallet (PgWalletStore) | ✅ code merged (incl. 2 MAOR money-bugs fixed) | rp-v2-apps #22 `27eb7923` |
| Launch **gated by balance** (402 HOLD) | ✅ in #22 (`402 launch-gate`); companion gate #17 open (CONFLICTING) | #22 merged; #17 open |
| Heart-V2 → rc-agent **game actually launches** on pod | ✅ built+merged+deployed (**flag-OFF**) | rc `b7067829` (delta A `c0a74c9f`); `.23`=`21531f31` |
| Real `launch_args` (car/track) delivered to agent | ✅ I2 built+merged (Gap-2) | rc #116 `690a8616`; `.23` cutover prepped |
| **Per-tick debit** during session | ✅ engine tick + durable store merged | #22; tag-fix on `feat/wallet-durable-tick` (LOCAL-only — see §2F at-risk) |
| Restart **without double-bill** | ✅ reconciler restart-safety + integration test | rc #119 merged; #122 open (tests, CLEAN) |
| End → settle → **₹ reconciled** | ✅ derived-request-id reconcile (double-credit bug fixed) | #22 |
| Under-bill **incident capture** (safety net) | ✅ slice-2 merged, **inert until `WALLET_STORE=pg`** | rp-v2-apps #29 `f7f1fdcc` |

**Eliminated result:** the money/launch **software is built and merged**. What remains is **not code** — it's three operator/Captain gates + one runtime-verification (§1B).

### 1B. The 4 owner-clustered blockers (the actual gaps)

| Cluster | What | Owner | Status |
|---|---|---|---|
| **#1 — game launches on pod** | heart-V2 → rc-agent launch handshake | bono (built) → **Captain + operator** (gates) | ⛔ built+merged+deployed, **flag `heart_v2_real_launch` OFF + runtime-unverified at scale**; `launch_args` now real (#116). Half-proven live on pod_1 (2026-06-01): launcher fired, session `Running` confirmed. Remaining: flag ON [Captain] + pods power [operator] |
| **#2 — money actually moves** | durable wallet + HOLD/402 + tick debit | bono (built; was Replit) | ⛔ **code merged (#22)**; remaining = **`.23` cutover `WALLET_STORE=pg`** [operator, not started] — MUST precede flag-ON |
| **#3 — venue physically ready** | heart `.23` live · rc-agent fleet · OTP · pods · seed:captain · keys | **operator + Captain** | 🟡 heart `.23`=`21531f31` LIVE (`/heart/pods`→200) · rc-agent `a826b100` uniform 8/8 · **Evolution OTP needs QR re-pair (or MSG91 cutover)** · **pods 0/8 OFF** · **seed:captain not run** · F6/B8 keys unprovisioned |
| **#4 — billing-start semantics** | `green_light_at` (launch-time vs loading-complete) | **Captain** (decision) → bono | ✅ DECIDED (delta A = confirm-before-bill on `verified_running`); awaiting Captain explicit §S-N ratify |

**14 money/launch bugs ledger:** 13 fixed+merged; only **#6** (sub-1-min 0-tick rate-reconstruction) open = **documented non-flip-blocker**.

### 1C. Gate sequence to the first rupee (ordered, post-merge)

1. **Captain merges** — rc #122 (tests) + money-path backlog + the A3-base decision (§2F)
2. **Operator** — wallet `.23` cutover `WALLET_STORE=pg`  *(MUST precede flag)*
3. **Captain** — flag `heart_v2_real_launch` **ON** + staff JWT
4. **Operator + bono** — single-pod ₹-float **canary**, eyes-on
5. **first-INR** e2e on a real pod → bar closed

Runbooks: `.bono-staging/RUNBOOK-FIRST-INR-GATES-INDEX-20260603.md` · `RUNBOOK-FLAG-FLIP-AND-CANARY-heart_v2_real_launch-20260603.md` · `RUNBOOK-WALLET-DURABLE-PG-CUTOVER-23-20260603.md`.

### 1D. Open-PR ledger (verified 2026-06-04 via `gh pr list`)

**racecontrol (base `main`) — bono money-path OPEN:**
- **#122** heart money-loop integration tests — MERGEABLE *(test-only; Captain-merge)*
- **#117** OTP→MSG91 GO-LIVE runbook — MERGEABLE *(docs)*
- **#113** loading-complete route (confirm-before-bill, G-NEW-9) — MERGEABLE

*Merged this wave (SHAs from main merge-commits):* #112 panic-fix `78691c9e` · #115 const-time PIN+OTP RCA `66a02154` · #116 launch_args I2 `690a8616` · #118 launch_args gate (merge `23e12d4c`) · #119 reconciler restart (merge `3e9c7c39`) · #121 slow-launch StopGame (merge `759fb68e`).

*racecontrol james-authored OPEN (UI/docs lane — NOT money-path):* #82/#86/#89/#95/#96 (docs/RCA/health) · **#90, #97 CONFLICTING** (pricing-ceiling, --rp-orange token) · #98–#106 kiosk V2-theme-migration cluster (11 style PRs) · #103 (preview-hud delete-flag). These are James's bounded UI lane — complementary, off the rupee path.

**rp-v2-apps (base `replit/coordinator`, except as noted) — bono money-path OPEN:**
- **#28** atomic single-use OTP verify (TOCTOU, G-CUT-2) — MERGEABLE
- **#27** OTP→MSG91 customer-contract F1/F2/F3 — MERGEABLE *(Captain-merge-gated)*
- **#24** session-402 body + exportSessionDebits operationId — MERGEABLE
- **#23** M5 captain-tier session money-export — MERGEABLE *(base `feat/wallet-durable-tick`)*
- **#17** sync-402 launch-gate (G-NEW-6) — **CONFLICTING** (needs rebase)
- **#16** CSV formula-injection + M2 envelope + M3 pagination — MERGEABLE
- **#15** D-1/D-6/D-2 design closures — **CONFLICTING**
- **#12** LicenseHeartbeat contract (B9 prep, DRAFT) — **CONFLICTING**
- **#9** autobill.tick source_tag — MERGEABLE
- **#8** console sign-in via james@racingpoint.in — **CONFLICTING**
- **#7** kiosk un-pad POD_IDS to heart wire format (MAOR U-B3) — MERGEABLE

*rp-v2-apps james-authored OPEN (UI lane, default-OFF):* #26 MSG91 OTP widget client · #25 pod-display grace/runout timers (base `feat/v3-ui-scaffold`) · #3 scope-map docs (base `main`) · #1 D-04 4-of-5 bundle (base `main`).

*Merged:* #22 durable wallet `27eb7923` · #29 incident store `f7f1fdcc` · #18/#19 OTP `92d5950`/`d47b4d3` · #13/#14 contracts `46e683`/`7e0b976` · #20/#30/#31/#32/#33 V3-UI+fixes · #6/#10/#11 console.
**#21 = CLOSED** (squash-subsumed by #22; lacks the fixes — do not reopen).

### 1E. ❄️ FROZEN to V2.1+ (do not build until both surfaces pass)

Multiplayer green-light trio · AC-MP lobby polish · pod-display error-display states (server-lost/updating/crash-relaunch) · telemetry/leaderboards · customer-email scope (WhatsApp + in-app only for V2.0) · V1 decommissioning · multi-tenant control plane · Console V2+ (Releases/Deploy Ring6, Billing Ring7, brand-pack).

One Captain-granted EXCEPTION: a single full-UX pass (customer+staff workflow RCA) — `memory/project_customer_staff_workflow_rca_human_perspective_20260603.md`.

---

## PART 2 — FULL ECOSYSTEM INVENTORY (appendix)

### 2A. Two surfaces

- **RaceControl** (Rust, repo `racecontrol`, **bono-sole lane**): crate `racecontrol` = the **heart** (now carries the `/heart/*` V2 session/launch/billing surface — the binding blocker that was closed 2026-05-30) · `rc-agent` (per-pod; fleet uniform `a826b100`, 8/8, 2026-06-02) · `rc-installer` (web-distributed trust core: ed25519+sha256, cross-language golden-vectors L1-5 verified).
- **Ecosystem V2** (TS, repo `rp-v2-apps`; james active-editor · replit `packages/contracts/**` · bono co-edit): ~10 apps, all **V3-UI rebuilt** (2026-06-03): pod-display · POS · staff-tablet · kiosk (+ launch portal / PWA) · racecontrol-console · captain-console (shell+cockpit) · admin-proxy-bono · admin-proxy-james. Plus `packages/contracts/**` (OpenAPI joints, **Replit-owned**) · billing-engine · SSE.

### 2B. 13-layer V2-PROGRESS-MAP rollup (doctrine-ledger lens — **~18 days stale, predates 06-02→06-04 merges; refresh recommended**)

~40% LIVE-BLOCKING DONE (mixed framing) / ~45% F3-pure (DONE+ENG). Largest remaining surface = ~23 TEST-SCAFFOLDED rows. By layer: L1 Kiosk-Wizard 2/20 · L4 comms-link batch done · L7 wallet/billing 3/8 (Phase-β deployed, soak) · L10 cloud-sync 1/3 · L14 operations 0/6 (runbooks). File: [`V2-PROGRESS-MAP.md`](./V2-PROGRESS-MAP.md).

### 2C. Closed-loop component map

59 components + the first-INR money loop overlaid: `memory/project_v2_component_workflows_closed_loop_20260531.md` (Captain-RATIFIED). 3-layer frame (L1 Contract/Trust → L2 Transport → L3 Runtime/Money-path): `memory/project_3layer_sync_debug_structure_and_first_inr_gapmap_20260531.md`.

### 2D. Deployed reality (claim — **verify on resume**, not re-probed in this compile)

heart `.23` = build **`21531f31`** (2026-05-31 panic-fix + cutover, verified `/heart/pods`→200) · pods **0/8 OFF** · flag `heart_v2_real_launch` **OFF** · rc-agent fleet **`a826b100`** uniform 8/8 (2026-06-02).
*Historical note:* an older 2026-05-17 SWAPLOG forensic about `8da500c7` / soak-reset is **superseded** by the 2026-05-31 cutover — do not treat as current. To re-verify without pod SSH: heart `GET /api/v1/fleet/health` on `.23` (`memory/reference_heart_fleet_view_reads_pod_state_without_pod_ssh_20260603.md`).

### 2E. Lane ownership (§S-450, RATIFIED 2026-06-02)

bono = **sole pilot + sole §S-N appender**; owns `racecontrol/**` + `comms-link/**`. James = bounded `rp-v2-apps` active-editor / UI lane (his open PRs are default-OFF UI, complementary, not on money path). Replit = `rp-v2-apps/packages/contracts/**` + coordinator base branch (relay **channel retired** — bono self-reviews, Captain merges). Captain = human owner. Enforced on bono harness by `lane-guard.js` + `pre-git-divergence-guard.js`. Canonical: `comms-link/LANE-CONTRACT.json`.

### 2F. Doc-ledger state & owed records

- **§S-N head = §S-450** (2026-06-02), confirmed by probe (`grep -oE '§S-[0-9]+' | sort -n | tail` → 450; last heading file-line 46377). **~2 days behind** the 06-03/06-04 merges → **bono-owed close-anchor** (gap-1/2 + OTP + #29–#33 + A1/A2). Bono is sole appender.
- **At-risk / unpushed:** `rp-v2-apps feat/wallet-durable-tick` HEAD **`c684a761`** (tick tag-fix; A3's dependency) is **committed local-only** (origin tip `7963355`) — the one genuine off-remote surface. **A3-base decision pending Captain:** (a, recommended) push+PR the tag-fix first, then ship A3 on top; (b) stack A3 directly; (c) ship A3's unentangled subset. Large stash counts + `[ahead N]` = automated/squash-merge artifacts, **not** lost work.

### 2G. Resolved contradictions (caught by elimination during this compile)

| Stale claim | Corrected |
|---|---|
| §S-N head = §S-440 | **§S-450** — §S-440 is the 2026-05-29 HSM doctrine, superseded-in-part by §S-446 |
| heart `.23` = `8da500c7`, soak reset | **`21531f31`** per verified 2026-05-31 cutover (older forensic superseded) |
| MEMORY.md banner "close PR #21" | **#21 already CLOSED** (squash-subsumed by #22) |

---

## PART 3 — Source map (where the truth lives)

| Question | Read-first source |
|---|---|
| Definition of done / scope freeze | `memory/project_v2_scope_freeze_definition_of_done_20260530.md` |
| First-INR gap clusters | `memory/project_3layer_sync_debug_structure_and_first_inr_gapmap_20260531.md` + `.bono-staging/HANDOFF-OVERNITE-SYNC-DEBUG-20260531.md` |
| Latest session (A1–A4 rollout-confidence) | `memory/project_rollout_confidence_tests_20260604.md` + `.claude/plans/sorted-waddling-zephyr.md` |
| Canonical §S-N ledger | `comms-link/V2-MASTER-STATE.md` (head §S-450) |
| % complete / 13-layer grid | [`V2-PROGRESS-MAP.md`](./V2-PROGRESS-MAP.md) |
| First-INR roadmap (9-blocker table) | `rp-v2-apps/coordinator/ROADMAP-TO-FIRST-INR-HYDERABAD-2026-05-30.md` |
| Component closed-loop map (59) | `memory/project_v2_component_workflows_closed_loop_20260531.md` |
| Lane ownership | `comms-link/LANE-CONTRACT.json` |
| Money-path canonical refs (drift guard) | `racecontrol/.planning/CANONICAL-REFS.json` |

---

## §Verification — re-run before trusting these numbers on a later date

```bash
# Open PRs (authoritative — supersedes §1D if changed)
gh pr list -R <racecontrol-remote>  --state open --limit 50 \
  --json number,title,author,mergeable
cd /root/rp-v2-apps && gh pr list --state open --limit 60 \
  --json number,title,author,baseRefName,mergeable
# §S-N ledger head (expect 450 until a new append)
grep -oE '§S-[0-9]+' /root/comms-link/V2-MASTER-STATE.md \
  | grep -oE '[0-9]+' | grep -v '^9999$' | sort -n | tail -1
# At-risk unpushed tag-fix (local ≠ origin ⇒ still unpushed)
git -C /root/rp-v2-apps rev-parse feat/wallet-durable-tick \
  origin/feat/wallet-durable-tick
# Deployed heart build + pod fleet (no pod SSH needed)
#   curl heart .23 GET /api/v1/fleet/health  → build_id + per-pod state
```
