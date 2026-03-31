**To**: Racing Point eSports Engineering Lead
**From**: Senior Application Security Auditor
**Date**: [Current Date]
**Subject**: Security Audit Report for Meshed Intelligence v29.0

This report contains the findings from a full security audit of the Meshed Intelligence application, version 29.0. The audit focused on the new maintenance, analytics, and HR endpoints, as well as related changes to the codebase. The stack audited is Rust/Axum with an SQLite backend, deployed on Windows.

The overall security posture is strong, benefiting significantly from Rust's memory safety and the `sqlx` library's robust protections against SQL injection. However, several findings related to authorization, data exposure, and rate limiting require attention.

---

### **FULL SECURITY AUDIT - MESHED INTELLIGENCE v29.0**

### 1. SQL INJECTION

**Analysis Summary**: A review of all listed SQL queries in `maintenance_store` was conducted. The application consistently and correctly uses `sqlx`'s parameterized query functions (`sqlx::query`, `sqlx::query_as`, `sqlx::query_scalar`). This practice effectively mitigates the risk of SQL injection by separating SQL logic from user-provided data. No instances of dynamic SQL string concatenation with user input were found in the provided code.

**Findings**:
*   **None.** The use of `sqlx` with compile-time checked queries and parameterized inputs across the `maintenance_store` module is exemplary and follows security best practices.

---

### 2. AUTHENTICATION & AUTHORIZATION (AUTH/AUTHZ)

**Analysis Summary**: The API routing is well-structured into public, customer, and staff tiers. The new v29.0 endpoints for maintenance and analytics have been correctly placed within the `staff_routes`, requiring a staff JWT. However, there is a lack of granular, role-based access control within the general staff population for these new, potentially sensitive endpoints.

**Findings**:

*   **P2, Missing Function-Level Access Control**
    *   **Endpoint**: `/analytics/telemetry`, `/analytics/trends`, `/maintenance/summary`, `/maintenance/events`, `/maintenance/tasks`
    *   **Description**: The new maintenance and analytics endpoints are accessible to any user with a valid staff JWT. This includes roles like `cashier`. These endpoints provide deep operational insights (telemetry trends, maintenance patterns) and control over maintenance records, which should be restricted based on the principle of least privilege. A junior staff member does not require access to this level of system-wide analytics or have the ability to create/modify maintenance tasks.
    *   **Fix**: Move these new routes from the general `staff_routes` group into the role-gated `Manager+` sub-router which is protected by `require_role_manager` middleware. This ensures only leadership and senior technical staff can access these sensitive operational endpoints.

---

### 3. INPUT VALIDATION

**Analysis Summary**: The new POST/PATCH endpoints use `serde` to deserialize request bodies into strongly typed Rust structs. While this prevents basic type-mismatch errors, there is no apparent application-level validation on the content of these structs' fields (e.g., string length, value ranges).

**Findings**:

*   **P3, Missing Server-Side Content Validation**
    *   **Endpoint/File:Line**: `/maintenance/events` (POST), `/maintenance/tasks` (POST), `/maintenance/tasks/{id}` (PATCH)
    *   **Description**: The handlers for creating and updating maintenance events and tasks deserialize directly into `MaintenanceEvent` and `MaintenanceTask` structs. The application does not validate the content of the fields within these structs. This could allow for the submission of excessively long strings (potentially causing UI issues or minor DoS), invalid enum values if represented as strings, or illogical data, which could corrupt application logic or data integrity.
    *   **Fix**: Implement validation on the `MaintenanceEvent` and `MaintenanceTask` models. Use a crate like `validator` to add attributes to the struct fields (e.g., `#[validate(length(min = 1, max = 255))]`). Call the `validate()` method on the deserialized object at the beginning of each handler and return a `400 Bad Request` if validation fails.

---

### 4. CORS/HEADERS

**Analysis Summary**: The provided routing files do not show the top-level application configuration where global middleware, such as `CorsLayer` and security header providers (e.g., for `Content-Security-Policy`, `X-Content-Type-Options`), are typically applied. Assuming these are applied globally to the Axum app, new routes will inherit them automatically. No per-route configuration omissions were identified.

**Findings**:
*   **None.** No evidence suggests new endpoints are missing headers. It is recommended, however, to quickly confirm that security headers and CORS policies are applied as a global `Layer` to the top-level application `Router` to ensure consistent coverage for all current and future routes.

