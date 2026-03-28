# OpenRouter MMA: x-ai/grok-4.1-fast

### BUG-001
**Severity**: P1  
**Category**: financial  
**Location**: 2. Wallet Refund (Rust server — routes.rs), reference_id branch (already_refunded_wallet and already_refunded_billing queries)  
**Description**: `query_as::<_, (i64,)>` on `SELECT SUM(amount_paise) ...` without `COALESCE(SUM(amount_paise), 0)`. SQLite returns a single row with NULL if no matching rows; sqlx fails to decode NULL to `i64` (ColumnDecode/TryFromPrimitive error).  
**Impact**: First refund per session always fails (cross-app: admin/web refund workflows broken). Repeat refunds blocked anyway, but prevents all refunds, causing financial disputes/customer loss/venue liability.  
**Fix**: Change to `SELECT COALESCE(SUM(amount_paise), 0) AS total_refunded_paise FROM ...`. Handle as `let already_refunded_wallet: (i64,) = ...?; let total_wallet = already_refunded_wallet.0;`.

### BUG-002
**Severity**: P1  
**Category**: financial/data-loss  
**Location**: 2. Wallet Refund (Rust server — routes.rs), reference_id branch; also implied in non-referenced `wallet::credit()`  
**Description**: `UPDATE wallets SET balance_paise = balance_paise + ? WHERE driver_id = ?` executes without checking `rows_affected == 1` or RETURNING. If no wallet row exists for `driver_id`, UPDATE affects 0 rows (no balance change), but `INSERT INTO wallet_transactions` proceeds and commits.  
**Impact**: Refunds credit transaction log but not balance (cross-app: web/admin sees txn history showing credit, but kiosk/web balance queries show 0; driver can't spend credited amount). Enables disputes/double-refund exploits; financial inconsistency/venue over-refund risk.  
**Fix**: Use SQLite UPSERT: `INSERT INTO wallets (driver_id, balance_paise) VALUES (?, ?) ON CONFLICT(driver_id) DO UPDATE SET balance_paise = balance_paise + excluded.balance_paise RETURNING balance_paise`. Or check `execute().await?.rows_affected() == 1`, else `INSERT` new wallet with `balance_paise = ?`. Apply to `wallet::credit()`.

### BUG-003
**Severity**: P2  
**Category**: correctness  
**Location**: 7. Kiosk fetchApi (TypeScript — kiosk/src/lib/api.ts)  
**Description**: `headers` referenced in `fetch(..., { headers, ... })` but never declared/initialized. `token` fetched from `sessionStorage.getItem("kiosk_staff_token")` but never added to `Authorization: Bearer`. No `Content-Type`.  
**Impact**: Kiosk REST calls to server (e.g., start_billing) unauthenticated/malformed (cross-app: kiosk -> server fails auth for staff actions; public endpoints work but staff-only broken). Kiosk workflows (billing start) fail silently or 401-loop without retries.  
**Fix**: Add before loop: `const headers: Record<string, string> = { "Content-Type": "application/json" }; if (token) headers["Authorization"] = `Bearer ${token}`;`. Match web/src/lib/api.ts pattern.

### BUG-004
**Severity**: P2  
**Category**: correctness  
**Location**: 4. WebSocket Agent Handler (Rust server — ws/mod.rs), Heartbeat handler  
**Description**: `pods.get_mut(&pod_info.id)` uses raw `pod_info.id`; Register uses `canonical_id = normalize_pod_id(&pod_info.id).unwrap_or_else(...)` as key for `pods.insert(canonical_id, ...)`. Mismatch if normalization changes ID (e.g., trim/lowercase).  
**Impact**: Agent heartbeats ignored (no updates to `ip_address`, `driving_state`, `game_state`). Server `state.pods` stale (cross-app: WS broadcasts to kiosk/web/admin show outdated pod data; billing/dynamic pricing/game sync fails).  
**Fix**: In Heartbeat: `let canonical_id = normalize_pod_id(&pod_info.id).unwrap_or_else(|| pod_info.id.clone());` then `pods.get_mut(&canonical_id)`. Ensure `normalize_pod_id` idempotent. Optionally normalize `pod_info.id` before `pods.insert` in Register.

### BUG-005
**Severity**: P2  
**Category**: functional  
**Location**: 1. Billing Session Start (Rust server — billing.rs), state.pods update after INSERT  
**Description**: `state.pods.write().await.get_mut(&pod_id)` (normalized) may return None if agent not yet registered/connected. Skips setting `pod.billing_session_id`, `current_driver`, `status=InSession`. Register later inserts agent `pod_info` (no billing fields).  
**Impact**: Transient pod state inconsistency until Register reconcile/resync (cross-app: kiosk/web WS/subscribe sees wrong pod status/driver pre-agent-connect; possible duplicate starts if DB-only check missed).  
**Fix**: Always load/update pod from DB post-INSERT: query latest `billing_sessions` for `pod_id`, upsert to `state.pods` (canonical key). Ensure Register reconcile queries DB for billing fields.

### BUG-006
**Severity**: P3  
**Category**: reliability/edge-case  
**Location**: 1. Billing Session Start (Rust server — billing.rs), `allocated_seconds` calc + 4. WS Handler  
**Description**: `split_duration_minutes.filter(|_| split_count.unwrap_or(1) > 1)` ignores `custom_duration_minutes` correctly, but no validation `allocated_seconds > 0` or cap (u32 overflow wrap on `*60`). Pods heartbeat doesn't merge `status`/`billing_session_id` (intentional, but edge concurrent disconnect/reconnect loses in-mem sync).  
**Impact**: Rare zero-duration sessions (cross-app: agent WS game_state drifts from server billing; kiosk shows invalid remaining_seconds). Low risk (frontend validation assumed).  
**Fix**: Add `if allocated_seconds == 0 { return Err("Invalid duration"); }`. Cap `allocated_seconds.min(2_592_000u32)` (30 days). Explicitly sync billing fields in Heartbeat if changed.

### BUG-007
**Severity**: P3  
**Category**: reliability/edge-case  
**Location**: 6. Web Dashboard fetchApi (TypeScript — web/src/lib/api.ts) + 7. Kiosk  
**Description**: Web auto-redirects/clears on 401 (window-only); Kiosk retries only network errors (TypeError/AbortError), throws on 401/4xx without token clear/redirect. No shared error typing from `shared-types`.  
**Impact**: Stale kiosk token -> repeated 401 fails (cross-app: kiosk billing stuck until manual token refresh/reload). Web SSR safe but no retry. Edge: high-latency venues.  
**Fix**: Kiosk: Add `if (res.status === 401) { sessionStorage.removeItem("kiosk_staff_token"); throw new Error("Session expired"); }`. Use shared `ApiError` type. Web: optional kiosk-mode param for retry.

---
Tokens: in=2620 out=9020
