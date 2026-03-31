Below is a focused v29.0 security audit of the **new maintenance / analytics / HR / mesh-intelligence / camera-health / Ollama-related surface** based on the code excerpts provided.

I’ve grouped findings by severity and mapped them to your requested categories:
- SQL injection
- Auth/Authz
- Input validation
- CORS/headers
- External calls
- Data exposure
- Rate limiting

---

# Executive summary

## Highest-risk issues
1. **P1: Sensitive HR data exposure risk** on staff routes that appear only protected by a **permissive staff JWT middleware** rather than explicit role-gating.
2. **P1: Public operational intelligence exposure** via `/mesh/incidents`, `/mesh/stats`, `/mesh/solutions`, `/cameras/health`, and kiosk/POS operational endpoints.
3. **P2: Missing input validation** on new maintenance task/event creation and patch flows; likely abuse/DoS/data-quality issues.
4. **P2: No visible rate limiting** on maintenance/analytics/mesh/HR endpoints; likely abuseable by authenticated low-privilege users and some public callers.
5. **P2: Ollama error handling may leak internal infrastructure details** if upstream errors are surfaced to clients; timeout exists, but failure text contains internal model/service detail.
6. **P3: No SQL injection evidence in provided maintenance_store queries**, but several dynamic-filter functions should be confirmed for raw SQL construction since line list alone is insufficient.

---

# Findings

## 1) P1 — HR endpoints likely under-protected; sensitive employee data exposure
**Category:** AUTH/AUTHZ, DATA EXPOSURE  
**Endpoint/file:line:**  
- `staff_routes(...)` route registrations:
  - `/hr/recognition`  
  - `/hr/sjts`
  - `/hr/sjts/{id}`
  - `/hr/job-preview`
  - `/hr/campaign-templates`
  - `/hr/nudge-templates`
- `hr_recognition_data` handler reference at `INPUT HANDLING 11329`
- `AUTH MIDDLEWARE`: `staff_routes(state)` described as using **permissive JWT middleware (logs warnings)**

### Description
The new HR endpoints are mounted in the main `staff_routes(...)` block, not in the explicitly role-gated `manager+` or `superadmin-only` section.

Given your own inline comment:

> `staff_routes(state) -- staff/admin routes with permissive JWT middleware (logs warnings)`

this is a major concern. HR data is typically not appropriate for all staff. Even if `/hr/sjts`, `/hr/job-preview`, `/hr/campaign-templates`, and `/hr/nudge-templates` are low-sensitivity content, `/hr/recognition` strongly suggests employee performance/recognition data. Combined with the employee/payroll SQL shown in `maintenance_store`, there is a real risk of exposing:
- employee names
- roles
- skills
- phone numbers
- hourly rates / payroll data
- attendance / recognition / performance metadata

If “permissive JWT” means requests may proceed despite missing/weakly validated staff claims, this becomes potentially critical.

### Fix
- Move HR endpoints into explicit role-gated routers:
  - `manager+` for operational HR views
  - `superadmin` or dedicated `hr_admin` role for PII/payroll/phone/rate access
- Apply **hard auth middleware**, not warning-only middleware.
- Split HR endpoints by sensitivity:
  - Public-ish/internal content templates: maybe `staff`
  - Recognition/performance/employee directory: `manager+`
  - Payroll/phone/hourly_rate/attendance/face enrollment: `hr_admin` or `superadmin`
- Add field-level response filtering to exclude:
  - phone
  - hourly_rate_paise
  - face_enrollment_id
  - detailed attendance/payroll unless explicitly authorized

---

## 2) P1 — Public mesh intelligence and incident endpoints expose internal operational data
**Category:** AUTH/AUTHZ, DATA EXPOSURE  
**Endpoint/file:line:**  
In `public_routes()`:
- `/mesh/solutions`
- `/mesh/solutions/{id}`
- `/mesh/incidents`
- `/mesh/stats`
- `/cameras/health`

### Description
These are explicitly public. “Read-only for dashboard” is not a sufficient reason to make them internet/public-LAN accessible.

Risks:
- `/mesh/incidents` may expose outage patterns, failure reasons, maintenance issues, pod IDs, internal topology, and operational weaknesses.
- `/mesh/stats` may expose fleet health, reliability issues, downtime trends, and capacity information.
- `/mesh/solutions` may expose internal troubleshooting knowledge or procedures useful to attackers.
- `/cameras/health` leaks existence/status of internal camera infrastructure and acts as a network oracle for an internal asset (`192.168.31.27:1984`).

