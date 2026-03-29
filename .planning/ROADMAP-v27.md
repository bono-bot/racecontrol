# Roadmap: v27.0 Workflow Integrity & Compliance Hardening

**Milestone defined:** 2026-03-29
**Phase range:** 251-260
**Granularity:** Standard (10 phases, justified by 83 requirements across 10 categories)
**Core value:** Every customer interaction — from registration to refund — is atomic, auditable, safe, and legally compliant

---

## Phases

- [x] **Phase 251: Database Foundation** — SQLite WAL mode, staggered timer writes, orphaned session detection
- [x] **Phase 252: Financial Atomicity Core** — Atomic billing start, wallet row locking, idempotency keys, CAS finalization, reconciliation
- [x] **Phase 253: State Machine Hardening** — Server-side FSM transition table, cross-FSM invariants, crash recovery atomicity, split session modeling
- [x] **Phase 254: Security Hardening** — INI injection whitelist, FFB cap, PIN CAS, RBAC, audit log, OTP hashing, WSS, agent mutex
- [x] **Phase 255: Legal Compliance** — GST separation, invoice generation, waiver gate, minor consent flow, data retention
- [x] **Phase 256: Game-Specific Hardening** — Steam checks, process name corrections, Forza enforcer, AC EVO adapter, iRacing check, DLC check
- [ ] **Phase 257: Billing Edge Cases** — Inactivity detection, countdown warnings, PWA timeout, extension pricing, billing start-time, recovery exclusion, multiplayer billing, dispute portal
- [ ] **Phase 258: Staff Controls & Deployment Safety** — Discount approval flow, self-service block, daily reports, shift handoff, OTA session drain, graceful shutdown, deploy window lock
- [ ] **Phase 259: Coupon & Discount System** — Extension atomicity, coupon lifecycle FSM, restoration on cancel, stacking floor, payment gateway idempotency
- [ ] **Phase 260: Notifications, Resilience & UX** — Notification outbox, OTP fallback, customer receipt, leaderboard integrity, lap evidence, hardware heartbeat, anomaly detection, clock sync, queue management

---

## Phase Details

### Phase 251: Database Foundation
**Goal**: The SQLite database layer is stable under concurrent writes, timer state survives server restarts, and orphaned sessions are automatically detected
**Depends on**: Nothing (foundational infrastructure)
**Requirements**: RESIL-01, RESIL-02, FSM-09, FSM-10, RESIL-03
**Success Criteria** (what must be TRUE):
  1. Server survives simultaneous writes from 8 pods without "database is locked" errors (WAL mode active, busy_timeout 5000ms)
  2. After a server restart mid-session, billing timer state is recovered from the DB (no silent time loss)
  3. Pod timer writes are staggered by pod index so writes never cluster at the same second
  4. Any billing session with no agent heartbeat for 5+ minutes is automatically flagged and staff is alerted within the next detection cycle
**Plans:** 2 plans
Plans:
- [x] 251-01-PLAN.md — WAL verification, schema migration (elapsed_seconds + last_timer_sync_at), staggered 60s timer persistence
- [x] 251-02-PLAN.md — Orphaned session detection on startup and background 5-minute job with WhatsApp alerting

### Phase 252: Financial Atomicity Core
**Goal**: Every money-moving operation is atomic, idempotent, and race-condition-free — no double charges, no overspend, no balance drift
**Depends on**: Phase 251 (DB stability required for reliable transactions)
**Requirements**: FATM-01, FATM-02, FATM-03, FATM-04, FATM-05, FATM-06, FATM-12
**Success Criteria** (what must be TRUE):
  1. A billing start that fails mid-way (wallet debited but session not created) leaves the wallet unchanged — no orphaned debits
  2. Submitting the same /topup or /billing/start request twice returns the original result, not a double charge
  3. Two simultaneous billing starts for the same wallet cannot both succeed if only one has sufficient balance
  4. Ending a session twice (double-click race condition) does not produce two refund entries
  5. The tier price shown to the customer exactly matches what compute_session_cost() would charge for that duration
