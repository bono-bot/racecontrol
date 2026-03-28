# OpenRouter MMA: xiaomi/mimo-v2-pro

# Cross-App Workflow Security Audit Report

## Executive Summary
Audited 8 integration points across the Racing Point eSports system. Found **3 P1 vulnerabilities**, **5 P2 bugs**, and **4 P3 reliability issues**. Critical financial and race condition risks identified.

---

## P1 (Critical/Security/Data-Loss) Bugs

### BUG-001: TOCTOU Race Condition in Billing Session Start
**Severity:** P1  
**Category:** Security/Financial  
**Location:** billing.rs - `start_billing_session()`  
**Description:** The in-memory check (`active_timers.read()`) and DB insert are not atomic. Between releasing the read lock and acquiring the write lock, another request could pass the in-memory check and proceed to DB insert. While the DB UNIQUE constraint prevents duplicate sessions, the error handling doesn't account for this race properly.  
**Impact:** Financial loss if billing sessions are created incorrectly; potential double-billing or missed billing.  
**Fix:** Remove the in-memory pre-check or implement proper optimistic concurrency control. The DB constraint is sufficient.

### BUG-002: Unvalidated Reference ID in Wallet Refund
**Severity:** P1  
**Category:** Financial/Security  
**Location:** routes.rs - `refund_wallet()`  
**Description:** The reference_id validation only checks if the session exists and belongs to the driver, but doesn't validate:
1. Session status (could refund active sessions)
2. Refund amount vs. session cost (could over-refund)
3. Session completion state  
**Impact:** Financial loss through fraudulent refunds; refunding active sessions disrupts billing.  
**Fix:** Add validation: `WHERE id = ? AND driver_id = ? AND status IN ('completed', 'ended_early') AND cost_paise >= ?`

### BUG-003: Path Traversal Bypass in Content Validation
**Severity:** P1  
**Category:** Security  
**Location:** ac_launcher.rs - `validate_content_id()`  
**Description:** Only checks for `..` but not:
1. Single dot sequences (`./`, `/.`)
2. URL-encoded traversal (`%2e%2e`)
3. Backslash on Windows agents (`\..\`)
4. Null bytes (`%00`)  
**Impact:** Arbitrary file read/write on pod agents; potential RCE.  
**Fix:** Use allowlist approach with canonical path validation; reject all path separators.

---

## P2 (Functional/Correctness) Bugs

### BUG-004: Missing Pod Existence Validation
**Severity:** P2  
**Category:** Functional  
**Location:** billing.rs - `start_billing_session()`  
**Description:** No validation that `pod_id` exists in `state.pods` before creating billing session. Creates orphaned sessions.  
**Impact:** Billing sessions for non-existent pods; UI inconsistencies.  
**Fix:** Add check: `if !state.pods.read().await.contains_key(&pod_id) { return Err("Pod not found"); }`

### BUG-005: Inconsistent Pod ID Normalization
**Severity:** P2  
**Category:** Functional  
**Location:** ws/mod.rs - Heartbeat handler  
**Description:** Register normalizes pod_id, but Heartbeat uses raw `pod_info.id` for lookup. If agent sends non-normalized ID, heartbeat updates fail.  
**Impact:** Pod state not updating; stale data in UI.  
**Fix:** Normalize in Heartbeat: `let canonical_id = normalize_pod_id(&pod_info.id).unwrap_or_else(|_| pod_info.id.clone());`

### BUG-006: Header Override Vulnerability in Kiosk API
**Severity:** P2  
**Category:** Security  
**Location:** kiosk/src/lib/api.ts - `fetchApi()`  
**Description:** `{ headers, ...options }` allows options.headers to override Content-Type and Authorization headers.  
**Impact:** API calls with missing/incorrect auth; potential CSRF or injection.  
**Fix:** Merge headers properly: `{ ...options, headers: { ...headers, ...options?.headers } }`

### BUG-007: Missing 401 Handling in Kiosk
**Severity:** P2  
**Category:** Functional  
**Location:** kiosk/src/lib/api.ts - `fetchApi()`  
**Description:** No 401 handling unlike web dashboard. Expired tokens cause silent failures.  
**Impact:** Kiosk becomes unusable until manual refresh; poor UX.  
**Fix:** Add 401 handling: `if (res.status === 401) { sessionStorage.removeItem("kiosk_staff_token"); window.location.reload(); }`

### BUG-008: Incomplete Transaction Rollback in Refund
**Severity:** P2  
**Category:** Financial  
**Location:** routes.rs - `refund_wallet()`  
**Description:** If `wallet_transactions` insert fails after `wallets` update, transaction rolls back but error message doesn't indicate partial failure.  
**Impact:** Confusing error messages; support overhead.  
**Fix:** Add transaction logging or ensure atomic operations include all related tables.

---

## P3 (Reliability/Edge-Case) Bugs

### BUG-009: No Timeout in Web Dashboard API
**Severity:** P3  
**Category:** Reliability  
**Location:** web/src/lib/api.ts - `fetchApi()`  
**Description:** No request timeout; browser default may be 30s-300s.  
**Impact:** UI hangs on network issues; poor user experience.  
**Fix:** Add AbortController with timeout like kiosk implementation.

### BUG-010: Missing Retry on Server Errors
**Severity:** P3  
**Category:** Reliability  
**Location:** kiosk/src/lib/api.ts - `fetchApi()`  
**Description:** Only retries on network errors, not 5xx server errors which may be transient.  
**Impact:** Unnecessary failures during brief server issues.  
**Fix:** Retry on 5xx: `if (res.status >= 500 && attempt < 2) continue;`

### BUG-011: Unbounded Memory in Agent Handlers
**Severity:** P3  
**Category:** Reliability  
**Location:** ws/mod.rs - Register handler  
**Description:** No cleanup of `agent_senders`/`agent_conn_ids` on disconnect. Memory leak over time.  
**Impact:** Server memory exhaustion after many agent reconnects.  
**Fix:** Add disconnect handler to remove entries; implement TTL for stale connections.

### BUG-012: SQLite Write Contention
**Severity:** P3  
**Category:** Performance  
**Location:** Multiple (billing.rs, wallet.rs)  
**Description:** Single-writer SQLite with concurrent billing operations from 8 pods.  
**Impact:** Database locks under high load; increased latency.  
**Fix:** Implement write queue or consider PostgreSQL for production.

---

## Cross-App Integration Risks

### Critical Path: Billing → Wallet → UI
1. **BUG-001** + **BUG-002**: Race condition + invalid refunds = financial loss
2. **BUG-004** + **BUG-005**: Orphan sessions + stale UI = operational confusion

### Agent Communication Chain
1. **BUG-003**: Path traversal in agent → server compromise
2. **BUG-005** + **BUG-011**: Heartbeat failures + memory leaks = unreliable pod management

---

## Recommended Immediate Actions

1. **Hotfix within 24h**: BUG-001, BUG-002, BUG-003
2. **Next release**: BUG-004, BUG-005, BUG-006, BUG-007
3. **Backlog**: BUG-008 through BUG-012

## Positive Findings
- Good use of DB constraints as race guards
- Proper atomic transactions in wallet operations
- Comprehensive status types covering all states
- Retry logic in kiosk for network resilience

---
**Auditor Note**: The system shows thoughtful architecture but has critical gaps in cross-component validation and error handling. Financial operations require immediate hardening.

---
Tokens: in=2780 out=6323