This is especially problematic in a venue environment where unauthenticated users may be on the same network or can query kiosk-facing APIs.

### Fix
- Move these endpoints to authenticated staff routes.
- If dashboards/kiosks need them, use:
  - service JWT/API key
  - kiosk-scoped token
  - IP allowlisting + auth
- For `/cameras/health`, expose only coarse status through internal admin endpoints; do not leave it public.
- For `/mesh/*`, return redacted summaries if any public usage is truly required.

---

## 3) P1 — Staff route auth model described as “permissive”; unsafe for write endpoints
**Category:** AUTH/AUTHZ  
**Endpoint/file:line:**  
`AUTH MIDDLEWARE` comment for `staff_routes(state)`

New write endpoints within staff routes include:
- `/maintenance/events` POST
- `/maintenance/tasks` POST
- `/maintenance/tasks/{id}` PATCH
- `/mesh/solutions/{id}/promote` POST
- `/mesh/solutions/{id}/retire` POST
- `/staff/...` many endpoints
- `/pods/{id}/clear-maintenance` POST

### Description
A “permissive JWT middleware (logs warnings)” is not suitable for privileged operational write actions. If middleware only logs JWT problems but still allows requests to reach handlers, attackers may be able to invoke maintenance actions, alter task state, manipulate mesh knowledge, or clear maintenance conditions.

Even if handlers do some in-handler checks elsewhere, the architectural note indicates the route grouping itself is not strongly enforced.

### Fix
- Replace permissive middleware with strict middleware:
  - reject missing/invalid JWT
  - reject expired tokens
  - enforce signature/audience/issuer
- Add role checks per endpoint:
  - `maintenance/events`, `maintenance/tasks`: staff or technician
  - `maintenance/tasks/{id}` patch: staff or technician
  - `mesh promote/retire`: manager+ or reliability_admin
  - `clear-maintenance`: technician/manager only
- Add audit logging with actor identity and role for every state-changing call.

---

## 4) P2 — No visible input validation on maintenance event/task creation
**Category:** INPUT VALIDATION  
**Endpoint/file:line:**  
- `1478: Json(event): Json<crate::maintenance_models::MaintenanceEvent>`
- `1525: Json(task): Json<crate::maintenance_models::MaintenanceTask>`
- `1542: Query(params): Query<MaintenanceTaskQuery>`
- `1489: Query(params): Query<MaintenanceEventQuery>`
- `maintenance_update_task` route via PATCH

### Description
The new maintenance handlers accept deserialized structs directly, but no validation is shown:
- no string length limits
- no enum/type restrictions shown
- no numeric range checks
- no pagination/limit caps shown
- no body size protections shown

Common risks:
- oversized descriptions/notes causing DB bloat or log flooding
- invalid status transitions
- invalid `pod_id` values
- negative or unreasonable durations/priorities
- arbitrary strings later reflected into dashboards/logs/LLM prompts
- query endpoints with unbounded date ranges or limits causing expensive scans

### Fix
Use explicit validated request DTOs instead of binding DB/domain structs directly:
- `title`: 1..128 chars
- `description`: max 2k or 4k
- `status`: enum
- `priority`: bounded enum/int
- `pod_id`: constrained to valid fleet range
- `component`: allowlist/enum if possible
- `limit`: clamp, e.g. `1..100`
- date range: max window, e.g. 31 or 90 days
- patch updates: validate allowed state transitions

In Rust/Axum:
- use `validator` crate or custom validation
- reject unknown fields if possible
- add request body size limits at router/layer level

---

## 5) P2 — Likely IDOR/over-broad staff access on maintenance and analytics endpoints
**Category:** AUTH/AUTHZ  
**Endpoint/file:line:**  
- `/maintenance/events`
- `/maintenance/summary`
- `/maintenance/tasks`
- `/maintenance/tasks/{id}`
- `/analytics/telemetry`
- `/analytics/trends`

### Description
These are in generic staff routes, with no visible role segregation. Operational telemetry and maintenance history may reveal:
- infrastructure failures
- pod reliability
- internal operations and staffing patterns
- incident frequency and root causes

Not all staff need all operational analytics. Cashiers/front-desk roles likely should not modify maintenance tasks or inspect deep telemetry.

