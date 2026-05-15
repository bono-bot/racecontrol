# FLEET-STALENESS-AUDIT-F0-20260515 — read-only prerequisite audit

**As-of:** 2026-05-15 ~16:43 IST (Fri)
**Authored by:** james (autonomous-proceed per Captain commission 2026-05-15 ~16:33 IST)
**Class:** read-only audit feeding pre-action gates for [`FLEET-STALENESS-FIX-20260515.md`](FLEET-STALENESS-FIX-20260515.md) F1/F2/F3
**WHERE:** James .27 → curl/tasklist/schtasks/tailscale/git probes against Server .23 + Bono VPS + James .27 + Tailnet

---

## §1 Findings summary

| # | Surface | Status | Action gate |
|---|---|---|---|
| F0.1a | James .27 comms-link relay :8766 | HEALTHY (REALTIME, fresh heartbeat 11:12:10 UTC) | F1/F2/F3 cleared |
| F0.1b | James .27 rc-watchdog.exe process | RUNNING (PID 18248) | F1/F2/F3 cleared |
| F0.1c | James .27 rc-sentry-ai.exe process | RUNNING (PID 15732) | F1/F2/F3 cleared |
| F0.1d | `CommsLink-DaemonWatchdog` schtask | **Ready** (next 16:45 IST today) | F1/F2/F3 cleared |
| F0.1e | `CommsLink-RealDaemonWatchdog` schtask | Ready (next 16:46) | sibling task — investigate naming overlap (separate ticket) |
| F0.1f | `AutoDetect-Daily` schtask | Ready (next 16-05 02:30) | cleared |
| F0.1g | `RCSentryAI-Watchdog` schtask | Ready | cleared |
| F0.1h | `FrontendStalenessCheck` schtask | **Ready (next 16-05 09:00 IST)** — **task EXISTS and is scheduled**; 16d drift on web/admin proves the check is either silently failing OR not surfacing alerts | **F5 finding sharpened** — root cause is not absence of check, it's silent-failure or non-surfacing |
| F0.1i | `RacingPoint-StagingHTTP` schtask | **DISABLED** | F1+F2 IMPACT — staging HTTP :18889 deploy path disabled; F1/F2 must use alternative (direct SCP or rc-sentry self-host) |
| F0.2 | Pod 8 bat-file hash (canary drift) | **DEFERRED** — classifier denied autonomous remote-shell on production pod; needs explicit Captain auth | gate retained on F2 |
| F0.3 | DB schema migration parity | 8 migrations in `crates/v2-db/migrations/` on main (latest `20260514174000_kitchen_orders_substrate_phase1.sql`); applied-state on venue + cloud NOT verified (requires DB access; deferred to F1 pre-action gate or separate sub-audit) | gate retained on F3 |
| F0.4 | Tailscale node hygiene | 15 nodes visible; **stale `bono@` `racing-point-server` 100.71.226.83 NOT visible** → likely already removed from admin console; CLAUDE.md current-blocker is STALE and can be updated. POS .130 `offline, last seen 19h ago` (confirms F4 P3) | F4 still required for POS recovery |
| F0.5a | racecontrol pre-commit hook | **INSTALLED** at `scripts/hooks/pre-commit` (11518 bytes, executable) via `core.hooksPath = scripts/hooks` config | cleared |
| F0.5b | comms-link pre-commit hook | INSTALLED at `scripts/hooks/pre-commit` (same pattern) | cleared |

---

## §2 Surfaces newly flagged (not in v0.2 plan)

### §2.1 `FrontendStalenessCheck` silent-failure or non-surfacing — P1

The scheduled task `FrontendStalenessCheck` **exists and runs daily 09:00 IST** on James .27. Yet web/admin frontends drifted 16d. Two failure modes:
- (a) Task runs but check function silently returns 0/no-drift incorrectly (logic bug)
- (b) Task runs, detects drift correctly, but ALERT path is broken (no WhatsApp / no log surfacing / no email)

