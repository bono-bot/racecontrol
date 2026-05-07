# Phase 1 wire-up EXECUTE PLAN — PACT-20260506-001 + §AMEND-1

**AUTHORED:** 2026-05-07 ~06:18 IST
**Author:** james-LEAD (per first-mover-LEAD §E.1)
**Class:** planning memo (NOT a PACT FILE; substrate-implementation work under FILED PACT-20260506-001 + ratified §AMEND-1 ABSORB-IN-FULL)
**Composes-with:**
- PACT-20260506-001 (FILED `b45cf13` 2026-05-06; james-LEAD; bono-AMPLIFIER pre-FILE pass `b692753` AGREE-WITH-CAVEATS)
- PACT-20260506-001 §AMEND-1 ABSORB-IN-FULL (james authored 2026-05-07 ~03:36 IST; absorbs bono NF-bono-1..7)
- PACT-20260505-001 Phase 0 substrate (MERGED `483562ac` — `cirs::lookup_by_phone`, `cirs::record_lookup`, canonicalize_phone, LookupInput enum, cirs_lookup_audit table)
- PACT-20260503-018 staff_id FK (MERGED `3119da30` — staff(id) FK target for cirs_lookup_audit.staff_id)
- PACT-20260503-020 F2 Feature Flag Service (MERGED — M3/M4 disable switches)
- §AMEND-1.E PrivilegedAction enum substrate (SHIPPED `7f193030` + NF-bono-1 absorption `26677e42`; bono AMPLIFIER AGREE-as-shipped + 8/8 cargo Linux parity at §S-79)
- Captain §S-82 dispositions (PACT-001 Q1-Q4 RESOLVED 2026-05-07 ~05:30 IST): Q1=PIN-LOCKOUT 5-wrong+rotate+WhatsApp+helpdesk reset · Q2=refund 3-band · Q3=idle-timeout 30 min sliding · Q4=discount auto-apply MI-adaptive
- V2.0 6-wave plan (Captain §S-82) — this PLAN executes Wave 0

**Status:** EXECUTING — Session 1 of estimated 5-8 sessions

**Verify-by:** 2026-05-19 (carry-forward from PACT-20260505-001 §5)

---

## §1 — Wave 0 scope (PACT-20260506-001 + §AMEND-1)

Wave 0 turns the FILED Phase 0 substrate (Phase 0 service skeleton + Phase 1 enum substrate) into a customer-touch surface at POS .130. Without it, V2.0 customer surface count stays 0 (the rate-limiter named in V2-MASTER-STATE.md scorecard since §S-32).

### Surfaces this PLAN delivers

| Surface | Layer | Captain disposition reference |
|---|---|---|
| `POST /api/v1/cirs/lookup` HTTP route | racecontrol crate (Rust/axum) | Q1-A single-route + method-enum (bono pre-FILE disposition) |
| ProfilePreview population (substrate joins; no new tables) | racecontrol crate | §AMEND-1.A `balance_credits` (NF-bono-1 absorbed) |
| POS .130 phone-lookup UI | `web-v2/src/app/v2/pos/lookup/page.tsx` | Q3 disposition (bono pre-FILE pass) |
| NF-james-B Indian-mobile-prefix WARN gate | UI input layer (pre-canonicalize per §AMEND-1.B) | digit[0] ∈ {6,7,8,9}; WARN-only with override |
| Walk-In Guest 1+2 fallback dropdown | UI component | DoD §1.2 path B (no-phone customer) |
| Audit discipline | every CIRS call writes `record_lookup` row | §AMEND-1.E §3.5 Q5-A enforcement substrate |
| Manager-pill UI for PrivilegedAction enum | `web-v2` PrivilegedAction guard component | §AMEND-1.E enum membership gates PIN re-entry |
| Idle-session-timeout (30 min sliding window) | racecontrol cookie middleware + config | §AMEND-1.F MECHANISM ABSORBED + Captain §S-82 Q3 PARAMETER=30 |

### Surfaces explicitly NOT in Wave 0 scope (Wave 1+ or V2.1)