### Fix
Suggested minimum access model:
- `/maintenance/summary`: staff read-only or technician+
- `/maintenance/events` GET: technician+/manager
- `/maintenance/events` POST: technician+
- `/maintenance/tasks` GET: technician+/manager
- `/maintenance/tasks` POST/PATCH: technician+/manager
- `/analytics/telemetry`: manager+/ops
- `/analytics/trends`: manager+/ops

Implement dedicated middleware/extractor requiring role claims.

---

## 6) P2 — No visible rate limiting on new maintenance/analytics/hr/mesh endpoints
**Category:** RATE LIMITING  
**Endpoint/file:line:**  
Applies to:
- `/maintenance/*`
- `/analytics/*`
- `/hr/*`
- `/mesh/*`
- `/cameras/health`
- `/kiosk/ping`
- `/pos/lockdown`
- `/pods/{id}/availability`

### Description
Only auth endpoints are visibly rate-limited. New operational endpoints do not show any limit layers.

Risks:
- brute-force enumeration of IDs and states
- flooding maintenance/task creation
- scraping HR/mesh/internal data
- repeated polling of `/cameras/health`, `/pos/lockdown`, `/pods/{id}/availability`
- DB/resource exhaustion through expensive analytics/trend queries
- LLM-adjacent endpoints (if analytics/maintenance trigger Ollama indirectly) becoming costly

### Fix
Add layered rate limits by endpoint class:
- public operational polling endpoints: low per-IP limits with burst
- authenticated staff reads: moderate per-user + per-IP
- authenticated writes: stricter per-user
- analytics/trends: strict due to DB cost
- any Ollama-backed route: very strict, concurrency-limited, with circuit breaker

Also add pagination and query window caps.

---

## 7) P2 — Potential sensitive internal error leakage from Ollama failures
**Category:** EXTERNAL CALLS, DATA EXPOSURE  
**Endpoint/file:line:**  
Ollama client:
- `diagnose(prompt: &str) -> anyhow::Result<String>`
- error construction:
  - `Err(anyhow::anyhow!("Ollama unavailable: primary={}, fallback={}", e, e2))`

### Description
The Ollama client includes solid basic timeouts, but the returned error string embeds:
- internal model names (`qwen2.5:3b`, `llama3.1:8b`)
- lower-level reqwest/HTTP error detail
- availability and behavior of internal host/service

If this `anyhow` error is passed through to API responses or logs visible to clients/admins with broad access, it leaks infrastructure details and assists attackers.

### Fix
- Return generic client-facing errors:
  - `"AI diagnosis service unavailable"`
- Keep detailed upstream errors only in server logs.
- Use structured logging with sanitized messages.
- Avoid embedding internal IP/stack details in user-visible responses.

---

## 8) P2 — Ollama prompt input likely unbounded; DoS and prompt/log injection risk
**Category:** INPUT VALIDATION, EXTERNAL CALLS, RATE LIMITING  
**Endpoint/file:line:**  
Ollama client:
- `diagnose(prompt: &str)`
- request body includes raw `prompt.to_string()`

### Description
The prompt is accepted as arbitrary string and posted upstream. No length cap or content restrictions are shown.

Risks:
- very large prompts causing memory/CPU pressure
- long LLM processing times even with timeout
- prompt injection if prompt is built from untrusted maintenance/task/user text
- reflected/stored malicious content entering logs, dashboards, or knowledge base outputs

### Fix
- enforce max prompt length, e.g. 2k–8k depending on use case
- sanitize/normalize embedded user-generated fields before composing prompts
- concurrency-limit LLM calls
- add per-user/per-route rate limits
- if responses are stored/displayed, HTML-escape on frontend and consider output length caps

---

## 9) P2 — Camera health proxy is an internal network oracle
**Category:** EXTERNAL CALLS, DATA EXPOSURE  
**Endpoint/file:line:**  
- `public_routes() -> .route("/cameras/health", get(cameras_health_proxy))`
- `cameras_health_proxy()`

### Description
This endpoint lets anyone query whether an internal service at `192.168.31.27:1984` is reachable. While the target is hardcoded, so classic SSRF is not present, the endpoint still:
- discloses internal host existence
- discloses service liveness
- can be abused for repeated low-cost probing against an internal dependency
- may help attackers time disruption or identify management infrastructure

### Fix
- move to staff/service-only route
- cache result server-side for short TTL to avoid repeated internal probes
- return generic health category instead of service name
- add rate limiting

---

## 10) P2 — Public POS/kiosk operational endpoints may leak venue state and support abuse
**Category:** AUTH/AUTHZ, DATA EXPOSURE, RATE LIMITING  
**Endpoint/file:line:**  
Public routes:
- `/pos/lockdown` GET
- `/kiosk/ping` POST
- `/pods/{id}/availability` GET

