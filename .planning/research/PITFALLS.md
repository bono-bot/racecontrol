# Pitfalls Research

**Domain:** Security hardening for a live eSports cafe operations stack (Rust/Axum server, React/TS kiosk PWA, 8-pod fleet, Linux VPS)
**Researched:** 2026-03-20
**Confidence:** HIGH for operational and API pitfalls (well-documented domain), MEDIUM for India-specific PII compliance (DPDP Act still maturing)

---

## Critical Pitfalls

### Pitfall 1: Big-Bang Auth Rollout Bricks the Pod Fleet

**What goes wrong:**
Server-side API authentication is enabled on racecontrol (:8080) in a single deploy. All 8 rc-agent pods and the kiosk PWA (:3300) are still sending unauthenticated requests. Every pod instantly fails health checks, billing calls, and session management. The cafe is down during peak hours until every pod binary and the PWA are rebuilt and redeployed -- which requires the fleet exec system that is itself now broken by the auth change.

**Why it happens:**
The natural instinct is "flip the switch" -- add an auth middleware to the Axum router and deploy. In a monolith this works. In a distributed fleet where the server, 8 agents, and a PWA are deployed independently, the server and clients are never updated atomically. The deploy tool (fleet exec via racecontrol) itself goes through the API that just broke.

**How to avoid:**
Implement auth in three discrete steps, each independently deployable:
1. **Server accepts both** -- add auth middleware that checks for a token BUT allows unauthenticated requests through (log a warning). Deploy server. Verify all pods still work.
2. **Clients send tokens** -- update rc-agent and PWA to include the auth token in requests. Deploy agents pod-by-pod (Pod 8 first per deploy rules). Verify each pod authenticates successfully.
3. **Server rejects unauthenticated** -- once all clients send tokens AND logs show zero unauthenticated requests for 24h, flip the middleware to reject. This is the only breaking change and by now nothing should break.

This is the "expand-migrate-contract" pattern. Each step is independently reversible.

**Warning signs:**
- Planning doc says "add auth middleware" as a single task rather than three
- No mention of a "dual mode" or "grace period" for the auth middleware
- Deploy plan does not sequence server-before-clients or mention Pod 8 canary

**Phase to address:**
Phase 1 (API Authentication) -- this must be the explicit structure of the phase, not a single requirement.

---

### Pitfall 2: Shared Secret Hardcoded in Source or Config Files

