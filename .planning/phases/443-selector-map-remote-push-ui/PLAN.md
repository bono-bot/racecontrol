---
phase: 443-selector-map-remote-push-ui
phase_number: 443
milestone: v50.0 rc-agent-mobile
name: "Selector-Map Remote Push UI"
status: ready-to-execute
goal: >
  Admin dashboard UI that lets staff (and James) ship a signed Ed25519 selector-map
  patch to Tab Plus + M07 in under 2 minutes. Admin uploads/pastes a YAML
  selectors file, server-side parses + schema-validates it, server signs with
  a private Ed25519 key held in racecontrol (NEVER on the admin frontend),
  stages the signed envelope to comms-link, pushes to target devices via the
  existing selector_push envelope (Phase 433 agent-side), tracks per-device
  apply status, and exposes a per-device "rollback to previous" button.
  Every push is audit-logged with actor (staff JWT), timestamp, YAML hash,
  target devices, push result, and per-device apply status. This is the
  critical path for "HyperPure UI changed at 2am — James must push a fix
  in < 5 min" scenarios. FRONTEND + SECURITY-CRITICAL: UI-SPEC mandatory
  pre-planning, UI-REVIEW mandatory before ship, MMA mandatory on signing
  key handling.
requirements: [ADMIN-05, SELECTOR-04]
depends_on:
  - 433-selector-dsl-hot-reload        # agent-side signature verify + apply + rollback
  - 441-admin-dashboard-reception-view  # admin shell, routing, auth pattern
wave: 7
plan_count: 9
plans:
  - 443-01-PLAN: UI-SPEC via gsd-ui-researcher (pre-req gate — no UI work without it)
  - 443-02-PLAN: Signing-key storage decision + ops runbook + rotation playbook
  - 443-03-PLAN: Server-side signing endpoint POST /api/v1/mobile/selectors/sign
  - 443-04-PLAN: Server-side push endpoint POST /api/v1/mobile/selectors/push + status/rollback routes
  - 443-05-PLAN: /mobile/selectors upload + schema-validate + preview + target-picker + confirm
  - 443-06-PLAN: Per-device push-status view (in-flight, applied, failed) with live updates
  - 443-07-PLAN: Rollback UI — per-device "rollback to previous" button + confirmation
  - 443-08-PLAN: UI-REVIEW via gsd-ui-auditor + security-check.js post-gate re-run
  - 443-09-PLAN: E2E drill — upload patch, push to Tab Plus, verify selector active, rollback, verify previous active
autonomous: false   # Plans 443-02, 443-05, 443-07, 443-09 contain checkpoints (decision, human-verify, security ack).

files_modified:
  # Backend (racecontrol server)
  - crates/racecontrol/src/api/mobile_selectors.rs                   # 443-03, 443-04 — new module
  - crates/racecontrol/src/api/routes.rs                             # 443-03, 443-04 — route registration
  - crates/racecontrol/src/api/mod.rs                                # 443-03 — module export
  - crates/racecontrol/src/signing/mobile_selector_signer.rs         # 443-03 — new module (Ed25519 signer)
  - crates/racecontrol/src/signing/mod.rs                            # 443-03 — module export
  - crates/racecontrol/src/config.rs                                 # 443-02 — [mobile.selectors] section
  - crates/racecontrol/src/db/migrations/NNN_mobile_selector_pushes.sql  # 443-04 — audit + status tables
  - crates/racecontrol/src/db/mobile_selector_pushes.rs              # 443-04 — DAO
  - crates/rc-common/src/protocol.rs                                 # 443-04 — relay envelope constants (read-only ref)

  # Admin frontend (Next.js)
  - racingpoint-admin/src/app/(dashboard)/mobile/selectors/page.tsx                  # 443-05 — upload + target + confirm
  - racingpoint-admin/src/app/(dashboard)/mobile/selectors/[pushId]/page.tsx         # 443-06 — per-push status view
  - racingpoint-admin/src/app/(dashboard)/mobile/selectors/devices/[deviceId]/page.tsx # 443-07 — rollback view
  - racingpoint-admin/src/components/mobile-selectors/YamlUploader.tsx               # 443-05
  - racingpoint-admin/src/components/mobile-selectors/SchemaPreview.tsx              # 443-05
  - racingpoint-admin/src/components/mobile-selectors/TargetPicker.tsx               # 443-05
  - racingpoint-admin/src/components/mobile-selectors/PushStatusTable.tsx            # 443-06
  - racingpoint-admin/src/components/mobile-selectors/RollbackButton.tsx             # 443-07
  - racingpoint-admin/src/components/mobile-selectors/SecurityWarningBanner.tsx      # 443-05 — signing-key-handling messaging
  - racingpoint-admin/src/lib/mobile-selectors-api.ts                                # 443-05, 443-06, 443-07 — client wrapper
  - racingpoint-admin/src/lib/hooks/useMobileSelectorPush.ts                         # 443-06 — polling hook (or WS subscribe)

  # Artifacts + docs
  - .planning/phases/443-selector-map-remote-push-ui/UI-SPEC.md                      # 443-01 (PRE-REQ)
  - .planning/phases/443-selector-map-remote-push-ui/UI-REVIEW.md                    # 443-08
  - .planning/phases/443-selector-map-remote-push-ui/SIGNING-KEY-OPS.md              # 443-02
  - .planning/phases/443-selector-map-remote-push-ui/SUMMARY.md                      # 443-09 close-out
  - docs/ARCHITECTURE.md                                                             # 443-04 section update
  - rc-agent-mobile/docs/SELECTORS.md                                                # 443-03 — "Signing" section cross-link

# DMP — Deploy Manifest Protocol (MANDATORY)
deploy:
  rust_binary: [racecontrol]                    # new routes + signing + DAO; rebuild required
  frontend_rebuild: [admin]                     # /mobile/selectors page is new
  config_change: "racecontrol.toml [mobile.selectors]"  # signing-key reference (env var or file path — see 443-02)
  db_migration: "mobile_selector_pushes + mobile_selector_push_targets"  # 443-04
  infrastructure: >
    New env var (if env-var strategy wins in 443-02): MOBILE_SELECTOR_SIGNING_KEY
    set on server .23 via schtasks env or a .env file loaded by start-racecontrol.bat.
    Cloud parity: same env var set on Bono VPS pm2 ecosystem file.
    PRIVATE KEY IS NOT IN GIT. Public key baked into rc-agent-mobile APK (Phase 433).
    Firewall: no new inbound ports; outbound to comms-link (existing).
  data_files: >
    No seed data files. Public key already present in APK assets per Phase 433.
    Private key provisioned via ops runbook (443-02 SIGNING-KEY-OPS.md).
  bat_file: start-racecontrol.bat  # may be amended to source MOBILE_SELECTOR_SIGNING_KEY env; see 443-02
  cloud_parity:
    - binary                # racecontrol rebuild on Bono VPS
    - frontend              # admin rebuild on Bono VPS (apps/admin or equivalent)
    - config                # [mobile.selectors] section present + env var set
  targets:
    - server                # .23 (ADMIN) — signing endpoint, push endpoint, admin frontend rebuild
    - cloud                 # Bono VPS — signing endpoint, push endpoint, admin frontend rebuild (DEPLOY PARITY)
    - tab_plus              # target of selector_push envelopes (Phase 433 handler already live)
    - m07                   # target of selector_push envelopes
    - james                 # comms-link relay env (docs + envelope-type registration if any)
  rollback:
    - "Rollback UI (443-07) is FIRST-CLASS — per-device button in admin. Uses existing Phase 433 handler; no new agent code."
    - "Server-side rollback: previous racecontrol binary preserved at C:\\RacingPoint\\racecontrol-prev.exe (72hr)."
    - "Frontend rollback: previous admin build preserved (Next.js standalone swap)."
    - "Signing key rotation: ops runbook in SIGNING-KEY-OPS.md covers compromise response."

# Subagent gates (per CLAUDE.md > Subagent Gates section)
gates:
  ui_researcher: required       # MANDATORY — frontend phase. 443-01 IS the UI-SPEC.md plan. No other plan starts without it.
  ui_auditor: required          # MANDATORY — frontend phase. 443-08 runs post-execution.
  nyquist_auditor: required     # Signing logic + push state machine + audit log are business-critical.
  mma_audit: required           # SIGNING KEY HANDLING IS SECURITY-CRITICAL. Dual reasoning modes REQUIRED (abstract + trace-level). Per CLAUDE.md: cross-trust-boundary flow (admin UI → server signer → comms-link → agent verify → apply → rollback) demands both modes.
  security_check: required      # node comms-link/test/security-check.js — PRE (before 443-03 ships) + POST (after 443-08, before milestone ship). No new unprotected routes, no credential leaks.
  integration_checker: required # Cross-phase flow admin→server-sign→comms-link→agent-verify→apply→rollback spans racecontrol + comms-link + rc-agent-mobile. Mandatory.
  codebase_mapper: skip         # No new top-level module; additions inside existing racecontrol + racingpoint-admin.

risks_summary:
  - "PRIVATE KEY LEAK = arbitrary UI actions on reception devices. Anyone with the key can push a selector that makes the agent tap 'Accept all orders for free'. Blast radius: Zomato/HyperPure/Blinkit accounts + ToS violation."
  - "UI mistake propagates bad selectors FLEET-WIDE. A wrong target-picker click (e.g. 'all devices' when intent was 'Tab Plus only') breaks both reception devices simultaneously. Mitigation: per-device target-picker with explicit toggle per device, preview diff against currently-deployed map, typed-confirmation modal on multi-device push."
  - "Server-side signing endpoint is a new surface — must require staff JWT + be listed in security-check.js route-auth-coverage. Leaking the endpoint to unauth would equal leaking the key."
  - "Rollback depends on Phase 433 agent-side .backup file being present. If the agent was rebooted between apply and rollback request, .backup may be missing. Mitigation: server also stores last N YAML payloads, admin can re-push the previous signed payload as a new patch_version."
  - "Replay protection — server MUST enforce monotonically-increasing patch_version per (app_package, app_version) tuple so a re-signed OLD YAML can't be shipped as a 'rollback'. Mitigation: DB uniqueness + CHECK constraint, covered in 443-04."
  - "HyperPure/Blinkit scenario: James at 2am on a phone. UI must work on mobile (responsive). Not assuming desktop-only."
  - "Admin UI is served from admin dashboard (racingpoint-admin). Admin is authenticated via admin-session; ADMIN-05 requires this endpoint respect the admin JWT. A staff-only-JWT path would be a bug."
  - "Comms-link delivery is best-effort — a target device may be offline. Push status UI must distinguish 'queued / sent / acked / applied / failed / timeout (30s)'."
  - "Ed25519 signature over YAML bytes is canonicalized per Phase 433 docs/SELECTORS.md §Signing (LF, no BOM, trim trailing whitespace per line). Server-side signer MUST use identical canonicalization or every device rejects the signature."
  - "Key rotation: Phase 433 BuildConfig.TRUSTED_SIGNING_KEY_IDS is a list. When rotating, new APK with [old, new] ships to devices BEFORE admin switches to new key. Coordinated rollout required — documented in 443-02."