### Description
These are intentionally unauthenticated for operational reasons, but they expose live state:
- whether POS is locked down
- kiosk heartbeat behavior
- pod availability by ID

Risks:
- attackers can enumerate pod IDs and infer occupancy/maintenance patterns
- scraping business utilization data
- spoofing or flooding `kiosk/ping`
- operational interference if downstream systems trust heartbeat freshness without auth

### Fix
- require signed device token or shared service secret for machine-to-server routes
- at minimum, add source/IP allowlisting for internal devices
- rate limit aggressively
- for pod availability, expose only what kiosk truly needs, and consider opaque kiosk-scoped identifiers instead of direct pod IDs

---

## 11) P3 — No direct SQL injection evidence in provided `maintenance_store` query list
**Category:** SQL INJECTION  
**Endpoint/file:line:**  
`maintenance_store` query references:
- multiple `sqlx::query(...)`
- `sqlx::query_as(...)`
- examples shown:
  - `UPDATE employees SET name = ?1 WHERE id = ?2`
  - `UPDATE employees SET role = ?1 WHERE id = ?2`
  - etc.

### Description
From the snippets provided, the SQL usage appears parameterized with SQLite placeholders (`?1`, `?2`) and `sqlx` APIs. That is good.

However, because only line references were supplied for most functions, I cannot fully verify whether any of these functions build SQL dynamically before passing to `sqlx::query(...)`, especially around:
- list/filter handlers
- analytics/trends queries
- optional sorting
- search conditions

The visible examples do **not** indicate injection.

### Fix
- Confirm that all query strings are static literals.
- If any dynamic filtering/sorting exists:
  - never interpolate user strings into SQL
  - use allowlisted sort columns/directions
  - use conditional query builders with bound params only
- Prefer `sqlx::QueryBuilder` with trusted fragments only when dynamic composition is unavoidable.

**Status:** No confirmed SQL injection in provided excerpt; review needed for dynamic list/filter functions.

---

## 12) P3 — Employee table contains highly sensitive fields; ensure endpoint redaction
**Category:** DATA EXPOSURE  
**Endpoint/file:line:**  
`maintenance_store` references:
- employee fields updated:
  - `hourly_rate_paise`
  - `phone`
  - `face_enrollment_id`
  - `skills`
  - `role`
- payroll/attendance query references around lines:
  - `1051`, `1063`, `1074`, `1085`, `1148`

### Description
The data model clearly includes sensitive HR/biometric-adjacent fields. Even though endpoint implementations weren’t fully shown, any endpoint serializing `EmployeeRow` directly risks exposing:
- compensation
- phone numbers
- face enrollment identifiers
- attendance/payroll details

This is a strong red flag especially with the loosely described staff auth model.

### Fix
- never return DB rows directly
- create separate response DTOs:
  - `EmployeePublicInternalView`
  - `EmployeeManagerView`
  - `EmployeeHRPrivateView`
- exclude `face_enrollment_id` from API responses unless absolutely necessary
- mask phones unless role explicitly requires full value
- treat hourly rate and payroll as HR/manager-only

---

## 13) P3 — Missing explicit security header/CORS review for newly added public endpoints
**Category:** CORS/HEADERS  
**Endpoint/file:line:**  
Applies globally, especially to newly public endpoints:
- `/mesh/*`
- `/cameras/health`
- `/pos/lockdown`
- `/kiosk/ping`
- `/pods/{id}/availability`

### Description
No CORS or security-header configuration is shown in the provided snippet. This may already exist elsewhere, but from the provided code we cannot confirm:
- restrictive `Access-Control-Allow-Origin`
- `Cache-Control: no-store` for sensitive operational data
- `X-Content-Type-Options: nosniff`
- `Content-Security-Policy` for any browser-served responses
- `Referrer-Policy`
- `X-Frame-Options` / `frame-ancestors`
- `Permissions-Policy`

For JSON APIs, the biggest concerns are:
- overly broad CORS allowing browser exfil from authenticated contexts
- cacheability of sensitive responses
- missing no-store on operational/HR endpoints

### Fix
- verify and enforce a global CORS layer with explicit allowed origins
- set `Cache-Control: no-store` on:
  - HR
  - analytics
  - maintenance
  - billing/admin
  - mesh incidents/stats
- ensure standard security headers are applied globally

