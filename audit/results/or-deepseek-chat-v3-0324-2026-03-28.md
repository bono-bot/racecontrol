# OpenRouter MMA: deepseek/deepseek-chat-v3-0324

Here's a comprehensive audit of cross-app workflow bugs, focusing on integration points:

### P1 (Critical/Security/Data Loss)

**ID: P1-001**
- **Category**: Race Condition
- **Location**: `billing.rs` - start_billing_session()
- **Description**: The two-phase check (in-memory lock + DB UNIQUE) doesn't fully prevent race conditions. Between the read lock release and DB insert, another session could start.
- **Impact**: Duplicate billing sessions leading to financial discrepancies.
- **Fix**: Use a distributed lock or DB transaction spanning the entire operation.

**ID: P1-002**
- **Category**: Authorization Bypass
- **Location**: `routes.rs` - refund_wallet()
- **Description**: Missing validation that staff making >₹500 refunds has appropriate privileges (claims.scope check).
- **Impact**: Privilege escalation allowing unauthorized staff to issue large refunds.
- **Fix**: Add scope check (e.g., `finance:refund`) for amounts >50,000.

**ID: P1-003**
- **Category**: Data Integrity
- **Location**: `ws/mod.rs` - Register/Heartbeat handlers
- **Description**: Pod state updates aren't atomic - core-managed fields could be corrupted if agent reconnects during billing session.
- **Impact**: Possible billing_session_id/current_driver desync between agent and core.
- **Fix**: Use full state reconciliation with version checks.

### P2 (Functional/Correctness)

**ID: P2-001**
- **Category**: Validation Gap
- **Location**: `billing.rs` - start_billing_session()
- **Description**: Missing validation that `split_count * split_duration_minutes <= tier.duration_minutes`.
- **Impact**: Could allocate more time than paid for.
- **Fix**: Add validation before calculating allocated_seconds.

**ID: P2-002**
- **Category**: State Management
- **Location**: `wallet.rs` - debit()
- **Description**: No transaction isolation level specified (could allow dirty reads during concurrent debits).
- **Impact**: Potential balance miscalculations under high concurrency.
- **Fix**: Use `SERIALIZABLE` isolation level.

**ID: P2-003**
- **Category**: Error Handling
- **Location**: `api.ts` (both web and kiosk)
- **Description**: Network retries in kiosk don't preserve request body on retry.
- **Impact**: POST/PUT requests may fail or corrupt data on retry.
- **Fix**: Clone request body for retries.

### P3 (Reliability/Edge Cases)

**ID: P3-001**
- **Category**: Resource Leak
- **Location**: `api.ts` (kiosk)
- **Description**: AbortController timeout isn't cleared if request succeeds before timeout.
- **Impact**: Memory leak over long-running kiosk sessions.
- **Fix**: Move clearTimeout before return.

**ID: P3-002**
- **Category**: Validation Gap
- **Location**: `ac_launcher.rs` - validate_content_id()
- **Description**: Doesn't prevent leading/trailing special chars that could affect filesystem paths.
- **Impact**: Potential path resolution issues.
- **Fix**: Add checks for leading/trailing '-', '_', or '.'.

**ID: P3-003**
- **Category**: State Sync
- **Location**: `ws/mod.rs` - Heartbeat handler
- **Description**: No periodic full-state sync between agents and core.
- **Impact**: Drift in non-heartbeat fields over long sessions.
- **Fix**: Implement periodic full-state sync (every N heartbeats).

**ID: P3-004**
- **Category**: Financial Logic
- **Location**: `routes.rs` - refund_wallet()
- **Description**: Doesn't verify that refund amount <= original session cost.
- **Impact**: Possible over-refunding.
- **Fix**: Add cross-check with billing_sessions.cost_paise.

Cross-App Integration Recommendations:
1. Implement distributed locking for billing session management
2. Add comprehensive audit logging for all financial transactions
3. Introduce state versioning for pod-agent synchronization
4. Standardize API error handling with request ID correlation
5. Implement heartbeat-based dead man switches for agent connections

The most critical issues are the race condition in billing session creation (P1-001) and the authorization gap in refund processing (P1-002), as these directly impact financial integrity. The state synchronization issues (P1-003, P3-003) are particularly important given the distributed nature of the system.

---
Tokens: in=2697 out=975