- Q1 PIN-LOCKOUT-policy auto-rotate + WhatsApp dispatch + helpdesk inbox + reset workflow — bigger sub-PACT, sequenced into Wave 1+
- Q2 refund 3-band routing logic (`<₹1000` staff PIN / `₹1000-2999` PIN+reason / `≥₹3000` ApproveRefundOverThreshold manager-mode) — sequenced into Wave 1 billing engine
- Q4 MI-adaptive discount Tier-1 deterministic formula — Wave 4 per Captain §S-82 6-wave plan
- M3 PWA-QR scanner / M4 NFC reader — Phase 3 (no hardware in V2.0)
- Cache layer (Q2-C disposition: NO cache for V2.0 — substrate query <50ms)
- PWA self-serve lookup — Phase 2 (V2.1+)
- Kiosk identity binding — Phase 2

---

## §2 — Session sequencing (estimated 5-8 sessions; per critical-path mapping)

### Session 1 (THIS SESSION, 2026-05-07 ~06:14 IST onwards)

**Deliverable:** scaffolding only — pure additive, no route registration.

| Item | Path | Status |
|---|---|---|
| This PLAN.md | `.planning/specs/v2/PHASE-1-WIREUP-PLAN.md` | authoring |
| `cirs_lookup.rs` handler scaffolding (DTOs + 501 stub + tests) | `crates/racecontrol/src/api/cirs_lookup.rs` | authoring |
| Module export | `crates/racecontrol/src/api/mod.rs` add `pub mod cirs_lookup;` | authoring |

**Out of scope this session:** route registration in `routes.rs`, ProfilePreview substrate joins, POS UI, manager-pill UI, idle-timeout middleware.

### Session 2

| Item | Path |
|---|---|
| ProfilePreview substrate join queries | `cirs_lookup.rs` (extend) — joins `customers`, `customer_profiles`, `wallets`, `sessions` |
| Replace 501 stub with real handler logic | `cirs_lookup.rs` |
| `record_lookup` post-call discipline | every Found/NotFound/Error path |
| Integration tests against real `v2-db` pool | `cirs_lookup.rs` test module |

### Session 3

| Item | Path |
|---|---|
| Route registration | `routes.rs` — `.route("/api/v1/cirs/lookup", post(cirs_lookup_handler))` chained under staff-JWT-protected sub-router |
| Auth wiring (staff session cookie via `auth::middleware::require_staff_jwt`) | route registration layer |
| Cargo test green confirmation | full `cargo test -p racecontrol-crate` + `cargo test -p v2-db` |

### Session 4

| Item | Path |
|---|---|
| POS .130 UI scaffolding | `web-v2/src/app/v2/pos/lookup/page.tsx` |
| `<PhoneLookupInput />` with NF-james-B Indian-mobile-prefix WARN gate | `web-v2/src/components/v2/pos/PhoneLookupInput.tsx` |
| `<ProfilePreviewCard />` | matching component |
| `<WalkInGuestDropdown />` (Guest 1, Guest 2 hardcoded; `discount_ineligible: true`) | matching component |
| `<NotFoundCTA />` + `<LookupErrorBanner />` | matching components |

### Session 5

| Item | Path |
|---|---|
| Manager-pill UI consuming PrivilegedAction enum | `web-v2/src/components/v2/auth/ManagerPill.tsx` |
| TypeScript type generation from Rust PrivilegedAction enum | `web-v2/src/lib/types/privileged-action.ts` (manual mirror or codegen) |
| Idle-timeout middleware | `crates/racecontrol/src/auth/middleware.rs` extension (30 min sliding cookie expiry) |
| Idle-timeout config TOML key | `racecontrol.toml` `[auth] idle_timeout_secs = 1800` |

### Session 6

| Item | Path |
|---|---|
| Component tests (Next.js test runner) | per §5.3 of PACT |
| E2E tests (Playwright) | per §5.4 of PACT — including Arabic-Indic codepoint test per §AMEND-1.D |
| Performance smoke test | p95 <500ms target per DoD §1.2 |

### Session 7 — PR readiness