---

# Phase 443 — Selector-Map Remote Push UI

## 1. Phase Header

| Field | Value |
|---|---|
| Phase | 443 |
| Name | Selector-Map Remote Push UI |
| Milestone | v50.0 rc-agent-mobile — Reception Automation Hub |
| REQ-IDs covered | ADMIN-05, SELECTOR-04 (UI-side; agent-side was Phase 433) |
| Dependencies | Phase 433 (agent-side signature verify + apply + rollback), Phase 441 (admin shell + routing) |
| Wave | 7 (after 433 + 441; can run parallel to 444 post-441) |
| Status | Ready to execute (UI-SPEC pre-req is 443-01) |
| Autonomous | No — human-verify checkpoints on signing-key decision, push confirmation UX, rollback UX, E2E drill |
| Ship test | (1) Admin uploads YAML → schema-validate passes; (2) Preview shows diff; (3) Target-picker selects Tab Plus; (4) Confirm push → signed patch shipped; (5) Device applies within 10s, UI reflects "applied"; (6) "Rollback" button restores previous, UI reflects "rolled back". End-to-end < 2 min. |

## 2. Success criteria (verbatim from ROADMAP-v50.md Phase 15)

1. **< 2 min end-to-end.** Upload + target + push flow completes in under 2 minutes (James at 2am scenario).
2. **Signature enforcement.** Signature verification rejects unsigned or tampered patches (Phase 433 handler).
3. **Rollback ≤ 10s.** Rollback restores previous selector-map within 10 seconds.

## 3. Goal-backward must-haves

Derived from "what must be TRUE for each success criterion?"

### Truths (user-observable)

- T-1: On admin dashboard, staff navigates to `/mobile/selectors`, sees an upload form within 1 click from reception page.
- T-2: Pasting/uploading a YAML file calls `POST /api/v1/mobile/selectors/validate` (staff JWT) which parses + schema-validates and returns either `{ok: true, summary: {app_package, app_version, screens_count, elements_count}}` or `{ok: false, errors: [{line, col, message}]}`.
- T-3: After validate-OK, UI shows a diff preview vs currently-deployed map per target device. If no map exists, shows "FIRST PUSH — no previous map".
- T-4: Target picker lists both devices (Tab Plus, M07) with last-seen, current patch_version per device, online/offline status. Defaults: NONE selected (explicit opt-in).
- T-5: "Push" button is DISABLED until (a) YAML validates, (b) at least one target is selected, (c) a typed confirmation "PUSH" is entered when targeting more than one device.
- T-6: On push, UI shows a per-device progress row: `queued → sent → ack_received → applied | failed (reason) | timeout (30s)`. Live updates via polling (1s) or WS subscription.
- T-7: Clicking "Rollback" on a device shows a confirmation modal listing the previous map's patch_version, generated_at, and generated_by. Confirm → calls rollback endpoint; within 10s the row shows "rolled_back, patch_version: N-1".
- T-8: Every push action is logged in `mobile_selector_pushes` with `{actor_staff_id, actor_jwt_hash, ts, yaml_sha256, target_devices[], push_result, per_device_apply_status[]}`. Admin dashboard exposes a simple history table at `/mobile/selectors?history=1`.
- T-9: Curl-unauth on `/api/v1/mobile/selectors/sign` or `/api/v1/mobile/selectors/push` returns HTTP 401. Curl with non-admin staff JWT returns HTTP 403. Verified by security-check.js route-auth assertion (443-08).
- T-10: Admin frontend NEVER sees the private signing key. Browser DevTools Network tab shows no `MOBILE_SELECTOR_SIGNING_KEY` value in any request/response.

### Required artifacts (files that must exist)