**Plans:** 3 plans
Plans:
- [x] 252-01-PLAN.md — Atomic billing start (single tx for wallet debit + session creation), idempotency keys on all money-moving endpoints, wallet row locking
- [x] 252-02-PLAN.md — CAS session finalization, unified compute_refund() function, tier/rate alignment verification
- [x] 252-03-PLAN.md — Background reconciliation job (30-min interval) comparing wallet balances to transaction sums

### Phase 253: State Machine Hardening
**Goal**: Billing and game states are always consistent — phantom billing and free gaming are structurally impossible
**Depends on**: Phase 252 (atomicity layer must exist before state guards are added)
**Requirements**: FSM-01, FSM-02, FSM-03, FSM-04, FSM-05, FSM-06, FSM-07, FSM-08
**Success Criteria** (what must be TRUE):
  1. An invalid state transition (e.g. active -> active, or cancelled -> ended) is rejected by the server with a logged error
  2. A billing session cannot remain active while the game is in Idle state — the server detects and resolves the phantom within one health cycle
  3. A game in Running state cannot exist without an active billing session — free gaming attempts are blocked at the server
  4. When a game crashes, billing is paused atomically before any relaunch is attempted — the customer is never charged for crash recovery time
  5. A split session is recorded to DB before any new launch command is issued — no launch without committed state
**Plans:** 3 plans
Plans:
- [x] 253-01-PLAN.md — Billing FSM transition table, validate_transition(), authoritative_end_session()
- [x] 253-02-PLAN.md — Phantom billing guard, free gaming guard, crash recovery atomicity, StopGame in all recovery states
- [x] 253-03-PLAN.md — Split session modeling (parent + child entitlements), DB-before-launch guard

### Phase 254: Security Hardening
**Goal**: The system rejects injection attacks, enforces role boundaries, and stores credentials safely
**Depends on**: Phase 252 (RBAC interacts with money-moving endpoints)
**Requirements**: SEC-01, SEC-02, SEC-03, SEC-04, SEC-05, SEC-06, SEC-07, SEC-08, SEC-09, SEC-10
**Success Criteria** (what must be TRUE):
  1. A launch_args value containing a newline, equals sign, or bracket character is rejected before reaching the agent
  2. An FFB GAIN value above 100 sent from the kiosk is capped to 100 at the server before forwarding to the agent
  3. A cashier cannot access pricing reports or system config; a manager cannot access system config — role gates enforced at API layer
  4. A staff member cannot top up their own wallet — the API rejects self-top-up for non-superadmin roles
  5. OTP codes stored in the database are bcrypt hashes — a database dump reveals no plaintext OTPs
**Plans:** 3 plans
Plans:
- [x] 254-01-PLAN.md — Server-side launch_args validation, FFB cap, RBAC role middleware and endpoint gating
- [x] 254-02-PLAN.md — OTP argon2 hashing, audit log immutability trigger, PIN CAS verification, PII masking
- [x] 254-03-PLAN.md — Self-topup block, WSS TLS configuration, agent game launch mutex

### Phase 255: Legal Compliance
**Goal**: Every session is legally auditable: GST is correctly separated, waivers are enforced, and minor protections are active
**Depends on**: Phase 252 (invoicing requires atomically-committed session data), Phase 254 (RBAC gates sensitive legal workflows)
**Requirements**: LEGAL-01, LEGAL-02, LEGAL-03, LEGAL-04, LEGAL-05, LEGAL-06, LEGAL-07, LEGAL-08, LEGAL-09
**Success Criteria** (what must be TRUE):
  1. Every session journal entry has a separate GST Payable line — the revenue and tax amounts are distinct rows in the ledger
  2. A GST-compliant invoice (with GSTIN, HSN code, and tax breakup) is generated for each session and accessible from the admin panel
  3. Billing cannot start on the POS path if the customer has not signed a waiver — the billing start endpoint rejects the request
  4. A minor customer (under 18) cannot be billed without a guardian OTP being verified and a staff presence toggle confirmed
  5. The pricing and refund policy is displayed to the customer on the kiosk before wallet top-up is accepted
**Plans:** 3 plans
Plans:
- [ ] 255-01-PLAN.md — GST separation in journal entries (18% inclusive), invoices table + generation, pricing/refund policy display
- [ ] 255-02-PLAN.md — Waiver gate in start_billing, minor detection from DOB, guardian OTP send/verify, guardian presence toggle, minor liability disclosure
- [ ] 255-03-PLAN.md — Data retention policy (8yr financial, 24mo PII), PII anonymization background job, consent revocation endpoint
**UI hint**: yes

