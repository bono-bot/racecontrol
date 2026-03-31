# SECURITY AUDIT - Meshed Intelligence for Racing Point eSports v29.0

## 🚨 CRITICAL FINDINGS (P1)

### **P1-001: No Authentication on New Maintenance/Analytics Endpoints**
**Location:** `staff_routes()` - lines 301-306  
**Finding:** New maintenance and analytics endpoints lack authentication middleware:
```rust
.route("/maintenance/events", post(maintenance_create_event).get(maintenance_list_events))
.route("/maintenance/summary", get(maintenance_summary))
.route("/maintenance/tasks", post(maintenance_create_task).get(maintenance_list_tasks))
.route("/analytics/telemetry", get(analytics_telemetry))
.route("/analytics/trends", get(analytics_trends))
```
**Impact:** Anyone can create/view maintenance events, tasks, and access sensitive analytics data without authentication.  
**Fix:** Add `require_staff_jwt()` middleware to these routes.

### **P1-002: HR Endpoints Exposing Employee PII Without Role Checks**
**Location:** `staff_routes()` - lines 248-254  
**Finding:** HR endpoints accessible to all staff without role validation:
```rust
.route("/hr/recognition", get(hr_recognition_data))
```
The `hr_recognition_data` function at line 11329 likely exposes employee salaries, phone numbers, and personal data.  
**Impact:** Cashiers can access sensitive HR data including wages and contact info.  
**Fix:** Move HR endpoints to manager+ role-gated router section.

### **P1-003: Hardcoded IP Address in External HTTP Calls**
**Location:** `ollama.rs` - lines 9, 86  
**Finding:** Hardcoded internal IP `192.168.31.27:11434` for Ollama calls:
```rust
const OLLAMA_URL: &str = "http://192.168.31.27:11434/api/generate";
```
**Impact:** Potential SSRF if input validation fails; debugging info exposure in error messages.  
**Fix:** Use configuration-based URLs, validate against allowlist, sanitize error messages.

## 🔴 HIGH FINDINGS (P2)

### **P2-004: SQL Injection Risk in Dynamic Queries**
**Location:** `maintenance_store.rs` - lines 891-923  
**Finding:** Dynamic employee updates use string concatenation in SQL queries:
```rust
sqlx::query("UPDATE employees SET name = ?1 WHERE id = ?2")
```
While these use parameterized queries, the pattern suggests risk if expanded.  
**Impact:** Potential SQL injection if similar patterns used elsewhere.  
**Fix:** Audit all dynamic SQL, use sqlx compile-time checking.

### **P2-005: No Rate Limiting on New Endpoints**
**Location:** Multiple new endpoints  
**Finding:** Maintenance/analytics/HR endpoints lack rate limiting:
- `/maintenance/*` 
- `/analytics/*`
- `/hr/*`  
**Impact:** DoS attacks, resource exhaustion.  
**Fix:** Apply rate limiting middleware to staff routes.

### **P2-006: Missing Input Validation on JSON Bodies**
**Location:** Lines 1478, 1525, 17189  
**Finding:** No length limits or validation on maintenance event/task JSON inputs:
```rust
Json(event): Json<crate::maintenance_models::MaintenanceEvent>
Json(task): Json<crate::maintenance_models::MaintenanceTask>
```
**Impact:** Large payload DoS, potential memory exhaustion.  
**Fix:** Add content-length limits, validate JSON structure and field lengths.

## 🟡 MEDIUM FINDINGS (P3)

### **P3-007: Information Disclosure in Error Responses**
**Location:** `ollama.rs` - line 52  
**Finding:** Detailed error messages expose internal state:
```rust
Err(anyhow::anyhow!("Ollama unavailable: primary={}, fallback={}", e, e2))
```
**Impact:** Internal architecture disclosure to attackers.  
**Fix:** Return generic error messages to clients, log details server-side.

### **P3-008: Missing CORS Headers on New Endpoints**
**Location:** All new maintenance/analytics/hr endpoints  
**Finding:** No explicit CORS configuration visible for new API routes.  
**Impact:** Potential browser-based attacks if CORS misconfigured.  
**Fix:** Verify CORS middleware applied to all new routes.

### **P3-009: Inconsistent Authorization Pattern**
**Location:** `unrestrict_pod()` - line 205  
**Finding:** Pod control functions lack consistent permission checks:
```rust
async fn unrestrict_pod(State(state): State<Arc<AppState>>, Path(id): Path<String>
```
**Impact:** Any staff member can unrestrict pods for "maintenance."  
**Fix:** Require manager+ role for pod unrestriction operations.

### **P3-010: External HTTP Timeout Too High**
**Location:** `ollama.rs` - line 12  
**Finding:** 30-second timeout for AI diagnosis calls:
```rust
const TIMEOUT_SECS: u64 = 30;
```
**Impact:** Request queue buildup during Ollama outages.  
**Fix:** Reduce timeout to 10 seconds, implement circuit breaker pattern.

## 📊 SUMMARY BY CATEGORY

| Category | P1 | P2 | P3 | Total |
|----------|----|----|----| ------|
| AUTH/AUTHZ | 2 | 0 | 1 | 3 |
| INPUT VALIDATION | 0 | 1 | 0 | 1 |
| EXTERNAL CALLS | 1 | 0 | 1 | 2 |
| DATA EXPOSURE | 0 | 0 | 1 | 1 |
| RATE LIMITING | 0 | 1 | 0 | 1 |
| SQL INJECTION | 0 | 1 | 0 | 1 |
| CORS/HEADERS | 0 | 0 | 1 | 1 |

**Total: 3 Critical, 3 High, 4 Medium**

## 🚨 IMMEDIATE ACTIONS REQUIRED

1. **Block unauthenticated access** to `/maintenance/*` and `/analytics/*` endpoints
2. **Move HR endpoints** to manager+ role-gated section  
3. **Configure Ollama URL** via environment variables, not hardcoded IPs
4. **Add rate limiting** to all staff routes
5. **Implement input validation** on all new JSON endpoints