| Path | Provides | Min lines | Contains |
|------|----------|-----------|----------|
| `crates/racecontrol/src/api/mobile_selectors.rs` | Sign + push + status + rollback handlers | 400 | `sign_handler`, `push_handler`, `push_status_handler`, `rollback_handler`, `validate_handler`, `history_handler` with staff-JWT guards |
| `crates/racecontrol/src/signing/mobile_selector_signer.rs` | Ed25519 signer | 150 | `MobileSelectorSigner::new(key_ref)`, `sign(canonical_yaml_bytes) -> (sig_base64, key_id)` with `java.security`-compatible canonicalization (LF, no BOM, trim trailing WS) |
| `crates/racecontrol/src/db/migrations/NNN_mobile_selector_pushes.sql` | Audit + status tables | 80 | `mobile_selector_pushes` (push_id, actor_staff_id, yaml_sha256, patch_version, created_at, app_package, app_version, status), `mobile_selector_push_targets` (push_id FK, device_id, status enum, applied_at, error_detail) |
| `crates/racecontrol/src/db/mobile_selector_pushes.rs` | DAO | 200 | `insert_push`, `update_target_status`, `get_push`, `get_current_patch_version(device_id, app_package, app_version)`, `get_history(limit)` |
| `crates/racecontrol/src/config.rs` (amend) | `[mobile.selectors]` config section | +30 | `MobileSelectorConfig { signing_key_ref: SigningKeyRef, key_id: String, trusted_key_ids: Vec<String> }` |
| `racingpoint-admin/src/app/(dashboard)/mobile/selectors/page.tsx` | Upload + target + push page | 250 | YamlUploader → SchemaPreview → TargetPicker → confirm modal; fetches from /api/v1/mobile/selectors/* |
| `racingpoint-admin/src/app/(dashboard)/mobile/selectors/[pushId]/page.tsx` | Per-push status | 120 | PushStatusTable with per-device rows; polls /status every 1s until terminal |
| `racingpoint-admin/src/app/(dashboard)/mobile/selectors/devices/[deviceId]/page.tsx` | Per-device rollback | 150 | Current patch_version, previous patch_version, rollback button + confirmation modal |
| `racingpoint-admin/src/components/mobile-selectors/YamlUploader.tsx` | Upload component | 90 | File input + paste textarea; validates via API before preview |
| `racingpoint-admin/src/components/mobile-selectors/TargetPicker.tsx` | Device selection | 120 | Per-device checkbox, last-seen, current patch_version, online/offline; typed-confirmation when N>=2 |
| `racingpoint-admin/src/components/mobile-selectors/PushStatusTable.tsx` | Live status table | 140 | Columns: device, status, ack_at, applied_at, error; colored status pills |
| `racingpoint-admin/src/components/mobile-selectors/RollbackButton.tsx` | Per-device rollback | 80 | Confirmation modal + POST /rollback |
| `racingpoint-admin/src/components/mobile-selectors/SecurityWarningBanner.tsx` | Security messaging | 40 | Yellow banner: "Pushes execute arbitrary UI actions on reception devices. Every push is logged with your staff ID. Verify YAML preview before confirming." |
| `racingpoint-admin/src/lib/mobile-selectors-api.ts` | Typed client | 120 | `validateYaml`, `pushSelectors`, `getPushStatus`, `rollbackDevice`, `getHistory`, `getDeviceState` |
| `racingpoint-admin/src/lib/hooks/useMobileSelectorPush.ts` | Live polling hook | 60 | SWR or polling; terminal-state detection |
| `.planning/phases/443-selector-map-remote-push-ui/UI-SPEC.md` | UI specification | 300 | Per gsd-ui-researcher — screens, states, error handling, a11y, mobile-responsive notes, security messaging |
| `.planning/phases/443-selector-map-remote-push-ui/UI-REVIEW.md` | Post-execution audit | 200 | Per gsd-ui-auditor — findings, severity, screenshots |
| `.planning/phases/443-selector-map-remote-push-ui/SIGNING-KEY-OPS.md` | Ops runbook | 180 | Where key lives, how to generate, how to rotate, how to backup, compromise response |
| `.planning/phases/443-selector-map-remote-push-ui/SUMMARY.md` | Close-out | 100 | Drill artifacts, MMA findings, security-check.js pre/post diff |

### Key links (wiring — most likely to break)

| From | To | Via | Pattern to verify |
|------|----|-----|-------------------|
| `/mobile/selectors page` | `POST /api/v1/mobile/selectors/validate` | fetchApi (admin JWT) | grep `mobile/selectors/validate` in `racingpoint-admin/src/lib/mobile-selectors-api.ts` |
| `/mobile/selectors page` | `POST /api/v1/mobile/selectors/sign` | fetchApi (admin JWT) | grep `mobile/selectors/sign` in mobile-selectors-api.ts |
| `POST /sign` handler | `MobileSelectorSigner::sign` | direct Rust call | grep `MobileSelectorSigner` in `crates/racecontrol/src/api/mobile_selectors.rs` |
| `POST /push` handler | comms-link relay `selector_push` envelope | HTTP POST to relay OR WS broadcast | grep `selector_push` in `crates/racecontrol/src/api/mobile_selectors.rs` |
| `POST /push` handler | `insert_push` + `insert_push_targets` | DAO call | grep `insert_push` in `mobile_selectors.rs` |
| Agent ACK (from Phase 433) | `update_target_status` | comms-link incoming WS handler | grep `selector_push_ack` in racecontrol WS handler |
| PushStatusTable.tsx | `GET /api/v1/mobile/selectors/pushes/:id/status` | polling | grep `/status` in `useMobileSelectorPush.ts` |
| RollbackButton.tsx | `POST /api/v1/mobile/selectors/devices/:id/rollback` | fetchApi | grep `rollback` in mobile-selectors-api.ts |
| Config loader | `SigningKeyRef` resolve (env var OR file path) | config.rs | grep `MOBILE_SELECTOR_SIGNING_KEY` in `crates/racecontrol/src/config.rs` |
| security-check.js | Route-auth coverage for all new endpoints | static assertion | grep `mobile/selectors` in `comms-link/test/security-check.js` after 443-04 |

## 4. Context — files to read before executing any plan

@./CLAUDE.md
@./.planning/REQUIREMENTS-v50.md  # ADMIN-05 + SELECTOR-04 blocks
@./.planning/ROADMAP-v50.md        # Phase 15 detail
@./.planning/PROJECT.md            # v50.0 remote selector push extensibility
@./.planning/phases/433-selector-dsl-hot-reload/PLAN.md   # agent-side contract — signature scheme, envelope shape, canonicalization, ACK shape, rollback path
@./.planning/phases/441-admin-dashboard-reception-view/PLAN.md  # admin shell + routing + auth pattern
@./rc-agent-mobile/docs/PROTOCOL.md                        # selector_push + selector_push_ack envelope schemas (authored in 433-07)
@./rc-agent-mobile/docs/SELECTORS.md                       # §Signing — canonicalization rule (LF, no BOM, trim trailing whitespace)
@./comms-link/test/security-check.js                       # MUST pass pre + post; reference for route-auth pattern
@./crates/racecontrol/src/api/auth_staff.rs                # staff JWT guard pattern
@./crates/racecontrol/src/api/routes.rs                    # route registration conventions
@./crates/racecontrol/src/db/mod.rs                        # DAO patterns, migration registration
@./comms-link/docs/PROTOCOL.md                             # relay forwarding semantics for selector_push
@./racingpoint-admin/src/app/(dashboard)/fleet/page.tsx    # existing admin page pattern (auth, layout, hooks)

### Interfaces executors will need

#### A. selector_push envelope (from Phase 433, authoritative)

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "selector_push",
  "from": "admin-dashboard",
  "to": "rcm-tab-plus",
  "ts": 1713440000000,
  "id": "uuid-v4",
  "payload": {
    "app_package": "com.zomato.partner",
    "app_version": "3.14.2",
    "yaml_canonical": "<UTF-8 YAML bytes, LF line endings, no BOM, no trailing whitespace>",
    "signature_ed25519": "<base64 detached signature over yaml_canonical>",
    "signed_by_key_id": "admin-v1-2026-04-18",
    "patch_version": 3,
    "supersedes_patch_version": 2
  }
}
```

**Canonicalization (MUST match Phase 433's PatchSignatureVerifier):**
- Line endings normalized to LF (no CRLF).
- No UTF-8 BOM.
- Trailing whitespace trimmed per line.
- UTF-8 encoding.

Phase 433's `PatchSignatureVerifier` applies identical normalization before verify. Any drift = device-side `register_rejected_signature` error → push fails.

#### B. selector_push_ack envelope (from Phase 433, authoritative)

```json
{
  "v": 1,
  "protocol_version": 1,
  "type": "selector_push_ack",
  "from": "rcm-tab-plus",
  "to": "admin-dashboard",
  "ts": 1713440000500,
  "id": "uuid-ack",
  "payload": {
    "push_envelope_id": "uuid-v4",
    "accepted": true,
    "reason": null,
    "patch_version": 3
  }
}
```

On failure: `accepted: false`, `reason: "signature_invalid" | "parse_error" | "write_failed" | "stale_patch" | "unknown_signing_key"`.

Server-side status machine (443-04) maps ACK payloads to DB `mobile_selector_push_targets.status` enum:
`queued → sent → ack_received → applied | rejected_signature | rejected_parse | rejected_stale | rejected_write | timeout_30s`.

#### C. Rollback handler contract

Phase 433 agent-side rollback restores `.backup` file. Server-side rollback endpoint:
1. Looks up per-device `current patch_version` + `previous patch_version` in DB.
2. Fetches previous YAML bytes from server-side storage (mobile_selector_pushes.yaml_canonical column).
3. Re-signs and pushes previous YAML as NEW patch_version (e.g. N+1) where payload content = previous map. This is safer than asking the agent to swap its local .backup because:
   - Server always has authoritative source.
   - patch_version monotonicity preserved.
   - Rollback is just "a push of the old content" — same code path, same audit trail.
4. UI shows "rolled_back_to_v2 (new patch_version: 5)" — clear provenance.

**Why server-authoritative rollback (not agent-local `.backup` swap):**
- Agent `.backup` may be missing after reboot.
- Auditability: every state change is a push with a patch_version.
- Phase 433 handler works unchanged — no new agent code in Phase 443.
- Consistent with CLAUDE.md "Smallest Reversible Fix First" — re-use existing push path.

Alternative (rejected): expose `POST /rollback` on rc-agent-mobile HTTP server that swaps .backup without a new push envelope. Rejected because: (a) new agent code needed → bumps APK version → deploy coordination, (b) not signed → trust-boundary violation, (c) state divergence between server DB and device if .backup missing.

#### D. Schema for mobile_selector_pushes

```sql
CREATE TABLE IF NOT EXISTS mobile_selector_pushes (
  push_id          TEXT PRIMARY KEY,           -- UUID v4
  actor_staff_id   TEXT NOT NULL,               -- from staff JWT
  actor_jwt_hash   TEXT NOT NULL,               -- SHA256(jwt) — for audit, not replay
  created_at       INTEGER NOT NULL,            -- ms since epoch, IST-displayed
  app_package      TEXT NOT NULL,
  app_version      TEXT NOT NULL,
  patch_version    INTEGER NOT NULL,            -- monotonic per (app_package, app_version)
  yaml_sha256      TEXT NOT NULL,
  yaml_canonical   BLOB NOT NULL,               -- full signed bytes (for rollback re-push)
  signature_b64    TEXT NOT NULL,
  signing_key_id   TEXT NOT NULL,
  is_rollback_of   TEXT,                        -- FK to push_id of source of a rollback re-push
  status           TEXT NOT NULL,               -- queued | in_flight | completed | partial | failed
  updated_at       INTEGER NOT NULL,
  UNIQUE (app_package, app_version, patch_version)
);

CREATE TABLE IF NOT EXISTS mobile_selector_push_targets (
  push_id          TEXT NOT NULL REFERENCES mobile_selector_pushes(push_id) ON DELETE CASCADE,
  device_id        TEXT NOT NULL,               -- "rcm-tab-plus" | "rcm-m07"
  status           TEXT NOT NULL,               -- queued | sent | ack_received | applied | rejected_* | timeout_30s
  sent_at          INTEGER,
  acked_at         INTEGER,
  applied_at       INTEGER,
  error_reason     TEXT,
  error_detail     TEXT,
  PRIMARY KEY (push_id, device_id)
);

CREATE INDEX idx_mspt_device_status ON mobile_selector_push_targets(device_id, status);
CREATE INDEX idx_msp_created_at ON mobile_selector_pushes(created_at);
```

The `UNIQUE (app_package, app_version, patch_version)` constraint is the replay/monotonicity guard — DB rejects a duplicate patch_version.

## 5. Atomic plan breakdown (9 plans)

Plans follow dependency order. Plans 443-01, 443-02, 443-08 are gates. Plans 443-03 → 443-07 are implementation. 443-09 is the E2E drill.

---

### 443-01-PLAN — UI-SPEC (PRE-REQ gate via gsd-ui-researcher)

**Goal:** Produce `.planning/phases/443-selector-map-remote-push-ui/UI-SPEC.md` as the authoritative UX contract for plans 443-05, 443-06, 443-07. No UI code may be written until this artifact exists.

**Covers:** Frontend gate (CLAUDE.md > Subagent Gates — "Any frontend" phase REQUIRES UI-SPEC.md before planning).

**Dependencies:** none (first plan)

**Type:** `auto` (invoke gsd-ui-researcher subagent; output is a markdown artifact)

**Tasks:**

1. Invoke `gsd-ui-researcher` subagent with this scope:
   - Entry point: `/mobile/selectors` on admin dashboard.
   - Flows: (A) upload+target+push, (B) per-device push-status view, (C) per-device rollback.
   - States to cover: idle, YAML-validating, YAML-invalid, YAML-valid+preview, target-selected, push-in-flight, push-partial-success, push-total-failure, rollback-confirming, rollback-applying, rollback-complete, rollback-failed.
   - Security messaging: SecurityWarningBanner text (explicit call-out of audit logging + arbitrary UI action capability + staff-ID attribution), typed-confirmation modal copy for multi-device push, rollback confirmation modal copy.
   - Mobile responsive: James on phone at 2am — MUST be usable on viewport >= 360px wide.
   - A11y: keyboard-only navigation, screen-reader labels on YAML errors, focus management on modals.
   - Error states per field + network failure + signing failure + relay-unreachable.
   - Empty state: "No pushes yet — upload a selector YAML to begin."
   - Existing visual patterns to match: cards (reception dashboard), status pills (fleet health), modals (admin auth).
   - Brand: Racing Red #E10600 for primary CTAs, Asphalt Black #1A1A1A bg, Montserrat body.

2. Researcher consults existing racingpoint-admin patterns:
   - Fleet health table (PushStatusTable pattern reference).
   - Admin settings dialog (modal pattern reference).
   - Session history (history table pattern reference).

3. UI-SPEC output MUST include:
   - Wireframe-level ASCII layout of each screen.
   - Exhaustive state table.
   - Error copy (exact strings) for every error state.
   - Security-warning copy (exact strings) — must be explicit about staff-ID attribution + blast radius.
   - Acceptance test list (what the UI-auditor will verify in 443-08).

**Acceptance:**
- `.planning/phases/443-selector-map-remote-push-ui/UI-SPEC.md` exists, ≥ 250 lines.
- Researcher output covers all 11 states listed above.
- Security messaging drafted explicitly (not placeholder).
- Mobile responsive + a11y notes present.

**G4 NOT TESTED list:**
- No runtime code. UI-SPEC is a design artifact; enforcement happens in 443-05/06/07 + 443-08.

**Commit message:**
```
docs(443-01): UI-SPEC for mobile selector remote push UI

Produced via gsd-ui-researcher. Covers upload+target+push, status view,
rollback. 11 states documented. Security messaging drafted.
Mobile responsive + a11y notes included.

Gate: frontend phase pre-req per CLAUDE.md Subagent Gates.
Covers: ADMIN-05, SELECTOR-04 (UI design contract)
```

---

### 443-02-PLAN — Signing-key storage decision + ops runbook + rotation playbook

**Goal:** Answer the open question "Where do we store the Ed25519 private signing key?" and produce `SIGNING-KEY-OPS.md` with generation, storage, rotation, compromise-response procedures.

**Covers:** Pre-req for 443-03 (signing endpoint cannot be built without key storage decided).

**Dependencies:** none (parallel to 443-01)

**Type:** `checkpoint:decision` (Uday decides storage mechanism; James cannot decide autonomously)

**The OPEN QUESTION (blocking):**

> Where do we store the Ed25519 private signing key used by the racecontrol server to sign mobile selector patches?

**Candidate options presented to Uday:**

| Option | Pros | Cons | Storage | Rotation Complexity |
|--------|------|------|---------|---------------------|
| A. **Environment variable** `MOBILE_SELECTOR_SIGNING_KEY` (PEM base64-encoded, single-line) | Simple. No new infra. Works on Bono VPS pm2 + server schtasks. Never in git. | Visible in `ps` environ on some systems. Must be managed in two places (server .23 + Bono VPS). | env vars on each host, sourced via `start-racecontrol.bat` + pm2 ecosystem file. | Low — update env + restart. |
| B. **File on disk** `C:\RacingPoint\secrets\mobile-selector-ed25519.pem`, path referenced in `racecontrol.toml` | Simple. Matches existing patterns (TLS certs on same machine). Access-controlled via NTFS ACL. | Still requires safe distribution to Bono VPS. File sprawl — one more secret file to track. | `C:\RacingPoint\secrets\mobile-selector-ed25519.pem` on server; `/etc/racecontrol/secrets/...` on Bono VPS. NEVER in git. | Low — replace file + restart. |
| C. **1Password CLI** (`op read`) — pulled at startup, cached in memory for process lifetime | Centralized secret store; rotation without touching servers directly; audit trail per access. | Adds 1Password dependency at startup; startup fails if 1Password is unreachable. Requires 1Password service account token on each host (which itself becomes a secret). | 1Password vault `racecontrol-secrets`, item `mobile-selector-ed25519`. | Medium — rotate in 1Password, trigger restart. |
| D. **HSM / Cloud KMS** (AWS KMS, GCP KMS, YubiHSM) | Strongest protection — key never leaves HSM. Signing happens inside HSM. | Adds cloud dependency + latency per sign call. Requires network egress from server. Overkill for 1-push-per-week operational cadence. | KMS-managed. | Medium — but rarely needed if done right. |

**Recommendation for Uday's decision (pre-consult, non-binding):**

**Option B (file on disk) with 1Password for rotation ceremonies** — pragmatic middle ground:
- Key file lives at `C:\RacingPoint\secrets\mobile-selector-ed25519.pem` (Windows) / `/etc/racecontrol/secrets/mobile-selector-ed25519.pem` (Bono VPS).
- NTFS ACL (Windows): owner ADMIN, no inherit, explicit ADMIN read-only.
- File mode (Bono VPS): 0400, owner racecontrol user.
- `racecontrol.toml [mobile.selectors]` references path:
  ```toml
  [mobile.selectors]
  signing_key_path = "C:\\RacingPoint\\secrets\\mobile-selector-ed25519.pem"
  signing_key_id   = "admin-v1-2026-04-18"
  trusted_key_ids  = ["admin-v1-2026-04-18"]  # expands during rotation overlap
  ```
- Canonical key is stored in 1Password (`racecontrol-secrets / mobile-selector-ed25519`) for disaster-recovery restore and rotation ceremonies.
- Env var fallback ALSO supported (`MOBILE_SELECTOR_SIGNING_KEY` — PEM content, base64): if set, takes precedence over `signing_key_path`. This preserves option A as an escape hatch if file-on-disk ever becomes problematic.

Why not Option A alone: env vars surface in `ps auxe` and some logging paths. File+ACL is stricter on both platforms.
Why not Option C alone: 1Password dependency at startup is a new failure mode for the single most important Racing Point service.
Why not Option D: Overkill for ~1 push/week. Revisit post-v50 if cadence grows.

**Tasks:**

1. **Checkpoint (decision):** Present options to Uday. Await decision. Record choice + rationale in SIGNING-KEY-OPS.md §"Key storage: locked decision".

2. Write `.planning/phases/443-selector-map-remote-push-ui/SIGNING-KEY-OPS.md`:
   - §1. Locked decision (from checkpoint).
   - §2. Key generation procedure: `openssl genpkey -algorithm ed25519 -out mobile-selector-ed25519.pem` + `openssl pkey -in ... -pubout -out mobile-selector-ed25519.pub.pem`.
   - §3. Key distribution: how to place on server .23 and Bono VPS (never via git; SCP from James's workstation; NTFS ACL / chmod after copy). Include exact commands.
   - §4. Public key distribution: baked into rc-agent-mobile APK `keystores/signing-pubkey-v1.pem` (Phase 433). When rotating, `TRUSTED_SIGNING_KEY_IDS = [old, new]` for overlap (ship new APK → rotate private key → remove old from APK in next release).
   - §5. Rotation procedure: step-by-step playbook. Includes APK rebuild coordination.
   - §6. Backup: canonical key in 1Password `racecontrol-secrets / mobile-selector-ed25519`. Restore procedure.
   - §7. Compromise response: revoke key ID from TRUSTED_SIGNING_KEY_IDS in Phase 433 BuildConfig, ship new APK, generate new key, push manual rollback of any suspect selectors via adb. Incident runbook ~30 min.
   - §8. NTFS ACL + chmod verification commands.
   - §9. Pre-commit hook: ensure `*.pem` is in `.gitignore` at repo root (verify).

3. Amend root `.gitignore` to explicitly deny `**/secrets/`, `**/*-ed25519.pem`, `**/*-ed25519.pub.pem` except the public pubkey-v1.pem explicitly allowed in rc-agent-mobile/keystores/.

4. Amend `crates/racecontrol/src/config.rs` to add `MobileSelectorConfig` struct reading `[mobile.selectors]` section. Support both `signing_key_path` (file) AND `MOBILE_SELECTOR_SIGNING_KEY` env var (override).

5. Update `start-racecontrol.bat` to (optionally) source `MOBILE_SELECTOR_SIGNING_KEY` from a sibling `.env` file if one exists. Do NOT log the var value. Document in bat comment.

**Acceptance:**
- Uday's decision recorded in SIGNING-KEY-OPS.md §1 (exact choice + rationale).
- `SIGNING-KEY-OPS.md` exists, ≥ 150 lines, all §§ present.
- `.gitignore` prevents `*-ed25519.pem` commit (verify by `git check-ignore mobile-selector-ed25519.pem` returns true).
- `config.rs` compiles with new `MobileSelectorConfig`.
- NO key material generated in this plan — the KEY GENERATION ceremony is run by James (or Uday) out-of-band and tracked in SIGNING-KEY-OPS.md.

**Checkpoint (decision):** Uday selects A/B/C/D and confirms SIGNING-KEY-OPS.md §§1-9. **Resume signal:** "DECISION: Option <X>. SIGNING-KEY-OPS approved." or "Revise §<N>: <feedback>".

**G4 NOT TESTED list:**
- Signer behavior (443-03).
- Key rotation drill (deferred post-v50; documented only).
- Compromise-response drill (deferred post-v50; documented only).

**Commit message:**
```
docs(443-02): mobile selector signing key — storage decision + ops runbook