### Phase 256: Game-Specific Hardening
**Goal**: Each supported game launches reliably with correct process monitoring and content verification
**Depends on**: Phase 253 (FSM must be hardened before per-game launch guards are added)
**Requirements**: GAME-01, GAME-02, GAME-03, GAME-04, GAME-05, GAME-06, GAME-07, GAME-08
**Success Criteria** (what must be TRUE):
  1. Launching a Steam game when Steam is not running or has pending updates produces a clear error — the session is not billed
  2. Fleet health monitoring correctly identifies running instances of F1, iRacing, LMU, and Forza by their actual executable names
  3. A Forza Horizon 5 session is force-terminated by the agent when the paid duration expires — the customer sees a save warning before termination
  4. An iRacing launch is blocked if the subscription check fails — the customer is not charged for a game they cannot play
  5. A launch request for a car or track not installed on the pod is rejected before billing starts
**Plans:** 3 plans
Plans:
- [ ] 256-01-PLAN.md — Steam pre-launch check, process name corrections, DLC verification, game window detection
- [ ] 256-02-PLAN.md — Forza Horizon 5 session enforcer, generic process exit monitoring for non-AC games
- [ ] 256-03-PLAN.md — AC EVO Unreal config adapter, iRacing subscription/launch verification

### Phase 257: Billing Edge Cases
**Goal**: Edge cases in session lifecycle are handled correctly — inactivity, timeouts, extensions, and disputes all have defined behaviors
**Depends on**: Phase 252 (financial atomicity), Phase 253 (FSM integrity)
**Requirements**: BILL-01, BILL-02, BILL-03, BILL-04, BILL-05, BILL-06, BILL-07, BILL-08
**Success Criteria** (what must be TRUE):
  1. A session with no lap progress or input for N minutes generates a staff alert — the customer is not silently billed for idle time
  2. A customer sees yellow warning at 5 minutes remaining and red warning at 1 minute remaining — the countdown is persistent on-screen
  3. A PWA game request not acted on within 10 minutes is automatically expired with a customer notification
  4. Billing starts when the game reaches Running state — not when the staff clicks launch — so the customer is only charged for actual play time
  5. A customer can flag a disputed charge from the PWA; staff can review logs and approve or deny a refund from the admin panel
**Plans**: TBD
**UI hint**: yes

### Phase 258: Staff Controls & Deployment Safety
**Goal**: Staff cannot abuse discounts or self-service their own accounts; deployments cannot disrupt active billing sessions
**Depends on**: Phase 252 (financial guards), Phase 254 (RBAC)
**Requirements**: STAFF-01, STAFF-02, STAFF-03, STAFF-04, STAFF-05, DEPLOY-01, DEPLOY-02, DEPLOY-03, DEPLOY-04, DEPLOY-05
**Success Criteria** (what must be TRUE):
  1. A staff discount above the configured threshold requires a manager approval code before being applied — unilateral large discounts are blocked
  2. Staff cannot perform wallet operations on their own account — the API enforces this regardless of UI state
  3. An end-of-day report shows every discount, manual refund, and tier change with the actor ID — override activity is fully auditable
  4. A pod with an active billing session defers binary swap until the session ends — customers are never mid-session during an OTA deploy
  5. A deployment attempted during the 6-11 PM weekend window requires a manual override — accidental peak-hour deploys are blocked
**Plans**: TBD

### Phase 259: Coupon & Discount System
**Goal**: Coupons have a stateful lifecycle with rollback on failure; discount stacking has a hard floor; payment gateway credits are idempotent
**Depends on**: Phase 252 (atomicity layer), Phase 253 (session state machine)
**Requirements**: FATM-07, FATM-08, FATM-09, FATM-10, FATM-11
**Success Criteria** (what must be TRUE):
  1. Purchasing a session extension debits the wallet and adds session time in a single atomic transaction — no debit without time, no time without debit
  2. A coupon moves through available -> reserved -> redeemed states; reserved coupons with expired TTL revert to available automatically
  3. A coupon reserved for a session that is cancelled or fails before billing commit is restored to available — no coupon lost to a failed session
  4. Stacking a coupon with a staff discount cannot reduce the payable amount below the configured floor — the floor is enforced server-side
  5. A payment gateway webhook that fires twice credits the wallet only once — duplicate webhooks are idempotently rejected
