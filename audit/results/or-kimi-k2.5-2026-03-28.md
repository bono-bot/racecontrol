# OpenRouter MMA: moonshotai/kimi-k2.5

 **Audit Report: Racing Point eSports Venue Management System**
*Focus: Cross-app integration points, data integrity, and financial workflows*

---

### **P1 (Critical / Security / Data Loss)**

#### **P1-001: Authentication Bypass in Financial Refund Endpoint**
- **Location:** `routes.rs` — `refund_wallet` function signature
- **Description:** The `claims` parameter is typed as `Option<Extension<StaffClaims>>`, allowing unauthenticated requests to reach the refund logic. When `claims` is `None`, `staff_id` becomes `None` but the refund still proceeds (both referenced and non-referenced paths).
- **Impact:** Unauthorized refunds, direct financial theft, untraceable transactions (staff_id NULL).
- **Fix:** Remove `Option<>` wrapper; require authentication. Explicitly reject if `claims.is_none()` before any logic.

#### **P1-002: Double-Spend Risk via Kiosk Retry Logic on Non-Idempotent Operations**
- **Location:** `kiosk/src/lib/api.ts` retry loop + server mutation endpoints (billing start, wallet debit)
- **Description:** The kiosk retries on `AbortError` (timeout) or `TypeError` (network). If a `POST /billing/start-session` or `POST /wallet/debit` request times out after the server has processed it but before the response reaches the kiosk, the retry creates a duplicate operation. While `billing_sessions` has a UNIQUE constraint, wallet debits lack idempotency protection, and the retry will charge the customer twice.
- **Impact:** Customer financial loss, duplicate billing sessions requiring manual cleanup.
- **Fix:** Implement **Idempotency-Key** header pattern. Server stores processed keys (UUID) for 24 hours with the resulting response, returning cached response for duplicates.

#### **P1-003: Refund Race Condition Allowing Over-Refund**
- **Location:** `routes.rs` — `refund_wallet` transaction block
- **Description:** The code checks `SUM(amount_paise)` of existing refunds, then inserts a new refund. Under SQLite's default isolation, two concurrent transactions can both read `SUM=0` simultaneously, then both insert refunds exceeding the original payment amount. The variable `total_refunded` is also undefined in the shown code (likely sum of two queries).
- **Impact:** Customer receives multiple refunds for single payment; venue financial loss.
- **Fix:** 
  1. Lock the wallet row at transaction start: `SELECT balance_paise FROM wallets WHERE driver_id = ? FOR UPDATE`.
  2. Add database constraint: `CHECK (amount_paise <= original_amount - refunded_amount)` via trigger or application logic with pessimistic locking.

#### **P1-004: Billing Session State Desync (DB vs. In-Memory)**
- **Location:** `billing.rs` — post-INSERT operations
- **Description:** If the database INSERT succeeds but the subsequent `state.billing.active_timers.write().await.insert()` fails (e.g., async task cancelled, deadlock, or panic), the session exists in SQLite but is not tracked in the server's active timer map. The pod appears free to the fast-path read lock check, but the DB UNIQUE constraint will block new sessions, leaving the pod in "soft lock" state.
- **Impact:** Pod becomes unusable until server restart or manual DB cleanup; revenue loss.
- **Fix:** Wrap DB insert and memory state update in a single atomic operation, or ensure cleanup (DELETE FROM billing_sessions) in a `catch_unwind` or `Drop` guard if memory insert fails.