Locked decision: Option <X> per Uday.
SIGNING-KEY-OPS.md covers generation, distribution, rotation, compromise
response, backup/restore. .gitignore hardened against PEM commit.
config.rs reads [mobile.selectors] with signing_key_path + env var override.
start-racecontrol.bat sources optional .env without logging values.

Covers: OPEN QUESTION Q443-1 (signing key storage)
Not tested: signer impl (443-03), rotation drill (deferred).
```

---

### 443-03-PLAN — Server-side signing endpoint `POST /api/v1/mobile/selectors/sign`

**Goal:** Server-side handler that takes YAML bytes + requested app metadata, canonicalizes per Phase 433 rule, signs with the private Ed25519 key (via path-or-env per 443-02), returns the signed envelope payload for the push endpoint to ship. Staff JWT required.

**Covers:** SELECTOR-04 (server-side signing)

**Dependencies:** 443-02 (key storage locked)

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: Given a valid YAML and staff JWT, `POST /sign` returns 200 with `{yaml_canonical, signature_ed25519, signed_by_key_id, yaml_sha256}`.
- Test 2: Given YAML with CRLF line endings, server canonicalizes to LF before signing (signature matches agent-side canonicalization).
- Test 3: Given YAML with trailing whitespace on lines, server trims before signing.
- Test 4: Given YAML with BOM, server strips before signing.
- Test 5: Missing staff JWT → 401.
- Test 6: Non-admin staff JWT (staff role not admin) → 403.
- Test 7: Malformed YAML (parse error) → 400 with `{errors: [{line, col, message}]}`.
- Test 8: Signing key path unreadable at call time → 500 with `{error: "signing_unavailable"}` + ERROR log with remediation pointer to SIGNING-KEY-OPS.md.
- Test 9: Signature verifies against the pubkey bundled in rc-agent-mobile APK (test uses the actual `signing-pubkey-v1.pem`).

**Tasks:**

1. Create `crates/racecontrol/src/signing/mobile_selector_signer.rs`:
   ```rust
   pub struct MobileSelectorSigner {
       key_id: String,
       private_key: ed25519_dalek::SigningKey,
   }
   impl MobileSelectorSigner {
       pub fn from_config(cfg: &MobileSelectorConfig) -> Result<Self, SignerError> { ... }
       pub fn sign(&self, raw_yaml: &[u8]) -> SignedPayload { ... }
       pub fn canonicalize(raw: &[u8]) -> Vec<u8> {
           // 1. Strip UTF-8 BOM (EF BB BF) if present.
           // 2. Normalize line endings CRLF → LF.
           // 3. Trim trailing whitespace per line (regex /[ \t]+$/m).
           // 4. Return UTF-8 bytes.
       }
   }
   pub struct SignedPayload {
       pub yaml_canonical: Vec<u8>,
       pub signature_ed25519_b64: String,
       pub signed_by_key_id: String,
       pub yaml_sha256_hex: String,
   }
   ```
   Dependency: `ed25519-dalek = "2"` (already in Cargo.lock for other crates — confirm at 443-03 kickoff; if not, add).

2. Create `crates/racecontrol/src/api/mobile_selectors.rs` with `sign_handler`:
   ```rust
   pub async fn sign_handler(
       State(st): State<AppState>,
       staff: StaffAuth,  // admin role required — extracted via existing middleware
       Json(req): Json<SignRequest>,
   ) -> Result<Json<SignResponse>, ApiError> {
       require_admin(&staff)?;   // 403 if not admin
       let signer = &st.mobile_selector_signer;
       let parsed = parse_and_validate_yaml(&req.yaml_utf8)?;  // 400 on parse fail
       let signed = signer.sign(req.yaml_utf8.as_bytes());
       Ok(Json(SignResponse { yaml_canonical_b64, signature_ed25519, signed_by_key_id, yaml_sha256, summary: parsed.summary() }))
   }
   ```
   `require_admin` — new helper or existing (grep `require_admin` or `admin_only` in auth_staff.rs; if missing, add).

3. Register route in `crates/racecontrol/src/api/routes.rs` under the staff-JWT-protected router:
   ```rust
   .route("/api/v1/mobile/selectors/validate", post(mobile_selectors::validate_handler))
   .route("/api/v1/mobile/selectors/sign",     post(mobile_selectors::sign_handler))
   ```
   Also add a separate lightweight `validate_handler` (parse + schema-check only, no signing — used by 443-05 preview before signing). Validate does NOT require admin, only staff JWT (schema-check is not sensitive on its own).

4. Wire `MobileSelectorSigner::from_config` into `AppState` construction at startup. If key unreadable, log ERROR with SIGNING-KEY-OPS.md pointer and mark the signer as unavailable — the handler returns 500 `signing_unavailable` instead of crashing the whole server.

5. Amend `comms-link/test/security-check.js` (coordinated with 443-04 amendment) to assert:
   - `/api/v1/mobile/selectors/sign` requires staff JWT + admin role (403 otherwise).
   - `/api/v1/mobile/selectors/validate` requires staff JWT (non-admin staff OK).

6. Amend `rc-agent-mobile/docs/SELECTORS.md` §Signing to cross-reference the server-side signer implementation path (`crates/racecontrol/src/signing/mobile_selector_signer.rs`) — per CLAUDE.md "Cascade updates RECURSIVE" rule.

7. Unit tests per tdd.behavior. Use a test-fixture private key (NOT the production key) committed in `crates/racecontrol/tests/fixtures/test-ed25519-priv.pem` (acceptable — it's a throwaway test key; documented as such).

**Acceptance:**
- `cargo test -p racecontrol-crate --test mobile_selector_signer` all pass.
- `cargo test -p racecontrol-crate --test mobile_selectors_api` all pass.
- `curl -X POST http://localhost:8080/api/v1/mobile/selectors/sign -H "Authorization: Bearer <staff_admin_jwt>" -H "Content-Type: application/json" -d @valid.json` returns 200 with signed payload.
- `curl -X POST ...` without JWT → 401.
- `curl -X POST ... -H "Authorization: Bearer <staff_non_admin_jwt>"` → 403.
- `security-check.js` passes with new assertions.

