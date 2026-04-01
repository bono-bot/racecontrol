# Phase 305: TLS for Internal HTTP - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Infrastructure phase — discuss skipped

<domain>
## Phase Boundary

Self-signed venue CA with mTLS on server :8080 and agents :8090. CA generation script, Axum TLS config for both server and agent, Tailscale IP bypass. Foundation for all v38.0 security hardening.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion. Key: `rustls` for TLS (already in Axum ecosystem), `rcgen` crate for CA/cert generation, TOML config for cert paths.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/main.rs` — Axum server setup on :8080
- `crates/rc-agent/src/remote_ops.rs` — Agent HTTP server on :8090
- `racecontrol.toml` — Config file pattern
- Tailscale IPs in `100.x.x.x` range — used for bypass detection

### Integration Points
- Server: Axum `serve()` call needs TLS acceptor
- Agent: `remote_ops` HTTP server needs TLS config
- Config: cert paths in racecontrol.toml / rc-agent.toml

</code_context>

<specifics>
No specific requirements — infrastructure phase.
</specifics>

<deferred>
None.
</deferred>