**What goes wrong:**
The API token or HMAC secret is placed in `racecontrol.toml`, committed to git, or baked into the rc-agent binary. Anyone with repo access (including CI artifacts, deploy-staging directory, or the pendrive at `D:\pod-deploy\`) can extract the secret. Since the repo is on James's workstation and synced to GitHub, the secret is effectively public to anyone with repo access.

**Why it happens:**
For a LAN-only system with a single developer, putting the secret in the config file feels "good enough." The threat model underestimates insider access -- but the config files are on every pod, the server, the POS PC, and the pendrive. That is 12+ copies of the secret across devices that customers physically sit in front of.

**How to avoid:**
- Store secrets in environment variables, not config files. On Windows pods: set via `setx` in the install script, read via `std::env::var("RACECONTROL_API_SECRET")` at startup.
- The `.bat` start scripts (`start-racecontrol.bat`, `start-rcagent.bat`) should reference the env var, not contain the value.
- Add `*secret*`, `*token*`, `*key*` patterns to `.gitignore` as a safety net.
- For the kiosk PWA: the token is inevitably in the browser (it must be sent in requests). Accept this -- the kiosk token grants kiosk-level permissions only, not admin. Separate the admin token (Uday's PIN) from the kiosk API token.

**Warning signs:**
- `grep -r "secret\|token\|api_key" *.toml` finds plaintext values
- The deploy pendrive contains a file with the secret in cleartext
- rc-agent.toml on any pod contains authentication credentials

**Phase to address:**
Phase 1 (API Authentication) -- secret management must be designed before the first token is issued.

---

### Pitfall 3: Admin PIN Stored as Plaintext in Config or Database

**What goes wrong:**
Uday's admin PIN is stored as `admin_pin = "1234"` in racecontrol.toml or as a plaintext column in SQLite. Anyone who can read the config file or database (which includes anyone with shell access to the server, or anyone who exploits the currently-unauthenticated API to read files) can bypass admin authentication entirely.

**Why it happens:**
"It's just a PIN, not a password" reasoning. PINs feel too simple to hash. But the PIN protects billing manipulation and customer data access -- the highest-privilege operation in the system.

**How to avoid:**
- Hash the PIN with argon2 (use the `argon2` crate -- it is the current recommended password hashing algorithm for Rust). Store only the hash.
- On first setup: prompt Uday to set a PIN, hash it, store the hash. Never log the plaintext PIN.
- Rate-limit PIN attempts (5 failures = 5-minute lockout) to prevent brute-force on a 4-6 digit PIN.
- The admin panel (web dashboard :3200) must enforce the PIN check server-side, not just client-side. A `fetch('/api/admin/billing')` without the PIN header must return 401, regardless of what the React app does.

**Warning signs:**
- Config file contains `pin`, `password`, or `admin` fields with plaintext values
- Admin API endpoints return 200 when called without credentials from curl
- No rate-limiting on the admin auth endpoint

**Phase to address:**
Phase 2 (Admin Panel Protection) -- but the hashing approach should be decided in Phase 1 when the auth library is chosen.

---

### Pitfall 4: HTTPS Breaks WebSocket Connections to Pod Agents

**What goes wrong:**
HTTPS is enabled on racecontrol (:8080). The server now speaks TLS. But the WebSocket connections from rc-agent pods use `ws://192.168.31.23:8080/ws` (plain WS). The TLS handshake fails silently -- rc-agent sees "connection reset" and enters its reconnect loop. All 8 pods disconnect and cycle through reconnection attempts. Fleet management is blind.

The kiosk PWA has the same issue: if the PWA is served over HTTPS, browsers enforce that all WebSocket connections must also be `wss://` -- mixed content is blocked. The PWA silently fails to connect.

**Why it happens:**
HTTPS and WSS are treated as separate concerns but they share the same TLS listener. Enabling TLS on the HTTP port automatically requires TLS on the WebSocket path. This is not obvious when planning "add HTTPS" as a line item.

**How to avoid:**
- **LAN traffic stays HTTP/WS.** The pods and server are on a private 192.168.31.x network behind a router. TLS on the LAN adds complexity (self-signed certs, cert distribution to 8 pods) with minimal security benefit -- the threat is not network sniffing on a wired LAN, it is unauthenticated API access.
- **HTTPS only for external-facing traffic** -- the cloud API (app.racingpoint.cloud on Bono's VPS) and any public-facing endpoint. Bono's VPS already has a domain and can use Let's Encrypt.
- **If HTTPS on LAN is required:** use a reverse proxy (nginx/caddy on the server) that terminates TLS and forwards to racecontrol on localhost:8080. Pods connect to the proxy. This isolates the TLS concern from the application code.
- Do NOT attempt to add TLS directly to the Axum server while pods are connecting -- it is a breaking change with no grace period.

**Warning signs:**
- Requirements list says "HTTPS for all communication" without distinguishing LAN vs. external
- Self-signed certificate generation is planned for the server without a cert distribution plan for 8 pods
- WSS is not mentioned alongside HTTPS in the same requirement

**Phase to address:**
Phase 3 (Data in Transit) -- must explicitly scope LAN vs. external and decide before implementation.

---

### Pitfall 5: Kiosk Escape via Developer Tools, Hotkeys, or URL Bar

**What goes wrong:**
The kiosk PWA runs in a browser on pod machines. A tech-savvy customer presses F12 (DevTools), Ctrl+L (URL bar), Ctrl+Shift+I (inspector), Alt+Tab (task switch), or Win+R (run dialog) and escapes the kiosk. From there they can access the filesystem, open another browser tab to the admin panel, or curl the unauthenticated API directly. This is the most common attack vector in eSports cafes -- the customers are gamers who know keyboard shortcuts.

**Why it happens:**
The PWA is "fullscreen" but the browser is not in a true kiosk lockdown mode. Standard Chrome fullscreen (F11) does not disable DevTools, task switching, or keyboard shortcuts. The developer assumes fullscreen = locked, but it is not.

**How to avoid:**
- Use Chrome's `--kiosk` flag (already may be in use) but supplement with:
  - `--disable-dev-tools` (or `--auto-open-devtools-for-tabs` disabled)
  - Group Policy on Windows to disable Task Manager (Ctrl+Alt+Del), Run dialog (Win+R), and Explorer shell
  - A process monitor (rc-agent already has `kiosk.rs` with process scanning) that kills unauthorized processes (explorer.exe, cmd.exe, powershell.exe, taskmgr.exe) when a session is active
  - Disable USB mass storage via Group Policy (already noted as pending in CLAUDE.md) to prevent booting from USB
- The keyboard shortcut lockdown must happen at the OS level (Group Policy or a keyboard hook), not in the browser -- JavaScript cannot intercept Ctrl+Alt+Del or Win+key combinations.
- Test the lockdown by having someone actually try to escape. Automated tests cannot catch all escape vectors.

**Warning signs:**
- Kiosk launch script uses `--kiosk` but no `--disable-` flags
- No Group Policy or registry hardening on pod machines
- rc-agent's process allowlist has not been updated to kill escape tools
- No manual escape testing documented

**Phase to address:**
Phase 4 (Kiosk Hardening) -- but the process allowlist update in rc-agent should be coordinated with Phase 1 (API auth) so that even if a customer escapes the kiosk, API calls require authentication.

---

### Pitfall 6: PII Audit Finds Data in Unexpected Locations

**What goes wrong:**
The security audit discovers customer phone numbers, names, and payment details scattered across: SQLite databases on the server, log files (`RUST_LOG=debug` includes request bodies), the cloud sync to Bono's VPS, Discord/WhatsApp bot message history, session backup files, and possibly browser localStorage on pod machines. The audit was scoped to "check the database" but PII leaked into 6+ locations over months of development.

**Why it happens:**
PII spreads through systems like water through cracks. Every `debug!()` log statement that includes a request body, every cloud sync payload, every bot notification that says "Customer Rahul (9876543210) started session" creates a new copy. Developers focus on the primary storage (SQLite) and miss the secondary copies.

**How to avoid:**
- The security audit (listed as a requirement) must be a full-system PII trace, not just a database check. Grep for phone number patterns (`\d{10}`), email patterns, and name fields across:
  - All SQLite databases (server + cloud)
  - All log files and log configuration
  - Discord/WhatsApp bot message templates
  - Cloud sync payloads (what fields are sent to Bono's VPS?)
  - Browser localStorage/sessionStorage on pods
  - Backup files and deploy artifacts
- After the audit: establish a PII boundary. Define which components are allowed to hold PII (the server database, the admin panel) and which must never contain it (logs, bot messages, cloud sync of billing data). Enforce the boundary with code review rules.
- Replace PII in logs with redacted versions: phone `987***3210`, name `R***l`.

**Warning signs:**
- `grep -r "phone\|mobile\|email\|name" crates/` finds PII fields in log statements
- Bot messages include customer names or phone numbers
- Cloud sync payload definition includes PII fields
- No log redaction middleware in the Axum server

**Phase to address:**
Phase 0 (Security Audit) -- this must happen BEFORE any data protection work. You cannot protect what you have not found.

---

### Pitfall 7: Auth Tokens Have No Expiry or Rotation Mechanism

**What goes wrong:**
A static API token is generated once and embedded in all 8 pods, the PWA, and the server. It works forever. If the token is ever leaked (a customer reads it from a pod's environment, a backup is compromised, a dismissed staff member remembers it), there is no way to revoke it without simultaneously updating all 8 pods, the PWA, and the server -- which is the same big-bang problem from Pitfall 1.

**Why it happens:**
Token rotation adds complexity that feels unnecessary for a small LAN system. "We'll rotate it if it's compromised" -- but without a rotation mechanism built in, rotating under pressure means downtime.

**How to avoid:**
- Design token rotation from day one, even if you do not rotate frequently:
  - The server accepts tokens from a list (current + previous). This allows a rolling update where new tokens are deployed pod-by-pod while the old token still works.
  - Token storage on pods is in an environment variable (not compiled in), so rotation = update env var + restart rc-agent.
  - A `/admin/rotate-token` endpoint (behind admin PIN auth) generates a new token, adds it to the accept list, and returns it. Uday deploys it to pods. After 24h, the old token is removed from the accept list.
- For the MVP: even if you do not build the rotation endpoint, the "accept multiple tokens" pattern in the middleware costs almost nothing and makes future rotation possible without downtime.

**Warning signs:**
- Token validation is `if token == THE_TOKEN` (single value) rather than `if VALID_TOKENS.contains(&token)`
- No documented procedure for "what to do if the API token is compromised"
- Token has been the same value for more than 90 days with no rotation

**Phase to address:**
Phase 1 (API Authentication) -- the multi-token acceptance pattern must be in the initial middleware design.

---

### Pitfall 8: Session Bypass via Direct Pod Communication

**What goes wrong:**
API auth is added to racecontrol (:8080). But rc-agent on each pod listens on :8090 for direct HTTP commands (`remote_ops.rs`). A customer who escapes the kiosk (Pitfall 5) can `curl http://localhost:8090/exec -d '{"cmd":"start game.exe"}'` to launch a game directly on the pod, bypassing billing entirely. The auth on the central server is irrelevant -- the pod agent accepts commands locally.

**Why it happens:**
Security hardening focuses on the "front door" (the central API) and forgets that every pod has its own API listener. rc-agent's remote_ops endpoint was designed for fleet management from the server, but it listens on `0.0.0.0:8090` -- accessible from localhost on the pod itself.

**How to avoid:**
- rc-agent :8090 must also require authentication. The simplest approach: the same API token used for racecontrol, validated in the remote_ops handler.
- Alternatively: bind rc-agent's HTTP listener to the server's IP only (`--allowed-source 192.168.31.23`) and reject connections from other IPs. This is defense-in-depth -- auth + IP restriction.
- rc-sentry (:8091) has the same exposure. It was deliberately designed as "no auth, LAN-only" but if kiosk escape is a threat, local access to :8091 is also a threat.
- The rc-agent process allowlist in `kiosk.rs` should kill `curl.exe`, `powershell.exe`, and `cmd.exe` during active sessions -- but this is a secondary defense. Auth is primary.

**Warning signs:**
- `curl http://localhost:8090/exec` from a pod returns 200 without any token
- rc-agent binds to `0.0.0.0` instead of a specific interface
- Security requirements mention "API auth" without specifying which APIs (central only, or pod agents too)

**Phase to address:**
Phase 1 (API Authentication) -- pod agent auth must be in scope alongside central server auth. If deferred, there is a window where the central API is locked but every pod is wide open.

---

### Pitfall 9: SQLite Encryption Breaks Existing Queries and Tooling

**What goes wrong:**
SQLCipher or similar encryption is added to the customer database. All existing tooling that reads the database directly -- Uday's ad-hoc queries, backup scripts, the cloud sync process, any `sqlite3` CLI usage -- breaks because the database is now encrypted. The cloud sync to Bono's VPS fails silently (it reads the raw file, which is now ciphertext). Backups still run but produce encrypted files that cannot be restored without the key.

**Why it happens:**
Encryption at rest is treated as a database concern ("just swap the SQLite driver") without auditing everything that touches the database file. SQLCipher is a drop-in replacement for the SQLite library but NOT for any external tool or process that reads the raw .db file.

**How to avoid:**
- Before encrypting: inventory every process that reads or writes the database file. Include backup scripts, cloud sync, CLI tools, and any monitoring.
- Consider field-level encryption instead of full-database encryption for the MVP: encrypt only PII columns (phone, email, payment details) using application-level encryption (AES-256-GCM via the `aes-gcm` crate). The database remains a normal SQLite file, all tooling works, but PII is encrypted in the columns.
- If full-database encryption is chosen: migrate all external tools to use the SQLCipher-aware library. Test backup restore with the encryption key. Document the key storage location.
- Key management: the encryption key must NOT be in `racecontrol.toml` alongside the database. Use a separate environment variable or a key file with restricted permissions.

**Warning signs:**
- Cloud sync starts returning empty or garbled data after encryption is enabled
- `sqlite3 customer.db .tables` fails with "file is not a database"
- Backup files grow in size (encrypted) but no one has tested restoring them
- Encryption key is in the same config file as the database path

**Phase to address:**
Phase 5 (Data at Rest) -- must include a tool/process audit before any encryption work begins.

---

### Pitfall 10: Security Hardening Introduces Latency That Breaks Real-Time Billing

**What goes wrong:**
Auth middleware adds token validation to every API call. If validation involves a database lookup or crypto operation on every request, the billing endpoints (`/api/v1/billing/start`, `/api/v1/billing/end`) gain 5-50ms latency. The billing system is timing-sensitive -- sessions are billed by the second, and the 10-second idle threshold is checked via periodic API calls. Added latency causes billing inaccuracy or session timeout false positives.

**Why it happens:**
Security middleware is added as a global layer (`Router::layer`) without considering the performance profile of different endpoint types. A health check endpoint can tolerate 50ms auth overhead. A billing endpoint called every few seconds from 8 pods simultaneously cannot.

**How to avoid:**
- Use HMAC token validation (symmetric, no database lookup, sub-microsecond on modern hardware) rather than JWT with database-backed session validation.
- Profile the auth middleware latency before deploying to production. Target: < 1ms overhead per request.
- If using Axum's middleware system: apply auth selectively. Health/version endpoints can skip auth. Billing endpoints must have auth but with the fastest validation path.
- Load test with 8 concurrent pods making billing calls every 5 seconds. Measure p99 latency with and without auth middleware.

**Warning signs:**
- Auth middleware does a database query on every request (e.g., "is this token in the valid_tokens table?")
- Billing timing tests pass in dev (single pod) but fail under fleet load
- Session end times drift by seconds compared to pre-auth behavior

**Phase to address:**
Phase 1 (API Authentication) -- token validation method must be chosen with latency in mind from the start.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Static token with no expiry | Simple to implement, no rotation logic | Compromised token = full system access until manual rotation across 10+ devices | Only in Phase 1 MVP, must add rotation mechanism within 30 days |
| Client-side-only admin PIN check | Quick to add in React, no server changes | Anyone who opens DevTools bypasses the PIN | Never -- server-side validation is non-negotiable for admin auth |
| HTTPS on LAN via self-signed certs | Encrypts traffic without a CA | Every pod needs the cert, cert expiry breaks connections, browsers show warnings | Only if regulatory requirement demands it -- prefer HTTP on LAN + auth tokens |
| `#[allow(unused)]` on auth fields during dual-mode rollout | Silences warnings during the transition period | Forgotten dual-mode code stays in production | Acceptable for 2 weeks during rollout, remove after migration complete |
| Field-level encryption instead of full-database encryption | Existing tooling still works, simpler key management | Does not protect non-PII data, schema is visible | Acceptable for current scale -- full encryption is overkill for 8 pods |
| Shared API token (same for all pods) | One token to manage | Compromising any pod compromises all | Acceptable for MVP, but design for per-pod tokens eventually |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Axum auth middleware + WebSocket upgrade | Auth middleware rejects the WS upgrade request because it does not carry a Bearer token in headers (WS upgrades are HTTP requests) | Handle WS auth via a query parameter (`?token=X`) or the first WS message, not HTTP headers |
| rc-agent + new auth headers | rc-agent's HTTP client (reqwest) does not include auth headers in existing calls | Add a wrapper function `authenticated_request()` that injects the token; update all call sites |
| Cloud sync (Bono's VPS) + auth | Cloud sync pushes/pulls via HTTP to racecontrol -- adding auth breaks the sync if the VPS does not have the token | Include cloud sync in the auth rollout plan; Bono's VPS needs the token before server rejects unauthenticated |
| Discord/WhatsApp bot + session commands | Bot sends session launch commands via the API -- bot must authenticate too | Bot service account gets its own token; bot token should have limited scope (session ops only, not admin) |
| kiosk PWA + CORS | Adding auth headers to fetch requests triggers CORS preflight (OPTIONS) that the server may not handle | Ensure Axum CORS middleware allows the Authorization header; test from a real browser, not just curl |
| admin dashboard (:3200) + same auth | Dashboard runs on same server as racecontrol but is a separate Next.js app -- it needs auth too | Dashboard uses the admin PIN for elevated access; regular API calls use the kiosk token |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Per-request DB lookup for token validation | Billing endpoint latency increases 10-50ms under load | Use HMAC validation (compute-only, no I/O) | At 8 concurrent pods polling every 5s = 1.6 req/s (low, but latency matters) |
| Argon2 hash on every admin PIN check | Admin panel feels sluggish (argon2 is deliberately slow: 100-500ms) | Hash once on login, issue a session cookie; do not re-hash on every admin API call | On first use -- argon2 is designed to be slow |
| Encryption/decryption of PII on every customer lookup | Customer list page takes seconds to load as every phone/email is decrypted | Decrypt in batch, cache decrypted values in memory for the session; or use deterministic encryption for search | At 500+ customer records |
| TLS handshake overhead on LAN connections | Pod reconnection time increases (TLS adds 1-2 RTT) | Keep LAN traffic as plain HTTP + auth tokens; TLS only for external | At pod restart -- every reconnect pays TLS cost |

---

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Auth token in URL query string (logged by proxies/browsers) | Token appears in server logs, browser history, and any intermediary | Use Authorization header for HTTP; for WebSocket, send token in first message after upgrade |
| Admin PIN attempts not rate-limited | 4-digit PIN brute-forced in < 30 minutes at 1 attempt/ms | Rate limit: 5 failures = 5-minute lockout; log all failed attempts |
| Error messages reveal auth internals | "Invalid token: expected HMAC-SHA256" tells attacker the algorithm | Generic 401 response: "Authentication required" -- no details about method |
| Forgetting to auth the /exec endpoint on rc-agent | Pod agent accepts arbitrary commands from localhost | Auth on rc-agent :8090 + bind to server IP only + process allowlist kills curl on pods |
| PII in structured logs shipped to monitoring | Phone numbers in Netdata/log aggregation | Log redaction middleware: mask PII before logging; never log request bodies containing PII fields |
| Session cookie without HttpOnly/Secure flags | XSS in the admin panel steals the session | Set `HttpOnly`, `Secure` (if HTTPS), `SameSite=Strict` on all auth cookies |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Auth failure shows generic "Error" to kiosk customer | Customer thinks the system is broken, calls staff | Kiosk should never show auth errors to customers; if auth fails, show "Connecting..." and retry silently; alert staff via bot |
| Admin PIN required on every page navigation | Uday enters PIN 50 times per day | Issue a session cookie after PIN entry; session lasts 8 hours (one business day) |
| Kiosk lockdown blocks game overlays (Steam, Discord) | Games fail to launch or crash because overlay processes are killed | Process allowlist must include game-required overlay processes; test with each game title |
| Security audit disrupts cafe operations | Audit requires pod downtime for testing | Run audit on Pod 8 only (canary); production pods continue serving customers |

---

## "Looks Done But Isn't" Checklist

- [ ] **API Auth:** Test with curl from a pod machine (not just the server). Verify `curl http://localhost:8090/exec` is rejected -- pod-local access must also require auth.
- [ ] **Admin PIN:** Test by calling admin endpoints directly with curl (no browser). Server-side must reject -- client-side React check is not sufficient.
- [ ] **HTTPS scope:** Verify LAN traffic decision is documented. If HTTP on LAN: document why. If HTTPS on LAN: verify all 8 pods have certificates and WSS works.
- [ ] **Kiosk lockdown:** Have a real person (not the developer) try to escape. Automated tests miss physical-access escape vectors like Ctrl+Alt+Del.
- [ ] **PII audit:** Check log files for customer data, not just the database. `grep -r "\d{10}" logs/` catches leaked phone numbers.
- [ ] **Token rotation:** Verify the middleware accepts 2+ tokens simultaneously. Test by adding a new token while the old one is still in use on pods.
- [ ] **Data at rest:** After encryption, run `sqlite3 customer.db .tables` from the CLI. If it works, encryption is not active.
- [ ] **Cloud sync:** After auth is enabled, verify Bono's VPS sync still works. Check the sync log for 401 errors.
- [ ] **Bot auth:** Send a session command via Discord/WhatsApp. Verify the bot includes auth credentials and the session actually starts.
- [ ] **Rate limiting:** Hit the admin PIN endpoint 10 times with wrong PINs. Verify lockout activates.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Big-bang auth breaks fleet | HIGH | Revert server binary to pre-auth version (`git revert` + deploy via pendrive if fleet exec is broken). All pods resume unauthenticated operation. |
| Leaked API token | MEDIUM | Generate new token, deploy to server (accepts both old+new), deploy new token to pods one-by-one, remove old token from server accept list after all pods updated. |
| Admin PIN forgotten | LOW | SSH to server, update the hashed PIN in config/DB directly. Or: add a "reset PIN via console" command to racecontrol CLI. |
| HTTPS breaks WS connections | MEDIUM | Revert to HTTP on the server (`git revert` TLS config). Pods reconnect automatically via WS. Then plan a proper TLS rollout. |
| Kiosk escape exploited during session | LOW | Immediate: rc-agent kills unauthorized processes automatically. Long-term: add the escape vector to the Group Policy blocklist. |
| PII found in logs after audit | MEDIUM | Purge log files containing PII. Add log redaction middleware. Re-run PII grep to verify. No customer notification needed if logs were server-local only. |
| SQLite encryption breaks cloud sync | MEDIUM | Disable encryption temporarily (`PRAGMA key` removal), restore sync, then fix sync to use SQLCipher-aware client before re-enabling encryption. |
| Auth latency breaks billing | LOW | Remove auth middleware from billing endpoints temporarily (they are the most latency-sensitive). Fix: switch to HMAC validation and re-enable. |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Big-bang auth bricks fleet | Phase 1: API Authentication | Deploy dual-mode middleware; verify pods work unauthenticated; then add tokens; then reject unauthenticated |
| Secret hardcoded in source | Phase 1: API Authentication | `grep -r "secret\|token" *.toml crates/` returns zero hits; secrets only in env vars |
| Admin PIN stored plaintext | Phase 2: Admin Panel Protection | `sqlite3` or `grep` on config shows only argon2 hashes, never plaintext PINs |
| HTTPS breaks WebSocket | Phase 3: Data in Transit | Document LAN/external split decision; if HTTPS on LAN: test WSS from all 8 pods before enabling |
| Kiosk escape vectors | Phase 4: Kiosk Hardening | Manual escape test by non-developer; process allowlist kills cmd/powershell/taskmgr during session |
| PII in unexpected locations | Phase 0: Security Audit | PII trace report lists every location; all non-authorized locations remediated |
| Tokens have no rotation | Phase 1: API Authentication | Middleware accepts token list (not single value); rotation procedure documented |
| Pod agent bypass | Phase 1: API Authentication | `curl http://pod:8090/exec` without token returns 401 |
| SQLite encryption breaks tooling | Phase 5: Data at Rest | Cloud sync, backup restore, and CLI tools tested after encryption enabled |
| Auth latency hits billing | Phase 1: API Authentication | p99 latency on billing endpoints < 5ms with auth enabled; load test with 8 concurrent pods |

---

## Sources

- [Mastering API Changes and Rollbacks Without Breaking Trust - Zuplo](https://zuplo.com/learning-center/api-changes-and-rollbacks) -- expand-migrate-contract pattern for API auth rollout
- [Managing API Changes: 8 Strategies That Reduce Disruption - Theneo](https://www.theneo.io/blog/managing-api-changes-strategies) -- phased rollout strategies
- [Android Kiosk Mode Security Hardening - VantageMDM](https://vantagemdm.wixsite.com/vantagemdm/post/android-kiosk-mode-security-hardening-technical-best-practices-2025) -- kiosk escape vectors and OS-level lockdown
- [Hexnode Windows Kiosk Security](https://www.hexnode.com/blogs/hardening-windows-kiosk-mode-security-best-practices-for-enterprise-protection/) -- Windows-specific kiosk hardening
- [Kiosk Hack Tips - Kiosk Industry](https://kioskindustry.org/kiosk-hacking-tips-to-harden-your-kiosk/) -- escape prevention techniques
- [Small Business PII Guide - Comparitech](https://www.comparitech.com/blog/information-security/small-business-pii/) -- PII handling for small businesses
- [6 Mistakes Handling PII - Integrate.io](https://www.integrate.io/blog/6-mistakes-to-avoid-when-handling-pii/) -- PII spread and audit practices
- [PII Compliance Checklist 2026 - Improvado](https://improvado.io/blog/what-is-personally-identifiable-information-pii) -- India DPDP Act fines up to 500 crore
- [SQLite Encryption and Secure Storage - SQLite Forum](https://www.sqliteforum.com/p/sqlite-encryption-and-secure-storage) -- SQLCipher pitfalls and key management
- [SQLite Security Hardening - ZuniWeb](https://zuniweb.com/blog/sqlite-security-and-hardening-encryption-backups-and-owasp-best-practices/) -- encryption-at-rest implementation gotchas
- [Zero-Downtime Schema Migration - Medium](https://medium.com/@systemdesignwithsage/the-schema-migration-strategy-that-finally-worked-without-downtime-36657492b8e2) -- dual-version compatibility during migration
- [Rust Axum JWT Auth - LogRocket](https://blog.logrocket.com/using-rust-axum-build-jwt-authentication-api/) -- Axum middleware patterns for auth
- [Axum Middleware Docs](https://docs.rs/axum/latest/axum/middleware/index.html) -- Router::layer and route-specific middleware application
- Project context: `.planning/PROJECT-v12.md`, `CLAUDE.md` (system architecture, fleet topology, deploy rules)

---
*Pitfalls research for: Security hardening of live eSports cafe operations stack*
*Researched: 2026-03-20*