**G4 NOT TESTED list:**
- Push endpoint (443-04) wiring — not yet connected.
- Admin frontend calls (443-05).
- Real device ACK roundtrip (443-09).

**Commit message:**
```
feat(443-03): server-side Ed25519 signer + /api/v1/mobile/selectors/sign

MobileSelectorSigner: canonicalize (strip BOM, CRLF→LF, trim trailing WS)
then sign with ed25519-dalek. Signature byte-for-byte matches agent-side
PatchSignatureVerifier from Phase 433. Sign endpoint requires staff JWT
+ admin role; validate endpoint requires staff JWT. security-check.js
amended with route-auth assertions.

Covers: SELECTOR-04 (server-side signing)
Not tested: push transport (443-04), admin UI (443-05), device ACK (443-09).
```

---

### 443-04-PLAN — Server-side push endpoint + status/rollback routes + DB migration

**Goal:** Server receives a signed payload (from 443-03 output), wraps in `selector_push` envelope, ships via comms-link to target devices, records per-device status, listens for `selector_push_ack`, exposes GET `/status` + POST `/rollback`.

**Covers:** ADMIN-05 (transport + status + rollback), SELECTOR-04 (server-authoritative rollback)

**Dependencies:** 443-03 (sign endpoint), Phase 433 (agent-side handler)

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: `POST /push` with a signed payload + target device list → 202 with `push_id`. DB row in `mobile_selector_pushes` + per-target rows in `mobile_selector_push_targets`.
- Test 2: Duplicate patch_version (violates UNIQUE constraint) → 409 `{error: "stale_patch_version", current: N}`.
- Test 3: `GET /pushes/:id/status` returns per-target status array.
- Test 4: Incoming `selector_push_ack` with `accepted: true` → target status → `applied`, `applied_at` set.
- Test 5: Incoming `selector_push_ack` with `accepted: false, reason: "signature_invalid"` → target status → `rejected_signature`, `error_reason` set.
- Test 6: No ACK within 30s of `sent` → target status → `timeout_30s` (background task sweeps).
- Test 7: `POST /devices/:id/rollback` finds previous patch_version, re-signs old YAML as N+1, ships, returns new `push_id`.
- Test 8: `POST /devices/:id/rollback` when no previous exists → 404 `{error: "no_previous_map"}`.
- Test 9: `POST /push` without staff JWT → 401; with non-admin JWT → 403.
- Test 10: `POST /push` with device_id that is not in known devices → 400 `{error: "unknown_device"}`.

**Tasks:**

1. Create migration `crates/racecontrol/src/db/migrations/NNN_mobile_selector_pushes.sql` — see §D above for full schema. Also add `DELETE FROM mobile_selector_push_targets WHERE ...` line to `customer_data_delete()` ONLY IF the table ends up keyed by driver_id — it is NOT (key is device_id). Per CLAUDE.md GDPR erase contract: no FK to `drivers(id)` → no cascade entry needed. Verify during execution.

2. Create `crates/racecontrol/src/db/mobile_selector_pushes.rs` — DAO with functions per the Required Artifacts list.

3. Extend `crates/racecontrol/src/api/mobile_selectors.rs` with:
   - `push_handler(State, StaffAuth-admin, Json<PushRequest>) -> Result<Json<PushAccepted>>` — writes DB rows, enqueues per-device envelopes to comms-link.
   - `push_status_handler(State, StaffAuth, Path<push_id>) -> Json<PushStatus>`.
   - `rollback_handler(State, StaffAuth-admin, Path<device_id>) -> Result<Json<PushAccepted>>` — look up previous map in DB, re-sign, ship as N+1.
   - `device_state_handler(State, StaffAuth, Path<device_id>) -> Json<DeviceSelectorState>` — current patch_version, previous patch_version, current map summary. Used by 443-05 target picker + 443-07 rollback page.
   - `history_handler(State, StaffAuth, Query<limit>) -> Json<Vec<HistoryRow>>` — audit trail.

4. comms-link integration:
   - Server-side racecontrol talks to comms-link relay via existing HTTP POST `/relay/message` (or equivalent — grep `comms-link-relay` or existing pattern in racecontrol). New envelope type `selector_push` is PASSTHROUGH per Phase 433 comms-link integration note.
   - Incoming `selector_push_ack` handling: amend the existing comms-link WS inbound handler in racecontrol to route `type: "selector_push_ack"` to `mobile_selector_pushes::on_ack(envelope)`.

5. Timeout sweeper: tokio task spawned at AppState init, every 10s scans for targets in state `sent` with `sent_at < now - 30s` → mark `timeout_30s`.

6. Register routes:
   ```
   .route("/api/v1/mobile/selectors/push",                    post(push_handler))            # admin only
   .route("/api/v1/mobile/selectors/pushes/:id/status",       get(push_status_handler))      # staff
   .route("/api/v1/mobile/selectors/devices/:id/rollback",    post(rollback_handler))        # admin only
   .route("/api/v1/mobile/selectors/devices/:id/state",       get(device_state_handler))     # staff
   .route("/api/v1/mobile/selectors/history",                 get(history_handler))          # staff
   ```

7. Amend `comms-link/test/security-check.js` to assert route-auth + admin-role on push and rollback; staff-JWT-only on status/state/history.

8. Update `docs/ARCHITECTURE.md` with a new subsection under §20 (or appropriate): "Mobile Selector Push Pipeline" — diagram + sequence: admin UI → sign endpoint → push endpoint → relay → agent → ACK → DB → UI.

**Acceptance:**
- All 10 unit tests pass.
- `sqlite3 racecontrol.db ".schema mobile_selector_pushes"` shows tables + UNIQUE + indexes.
- `curl -X POST /api/v1/mobile/selectors/push -H "Authorization: Bearer <admin>" -d @signed.json` → 202 with `push_id`.
- Manual E2E with a stub agent (not a real device) that ACKs via direct WS: push → ACK applied → status endpoint reflects `applied`.
- security-check.js passes with all new assertions.

**G4 NOT TESTED list:**
- Real device ACK (443-09).
- Admin UI (443-05).
- Rollback E2E on Tab Plus (443-09).

**Commit message:**
```
feat(443-04): mobile selector push endpoint + status + rollback + audit tables

mobile_selector_pushes + mobile_selector_push_targets tables with
UNIQUE(app_package, app_version, patch_version) for monotonicity.
push_handler writes DB + enqueues selector_push to comms-link.
ACK handler updates per-target status. Timeout sweeper every 10s.
rollback_handler re-signs previous YAML as N+1 (server-authoritative).
All write endpoints require staff-admin JWT; reads require staff JWT.
security-check.js amended. ARCHITECTURE.md updated with pipeline diagram.

Covers: ADMIN-05 (transport+rollback), SELECTOR-04 (server-auth rollback)
Not tested: real device E2E (443-09), admin UI (443-05).
```

---

### 443-05-PLAN — `/mobile/selectors` page: upload + validate + preview + target picker + confirm

**Goal:** Admin dashboard page where staff uploads/pastes YAML, sees schema-validated preview, picks target devices, confirms with typed modal on multi-device, submits signed push.

**Covers:** ADMIN-05 (UI — upload/target/push happy path)

**Dependencies:** 443-01 (UI-SPEC), 443-03 (sign+validate endpoints), 443-04 (push endpoint)

**Type:** `auto` + `tdd="true"` (component unit tests via React Testing Library / Vitest)

