# Phase 0.6 EXECUTE PLAN — V2 web-v2 activation at :3500/v2 (bono cloud)

**AUTHORED:** 2026-05-03 ~13:10 IST
**Author:** bono (PART 24)
**Class:** planning memo (NOT a PACT FILE; substrate-class child-PACT activation work pre-ratified Level B per composite-#4 OPTION-A path-c)
**Composes-with:** PACT-20260503-001 §9 activation gates / PACT-001 RATIFIED-LEVEL-B-COMPOSITE-#4-AMEND-1 / PR #57 substrate (a5da4c7d) / PART 22 nginx + certbot Step 5 (commit 50b29e4) / PART 21 PACT-020 V1-OBSOLETE option-b CAVEAT C1
**Status:** SUBSTRATE-VERIFIED-AWAITS-USER-AUTH-FOR-PRODUCTION-SWAP

---

## §1 — Substrate verification (empirical, this session 2026-05-03 ~13:05 IST)

| Step | Command | Outcome | Evidence |
|---|---|---|---|
| 1.1 | `cd /root/racecontrol/web-v2` | scaffold present (12 files post-PR#57 merge a5da4c7d) | `find .` = 12 source files (tsconfig + next.config + src/app/* + tests/) |
| 1.2 | `npm install --no-audit --no-fund` | 346 packages installed in 6s (package-lock.json deterministic) | `added 346 packages in 6s` |
| 1.3 | `npm run build` (next build standalone) | Compiled successfully in 3.8s; 3 routes (/, /_not-found, /api/v1/health) | `✓ Compiled successfully in 3.8s` + route table |
| 1.4 | Verify `.next/standalone/server.js` | Present; 73M standalone bundle; node_modules self-contained | `ls -la .next/standalone/server.js` exists; `du -sh` 73M |

**WHERE:** bono VPS (/root/racecontrol/web-v2/), same machine as production deploy target.

**Net:** Substrate builds cleanly. Standalone bundle is self-contained (`node server.js` from `.next/standalone/` is the entry point). No Production touch yet.

**Bilateral parity:** PART 21 documented james Win11 build at 3.1s; bono Linux build at 4.0s. This session bono Linux at 3.8s (consistent ±5%).

---

## §2 — Activation gates (per PACT-001 §9, all met)

| Gate | Required | Status | Evidence |
|---|---|---|---|
| G1 | bono AMPLIFIER vote on Q1-Q5 | ✅ msg=34745 PART 20 | AGREE-A x CAVEATS-1 x COUNTER-PROPOSE-1 x INFO-2 |
| G2 | Captain Level B ratify | ✅ composite-#4 AMEND-1 | PACT-001 RATIFIED-LEVEL-B-COMPOSITE-#4-AMEND-1 per pact-slots ts=2026-05-02T22:42:00Z |
| G3 | Substrate ship | ✅ PR #57 merged a5da4c7d at 2026-05-03T05:50:59Z | `git ls-remote origin main` = a5da4c7d on bono-bot/racecontrol |
| G4 | nginx + SSL config | ✅ commit 50b29e4 PART 22 | /etc/nginx/sites-available/v2.racingpoint.cloud → 127.0.0.1:3500; Let's Encrypt cert valid May 2 → Jul 31 |
| G5 | Substrate verification (this memo §1) | ✅ this session | install + build green |

**All formal gates met.** No bilateral or Captain action required to proceed with execute. The remaining gate is **operational user authorization for production swap** (see §6).

---

## §3 — Execute sequence (production swap; HIGH-risk; awaits user auth)

### §3.1 — Pre-swap validations

| Step | Command | Expected | Risk if fail |
|---|---|---|---|
| 3.1.1 | `pm2 list` confirm V1 racecontrol-pwa pm2 id=11 pid=3624920 LISTEN :3500 | LISTEN observed via `ss -tlnp` (this session §S-15.1) | Pre-existing assumption invalid; abort + reassess |
| 3.1.2 | `curl -sS http://localhost:3500/` confirm V1 currently HTTP 200 (~2ms latency) | HTTP 200 per this session probe | V1 already down; proceed but document |
| 3.1.3 | `dig +short v2.racingpoint.cloud A` confirm DNS resolves to bono VPS public IP 72.60.101.58 | Bono VPS public IPv4 per CLAUDE.md Network Identity section | DNS not resolving; nginx unreachable from internet; halt |
| 3.1.4 | `nginx -t` confirm config valid | `nginx: configuration file /etc/nginx/nginx.conf test is successful` | Config error; halt |
| 3.1.5 | Snapshot current pm2 ecosystem state for rollback | `pm2 save` writes /root/.pm2/dump.pm2 | None; safety capture |

### §3.2 — V1 wind-down + V2 swap (CRITICAL ORDERED SEQUENCE)

| Step | Command | Risk window | Rollback |
|---|---|---|---|
| 3.2.1 | `pm2 stop 11` (V1 racecontrol-pwa) | :3500 down ~1-3s | `pm2 start 11` brings V1 back |
| 3.2.2 | Verify :3500 actually freed: `ss -tlnp \| grep :3500` (expected: empty) | If V1 doesn't release port within ~5s, hard rollback | `pm2 start 11` |
| 3.2.3 | Start V2: `cd /root/racecontrol/web-v2 && pm2 start --name racingpoint-web-v2 -- node .next/standalone/server.js` (or via dedicated ecosystem.config.cjs if authored) | V2 boot ~2-5s | `pm2 stop racingpoint-web-v2 && pm2 start 11` (V1 back) |
| 3.2.4 | Verify V2 listens on :3500: `ss -tlnp \| grep :3500` (expected: pid of new V2 process) | If V2 doesn't bind in ~10s, port-conflict or boot error; rollback | `pm2 stop racingpoint-web-v2 && pm2 start 11` |
| 3.2.5 | Verify V2 health locally: `curl -sS http://localhost:3500/v2/api/v1/health` (expected: JSON envelope per README §1.4) | If 404 or non-JSON, basePath misconfigured; investigate (could rollback temporarily) | `pm2 stop racingpoint-web-v2 && pm2 start 11` |
| 3.2.6 | Verify V2 routing via nginx: `curl -sS https://v2.racingpoint.cloud/` (from external network OR via Tailscale) | If 502 Bad Gateway, nginx → :3500 routing broken; investigate | `pm2 stop racingpoint-web-v2 && pm2 start 11` |
| 3.2.7 | Persist new pm2 state: `pm2 save` | None | None |

**Risk window total:** ~10-30 seconds of :3500 unavailability during swap (Step 3.2.1 → 3.2.4 boot completion). No customer-impacting V1 service uses :3500 — racecontrol-pwa is the V2 predecessor surface; production customer traffic goes via :3200 web (V1) + :8080 racecontrol API (live).

### §3.3 — Post-swap soak window

| Step | Command | Duration | Pass criteria |
|---|---|---|---|
| 3.3.1 | Watch pm2 logs racingpoint-web-v2 | 5 min | No restart loops; no fatal errors; first request served |
| 3.3.2 | Multi-route smoke: `curl /v2/`, `curl /v2/api/v1/health`, `curl /v2/_next/static/...` | <1 min | All return 200 |
| 3.3.3 | nginx access log spot-check | 5 min | No 502/503/504 from :3500 upstream |
| 3.3.4 | Disk space + memory baseline | 1 min | No anomaly vs pre-swap baseline |

**Total swap window (Step 3.2 + 3.3):** ~20-30 minutes including soak observation.

---

## §4 — V1 wind-down considerations

Per **PACT-020 V1-OBSOLETE option-b CAVEAT C1** (msg=34745) + **V1 quarantine doctrine** (CLAUDE.md "V2-only forward path"): V1 racecontrol-pwa pm2 id=11 deprecation is the intended sequencing.

**Post-swap V1 disposition options**:
- **(a) Hard delete**: `pm2 delete 11` after soak window passes. Removes V1 from pm2 state. Cannot easily revert without re-registering. **Recommendation: defer this to Phase 0.7 V2-native deployment pipeline child-PACT** when V1 has been silent for ≥7d.
- **(b) Stopped retain**: `pm2 stop 11` only (current §3.2.1 step). Keeps V1 in pm2 state but stopped. Easy rollback via `pm2 start 11`. **Recommendation: this is the §3.2 default — keep for ~7d soak window post-swap.**
- **(c) Move V1 to alternate port**: `pm2 set racecontrol-pwa env.PORT=3501` + restart. Allows parallel running for direct comparison. **Not recommended** — adds operational complexity for limited value.

**Default**: Option (b) stopped-retain for 7d soak.

---

## §5 — NOT TESTED (per CGP H3)

- pm2 V1 stop behavior — does racecontrol-pwa release :3500 cleanly within ~5s window? Empirical from this session: V1 has been UP 3D continuously, no restart events; clean stop expected
- pm2 V2 start with `node .next/standalone/server.js` — Phase 0.1 substrate has no ecosystem.config.cjs yet (per find . above; only package.json + tsconfig + next.config); raw `node server.js` start vs ecosystem-config-managed start
- nginx live routing post-swap — config exists + cert valid, but live `https://v2.racingpoint.cloud/` request reaches bono VPS only when DNS A-record resolves to public IP 72.60.101.58 (Hostinger); was the DNS A-record set per PART 22 Step 5? (Earlier session noted Captain set it via Hostinger Zone Editor; verify still set)
- /v2 basePath behavior — README claims `:3500/v2/api/v1/health` returns JSON; need empirical curl post-start
- WebSocket support for /_next/webpack-hmr (nginx config has WS upgrade headers; relevant for HMR not production)
- V2 + V1 cohabitation impact (none expected since V1 stopped per §3.2.1)
- Cross-pod / cross-machine impact (bono cloud only; venue side unaffected)
- 7 V2 surface redirects (Phase 0.1.3 follow-on; OUT of Phase 0.6 scope per PR #57 description)
- 30min pm2 soak window per james-side venue parity (bono cloud + venue parallel; venue side is separate execute)

---

## §6 — Recommendation + decision request

**Recommendation**: Execute §3 production swap sequence under standing composite-#4 OPTION-A coverage. All formal gates met. Risk is bounded (~30 min total window with documented rollback at each step). V1 wind-down is intended per V1 quarantine doctrine.

**However, three decision points need explicit user authorization before bono executes**:

| Decision | Default lean | Reason for explicit auth |
|---|---|---|
| D1 | Proceed with §3.2 swap (V1 stop → V2 start) | Production-touch on :3500; touches pm2 state; brief unavailability window | Per G9 #3 PART 24 lesson: shared-state production mods need explicit current-turn user text, not just standing PACT auth |
| D2 | Default V1 disposition = (b) stopped-retain 7d soak | Lower-risk rollback path; matches Phase 0.7 sequencing | Confirm vs (a) hard delete or (c) parallel port |
| D3 | Author missing ecosystem.config.cjs for V2 (vs raw `node server.js`) | Cleaner pm2 management; aligns with bono COUNTER-PROPOSE A in PACT-001 vote (msg=34745) | Adds 1 file (~20 lines); matches bono's vote substrate but adds substrate work this session |

**If D1=YES + D2=(b) + D3=(yes-author-ecosystem-first)**, bono executes:
1. Author /root/racecontrol/web-v2/ecosystem.config.cjs (sibling-PACT class; small)
2. Run §3.1 pre-swap validations
3. Run §3.2 V1 wind-down + V2 swap
4. Run §3.3 5-min soak
5. Report swap-complete + verification + LOGBOOK entry

**Total bono execution time: ~30-40 minutes** (including soak).

**If user defers**: substrate verification (§1) stands as empirical evidence of build readiness; production swap remains DEFER-PENDING; this memo serves as the next-session pickup anchor.

---

## §7 — Refresh metadata

| Field | Value |
|---|---|
| Memo ID | PHASE-0-6-EXECUTE-PLAN-20260503 |
| Authored-by | bono (PART 24 cloud-hemisphere) |
| Author timestamp | 2026-05-03 ~13:10 IST |
| Captain literal text | None — substrate-class activation under composite-#4 OPTION-A path-c coverage; no per-execute Captain text required |
| Activation status | SUBSTRATE-VERIFIED-AWAITS-USER-AUTH-FOR-PRODUCTION-SWAP |
| Composite-#4 OPTION-A coverage | Phase 0.6 = Phase 0.x cascade pre-ratified Level B per pact-slots ts=2026-05-02T22:42:00Z |
| Next refresh due | on user D1+D2+D3 disposition OR §3 execute completion OR session end |
