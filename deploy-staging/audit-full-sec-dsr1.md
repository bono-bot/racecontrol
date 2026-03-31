# SECURITY AUDIT: v29.0 Meshed Intelligence for Racing Point eSports
**Auditor:** Senior Application Security Auditor  
**Date:** 2024-03-15  
**Scope:** Rust/Axum application with SQLite backend, Windows deployment  
**Focus:** Maintenance/HR/Analytics endpoints & general security posture  

---

## EXECUTIVE SUMMARY

The application demonstrates good security practices in some areas (parameterized SQL queries, structured auth tiers) but has **critical vulnerabilities** in data exposure, authorization, and SSRF. Most concerning are **PII exposure in HR endpoints**, **incomplete authorization on sensitive routes**, and **SSRF vectors via Ollama client**. Immediate remediation required for production deployment.

---

## DETAILED FINDINGS

### 1. SQL INJECTION
**STATUS:** ✅ **LOW RISK** - Good use of parameterized queries

**P3 - SQL Queries**  
**File:** maintenance_store.rs (multiple lines)  
**Description:** All SQL queries use `sqlx` parameterized placeholders (`?1`, `?2`). No evidence of string concatenation or `format!()` in queries.  
**Fix:** Continue current practice; add automated SQL injection testing in CI/CD.

### 2. AUTH/AUTHZ (CRITICAL)
**P1 - Missing Role-Based Access Control**  
**Endpoint:** `/hr/recognition` (GET)  
**File:** routes.rs:11329  
**Description:** HR recognition endpoint returns employee performance data including potentially sensitive evaluation scores, but is only protected by `require_staff_jwt()` which allows any staff member (including cashiers, kiosk staff) to access HR/performance data.  
**Fix:** Move to manager+ role-gated router or add `require_role_manager()` middleware.

**P1 - Incomplete Protection on HR Endpoints**  
**Endpoints:** 
- `/hr/sjts` (GET)
- `/hr/sjts/{id}` (GET)  
- `/hr/job-preview` (GET)
**Description:** Situational Judgment Tests (SJTs) and job preview data should be restricted to hiring managers/HR staff, not all staff. Currently in main `staff_routes()` without role checks.  
**Fix:** Move to manager+ router or apply HR-specific role middleware.

**P2 - Missing RBAC on Maintenance Analytics**  
**Endpoints:**
- `/analytics/telemetry` (GET)
- `/analytics/trends` (GET)  
**Description:** Business intelligence/trend analytics should be manager+ only to prevent data misuse (e.g., cashier analyzing patterns for theft).  
**Fix:** Move to manager+ router or add `require_role_manager()`.

**P2 - Pod Unrestrict Missing Authorization**  
**Endpoint:** `/pods/{id}/unrestrict` (POST)  
**File:** routes.rs (unrestrict_pod handler)  
**Description:** Allows unrestricted pod access (debug mode, lockdown bypass) - should be superadmin-only due to security impact. Currently in `staff_routes()` with only basic JWT.  
**Fix:** Add `require_role_superadmin()` or move to role-gated superadmin section.

**P3 - Mesh Intelligence Write Ops Missing Role Checks**  
**Endpoints:**
- `/mesh/solutions/{id}/promote` (POST)
- `/mesh/solutions/{id}/retire` (POST)  
**Description:** Promoting/retiring AI solutions affects operational intelligence; should require manager+ authorization.  
**Fix:** Add `require_role_manager()` middleware.

### 3. INPUT VALIDATION
**P2 - Missing Validation on Maintenance Queries**  
**File:** routes.rs:1489, 1542  
**Endpoints:** 
- `/maintenance/events` (GET with `MaintenanceEventQuery`)
- `/maintenance/tasks` (GET with `MaintenanceTaskQuery`)  
**Description:** Query structs lack validation:
  - `pod_id: Option<u8>` - assumes pod IDs fit in u8 (0-255), may overflow
  - No date range limits on queries (could query all historical data)
  - No pagination limits  
**Fix:** Add validation:
```rust
#[derive(Deserialize, Validate)]
struct MaintenanceEventQuery {
    #[validate(range(min = 1, max = 255))]
    pod_id: Option<u8>,
    #[validate(range(min = 0, max = 1000))]
    limit: Option<i32>,
}
```

**P2 - Watchdog Crash Report Missing Size Limits**  
**Endpoint:** `/pods/{pod_id}/watchdog-crash` (POST)  
**File:** routes.rs:17189  
**Description:** `Json(report): Json<WatchdogCrashReport>` accepts arbitrary JSON size; crash reports could contain large memory dumps.  
**Fix:** Add Axum `DefaultBodyLimit` or custom middleware limiting to 1MB.

### 4. CORS/HEADERS
**P3 - New Endpoints Missing Security Headers**  
**Endpoints:** All new v29.0 endpoints (maintenance/analytics/hr)  
**Description:** No evidence of security headers (CSP, HSTS, X-Frame-Options, X-Content-Type-Options) specific to these endpoints. Relying on global middleware which may be incomplete.  
**Fix:** Ensure headers set globally:
- `Content-Security-Policy: default-src 'self'`
- `X-Frame-Options: DENY`
- `X-Content-Type-Options: nosniff`
- `Referrer-Policy: strict-origin-when-cross-origin`