**tdd behavior:**
- Test 1: Rendering the page without YAML shows the SecurityWarningBanner + upload form (file input + textarea).
- Test 2: Uploading a valid YAML fires `validateYaml` → shows SchemaPreview with `app_package`, `app_version`, `screens_count`, `elements_count`.
- Test 3: Uploading invalid YAML shows per-error table (line, col, message) and DISABLES the Push button.
- Test 4: Target picker renders devices fetched from `GET /devices` (or equivalent fleet endpoint) with last-seen + current patch_version per device.
- Test 5: Selecting 2+ devices shows typed-confirmation input. Button remains disabled until user types exactly `PUSH`.
- Test 6: Clicking Push fires sign → then push → navigates to `/mobile/selectors/<push_id>`.
- Test 7: A network failure during sign shows an inline error with retry action; push state unchanged.
- Test 8: A 409 stale_patch_version on push shows an inline error with the current version + suggestion to bump.
- Test 9: Mobile viewport (375px) — upload form + preview + target picker stack vertically, scrollable.
- Test 10: Keyboard-only navigation — tab order covers upload → preview → target checkboxes → typed-confirmation → Push button.

**Tasks:**

1. Read `UI-SPEC.md` (from 443-01). Any deviation = commit message note + update to UI-SPEC.

2. Create `racingpoint-admin/src/lib/mobile-selectors-api.ts` — typed client functions: `validateYaml`, `signYaml`, `pushSelectors`, `getDeviceState`, `getHistory`.

3. Create components:
   - `YamlUploader.tsx` — file input (accept .yaml, .yml) + paste textarea; fires `validateYaml` on blur / file select.
   - `SchemaPreview.tsx` — renders validation result; on success, renders a diff preview against the currently-deployed map per target device (fetched from `/devices/:id/state`).
   - `TargetPicker.tsx` — checkbox list of devices; fetches `/devices` list (or uses fleet health); shows last-seen + current patch_version.
   - `SecurityWarningBanner.tsx` — fixed banner at top of the page with copy from UI-SPEC §Security.

4. Create `racingpoint-admin/src/app/(dashboard)/mobile/selectors/page.tsx`:
   - Server component: calls `requireAdmin()` — redirects non-admin staff to `/403`.
   - Client island: composes YamlUploader → SchemaPreview → TargetPicker → Confirm button.
   - State machine (React): `idle → validating → valid → selecting_targets → confirming → pushing → redirect`.
   - On push success, `router.push(\`/mobile/selectors/${push_id}\`)`.

5. Handle the "first push" case (no previous map) in SchemaPreview: "FIRST PUSH to this device — no diff available."

6. Error handling exhaustive per UI-SPEC — all 11 error states have explicit copy.

7. Component tests per tdd.behavior using Vitest + Testing Library.

**Acceptance:**
- All 10 component tests pass.
- Manual: `/mobile/selectors` loads in staff browser; upload a valid YAML; see preview; select Tab Plus; type PUSH (since N=1 typed confirmation NOT required per UI-SPEC — only N>=2); click Push; redirected to `/mobile/selectors/<id>`.
- Mobile viewport 375px — page usable without horizontal scroll.
- Keyboard-only traversal completes end-to-end.
- DevTools Network tab — NO `signing_key`, `MOBILE_SELECTOR_SIGNING_KEY`, or raw PEM in any request/response payload (per T-10).

**G4 NOT TESTED list:**
- Per-push status view (443-06).
- Rollback UI (443-07).
- Real E2E on Tab Plus (443-09).

**Commit message:**
```
feat(443-05): /mobile/selectors upload+validate+preview+target+confirm

React page on admin dashboard. YamlUploader → validate via API → SchemaPreview
with per-target diff → TargetPicker with online status + current patch_version →
typed-confirmation modal when targeting 2+ devices → Push button triggers
sign+push sequence → navigate to status page. SecurityWarningBanner per UI-SPEC.
Mobile responsive. Keyboard-nav complete. Private key never touches browser.

Covers: ADMIN-05 (upload/target/push UI)
Not tested: status polling (443-06), rollback (443-07), device E2E (443-09).
```

---

### 443-06-PLAN — Per-device push-status view (live)

**Goal:** `/mobile/selectors/[pushId]` page showing real-time per-device status of an in-flight push. Polls status endpoint until terminal state; shows actor, timestamps, errors clearly.

**Covers:** ADMIN-05 (status view)

**Dependencies:** 443-04 (status endpoint), 443-05 (navigation redirect target)

**Type:** `auto` + `tdd="true"`

**tdd behavior:**
- Test 1: Mount with `pushId` → initial fetch, renders PushHeader (push_id, actor, yaml_sha256, patch_version, created_at IST) + PushStatusTable.
- Test 2: Each target row shows status pill (queued=grey, sent=blue, ack_received=blue-dark, applied=green, rejected_*/timeout_30s=red), acked_at/applied_at/error_reason as applicable.
- Test 3: Poll every 1s while any target is in non-terminal state; stop polling when all terminal.
- Test 4: Retry button appears per failed target (calls `POST /push/retry?push_id=...&device_id=...` — thin wrapper over push using same signed payload).
- Test 5: Click "Back to /mobile/selectors" navigates back.
- Test 6: A failed target with `error_reason: signature_invalid` shows a direct link to SIGNING-KEY-OPS.md §Compromise Response.
- Test 7: A failed target with `error_reason: parse_error` shows `error_detail` (line/col) and suggests re-uploading.

**Tasks:**

1. Create `useMobileSelectorPush.ts` hook — SWR or simple polling; exposes `{push, targets, isTerminal}`. Auto-stops polling when all targets terminal.

2. Create `PushStatusTable.tsx` — table component; maps status → pill color + icon.

3. Create `racingpoint-admin/src/app/(dashboard)/mobile/selectors/[pushId]/page.tsx`:
   - Server-side: fetch initial state via `/api/v1/mobile/selectors/pushes/:id/status`.
   - Client island: live polling via hook.
   - Header with push metadata.
   - Action buttons: "Retry failed targets" (enabled if any in terminal-failure state), "Back".

4. Retry endpoint: amend `mobile_selectors.rs` with `POST /api/v1/mobile/selectors/pushes/:id/retry?device_id=...` that re-enqueues the envelope (does NOT re-sign; uses stored yaml_canonical + signature_b64).

5. Component tests per tdd.behavior with mocked API.

**Acceptance:**
- All 7 component tests pass.
- Manual: complete a push from 443-05 → land on `/mobile/selectors/<id>` → see live status updates every 1s.
- Polling stops once all targets terminal.
- Retry button works on a simulated failure (use stub target via dev mode or tolerant manual test).

**G4 NOT TESTED list:**
- Rollback UI (443-07).
- Real device failure recovery (443-09).

**Commit message:**
```
feat(443-06): per-push live status view with 1s polling + retry

/mobile/selectors/[pushId] page. PushStatusTable maps status enum to
colored pills. useMobileSelectorPush hook polls every 1s until all
targets terminal. Retry button re-enqueues stored envelope without
re-signing. signature_invalid errors link to SIGNING-KEY-OPS §Compromise.

Covers: ADMIN-05 (live status UX)
Not tested: rollback (443-07), device failure E2E (443-09).
```

---

### 443-07-PLAN — Rollback UI: per-device rollback button + confirmation

**Goal:** `/mobile/selectors/devices/[deviceId]` page showing current + previous map with a "Rollback" button that calls the rollback endpoint. Confirmation modal lists previous map metadata explicitly.

**Covers:** ADMIN-05 (rollback UX), SELECTOR-04 (rollback delivery — already done agent-side in Phase 433, server-auth rollback done in 443-04)

**Dependencies:** 443-04 (rollback endpoint), 443-06 (status view — rollback creates a new push)

**Type:** `auto` + `checkpoint:human-verify` at end (a visual verification against UI-SPEC rollback flow)

**tdd behavior:**
- Test 1: Page renders current map summary (patch_version, app_version, yaml_sha256, generated_at) + previous map summary.
- Test 2: "Rollback to previous" button disabled if no previous exists; shows "No previous map to roll back to." message.
- Test 3: Clicking Rollback opens confirmation modal with explicit copy: "Rolling back {device_id} from patch_version {N} to patch_version {N-1}. Generated {iso}. Confirm?" + typed `ROLLBACK` input.
- Test 4: Confirm fires `POST /devices/:id/rollback` → new push_id returned → navigate to `/mobile/selectors/<new_push_id>`.
- Test 5: Rollback failure (server 500 / network) shows inline error + keeps modal open for retry.
- Test 6: Successful rollback shows a toast "Rollback enqueued — view status".

**Tasks:**

1. Create `RollbackButton.tsx` — button + confirmation modal + typed-confirmation input.

2. Create `racingpoint-admin/src/app/(dashboard)/mobile/selectors/devices/[deviceId]/page.tsx`:
   - Server-side: `GET /devices/:id/state` for current + previous map.
   - Client island: RollbackButton + current/previous summary cards.

3. Ensure modal copy matches UI-SPEC (exact strings).

4. Component tests per tdd.behavior.

**Acceptance:**
- All 6 component tests pass.
- Manual: `/mobile/selectors/devices/rcm-tab-plus` shows current + previous maps.
- Click Rollback → modal appears with explicit copy.
- Typing `ROLLBACK` enables Confirm button.
- Confirm → navigates to a new `/mobile/selectors/<new_push_id>` page.

**Checkpoint (human-verify):** James verifies the rollback flow visually against UI-SPEC — modal copy, disabled states, error states. **Resume signal:** "Rollback UI matches UI-SPEC." or describe deviations.

**G4 NOT TESTED list:**
- Real device rollback (443-09).
- Rollback when agent is offline (error-path UX is tested in Test 5 via mock; real device offline drill in 443-09).

**Commit message:**
```
feat(443-07): per-device rollback UI with typed confirmation

/mobile/selectors/devices/[deviceId] page. RollbackButton opens modal
with previous-map metadata + typed ROLLBACK input. Confirm triggers
POST /devices/:id/rollback → navigate to new push status page.
Server-authoritative rollback — no new agent code.

Covers: ADMIN-05 (rollback UX), SELECTOR-04 (UI-side rollback trigger)
Not tested: real device E2E (443-09).
```

---

### 443-08-PLAN — UI-REVIEW via gsd-ui-auditor + security-check.js post-gate

**Goal:** Validate that 443-05/06/07 implementation matches 443-01 UI-SPEC; re-run security-check.js to confirm no regressions.

**Covers:** Frontend gate (CLAUDE.md > Subagent Gates — "Any frontend" REQUIRES UI-REVIEW.md before ship) + security gate post-check.

**Dependencies:** 443-05, 443-06, 443-07

**Type:** `auto` + `checkpoint:human-verify` at end

**Tasks:**

