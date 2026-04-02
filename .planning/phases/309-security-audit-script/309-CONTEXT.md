# Phase 309: Security Audit Script - Context

**Gathered:** 2026-04-02
**Status:** Complete
**Mode:** Tooling phase — capstone for v38.0

<domain>
## Phase Boundary

Single-command security audit that checks all v38.0 hardening (TLS, WS auth, audit chain, RBAC). Outputs JSON scorecard. Integrated into gate-check.sh as Suite 7 pre-deploy gate.

</domain>

<decisions>
## Implementation Decisions

- Bash script with Python for JSON handling (no new deps)
- 28 checks across 5 categories: tls (5), ws_auth (7), audit_chain (4), rbac (6), general (7)
- Live chain integrity check via HTTP (skips if server unreachable)
- Temp file for JSON accumulation (avoids shell escaping issues)
- gate-check.sh Suite 7 integration — runs security-audit.sh, fails deploy on overall=fail

</decisions>

<code_context>
## Key Files
- `scripts/security-audit.sh` — main audit script (28 checks, JSON scorecard output)
- `test/gate-check.sh` — Suite 7 added for security audit integration
- `security-scorecard.json` — output artifact

</code_context>

<specifics>
## Requirements
- SECAUDIT-01: Scans ports, TLS config, JWT validity, default credentials, chain integrity ✅
- SECAUDIT-02: Structured JSON scorecard with pass/fail per check ✅
- SECAUDIT-03: Integrated into gate-check.sh as pre-deploy gate ✅
</specifics>

<deferred>
None.
</deferred>