This sharpens F5 from "audit `frontend-staleness-check.sh`" to "**investigate why an active scheduled check missed 16-day drift**". Either logic or alerting layer is silently broken — Phase F5 root cause work should start by reading the task's last 30 run logs.

### §2.2 `RacingPoint-StagingHTTP` DISABLED — F1/F2 deploy-path BLOCKER

The staging HTTP server :18889 scheduled task is in `Disabled` state on James .27. This is the http server pods download new binaries from per CLAUDE.md "Remote deploy sequence (rc-agent)" step 3. F1 (frontend deploy) and F2 (pod fleet deploy) cannot proceed using the standard staged-binary path without re-enabling this task first.

Three remediation options:
- (a) Re-enable schtask + verify it actually serves binaries (deploy-staging-parity-check.py)
- (b) Use direct SCP path (bypass :18889) — slower per-pod, but no schtask dependency
- (c) Use rc-sentry self-host pattern (each pod fetches via its own sentry binary)

Recommend (a) as primary, (b) as fallback. F1 cannot fire until decided.

### §2.3 CLAUDE.md current-blocker drift (stale `bono@` node) — P3 cleanup

CLAUDE.md "Current Blockers" lists *"Server .23 Tailscale re-authenticated under `james@`... Old `bono@` node (`racing-point-server`, 100.71.226.83) is stale — remove from Tailscale admin console"* but Tailnet status today shows no such node. Either already removed or never propagated to LAN visibility. CLAUDE.md should be updated to remove the stale blocker entry (out-of-scope for this audit; flagging for next harness-mechanism-auth opportunity).

---

## §3 Updated gate dispositions for v0.2 plan

| Phase | Pre-action gate status | Blockers found this audit |
|---|---|---|
| F1 frontend | DEPLOY_IN_PROGRESS sentinel + watchdog disable + API proxy verify gates retained; **NEW:** decide §2.2 staging HTTP remediation path BEFORE F1 fires | §2.2 RacingPoint-StagingHTTP disabled |
| F2 pod fleet | Per-pod billing-drain gate retained; **NEW:** Pod 8 canary bat-hash check DEFERRED pending Captain explicit auth on pod /exec | §2.2 + F0.2 deferred |
| F3 racecontrol | F3d (bifurcate) recommended; venue DB migration applied-state still UNKNOWN | F0.3 deferred |
| F4 POS | Confirmed offline 19h; physical-check path required | none new |
| F5 root-cause | **REFINED from §2.1:** investigate WHY active `FrontendStalenessCheck` missed 16d drift (logic or alerting layer); also `start-comms-link.bat` daemon naming-overlap (CommsLink-DaemonWatchdog vs CommsLink-RealDaemonWatchdog vs CommsLink-Watchdog — three sibling tasks) | none blocking F5 |

---

## §4 What this audit did NOT verify

- Pod 8 bat-file hash (deferred pending Captain pod /exec auth)
- Pod 1-7 bat-file hashes (same gate)
- Venue DB applied-migrations vs main HEAD (deferred; needs DB access)
- Bono VPS applied-migrations (same)
- Bono VPS pm2 comms-link cloud build vs comms-link main HEAD (MAO MISSED-7; separate from racecontrol F3 scope)
- Cloud frontend `admin.racingpoint.cloud` /coming-soon intentional vs missed-deploy (MAO MISSED-2)
- go2rtc :1984 / Ollama models versioning on James .27 (out-of-scope F0; surface for future F0-extension)
- end-to-end customer browser test against any URL

---

## §5 Stale-at

- (a) Any §S-N anchor naming a row in §1 status table
- (b) Any deploy that touches F1/F2/F3 surface
- (c) Hard 48h fire: **2026-05-17 16:43 IST** (F0 is fast-decay scoping; pre-action gates need fresh ground truth before any deploy phase)
- (d) Captain ratify on §2.2 staging HTTP remediation path