1. Invoke `gsd-ui-auditor` subagent with these inputs:
   - UI-SPEC.md (from 443-01).
   - Running admin dev server with the 3 new pages.
   - Screenshots at viewports 360, 768, 1280.
   - List of 11 states to verify.

2. Auditor produces `.planning/phases/443-selector-map-remote-push-ui/UI-REVIEW.md` with:
   - Per-state findings (matches UI-SPEC / deviates / missing).
   - Accessibility audit (contrast, focus visible, screen-reader labels).
   - Mobile responsive audit.
   - Security messaging review (is SecurityWarningBanner prominent enough? Does typed-confirmation copy match?).
   - Severity-graded fixes.

3. For each P0/P1 finding in UI-REVIEW.md, open a targeted fix commit within 443-08 (not a new plan). P2/P3 findings can defer to 443-09 if blocking, else to a backlog item.

4. Re-run `node comms-link/test/security-check.js`:
   - Pre-443: record pass count.
   - Post-443: assert pass count has INCREASED (new assertions from 443-03 + 443-04).
   - All pre-existing assertions still pass.
   - New assertions present:
     - `/api/v1/mobile/selectors/validate` requires staff JWT.
     - `/api/v1/mobile/selectors/sign` requires admin role.
     - `/api/v1/mobile/selectors/push` requires admin role.
     - `/api/v1/mobile/selectors/devices/:id/rollback` requires admin role.
     - `/api/v1/mobile/selectors/pushes/:id/status` requires staff JWT.
     - No PEM strings in git (pre-commit hook assertion).
     - No `MOBILE_SELECTOR_SIGNING_KEY` literal anywhere in `racingpoint-admin/` (frontend never sees key).

5. Run full admin-frontend a11y scan (e.g. axe-core CLI on built output) — record findings in UI-REVIEW.md.

**Acceptance:**
- `UI-REVIEW.md` exists, covers all 11 states, severity-graded.
- All P0/P1 findings fixed in this plan.
- `security-check.js` pre vs post assertion count: delta ≥ +7.
- Zero PEM strings in `racingpoint-admin/` (grep `-----BEGIN` → zero matches).

**Checkpoint (human-verify):** James + Uday review UI-REVIEW.md. **Resume signal:** "UI-REVIEW approved, no P0/P1 remain." or list remaining issues.

**G4 NOT TESTED list:**
- Real device E2E (443-09).
- Post-deploy frontend verification (done in 443-09).

**Commit message:**
```
test(443-08): UI-REVIEW via gsd-ui-auditor + security-check.js post-gate

UI-REVIEW.md covers all 11 states per UI-SPEC. A11y audit via axe-core.
Mobile responsive + keyboard-nav verified. P0/P1 findings fixed inline.
security-check.js: +7 new assertions (route-auth + key-absence).
No PEM or signing-key strings in racingpoint-admin/ bundle.

Gate: frontend phase post-ship per CLAUDE.md Subagent Gates.
Covers: ADMIN-05 (UI quality + security)
```

---

### 443-09-PLAN — E2E drill on Tab Plus: upload → push → apply → rollback → verify

**Goal:** On a live Tab Plus, exercise the entire flow: upload a YAML in admin, push to Tab Plus, confirm the new selector is active in rc-agent-mobile, roll back, confirm previous selector is active. Stopwatch + artifacts in SUMMARY.md.

**Covers:** Phase 443 ship gate (SELECTOR-04, ADMIN-05)

**Dependencies:** 443-03, 443-04, 443-05, 443-06, 443-07, 443-08, Phase 433 (agent live on Tab Plus)

**Type:** `checkpoint:human-verify` (physical Tab Plus)

**Preconditions:**
- Tab Plus running latest rc-agent-mobile APK from Phase 433 with the production public key baked in.
- racecontrol server on .23 and Bono VPS running the 443-built binary.
- Admin dashboard (racingpoint-admin) on server .23 + Bono VPS running the 443-built frontend.
- Production Ed25519 private key in place per SIGNING-KEY-OPS.md (443-02).
- Tab Plus `filesDir/selectors/zomato-partner/v3.14.2/selectors.yaml` seeded from Phase 433.

**Drill script:**

1. **SC-1: Upload + push (< 2 min end-to-end).**
   - Open `/mobile/selectors` in admin (on phone — James 2am scenario).
   - Paste a modified YAML (e.g. bump timeout_ms of one attempt).
   - Observe SchemaPreview → OK.
   - Select Tab Plus only.
   - Click Push.
   - Start stopwatch when Push is clicked.
   - Verify navigation to `/mobile/selectors/<push_id>`.
   - Poll status — watch `queued → sent → ack_received → applied`.
   - Stop stopwatch when status reflects `applied`.
   - **Target: < 30s from Push click to applied (well within 2min total including upload).**

2. **SC-2: Agent-side selector effect.**
   - On Tab Plus via adb: `adb shell cat filesDir/selectors/zomato-partner/v3.14.2/selectors.yaml` — verify the byte-for-byte content matches the YAML uploaded in admin.
   - Hit `GET http://<tab_plus_ip>:8090/health` — verify `selector_catalog_last_reloaded_at_ms` is within 10s of the push ack.
   - Trigger a driver run (stub Zomato run or `/debug/match_test` endpoint from Phase 433) with the new YAML's selector — verify it matches.

3. **SC-3: Signature enforcement.**
   - From CLI, construct a fake unsigned `selector_push` envelope and send via comms-link relay directly (bypassing server endpoint). Verify Tab Plus rejects with `register_rejected_signature`.
   - Verify the admin status view for the corresponding push_id (if any) shows `rejected_signature`.

4. **SC-4: Rollback ≤ 10s.**
   - Navigate to `/mobile/selectors/devices/rcm-tab-plus`.
   - Click Rollback → type `ROLLBACK` → Confirm. Start stopwatch.
   - Watch status page for new push_id (the rollback re-push) → `applied`.
   - Stop stopwatch. **Target: < 10s.**
   - On Tab Plus: verify `selectors.yaml` content matches the PRE-SC-1 content.

5. **SC-5: Audit trail integrity.**
   - `GET /api/v1/mobile/selectors/history?limit=10` — verify the SC-1 push AND the SC-4 rollback push both appear, with correct `actor_staff_id`, `yaml_sha256`, `is_rollback_of` on the rollback row.

6. **SC-6: Post-deploy frontend verification.**
   - From a machine that is NOT the server (James's phone or Bono's browser pointing at cloud admin):
   - `/mobile/selectors` renders correctly (per CLAUDE.md "verify from the user's browser" rule).
   - `GET <admin>/mobile/selectors` on cloud Bono VPS also renders correctly (DEPLOY PARITY).

**Artifacts in SUMMARY.md:**
- Stopwatch values for SC-1 and SC-4.
- Status page screenshots at each state.
- DB row dumps (`SELECT * FROM mobile_selector_pushes ORDER BY created_at DESC LIMIT 5`).
- adb shell output of post-SC-1 and post-SC-4 `selectors.yaml` on Tab Plus.
- axe-core a11y scan result.
- security-check.js pre/post diff.

**Checkpoint (human-verify):** James runs SC-1..SC-6 on physical Tab Plus, records artifacts. **Resume signal:** "All SC pass with artifacts." or list failures.

**If any SC fails:** create a gap-closure plan per CLAUDE.md backlog gate — do NOT mark Phase 443 shipped.

**Post-drill:**
- Update `.planning/ROADMAP.md` Phase 443 plan checklist to `[x]`.
- Memory update: append Phase 443 shipped entry with commit hash to `~/.claude/projects/C--Users-bono/memory/gsd-projects.md`.
- Update `docs/ARCHITECTURE.md` §20.3 Shipped Milestones (if v50.0 is ready to increment).
- LOGBOOK.md entry per CLAUDE.md LOGBOOK rule.

**Commit message:**
```
test(443-09): E2E drill — mobile selector upload → push → apply → rollback

Verified on Tab Plus (rcm-tab-plus):
- SC-1 Push-to-applied: <X>s (target <30s), total upload-to-applied <Y>s (target <120s)
- SC-2 Agent-side: YAML on disk matches; /health reload fresh; matcher hits new selector
- SC-3 Signature enforcement: unsigned envelope rejected with register_rejected_signature
- SC-4 Rollback: <Z>s (target <10s); on-device YAML reverts to pre-SC-1
- SC-5 Audit: history endpoint shows push + rollback with is_rollback_of linkage
- SC-6 Deploy parity: server .23 + Bono VPS both render /mobile/selectors correctly

Artifacts: .planning/phases/443-selector-map-remote-push-ui/SUMMARY.md.
ROADMAP.md + memory + LOGBOOK updated.

Covers: ADMIN-05, SELECTOR-04 (E2E ship gate)
```

---

## 6. Risks and pitfalls (phase-specific)

