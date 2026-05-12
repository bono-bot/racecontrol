# P2 nginx sites-enabled-vs-available drift audit — customer-facing surfaces

**Authored:** 2026-05-11 ~10:18 IST (bono · Mode 4 subagent dispatch under Option D hybrid)
**Scope:** READ-ONLY audit of `/etc/nginx/sites-enabled/` vs `/etc/nginx/sites-available/` for 4 customer-facing surfaces flagged in §S-199.3
**Closes carryforward:** §S-199.3 P2 Captain-pending item
**Method:** `ls -la` + `diff` for each surface; no edits, no reload, no service touch

---

## Top-line

3 of 4 customer-facing surfaces are clean **SYMLINK** state (post-fix from §S-196 + §S-199 6-fix substrate). 1 surface — `racingpoint.cloud` (load-bearing apex routing 5 subdomains: www / api / cloud / staff / apex) — is **DRIFT-BEHAVIORAL**: sites-enabled is a regular file 5,229B larger than sites-available, containing 5 additional server blocks AND port-corrected proxy_pass on the main `/` location. sites-enabled is the authoritative live config; sites-available is a stale snapshot from 2026-03-13.

**No URGENT-CAPTAIN class findings** (no internal-only endpoints exposed; all proxy_pass targets are 127.0.0.1 loopback to known service ports).

---

## §1 — Per-surface classification

| Surface | Enabled type | Available exists | Classification | Notes |
|---|---|---|---|---|
| `racingpoint.cloud` | regular file (14753B, May 4) | yes (9524B, Mar 13) | **DRIFT-BEHAVIORAL** | +5229B; +5 server blocks (www/api/cloud/staff); proxy_pass ports diverge (3500→3501, 3200→3201); /register location dropped in enabled |
| `racingpoint.cloud-apex` | symlink → sites-available | yes | **SYMLINK** | Clean (§S-196.1 ship 03:29 IST 2026-05-11) |
| `kiosk.racingpoint.cloud` | symlink → sites-available | yes | **SYMLINK** | Clean (§S-196.1 ship 03:43 IST 2026-05-11) |
| `v2.racingpoint.cloud` | symlink → sites-available | yes | **SYMLINK** | Clean (originally landed 2026-05-03 05:00 IST) |
| `default` | regular file (2412B, Mar 26) | yes (2412B, Dec 2023) | **DRIFT-COSMETIC** | Byte-identical (`diff` exit 0); only mtime differs. Not customer-facing. |

No ENABLED-ONLY orphans. No AVAILABLE-ONLY-DISABLED. No unknown surfaces.

---

## §2 — DRIFT-BEHAVIORAL diff (verbatim from `diff /etc/nginx/sites-enabled/racingpoint.cloud /etc/nginx/sites-available/racingpoint.cloud`)

```
2,7c2,3
< # Routes to rc-core (port 8080) for /api/v1
< # Routes to PWA (port 3501) for everything else
< # Port 3501 (was 3500): moved 2026-05-04 ~15:35 IST to resolve Phase 0.6 ACTIVATION port-conflict.
< # V2 host racingpoint-web-v2 took port 3500 on 2026-05-03 13:18 IST (commit 65889803);
< # racecontrol-pwa EADDRINUSE crash-loop until canonical ecosystem.config.cjs (commit 005c09ec)
< # moved PWA to 3501. V2 host stays on 3500 for v2.racingpoint.cloud + path-based /v2 access.
---
> # Routes to rc-core (port 8080) for /register and /api/v1
> # Routes to PWA (port 3500) for everything else
10a7,14
>     # Registration page and API served by rc-core
>     location /register {
>         proxy_pass http://127.0.0.1:8080;
>         proxy_set_header Host $host;
>         proxy_set_header X-Real-IP $remote_addr;
>         proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
>         proxy_set_header X-Forwarded-Proto $scheme;
>     }
50c54
<     # Everything else goes to PWA (port 3501; moved from 3500 — see comment block above)
52c56
<         proxy_pass http://127.0.0.1:3501;
---
>         proxy_pass http://127.0.0.1:3500;
65c69
<         proxy_pass http://127.0.0.1:3501;
---
>         proxy_pass http://127.0.0.1:3500;
83c87
<         proxy_pass http://127.0.0.1:3201;
---
>         proxy_pass http://127.0.0.1:3200;
264,286d267
< # www.racingpoint.cloud — Public Website (Hayyu's ManagerXP)
< server {
<     server_name www.racingpoint.cloud;
<     location / { proxy_pass http://127.0.0.1:3600; ... }
<     location /_next/static/ { proxy_pass http://127.0.0.1:3600; ... }
<     listen 80;
< }
307,445d287
< # api.racingpoint.cloud — RaceControl API
< server { server_name api.racingpoint.cloud; location /webhook/kapso { proxy_pass http://127.0.0.1:3000/webhook; ... } location / { proxy_pass http://127.0.0.1:8080; ... } listen 443 ssl; ... }
< # cloud.racingpoint.cloud — Cloud Dashboard
< server { server_name cloud.racingpoint.cloud; location / { proxy_pass http://127.0.0.1:3700; ... } listen 443 ssl; ... }
< # staff.racingpoint.cloud — Kiosk Staff Terminal
< server { server_name staff.racingpoint.cloud; location / { proxy_pass http://127.0.0.1:3300; ... } location /ws/ { proxy_pass http://127.0.0.1:8080; proxy_http_version 1.1; Upgrade $http_upgrade; ... } listen 443 ssl; ... }
```

**Behavioral classes of divergence:**