**Status:** Potential issue; configuration not shown.

---

# Endpoint protection recommendations

## Public
These may remain public only if business-critical and responses are minimal:
- `/health`
- `/venue`
- public leaderboard/customer registration paths

## Should NOT be public
Move to authenticated staff/service:
- `/mesh/solutions`
- `/mesh/solutions/{id}`
- `/mesh/incidents`
- `/mesh/stats`
- `/cameras/health`
- likely `/pods/{id}/availability` unless kiosk-scoped and protected

## Device/service-auth only
Not browser-public:
- `/kiosk/ping`
- `/pos/lockdown` if used by POS/kiosk agent
- `/billing/{id}/agent-shutdown`
- `/billing/pod/{pod_id}/interrupted`

## Staff-only
Basic venue operations:
- `/maintenance/summary`
- maybe limited `/maintenance/events` read

## Technician or manager+
- `/maintenance/events` POST
- `/maintenance/tasks` GET/POST
- `/maintenance/tasks/{id}` PATCH
- `/pods/{id}/clear-maintenance`
- mesh promote/retire

## Manager+/HR-admin only
- `/analytics/telemetry`
- `/analytics/trends`
- `/hr/recognition`
- any employee/attendance/payroll/phone/rate endpoints

---

# SQL injection conclusion

## Confirmed
- **No confirmed SQL injection** in the provided excerpt.

## Residual concern
Because only line references were supplied for most `maintenance_store` functions, review these patterns specifically:
- optional filters in list endpoints
- sort/order parameters
- analytics date-range/group-by logic
- employee search/list functions

If all are static SQL + bind params, this category is likely clean.

---

# Prioritized remediation plan

## Immediate
1. **Lock down HR endpoints** to manager+/HR-admin.
2. **Remove public access** to mesh and camera health endpoints.
3. **Replace permissive staff middleware** with strict enforcement.
4. **Add validation** to maintenance create/update/query inputs.
5. **Add rate limiting** to maintenance/analytics/public operational endpoints.

## Next
6. Sanitize Ollama-facing errors and cap prompt length.
7. Redact employee/payroll/phone/face-enrollment fields from API responses.
8. Add no-store/cache and verify CORS/header policy.

---

# Compact findings table

| Severity | Endpoint/file:line | Description | Fix |
|---|---|---|---|
| P1 | `staff_routes` HR routes; `hr_recognition_data` | HR endpoints likely accessible under permissive staff auth; potential employee PII/pay/performance exposure | Move to manager+/HR-admin, strict JWT, response redaction |
| P1 | `public_routes` `/mesh/*`, `/cameras/health` | Public exposure of internal incidents, stats, solutions, camera service health | Move to staff/service auth, redact responses |
| P1 | `AUTH MIDDLEWARE` comment on `staff_routes` | “Permissive JWT middleware” unsafe for privileged writes | Enforce strict JWT + role checks |
| P2 | Maintenance handlers `1478/1489/1525/1542` | Missing validation on create/list/update inputs | Add DTO validation, body limits, bounded query params |
| P2 | `/maintenance/*`, `/analytics/*` | Likely over-broad staff access to ops data/actions | Split by role: technician/manager/ops |
| P2 | Most new endpoints | No visible rate limiting beyond auth endpoints | Add rate limiting per IP/user/route class |
| P2 | Ollama client `diagnose()` | Upstream/internal error detail may leak to clients | Return generic errors, detailed logs only |
| P2 | Ollama `prompt` handling | No visible prompt length/content controls; DoS risk | Cap length, sanitize inputs, limit concurrency |
| P2 | `/cameras/health` | Internal network oracle / dependency probe | Restrict auth, cache, rate limit |
| P2 | `/pos/lockdown`, `/kiosk/ping`, `/pods/{id}/availability` | Public operational state leakage and abuse surface | Require device auth or allowlist, rate limit |
| P3 | `maintenance_store` SQL refs | No direct SQLi evidence; dynamic query construction not fully visible | Verify all SQL strings static or allowlisted QueryBuilder |
| P3 | Employee/payroll SQL refs | Sensitive fields exist and may be overexposed via API DTOs | Use redacted response models and role-based field access |
| P3 | Global headers/CORS not shown | New endpoints may miss restrictive CORS/no-store/security headers | Verify global middleware and add no-store for sensitive APIs |

If you want, I can turn this into a **developer-ready remediation checklist** or a **CSV/Jira-style finding list with owners and test cases**.