#### **P1-005: Pod ID Normalization Mismatch Leading to State Corruption**
- **Location:** `ws/mod.rs` (Register vs. Heartbeat), `billing.rs`
- **Description:** `Register` normalizes the pod ID (`normalize_pod_id`) but falls back to the raw ID on error. `Heartbeat` uses `pod_info.id` raw. If an agent connects with `"POD-01"` (normalized to `"pod-01"`) but sends heartbeats with `"pod-01"` or `"POD-01"` inconsistently, the heartbeat updates a different hashmap entry or none at all. Billing uses normalized IDs.
- **Impact:** Billing sessions attached to wrong pods; heartbeats updating ghost entries; "zombie" pods appearing online when physically offline.
- **Fix:** Enforce canonical ID validation at connection time; reject Register if normalization fails (don't fall back). Store canonical ID in the agent connection context and reject Heartbeats where `pod_info.id != canonical_id`.

#### **P1-006: Path Traversal via Content ID Edge Cases**
- **Location:** `ac_launcher.rs` — `validate_content_id`
- **Description:** Function allows empty strings (`""`), which may resolve to the current working directory in file operations. Also allows `"...."` (4 dots), which on some legacy systems or specific path resolution libraries may be interpreted as `../.`. No check for absolute paths (e.g., `/etc/passwd` on Unix, `C:\Windows` on Windows) if the validation is bypassed or the check is case-insensitive.
- **Impact:** Arbitrary file read/execution on agent host.
- **Fix:** 
  1. Reject empty strings explicitly.
  2. Use `std::path::Path::canonicalize()` after validation and ensure the resolved path is within an allowed content directory.
  3. Reject absolute paths (starting with `/` or `[A-Z]:\`).

---

### **P2 (Functional / Correctness)**

#### **P2-001: Undefined Variable in Refund Validation**
- **Location:** `routes.rs` — `refund_wallet`
- **Description:** The code references `total_refunded` in the comparison `if total_refunded > 0`, but `total_refunded` is not defined in the shown scope (only `already_refunded_wallet` and `already_refunded_billing` are queried). This likely causes a compile error or uses a variable from outer scope incorrectly.
- **Impact:** Refund logic fails to compile or validates incorrectly.
- **Fix:** Define `total_refunded = already_refunded_wallet + already_refunded_billing` (handling NULL as 0).

#### **P2-002: Integer Overflow in Duration Calculation**
- **Location:** `billing.rs` — `allocated_seconds` calculation
- **Description:** `split_duration_minutes * 60` and `custom_duration_minutes * 60` use `u32` arithmetic. With `custom_duration_minutes` potentially coming from user input (untrusted), values > 71,582 cause overflow/wrap-around. Also `tier.2 as u32` casts `i64` to `u32`, truncating values > 4 billion or negative values.
- **Impact:** Sessions allocated with incorrect (very short or negative) durations.
- **Fix:** Use `checked_mul` and `checked_add`; validate `duration_minutes < 10_000` (approx 1 week) before calculation.

#### **P2-003: Missing Pod Existence Validation**
- **Location:** `billing.rs` — `start_billing_session`
- **Description:** The function does not verify that `pod_id` exists in `state.pods` before creating a billing session. It only checks for existing active billing sessions.
- **Impact:** Sessions created for non-existent or unregistered pods; orphaned database records.
- **Fix:** Check `state.pods.read().await.contains_key(&pod_id)` before DB insert.

#### **P2-004: WebSocket Heartbeat Lacks Sender Authentication**
- **Location:** `ws/mod.rs` — `AgentMessage::Heartbeat`
- **Description:** Any WebSocket connection can send a Heartbeat for any `pod_id`, updating the IP address and game state of pods they don't own. No verification that the sender is the registered agent for that pod.
- **Impact:** Malicious agent can spoof status of other pods (e.g., mark busy pod as idle).
- **Fix:** Look up the expected `pod_id` for this connection from `state.agent_conn_ids` (reverse lookup) and reject heartbeats with mismatched `pod_info.id`.

#### **P2-005: Kiosk Missing Authentication Redirect**
- **Location:** `kiosk/src/lib/api.ts` — `fetchApi`
- **Description:** Unlike the web dashboard, the kiosk fetcher does not handle HTTP 401 (Unauthorized) by clearing the token and redirecting to login. Staff using an expired kiosk session sees only a generic error.
- **Impact:** Poor UX; staff cannot recover from session expiry without manual refresh.
- **Fix:** Add `if (res.status === 401) { sessionStorage.removeItem("kiosk_staff_token"); window.location.href = "/login"; }` before the generic error throw.

#### **P2-006: Web Dashboard Missing Request Timeout**
- **Location:** `web/src/lib/api.ts` — `fetchApi`
- **Description:** No `AbortController` or timeout is set. If the Rust server hangs (e.g., SQLite lock), the browser request remains pending indefinitely, blocking the UI.
- **Impact:** Frozen UI requiring manual browser refresh.
- **Fix:** Add 30-second timeout using `AbortController` (consistent with kiosk implementation).

#### **P2-007: Partial Refund Logic Error**
- **Location:** `routes.rs` — `refund_wallet`
- **Description:** The check `if total_refunded > 0` prevents any refund if *any* refund exists, rather than checking if the *cumulative* refund exceeds the original amount. This prevents legitimate partial refunds (e.g., refunding ₹200 of a ₹500 session).
- **Impact:** Operational inflexibility; requires manual database intervention for partial refunds.
- **Fix:** Compare `total_refunded + req.amount_paise > original_session_amount` (fetched from the billing session).

---

### **P3 (Reliability / Edge Cases)**

#### **P3-001: SQLite Single-Writer Bottleneck**
- **Location:** All database interactions
- **Description:** SQLite allows only one writer at a time. Under concurrent load (8 pods + 3 frontends), write operations queue up. The kiosk 30s timeout may trigger while waiting for the DB lock, causing unnecessary retries.
- **Impact:** Degraded performance; false-positive timeout errors.
- **Fix:** Implement a request coalescing queue or migrate to PostgreSQL/MySQL for production multi-pod setups.

#### **P3-002: In-Memory State Lost on Restart**
- **Location:** `state.billing.active_timers`, `state.pods`
- **Description:** Server restart clears all in-memory hashmaps. Active billing sessions in SQLite remain, but the server loses track of timer ticks, pod statuses, and WebSocket sender channels.
- **Impact:** Billing continues but server thinks pods are idle; WebSocket commands fail until agents re-register.
- **Fix:** On startup, rehydrate `active_timers` and `pods` from `billing_sessions` table (SELECT WHERE status IN active/paused states).

#### **P3-003: Non-Atomic WebSocket Registration**
- **Location:** `ws/mod.rs` — `AgentMessage::Register`
- **Description:** Three separate write locks acquired sequentially (`agent_senders`, `agent_conn_ids`, `pods`). If the task panics or is cancelled between the second and third lock, the agent is partially registered.
- **Impact:** Memory leaks; inconsistent connection tracking.
- **Fix:** Combine related state into a single struct behind one lock, or use a transaction-like pattern with rollback on failure.

#### **P3-004: Content ID Length vs. Filesystem Limits**
- **Location:** `ac_launcher.rs`
- **Description:** 128-character limit exceeds typical filesystem filename limits (255 bytes) when combined with path prefixes, risking creation failures on encrypted filesystems (eCryptfs limited to ~140 chars total path).
- **Impact:** Content fails to launch on specific filesystem configurations.
- **Fix:** Reduce limit to 100 chars or validate full path length against `PATH_MAX` (4096 on Linux).

#### **P3-005: Retry Logic Misclassification**
- **Location:** `kiosk/src/lib/api.ts`
- **Description:** `err instanceof TypeError` catches not only network errors but also programming errors (e.g., invalid URL construction, null reference). These should not be retried.
- **Impact:** Infinite retry loops on client bugs (if bug is in request construction).
- **Fix:** Check `err.message` for "fetch" or "network" specific strings, or verify `err.name === 'TypeError' && !err.message.includes('JSON')` etc.

#### **P3-

---
Tokens: in=2494 out=8000