| Item | Path |
|---|---|
| MMA Cross-System Bridge audit (mandatory per CGP) | OpenRouter 5-model DIAGNOSE+PLAN, 3-model VERIFY |
| DEPLOY MANIFEST | `.planning/specs/v2/PHASE-1-WIREUP-DEPLOY-MANIFEST.md` (POS .130 + Server .23 + Bono VPS + racecontrol-web kiosk/admin builds) |
| `bash scripts/deploy/deploy-audit.sh <enum-base-hash> <wireup-head-hash>` (DMP) | LOGBOOK row |

### Session 8 — PR open + Captain auth + verify

| Item | Path |
|---|---|
| Captain explicit per-PR auth (PROMOTED-N=1 gate) | required at PR-open time |
| PR open + bono Linux-parity AMPLIFIER vote | github.com/bono-bot/racecontrol PR |
| Merge + DEPLOY PARITY (POS .130 + .23 + Bono VPS + cloud apps) | per CLAUDE.md DEPLOY PARITY rule |
| Behavior verify on POS .130 (the actual customer surface) | screenshot of staff lookup flow |

---

## §3 — Captain-reserve items still open at start of Wave 0

Carried forward from PACT-20260506-001 §AMEND-1.I. THIS PLAN does NOT dispose any of them — surfaced for traceability.

1. **Per-PR auth gate at PR-open time** (PROMOTED-N=1) — Phase 1 wire-up PR-open requires explicit per-PR Captain auth at Session 8.
2. **Q1 PIN-LOCKOUT-policy 5-wrong/lockout duration** — Captain §S-82 Q1 disposed (5 wrong → auto-rotate + WhatsApp + helpdesk HUMAN gate). Sub-question 1.e-h (PIN cadence / delivery time / reset channel / fallback) STILL Captain-reserve.
3. **Q9 discount auto-apply vs staff-toggle** — Captain §S-82 Q4 disposed (YES MI-adaptive auto-apply); sequenced to Wave 4.
4. **₹X refund-threshold for `ApproveRefundOverThreshold`** — Captain Q-DECISION queue (§AMEND-1.E enum entry).
5. **Idle-session-timeout PARAMETER (5/15/30 min)** — Captain §S-82 Q3 disposed = **30 min sliding window**. Default config ships at 30min.

---

## §4 — Open architectural decision (bono NF-bono-2)

**NF-bono-2:** "wire-up centralized-middleware vs per-handler-trait architecture clarification"

**Surface where this matters:** PIN-gate enforcement on PrivilegedAction-bearing handlers (refund, manager-mode, comp-session, etc.). NOT on CIRS lookup (lookup is non-privileged per §AMEND-1.E doc + Q5-A disposition).

**Two patterns:**

- **Pattern A (centralized middleware):** axum middleware layer reads request route → looks up `Option<PrivilegedAction>` for that route → if `Some(action) && action.requires_pin()` → require fresh PIN cookie else 401. Single enforcement point; one place to audit.
- **Pattern B (per-handler trait):** each privileged handler explicitly invokes `verify_pin_for(action).await?` inside the handler body. PrivilegedAction membership check is at the handler boundary; harder to drift but audit requires reading every handler.

**Recommended default for Session 5 implementation:** Pattern A (centralized middleware). Rationale:
- §AMEND-1.E §3.5 doc: "PIN-required surface non-fuzzy and auditable" — middleware-level route table IS the audit surface
- Composes-with `auth::middleware::require_staff_jwt` existing pattern (centralized, layer-stacked)
- Drift detection trivially scriptable: grep `routes.rs` for `with_privileged_action(...)` markers vs PrivilegedAction enum cardinality
- Reversible to Pattern B per-handler call if Pattern A proves brittle (one-PR refactor, no schema impact)

**Captain ratification path:** surface to Captain at Session 5 start; if no objection, ship Pattern A.

---

## §5 — Test plan (per PACT §5)

### §5.1 Unit (Rust, `cargo test -p racecontrol-crate`)

THIS SESSION delivers DTO + handler-stub tests. Subsequent sessions add behavioral tests.

- `cirs_lookup_request_serde_phone_roundtrips` — phone-method JSON ↔ Rust struct parity
- `cirs_lookup_request_serde_qr_payload_roundtrips` — qr_payload-method
- `cirs_lookup_request_serde_nfc_tag_id_roundtrips` — nfc_tag_id-method
- `cirs_lookup_request_serde_walk_in_guest_id_roundtrips` — walk_in_guest_id-method
- `profile_preview_serde_canonical_shape_roundtrips` — `balance_credits` (NF-bono-1) field naming verified
- `cirs_lookup_handler_returns_501_until_session_2_lands` — placeholder gate (will be replaced in Session 2)