| # | Risk | Mitigation | Owner |
|---|------|------------|-------|
| R-1 | **Private signing key leak** — Git commit, env var leakage via logs, file ACL misconfigured, 1Password token compromise | (a) `.gitignore` hardening in 443-02 + pre-commit hook assertion; (b) env var NEVER logged (audited in 443-02 start-racecontrol.bat); (c) NTFS ACL + chmod documented + verification commands; (d) SIGNING-KEY-OPS.md Compromise-Response §7 includes rotation playbook and APK rebuild coordination; (e) security-check.js asserts zero PEM strings in racingpoint-admin/ bundle (443-08) | James + Uday |
| R-2 | **UI mistake ships fleet-wide bad selectors** | (a) Per-device target picker (default: NONE selected); (b) typed-confirmation modal when N>=2 devices; (c) preview diff vs currently-deployed map per device; (d) rollback button visible on every device state page | 443-05, 443-07 |
| R-3 | **Canonicalization drift** between server signer and Phase 433 agent verifier | (a) Both use same rule: strip BOM, CRLF→LF, trim trailing WS; (b) cross-reference in docs/SELECTORS.md §Signing (authored in 433; amended in 443-03); (c) 443-03 Test 9 verifies server signature against agent-side PatchSignatureVerifier using the real pubkey | 443-03 |
| R-4 | **Rollback fails when agent .backup missing after reboot** | Server-authoritative rollback: re-sign previous YAML as new patch_version (443-04). No dependency on agent-side .backup. Applies the existing push path. | 443-04, 443-07 |
| R-5 | **Replay attack** — old signed YAML re-submitted as "new" to downgrade | DB UNIQUE(app_package, app_version, patch_version) rejects duplicate patch_version at server + agent-side supersedes_patch_version check rejects at device (Phase 433) | 443-04 |
| R-6 | **Admin UI served from browser contains key material** (critical — must not) | (a) Server signs on server; admin frontend receives only signed envelope payload; (b) 443-08 security-check assertion: zero PEM in admin bundle; (c) manual DevTools Network tab check in 443-05 acceptance | 443-03, 443-05, 443-08 |
| R-7 | **comms-link relay offline at push time** | Server queues push envelopes; timeout sweeper marks `timeout_30s` after 30s; UI clearly distinguishes queued/sent/applied/timeout; Retry button available. | 443-04, 443-06 |
| R-8 | **Staff with non-admin JWT can push** (would bypass audit intent) | All write endpoints require admin role enforced in handler + security-check.js assertion; ADMIN-05 requires admin role. | 443-03, 443-04, 443-08 |
| R-9 | **HyperPure 2am scenario — desktop-only UI fails on phone** | UI-SPEC 443-01 REQUIRES mobile viewport ≥360px + a11y; 443-05 Test 9 covers 375px; 443-08 UI-REVIEW verifies. | 443-01, 443-05, 443-08 |
| R-10 | **Signature verification failure shown as opaque "Failed"** | PushStatusTable maps `rejected_signature` → "Signature rejected — see SIGNING-KEY-OPS.md §Compromise"; 443-06 Test 6 enforces. | 443-06 |
| R-11 | **Key rotation requires APK rebuild — staff pushes during overlap window** | SIGNING-KEY-OPS.md §5 documents: (1) ship new APK with TRUSTED_SIGNING_KEY_IDS=[old, new], (2) roll out APK to both devices, (3) switch server key to new, (4) verify push works, (5) next APK removes old from TRUSTED list. 443-02 Rotation §5 is the runbook. | 443-02 |
| R-12 | **Deploy parity drift** — server .23 and Bono VPS out of sync → admin cloud shows stale UI while venue works (or vice versa) | CLAUDE.md DEPLOY PARITY rule enforced in DMP targets list; 443-09 SC-6 explicitly verifies Bono VPS parity. | 443-09 |
| R-13 | **Server handler panics on malformed comms-link ACK** | Handler wraps ACK processing in `?` / Result — no `.unwrap()`. tower error handler returns 500 with correlation id. Per CLAUDE.md "No .unwrap() in production Rust". | 443-04 |
| R-14 | **DB grows unbounded** — `yaml_canonical BLOB` stored per push | In 443-04, add index + note in ARCHITECTURE.md that a retention sweep (>6 months) is a backlog item. For v50.0 cadence (~1 push/week) at ~5KB per YAML, 250KB/year — acceptable. | 443-04 |
| R-15 | **Phase 433 agent-side bug masquerades as Phase 443 issue** during 443-09 | 443-09 SC-3 specifically isolates: craft an unsigned envelope bypassing the server to confirm agent-side signature enforcement is live. If SC-3 fails, the gap is in Phase 433 — file a 433 gap-closure plan, not 443. | 443-09 |

## 7. Test plan summary

### Unit tests (JVM-fast where possible)

- `mobile_selector_signer` Rust unit tests — 9 tests (443-03)
- `mobile_selectors_api` Rust integration tests (handlers + DB) — 10 tests (443-04)
- `mobile-selectors-api.ts` — happy-path + error-path client tests (443-05)
- React component tests (Vitest) — YamlUploader, SchemaPreview, TargetPicker, PushStatusTable, RollbackButton (443-05/06/07)

### Integration tests

- In-memory racecontrol + stub comms-link relay + stub agent — full push → ACK → DB update (443-04).
- Admin frontend against running racecontrol — validate + push flow (443-05).

### Real-device E2E (443-09)

- Tab Plus physical — SC-1..SC-6 drill.
- Bono VPS deploy parity — SC-6.

### Security tests

- `security-check.js` pre-gate before 443-03 ships — baseline pass count.
- `security-check.js` post-gate after 443-08 — assertion count +7.
- Grep admin bundle for PEM strings — zero matches.
- Manual: DevTools Network tab during push flow — no key material in any payload.

### A11y tests

- axe-core CLI scan on built admin output — 0 P0/P1 violations.
- Keyboard-only traversal per 443-05 Test 10, 443-06, 443-07.

## 8. Open questions

| # | Question | Owner | Blocking | Resolution path |
|---|----------|-------|----------|-----------------|
| **OQ-1 (MOST IMPORTANT)** | **Where do we store the Ed25519 private signing key? (env var / file / 1Password / HSM)** | Uday | Yes — blocks 443-03 | 443-02 `checkpoint:decision` with 4 candidates + James's recommendation (Option B). Decision recorded in SIGNING-KEY-OPS.md §1. |
| OQ-2 | Typed-confirmation threshold — require typed `PUSH` for N=1 device too, or only N>=2? | Uday | No — can default | Default: only N>=2 requires typed confirmation. Rationale: single-device pushes are the common case; friction there deters James at 2am. Revisit if drill reveals risk. |
| OQ-3 | Per-push retry on transient relay failure — automatic retry with backoff, or manual-only via UI button? | James (discretion) | No | Default: manual-only (443-06 Retry button). Automatic retry masks underlying relay issues that should be surfaced. Revisit post-v50. |
| OQ-4 | Do we want a "dry-run" mode — push to a device in validate-only mode (sign + ship but agent does NOT apply)? | Uday | No | Default: NO dry-run in v50.0. Preview diff in admin is the dry-run. Agent-side dry-run would require a Phase 433 amendment. Backlog item if demand emerges. |
| OQ-5 | Signed audit trail — should each push audit row itself be signed (tamper-evident log)? | Uday | No | Default: NO for v50.0. DB rows have `yaml_sha256` + `signature_b64` + `actor_staff_id` which is sufficient for forensic. Tamper-evident log is a post-v50 hardening. |

## 9. Deploy Manifest check (DMP per CLAUDE.md)

| Layer | Action | Owner | Verify |
|-------|--------|-------|--------|
| rust_binary | Rebuild `racecontrol` | 443-03, 443-04 | `cargo build --release --bin racecontrol`; deploy via deploy-server.sh |
| frontend_rebuild | Rebuild `racingpoint-admin` | 443-05, 443-06, 443-07 | `npm run build`; standalone deploy with `.next/static` copied; curl `/_next/static/...` returns 200 |
| config_change | `[mobile.selectors]` section in racecontrol.toml | 443-02 | `cat C:\RacingPoint\racecontrol.toml | grep -A3 'mobile.selectors'` on both server .23 + Bono VPS |
| db_migration | `mobile_selector_pushes` + `mobile_selector_push_targets` | 443-04 | `sqlite3 racecontrol.db ".schema mobile_selector_pushes"` returns table |
| infrastructure | Private key file on both hosts (or env var) | 443-02 | NTFS ACL / chmod verify per SIGNING-KEY-OPS.md §8; `curl ...` sign endpoint returns 200 (not `signing_unavailable`) |
| data_files | None | — | — |
| bat_file | `start-racecontrol.bat` amended (optional env source) | 443-02 | File-diff vs repo; deployed via SCP; verified on both hosts |
| cloud_parity | Binary + frontend + config on Bono VPS | 443-09 SC-6 | `curl https://racingpoint.cloud/admin/mobile/selectors` renders |
| targets | server, cloud, tab_plus, m07 (verified agent-side), james (docs) | 443-09 | Per-target acceptance in 443-09 |

**Rollback plan:**
- Binary: `racecontrol-prev.exe` preserved 72h on both hosts.
- Frontend: Next.js standalone swap — previous build retained.
- Config: revert `[mobile.selectors]` (handler falls back to disabled → `signing_unavailable`; UI shows "Signing unavailable — contact James" per 443-05 error state).
- DB: migration is additive; no rollback SQL needed for v50.0. If needed, drop tables manually (ops runbook entry in SIGNING-KEY-OPS.md §10).
- Signing key: rotate per SIGNING-KEY-OPS.md §5 if compromise suspected.

## 10. Extension points (post-v50.0)

- **Batch push** — upload multiple selector files at once (HyperPure + Blinkit on same device in one operation).
- **Scheduled push** — schedule a push for next maintenance window.
- **Dry-run** — agent-side validate-only mode (requires Phase 433 amendment).
- **Tamper-evident audit log** — chain-signed audit rows.
- **KMS integration** — migrate from file/env to cloud KMS if cadence grows.
- **Preview sandbox** — agent-side "preview screen" that applies a patch to a sandboxed copy and screenshots the result for admin review before real apply.

Each extension point has a clean hook in v50.0's code — the push/rollback endpoints are side-effect-free boundaries; adding new flows reuses them.

## 11. Close-out checklist (before marking Phase 443 shipped)

- [ ] UI-SPEC.md exists and was consulted by 443-05/06/07 executors
- [ ] SIGNING-KEY-OPS.md exists with Uday's locked decision on key storage
- [ ] Private key on disk with correct ACL on both server .23 + Bono VPS (or env var set)
- [ ] `.gitignore` rejects `*-ed25519.pem`; pre-commit hook active
- [ ] `cargo test -p racecontrol-crate` all pass
- [ ] Admin frontend unit tests (Vitest) all pass
- [ ] `security-check.js` passes with +7 new assertions
- [ ] UI-REVIEW.md exists; zero P0/P1 findings remain
- [ ] axe-core a11y scan: 0 P0/P1 violations
- [ ] MMA audit on signing-key handling completed with 3+ vendor diversity; P0/P1 findings closed
- [ ] Integration-checker run across 433+443 (cross-phase) — passed
- [ ] nyquist-auditor run on signing + push state machine — passed
- [ ] DB migration applied on server .23 + Bono VPS
- [ ] racecontrol binary deployed on server .23 + Bono VPS (build_id match)
- [ ] racingpoint-admin rebuilt + deployed on server .23 + Bono VPS
- [ ] Tab Plus drill (443-09 SC-1..SC-6) all pass with artifacts in SUMMARY.md
- [ ] Bono VPS `/mobile/selectors` renders correctly (deploy parity)
- [ ] ROADMAP.md Phase 443 plan checklist all `[x]`
- [ ] Memory update (gsd-projects.md) with shipped entry
- [ ] LOGBOOK.md entry appended
- [ ] Deploy parity verified (CLAUDE.md)
- [ ] Git push + Bono comms-link notification + INBOX.md entry (CLAUDE.md atomic sequence)
- [ ] `.planning/phases/443-selector-map-remote-push-ui/SUMMARY.md` complete