---

### 5. EXTERNAL CALLS

**Analysis Summary**: The new `ollama_client` module makes external HTTP calls to a local AI service. The implementation correctly handles timeouts and prevents Server-Side Request Forgery (SSRF) by using a hardcoded URL. However, it leaks detailed internal error messages.

**Findings**:

*   **P3, Information Disclosure via Error Messages**
    *   **Endpoint/File:Line**: `ollama_client.rs:50`
    *   **Description**: The `diagnose` function, upon failure of both the primary and fallback models, constructs and returns an `anyhow` error containing the full error text from the underlying `reqwest` calls: `Err(anyhow::anyhow!("Ollama unavailable: primary={}, fallback={}", e, e2))`. If the API handler that calls `diagnose()` propagates this error to the client, it will leak internal network details (e.g., "error trying to connect: connection refused: 192.168.31.27:11434").
    *   **Fix**: The `diagnose` function should log the detailed error for internal debugging but return a generic, opaque error to its caller. The calling API handler should also ensure it returns a generic error message to the end-user.
        ```rust
        // In ollama_client.rs
        Err(e2) => {
            tracing::error!(target: LOG_TARGET, primary_error = %e, fallback_error = %e2, "Both Ollama models failed");
            // Return a generic error to the caller
            Err(anyhow::anyhow!("AI diagnosis service is unavailable"))
        }
        ```

---

### 6. DATA EXPOSURE

**Analysis Summary**: A review of the available routes and database schema information (inferred from SQL `UPDATE` statements) reveals a high risk of sensitive employee PII and financial data exposure through staff-facing gamification and HR endpoints.

**Findings**:

*   **P1, Sensitive Employee PII and Salary Exposure**
    *   **Endpoint**: `/hr/recognition`, `/staff/gamification/leaderboard`, `/staff/{id}/badges`
    *   **Description**: The `employees` table contains highly sensitive fields like `phone` and `hourly_rate_paise` (salary). Endpoints related to staff gamification and recognition (e.g., `/staff/gamification/leaderboard`) are likely to query and serialize the `EmployeeRow` model directly. If this model maps 1:1 with the database table, these endpoints will leak the private phone numbers and salary details of all employees to any staff member who can access them, creating a significant privacy and security incident.
    *   **Fix**: Immediately create and use dedicated Data Transfer Objects (DTOs) or ViewModels for these endpoints. For example, create a `StaffLeaderboardEntry` struct containing only non-sensitive fields like `id`, `name`, and `points`. The handler should transform the full `EmployeeRow` from the database into this safe DTO before serialization and response. **Never serialize an internal data model containing sensitive PII directly to an API client.**

*   **P3, Internal Network Topology Disclosure**
    *   **Endpoint**: `/cameras/health`
    *   **Description**: This public endpoint acts as a proxy to an internal service. In a failure case, its response `{"status":"down", "service":"go2rtc"}` discloses the name of an internal software component. While low risk, it provides unnecessary information about the internal architecture to the public, which can aid an attacker in mapping the network.
    *   **Fix**: Generalize the error response to not include specific service names. For example: `(StatusCode::SERVICE_UNAVAILABLE, Json(json!({"status": "degraded", "component": "camera_system"})))`.

---

### 7. RATE LIMITING

**Analysis Summary**: The application correctly applies rate limiting to sensitive authentication endpoints. However, this protection is not extended to the general authenticated `staff_routes`, leaving resource-intensive and database-write endpoints vulnerable to abuse.

**Findings**:

*   **P2, Missing Rate Limiting on Authenticated Endpoints**
    *   **Endpoint**: All new v29.0 endpoints under `staff_routes`: `/maintenance/*`, `/analytics/*`.
    *   **Description**: The staff-only endpoints are not rate-limited. An authenticated but malicious user, or a buggy client-side script, could send a high volume of requests to these endpoints. This could lead to a denial-of-service condition by overwhelming the database with complex queries (e.g., `GET /analytics/trends`) or filling tables with junk data (e.g., `POST /maintenance/events`).
    *   **Fix**: Apply a defense-in-depth rate-limiting layer to the `staff_routes` router. A reasonable limit, such as 100 requests per minute per user or IP, would prevent abuse while not impacting normal staff operations. This can be achieved with `tower_governor` or a similar library applied to the router group.

---
**END OF REPORT**