### §5.2 Integration (Session 2+)

Per PACT §5.2 — full v2-db pool, real cirs_lookup_audit row writes.

### §5.3 Component (Session 6)

Per PACT §5.3 — Next.js test runner, Indian-mobile-prefix WARN gate, ProfilePreview render shapes.

### §5.4 E2E (Session 6)

Per PACT §5.4 — Playwright on POS .130 staff-flow including §AMEND-1.D Arabic-Indic codepoint annotation.

### §5.5 Performance (Session 6)

Per PACT §5.5 — p50 ≤ 50ms / p95 ≤ 500ms / p99 ≤ 1000ms against canonical-customer-set.

---

## §6 — Deploy targets (per CLAUDE.md DEPLOY PARITY rule)

| Target | What gets deployed | Verification command |
|---|---|---|
| POS .130 staff terminal | Next.js v2 app `/v2/pos/lookup` | open in Edge kiosk, screenshot |
| Server .23 racecontrol binary | Rust handler + auth middleware extension | `curl -s http://192.168.31.23:8080/api/v1/health` build_id match |
| Bono VPS racecontrol binary | same Rust binary on cloud parity | `curl -s http://srv1422716.hstgr.cloud:8080/api/v1/health` build_id match |
| racecontrol-web kiosk + admin builds | rebuilt Next.js bundles for any shared component dependency | `frontend-staleness-check.sh` clean |
| Bono VPS web-v2 (`:3500/v2/pos/lookup`) | Next.js v2 app cloud parity | curl `https://v2.racingpoint.cloud/v2/pos/lookup` HTTP 200 + static asset 200 |

**Deploy-audit sequence (Session 7):**
```bash
cd ~/racingpoint/racecontrol
bash scripts/deploy/deploy-audit.sh 26677e42 <wireup-head-hash>
```

---

## §7 — Cross-pilot coordination

| Item | Pilot | Channel |
|---|---|---|
| bono Linux-parity verify (Session 7-8) | bono | branch push → bono pulls + cargo test on Linux |
| bono AMPLIFIER vote on wire-up PR | bono | comms-link `proposals/PACT-20260506-001-AMPLIFIER-bono-phase-1-wireup-code-review.md` |
| Captain per-PR auth at PR-open | Captain | direct INBOX or whatsapp channel |
| NF-bono-2 architectural ratify (Session 5 start) | Captain | sub-question on wire-up branch INBOX |

---

## §8 — Stale-at conditions

This PLAN durable until any of:
- (a) Phase 1 wire-up implementation PR merges (PLAN evolves into SUMMARY.md + LOGBOOK rows)
- (b) Captain V2.0 6-wave plan re-scope (e.g., compress Wave 0 sessions)
- (c) Substrate enum branch merge target changes (currently merging into `main` via per-PR auth on enum substrate first, then wire-up rebases onto `main`)
- (d) Captain disposes a Captain-reserve item that materially changes scope (e.g., Q-DECISION on ₹X refund threshold landing during Wave 0 vs deferring to Wave 1)
- (e) Verify-by 2026-05-19 — if Wave 0 not Session-8-complete by then, escalate

---

## §9 — Session metrics tracking

Per CGP v4.3 — appended to LOGBOOK at end of each session.

| Session | Date IST | Claims | Corrections | G9s | UCAs | FCR | Notes |
|---|---|---|---|---|---|---|---|
| 1 | 2026-05-07 | TBD | TBD | TBD | TBD | TBD | this session — scaffolding only |

---

— james-LEAD / 2026-05-07 ~06:18 IST · Phase 1 wire-up Session 1 of est. 5-8 · branch `feat/pact-001-phase-1-wireup` off `feat/pact-001-phase-1-privileged-actions-enum` HEAD `26677e42` · per-PR Captain auth gate STANDS for both enum substrate PR-open and wire-up PR-open · NF-bono-2 architectural decision deferred to Session 5