**Plans**: TBD

### Phase 260: Notifications, Resilience & UX
**Goal**: Notifications are durable, hardware disconnects are detected, anomalies are caught early, and customers have a reliable queue and receipt experience
**Depends on**: Phase 252 (receipts require committed session data), Phase 254 (PII masking), Phase 255 (receipt includes GST breakup)
**Requirements**: UX-01, UX-02, UX-03, UX-04, UX-05, UX-06, UX-07, UX-08, RESIL-04, RESIL-05, RESIL-06, RESIL-07, RESIL-08
**Success Criteria** (what must be TRUE):
  1. A WhatsApp notification that fails delivery is retried automatically with backoff — no notification is silently dropped
  2. When WhatsApp delivery fails, the customer is offered an on-screen OTP or SMS fallback — they are never left without their code
  3. After a session ends, the customer automatically receives a receipt showing before/after balance, duration, charges, and any refunds
  4. A leaderboard entry can only be created from a verified automatic session record — manual entry is structurally impossible
  5. A wheel or pedal USB disconnect during a session triggers billing pause and a staff alert within 5 seconds
  6. A pod with more than 3 crashes in 1 hour is automatically flagged for maintenance — the staff dashboard shows the maintenance flag
**Plans**: TBD
**UI hint**: yes

---

## Progress Table

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 251. Database Foundation | 2/2 | Complete | 2026-03-28 |
| 252. Financial Atomicity Core | 3/3 | Complete | 2026-03-28 |
| 253. State Machine Hardening | 3/3 | Complete | 2026-03-28 |
| 254. Security Hardening | 0/3 | Planned | - |
| 255. Legal Compliance | 0/3 | Planned | - |
| 256. Game-Specific Hardening | 0/3 | Planned | - |
| 257. Billing Edge Cases | 0/? | Not started | - |
| 258. Staff Controls & Deployment Safety | 0/? | Not started | - |
| 259. Coupon & Discount System | 0/? | Not started | - |
| 260. Notifications, Resilience & UX | 0/? | Not started | - |

---

## Coverage

**Total requirements:** 83
**Mapped:** 83/83

| Phase | Requirements |
|-------|-------------|
| 251 | RESIL-01, RESIL-02, FSM-09, FSM-10, RESIL-03 |
| 252 | FATM-01, FATM-02, FATM-03, FATM-04, FATM-05, FATM-06, FATM-12 |
| 253 | FSM-01, FSM-02, FSM-03, FSM-04, FSM-05, FSM-06, FSM-07, FSM-08 |
| 254 | SEC-01, SEC-02, SEC-03, SEC-04, SEC-05, SEC-06, SEC-07, SEC-08, SEC-09, SEC-10 |
| 255 | LEGAL-01, LEGAL-02, LEGAL-03, LEGAL-04, LEGAL-05, LEGAL-06, LEGAL-07, LEGAL-08, LEGAL-09 |
| 256 | GAME-01, GAME-02, GAME-03, GAME-04, GAME-05, GAME-06, GAME-07, GAME-08 |
| 257 | BILL-01, BILL-02, BILL-03, BILL-04, BILL-05, BILL-06, BILL-07, BILL-08 |
| 258 | STAFF-01, STAFF-02, STAFF-03, STAFF-04, STAFF-05, DEPLOY-01, DEPLOY-02, DEPLOY-03, DEPLOY-04, DEPLOY-05 |
| 259 | FATM-07, FATM-08, FATM-09, FATM-10, FATM-11 |
| 260 | UX-01, UX-02, UX-03, UX-04, UX-05, UX-06, UX-07, UX-08, RESIL-04, RESIL-05, RESIL-06, RESIL-07, RESIL-08 |

---
*Roadmap created: 2026-03-29*
*Previous milestone: v26.0 Meshed Intelligence (Phases 229-250)*
