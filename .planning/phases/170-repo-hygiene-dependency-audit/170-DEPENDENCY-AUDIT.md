# Dependency Audit Report

Phase 170 — 2026-03-23 (IST)

---

## npm Audit Summary

| Repo | Critical | High | Moderate | Low | Action Taken |
|------|----------|------|----------|-----|--------------|
| comms-link | 0 | 0 | 0 | 0 | None needed |
| racecontrol | 0 | 0 | 0 | 0 | None needed |
| racingpoint-admin | 0 | 0 | 1 | 0 | `npm audit fix` — fixed high (flatted DoS); 1 moderate remains (deferred) |
| racingpoint-api-gateway | 0 | 0 | 0 | 0 | None needed |
| racingpoint-discord-bot | 0 | 1 | 3 | 0 | `npm audit fix` — no effect; deferred (transitive via discord.js 14.25.1) |
| racingpoint-google | 0 | 0 | 0 | 0 | None needed |
| racingpoint-mcp-calendar | 0 | 0 | 0 | 0 | None needed |
| racingpoint-mcp-drive | 0 | 0 | 0 | 0 | `npm audit fix` — fixed 3 high (@hono/node-server, hono, express-rate-limit) |
| racingpoint-mcp-gmail | 0 | 0 | 0 | 0 | `npm audit fix` — fixed 3 high (@hono/node-server, hono, express-rate-limit) |
| racingpoint-mcp-sheets | 0 | 0 | 0 | 0 | None needed |
| racingpoint-whatsapp-bot | 0 | 0 | 0 | 0 | None needed |
| rc-ops-mcp | 0 | 0 | 0 | 0 | None needed |
| whatsapp-bot | 0 | 0 | 0 | 0 | None needed |

**Summary:** 13 repos audited. 7 high vulnerabilities fixed across 3 repos. 1 high remains (discord-bot, deferred). 1 moderate remains (racingpoint-admin, deferred).

---

## npm Vulnerability Details

### racingpoint-admin — 1 vulnerability remaining

| Package | Severity | Description | Decision |
|---------|----------|-------------|----------|
| flatted | ~~high~~ | Unbounded recursion DoS in parse() revive phase | Fixed via `npm audit fix` (commit f11480c) |
| next | moderate | Next.js HTTP request smuggling in rewrites | **Deferred** — requires `--force` upgrade (breaking Next.js major version). Low exploitability in internal admin tool on LAN only. Schedule for next major Next.js upgrade cycle. |

### racingpoint-discord-bot — 4 vulnerabilities remaining

| Package | Severity | Advisory | Description | Decision |
|---------|----------|----------|-------------|----------|
| undici | high | GHSA-f269-vfmq-vjvj | Malicious WebSocket 64-bit length overflows parser and crashes client | **Deferred** — transitive dependency of `discord.js@14.25.1` (latest). discord.js bundles undici 6.21.3; fix requires undici >=6.24.0. Latest discord.js still includes vulnerable version. No direct override possible without breaking discord.js. Track discord.js release for bundled undici update. |
| undici | high | GHSA-vrm6-8vpv-qv8q | Unbounded memory consumption in WebSocket permessage-deflate decompression | **Deferred** — same transitive chain as above |
| undici | moderate | GHSA-g9mf-h72j-4rw9 | Unbounded decompression chain via Content-Encoding | **Deferred** — same transitive chain as above |
| undici | moderate | GHSA-2mjp-6q6p-2qxm | HTTP Request/Response Smuggling | **Deferred** — same transitive chain as above |
| undici | moderate | GHSA-4992-7rv2-5pvq | CRLF Injection via upgrade option | **Deferred** — same transitive chain as above |

### racingpoint-mcp-drive — 3 vulnerabilities (all fixed)

| Package | Severity | Description | Decision |
|---------|----------|-------------|----------|
| @hono/node-server | high | Authorization bypass via encoded slashes in Serve Static Middleware | Fixed via `npm audit fix` (commit 5b182ea) |
| hono | high | Cookie attribute injection via unsanitized domain/path in setCookie() | Fixed via `npm audit fix` (commit 5b182ea) |
| express-rate-limit | high | IPv4-mapped IPv6 bypass of per-client rate limiting | Fixed via `npm audit fix` (commit 5b182ea) |

### racingpoint-mcp-gmail — 3 vulnerabilities (all fixed)