- **B1 — Port correction (3500→3501, 3200→3201):** sites-enabled reflects the 2026-05-04 ~15:35 IST port-conflict resolution; sites-available is pre-move. **This is the load-bearing live correction.**
- **B2 — Dropped `/register` location:** sites-enabled removed the rc-core `/register` proxy. Sites-available still has it.
- **B3 — Added 5 server blocks:** sites-enabled adds `www.racingpoint.cloud` (→3600 racingpoint-website), `api.racingpoint.cloud` (→8080 + kapso webhook →3000), `cloud.racingpoint.cloud` (→3700 dashboard), `staff.racingpoint.cloud` (→3300 + `/ws/` →8080). All 5 are currently live and customer-/staff-facing. None are present in sites-available.
- **B4 — Trailing newline:** sites-available has trailing newline; sites-enabled does not.

---

## §3 — Captain decision question

The drift on `racingpoint.cloud` is the canonical "sites-enabled-is-truth, sites-available-is-stale" anti-pattern. Three fix options:

- **(a) Reconcile drift forward** — port the sites-enabled delta into sites-available (preserve all 5 added server blocks + port corrections + comment block), then replace sites-enabled regular file with symlink. Requires a per-surface audit-grep first to confirm no other latent-active vs latent-dead behavioral differences (e.g., any cert-managed-by-Certbot blocks Certbot may rewrite in-place against sites-enabled).
- **(b) Document-only** — leave the drift as-is, create `/etc/nginx/README-INFRA-DRIFT.md` (NEW) documenting that `racingpoint.cloud` is intentionally regular-file authoritative, sites-available is a snapshot, and list the 5 subdomains served from this single vhost-file. Lower change risk, accepts permanent convention break for this one surface.
- **(c) Hybrid per-surface** — symlink convention is already in place for 3 of 4 customer-facing surfaces; for `racingpoint.cloud` specifically, do (a) reconcile-then-symlink because this surface routes 5 subdomains and convention-break here propagates surprise risk to future ops (e.g., Certbot renewals, future edits via sites-available getting silently ignored).

---

## §4 — Bono recommendation

**(c) hybrid — apply option (a) to `racingpoint.cloud` only**, with the per-surface audit-grep gate, executed as a separate phase.

Rationale:

1. **Convention-uniformity has operational value** — 3/4 surfaces are already symlinks; making `racingpoint.cloud` a 4/4 symlink eliminates the "is this surface special?" classifier load for future ops.
2. **Certbot interaction risk** — Certbot's `--nginx` plugin rewrites server blocks in-place. When sites-enabled is a regular file separate from sites-available, Certbot updates the enabled copy but not available, perpetuating drift. Symlink state means Certbot touches the canonical once.
3. **Mechanism-trust-check applies** (§S-146 + §S-172 doctrine) — reconciliation is "patch V1-shaped delivery mechanism forward" class; needs RCA-light:
   - (i) port the delta verbatim;
   - (ii) `nginx -t` against the new sites-available before symlink swap;
   - (iii) `nginx -s reload` only after `-t` PASS;
   - (iv) post-reload HTTPS reachability re-verify on all 5 subdomains (www / api / cloud / staff / apex).
4. **Per-surface risk:**
   - `racingpoint.cloud` main vhost — **HIGH risk** (5 subdomains in one file; one syntax error blocks all 5)
   - `default` — LOW risk (byte-identical; cosmetic mtime drift; can become symlink in same window with zero behavioral change)
   - `racingpoint.cloud-apex`, `kiosk.racingpoint.cloud`, `v2.racingpoint.cloud` — N/A (already SYMLINK)

**Pre-execution gate:** before starting reconciliation, run audit-grep for any `# managed by Certbot` blocks in sites-enabled that aren't reflected in sites-available (Certbot deltas) — those need to be ported atomically, not piecemeal.

---

## §5 — Out-of-scope at this audit

- Live HTTPS reachability re-verify (already covered in §S-199.1 + this session's 6-fix substrate verifications)
- Cert validity / expiry / SAN coverage check
- pm2 service health (rc-core 8080, PWA 3501, racingpoint-website 3600, dashboard 3700, staff terminal 3300, WhatsApp bot 3000)
- WS endpoint check (`/ws/` on staff.racingpoint.cloud → 8080)
- `nginx -T` full config dump cross-check
- `include` directives / fastcgi / upstream blocks
- HTTP/2, gzip, security headers (HSTS, CSP) audit — separate hygiene pass

---

## §6 — Composes-with

- **§S-199.3** — original drift finding from `www.racingpoint.cloud` vhost edit session; carryforward closed by this audit (Captain decision still pending on options (a)/(b)/(c))
- **§S-172** — mechanism-trust-check rule; (a) reconciliation is exactly the kind of "shared infrastructure surface" that needs 5-question check before edit
- **§S-146** — V1-dependent V2 RCA rule; the drift IS V1-shaped delivery (sites-enabled-only edits), V2 reconciliation = symlink-convention enforcement
- **`feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md`** — applies to the reconcile-execute phase

— bono · 2026-05-11 ~10:18 IST · sites-enabled drift audit READ-ONLY · 1 DRIFT-BEHAVIORAL (`racingpoint.cloud`) + 3 SYMLINK (clean) + 1 DRIFT-COSMETIC (`default`) · 0 URGENT-CAPTAIN · recommendation (c) hybrid + pre-exec audit-grep gate · §S-199.3 carryforward closed (audit-side) · Captain decision pending on reconciliation strategy · authored under Mode 4 subagent dispatch (Option D hybrid)