### 5. EXTERNAL CALLS (CRITICAL)
**P1 - SSRF via Ollama Client**  
**File:** ollama_client.rs  
**Description:** Hardcoded internal IP `192.168.31.27:11434` but:
1. **No network segmentation validation** - if service migrates, could call external addresses
2. **Error messages leak internal network info**: `"Ollama unavailable: primary={}, fallback={}"` exposes internal errors
3. **Timeout but no retry limits** - could be used for DoS by repeatedly triggering via API  
**Fix:**
```rust
const OLLAMA_URL: &str = "http://192.168.31.27:11434";
// Add validation
fn validate_internal_url(url: &str) -> bool {
    url.starts_with("http://192.168.31.") || url.starts_with("http://10.")
}
// Generic error messages
Err(anyhow::anyhow!("AI service temporarily unavailable"))
```

**P2 - Cameras Health Proxy SSRF Risk**  
**Endpoint:** `/cameras/health` (GET)  
**File:** routes.rs (cameras_health_proxy handler)  
**Description:** While hardcoded to `.27:1984`, uses `reqwest::Client` that could be reused/contaminated. No validation that target is internal-only.  
**Fix:** Add IP validation or use separate client with restricted network policy.

### 6. DATA EXPOSURE (CRITICAL)
**P1 - HR Recognition Data Exposure**  
**Endpoint:** `/hr/recognition` (GET)  
**File:** routes.rs:11329  
**Description:** Returns `Json<Value>` - likely includes employee performance reviews, recognition awards, potentially salary adjustments. Accessible to all staff.  
**Fix:** 
1. Move to manager+ router
2. Implement data masking: remove PII, aggregate data
3. Add audit logging for access

**P1 - Employee PII in SQL Queries**  
**File:** maintenance_store.rs:908, 913  
**Queries:** 
- `UPDATE employees SET hourly_rate_paise = ?1 WHERE id = ?2`
- `UPDATE employees SET phone = ?1 WHERE id = ?2`  
**Description:** Salary (in paise) and phone numbers stored/retrieved. Need to confirm these aren't exposed via `/hr/` endpoints.  
**Fix:** Audit all HR endpoints for PII exposure; ensure salary only visible to finance roles.

**P2 - Analytics Telemetry Data Leak**  
**Endpoint:** `/analytics/telemetry` (GET)  
**Description:** Likely includes business metrics (revenue, utilization) that could aid competitors. Currently in `staff_routes()` accessible to all staff.  
**Fix:** Move to manager+ router; implement data aggregation/anonymization.

### 7. RATE LIMITING
**P2 - New Endpoints Lack Rate Limiting**  
**Endpoints:**
- `/maintenance/*` (POST operations)
- `/analytics/*` (data-intensive queries)
- `/hr/*` (data queries)  
**Description:** No rate limiting on these new endpoints. Analytics queries could be expensive (DB load), maintenance operations could be abused (flood pod commands).  
**Fix:** Apply tiered rate limiting:
- `maintenance/*` POST: 10/min per staff ID
- `analytics/*` GET: 30/min per IP (cached responses)
- `hr/*` GET: 20/min per staff ID

**P2 - Ollama Endpoint Missing Rate Limit**  
**Description:** Any endpoint calling `ollama_client::diagnose()` could trigger expensive AI inference. No rate limiting evident.  
**Fix:** Add global rate limit of 5 calls/min per staff ID for Ollama diagnosis.

---

## PRIORITY SUMMARY

### P1 (CRITICAL - Immediate Fix):
1. HR data exposure to all staff
2. SSRF via Ollama client (internal network access)
3. Missing RBAC on sensitive HR/maintenance endpoints

### P2 (HIGH - Fix Before Next Release):
1. Input validation missing on query parameters
2. Rate limiting gaps on new endpoints
3. Analytics data leakage to non-managers
4. Cameras health proxy SSRF risk

### P3 (MEDIUM - Schedule Fix):
1. CORS/security headers consistency
2. Mesh intelligence authorization gaps
3. SQL query validation (though parameterized)

---

## RECOMMENDATIONS

### Immediate Actions (Week 1):
1. **Move HR endpoints** to manager+ role-gated router
2. **Implement IP validation** for Ollama/camera calls
3. **Add RBAC middleware** to `/pods/{id}/unrestrict`
4. **Deploy rate limiting** for all new v29.0 endpoints

### Short-term (Month 1):
1. **Complete input validation** with `validator` crate
2. **Audit all data exposure** - implement data classification
3. **Add security headers** middleware test suite
4. **Implement audit logging** for sensitive data access

### Long-term (Quarter 1):
1. **Network segmentation** - isolate Ollama/AI services
2. **Data loss prevention** - monitor PII in responses
3. **Automated security testing** - integrate OWASP ZAP/scan
4. **Staff training** - secure coding for Rust/Axum

---

**AUDITOR SIGN-OFF:**  
This audit reveals systemic issues in authorization design and data protection. The codebase shows good patterns in SQL safety but critical gaps in access control. Immediate prioritization should be HR data protection and network isolation of AI services.

**Next Steps:** Schedule remediation sprint focusing on P1 items; conduct penetration test after fixes.