| Package | Severity | Description | Decision |
|---------|----------|-------------|----------|
| @hono/node-server | high | Authorization bypass via encoded slashes in Serve Static Middleware | Fixed via `npm audit fix` (commit 95599fc) |
| hono | high | Cookie attribute injection via unsanitized domain/path in setCookie() | Fixed via `npm audit fix` (commit 95599fc) |
| express-rate-limit | high | IPv4-mapped IPv6 bypass of per-client rate limiting | Fixed via `npm audit fix` (commit 95599fc) |

---

## Outdated Packages (2+ major versions behind)

| Repo | Package | Current | Latest | Decision |
|------|---------|---------|--------|----------|
| racingpoint-admin | @types/node | 20.19.35 | 25.5.0 | **Defer** — @types/node v20 matches Node.js 20 LTS on server. Upgrading to v25 types would track Node.js 25 prerelease. No runtime impact (dev dependency). Upgrade when server Node.js is upgraded. |
| racingpoint-google | googleapis | 144.0.0 | 171.4.0 | **Defer** — googleapis 144→171 is 27 major versions in 8 months (Google's versioning). API surface used (Calendar, Drive, Gmail OAuth) is stable. No security advisories. Upgrade in next mcp-* maintenance cycle with integration testing. |

---

## Cargo Audit Summary

| Repo | Vulnerabilities | Warnings | Action Taken |
|------|-----------------|----------|--------------|
| racecontrol | 1 (medium) | 1 (unmaintained) | Updated rustls-webpki 0.103.9→0.103.10 (commit ee803b83). rsa deferred (no fix). |
| pod-agent | 0 | 0 | Updated rustls-webpki 0.103.9→0.103.10 (commit 6faa7f0). |

---

## Cargo Vulnerability Details

### racecontrol — 1 vulnerability, 1 warning

| Crate | Advisory | Severity | Description | Decision |
|-------|----------|----------|-------------|----------|
| rustls-webpki 0.103.9 | RUSTSEC-2026-0049 | — | CRLs not considered authoritative by Distribution Point due to faulty matching logic | **Fixed** — `cargo update rustls-webpki` → 0.103.10. Committed (ee803b83) and pushed. |
| rsa 0.9.10 | RUSTSEC-2023-0071 | 5.9 medium | Marvin Attack: potential key recovery through timing sidechannels | **Deferred** — No fix available upstream. rsa is a transitive dep of `sqlx-mysql`. sqlx itself doesn't expose this (no RSA key operations in ORM). Acceptable risk for internal service. Monitor sqlx releases. |
| paste 1.0.15 | RUSTSEC-2024-0436 | warning | paste crate no longer maintained | **Deferred** — warning only (not vulnerability). Transitive dep via `nalgebra` → `simba` → `imageproc` → `rc-sentry-ai`. No replacement available in dependency chain. Monitor nalgebra for migration. |

### pod-agent — 0 vulnerabilities

| Crate | Advisory | Severity | Description | Decision |
|-------|----------|----------|-------------|----------|
| rustls-webpki 0.103.9 | RUSTSEC-2026-0049 | — | CRLs not considered authoritative by Distribution Point due to faulty matching logic | **Fixed** — `cargo update rustls-webpki` → 0.103.10. Committed (6faa7f0) and pushed. |

---

## Cargo Outdated Dependencies

Cargo outdated tool not installed (skipped). Key crate versions from Cargo.toml reviewed manually:

| Repo | Crate | Notes |
|------|-------|-------|
| racecontrol | sqlx 0.8.6 | Current stable. No known security issues beyond RUSTSEC-2023-0071 (deferred above). |
| racecontrol | tokio 1.x | Current stable 1.x line. No advisories. |
| pod-agent | reqwest 0.12.28 | Current stable. Clean audit. |

---

## Summary of Actions Taken

| Action | Repos Affected | Commits |
|--------|---------------|---------|
| `npm audit fix` | racingpoint-admin, racingpoint-mcp-drive, racingpoint-mcp-gmail | f11480c, 5b182ea, 95599fc |
| `cargo update rustls-webpki` | racecontrol, pod-agent | ee803b83, 6faa7f0 |

**Deferred items (total 8):**
1. racingpoint-admin: next.js moderate (HTTP smuggling) — requires major version bump
2. racingpoint-discord-bot: 2 high + 3 moderate undici — transitive via discord.js, no fix in latest release
3. racecontrol: rsa medium (Marvin Attack) — no fix available upstream in sqlx-mysql
4. racecontrol: paste unmaintained warning — transitive via nalgebra image processing chain
5. racingpoint-admin: @types/node v20→v25 — dev dependency, tied to Node.js LTS version
6. racingpoint-google: googleapis v144→v171 — stable API surface, upgrade in next maintenance cycle
