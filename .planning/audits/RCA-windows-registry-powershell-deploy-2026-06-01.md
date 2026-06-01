# RCA — Windows Registry + PowerShell-edit deploy breakage (recurring class)

**Author:** bono (VPS) · 2026-06-01 IST · **Captain directive:** *"Lets do a RCA first… It is mostly Windows Registry edits and PowerShell edits causing issues for new deployment."* · **Authority:** Captain "full authority for this Session" 2026-06-01 ~22:00 IST.
**Class:** V1-era shared delivery infrastructure → 5-section RCA + mechanism-trust-check (§S-146 / mechanism-trust-check-upstream-of-fix-RCA doctrine).
**Gate role:** upstream gate for `HANDOFF-rc-agent-fleet-rerollout-20260601.md` — discharges its §5.1 mechanism-trust-check.

---

## §0 Problem statement
New deployments to Windows targets (Server .23, Pods 1-8, POS .130) repeatedly break on **Windows Registry edits** (HKLM/HKCU `\Run` keys, schtasks, Session-0-vs-1) and **PowerShell edits** (inline PS over SSH, `Set-Content`, `.ps1`/`.bat` line-ending/encoding) — **despite ~15 standing rules** already warning about them in `racecontrol/CLAUDE.md`. The question this RCA answers: **why do they recur despite the rules?** Hypothesis (confirmed below): **prose rules do not hold; only mechanical enforcement does** (same finding as the §S-146 enforcement RCA — text-only rules carry ≥1 repeat-violation per 30d, hook-enforced rules carry zero).

---

## §1 Boundary map (enumerated 2026-06-01, probe-backed)
Full surface: **77** `.bat`/`.ps1` scripts under `scripts/` + `deploy-staging/` (`find scripts deploy-staging -type f \( -name '*.bat' -o -name '*.ps1' \)`). The deploy-path edit surface splits into three tiers:

**Tier A — DANGEROUS: inline `ssh "powershell … <writes>"` over SSH (the 2026-04-08 fleet-wipe class). 6 files:**
| File:line | Operation | Severity |
|---|---|---|
| `scripts/rotate-credentials.sh:56-62` | `$SERVER_SSH "powershell … Set-Content racecontrol.toml"` (server creds) | **CRITICAL** (the named smoking gun; credential rotation, fleet-wide blast) |
| `scripts/rotate-credentials.sh:104` | `/exec` `powershell … Set-Content rc-agent.toml` (×8 pods) | **CRITICAL** |
| `scripts/deploy-sentry.sh:148,158` | `ssh "powershell … Stop-Process; Rename-Item rc-sentry.exe"` (binary swap) | HIGH |
| `scripts/deploy/deploy-nextjs.sh:299,563` | `ssh "powershell … Set-Content"` / multiline PS (frontend deploy) | HIGH |
| `scripts/audit/cleanup-pod-edge-stale-ps1.sh:86` | `ssh "powershell … Remove-Item -Force"` (deletion) | MEDIUM |
| `scripts/audit/cleanup-pos-edge-backups.sh:94` | `ssh "powershell … Remove-Item -Force"` (deletion) | MEDIUM |

**Tier B — read-only inline PS over SSH (lower risk; no write):** `scripts/diagnose/venue-down.sh:65`, `scripts/deploy/deploy-all-pods.sh:79` (`/exec` Get-Process), `scripts/deploy-sentry.sh:113,176` (Get-Item .Length), `scripts/deploy/deploy-nextjs.sh:139,554` (Test-Path / Get-NetTCPConnection). **Not a fix target** — they read, they don't corrupt.

**Tier C — LOCAL `Set-Content`/`Out-File` inside `.ps1` SCP'd-then-run (the RECOMMENDED safe pattern, NOT a violation):** `scripts/diagnostics/ac-bootstrap.ps1`, `fix-race-ini.ps1`, `deploy/start-racecontrol-watchdog.ps1:96` (writes MAINTENANCE_MODE), `fleet-uniformity-audit.ps1`, etc. These are the *target state* the fix moves Tier A toward.

**Registry-edit surfaces (`\Run` keys, the Session-0/1 + Pod-7-missing-key class):** ~24 files incl. `scripts/pod-uniformity-cleanup.ps1`, `fleet-uniformity-audit.ps1`, `install-pod-service.sh`, `deploy/start-rcagent.bat`, `deploy/install-server-services.bat`. **Authoritative `\Run` writer:** `start-rcagent.bat` (HKLM Run on pods) / `start-racecontrol.bat` (server). **Open RCA question (James recon-pending):** which pods currently have/lack `HKLM\Run\RCAgent`, and Session-0-vs-1 per pod — filled by the directive's Phase-1 recon.

---

## §2 Inherited-issue catalogue (incident → standing rule)
| # | Incident | Standing rule it authored | Class |
|---|---|---|---|
| 1 | 2026-04-08 inline-PS-over-SSH wiped ALL 8 pod TOMLs → fleet down 25 min | "NEVER edit remote files via inline PowerShell over SSH" (Code Quality) | config-loss |
| 2 | SSH-banner corruption prepended MOTD to TOML → empty defaults 2h | "Never pipe SSH output into config files" (use scp) | config-corrupt |
| 3 | Pod-7 missing `HKLM\Run\RCAgent` + RCWatchdog `spawn_verified=false` ×5 | rc-agent Session-1 + `\Run` repair rules | registry-missing |
| 4 | PowerShell watchdog multiplication ×6 (port 8080 fight) | `taskkill /F /IM powershell.exe` + `Global\RaceControlWatchdog` mutex | process-multiply |
| 5 | Session-0 vs Session-1 (`schtasks /Run`/services → no GUI) | "rc-agent MUST run in Session 1"; `StartRCTemp`→`StartRCDirect` | session-context |
| 6 | LF-only `.ps1` → "missing terminator" | `.gitattributes` CRLF for `.ps1`/`.bat`/`.cmd` | encoding |
| 7 | `.bat` BOM + parentheses in if/else | "`.bat`: clean ASCII + CRLF, `goto` not parens" | encoding |
| 8 | `start … 2>>` redirect in schtask → exit 1, child never created (8 pods) | "Never use file redirects on `start` in bat files" | schtask-io |

---

## §3 Past-bug disposition
| Incident | Disposition | Evidence |
|---|---|---|
| #1 inline-PS-over-SSH TOML wipe | **RULE-EXISTS-BUT-UNENFORCED** ⚠️ | Rule written 2026-04-08; **the banned pattern is live in 6 files today** (§1 Tier A). A prose rule did not stop proliferation. **This is the root cause.** |
| #2 SSH-banner-into-config | PATCHED-ONLY | `2>/dev/null` + head-1 validation rules exist; no mechanical gate |
| #3 Pod-7 `\Run` missing | UNRESOLVED (live-state unknown) | James Phase-1 recon will confirm current per-pod state |
| #4 watchdog ×6 | ROOT-CAUSED-AND-FIXED | singleton mutex in watchdog.ps1 |
| #5 Session-0/1 | ROOT-CAUSED-AND-FIXED | RCWatchdog `WTSQueryUserToken`+`CreateProcessAsUser`; but no *audit* that asserts Session=Console post-deploy (proxy-check gap) |
| #6 LF `.ps1` | ROOT-CAUSED-AND-FIXED | `.gitattributes` (mechanical — and it holds: zero recurrence) |
| #7 `.bat` BOM/parens | PATCHED-ONLY | rule exists; no commit-time gate |
| #8 `start` redirect | ROOT-CAUSED-AND-FIXED | removed from bats |

**Key:** the two classes that *recur* (#1, #2, #7) are the **prose-only** ones. The two that *hold* (#5, #6) became **mechanical** (`.gitattributes`, watchdog mutex). This is direct empirical confirmation of the hypothesis.

---

## §4 V2-alignment delta (what a hardened deploy mechanism looks like)
- **No inline-PS-over-SSH writes.** Pattern → **write `.ps1` locally → `scp` → `ssh host "powershell -ExecutionPolicy Bypass -File script.ps1"`** (the Tier-C pattern, already used safely elsewhere). The local `.ps1` is CRLF-enforced by `.gitattributes`, git-reviewable, and Git-Bash can't mangle `$_`/`$env:`.
- **Behavioral verify, not echo:** after a config write, read the value back + parse it (`head -1 | grep '^['` for TOML), not `&& echo OK`.
- **Single-target dry-run / canary:** every fleet-touching script gets a `--canary <pod>` first-target path before the loop.
- **Authoritative `\Run` writer + repair:** one writer per key + a repair step (Pod-7 class).
- **Session-1 guarantee + assert:** post-deploy `tasklist /V` must show `Console`, not `Services`.
- **Mechanical enforcement over prose:** a commit-time / deploy-time grep gate that BLOCKS new Tier-A patterns (the thing `.gitattributes` did for encoding, applied to inline-PS-over-SSH).

---

## §5 V2-framed proposal (smallest-reversible, mechanical-first)
**P1 (this RCA, landing now):**
1. **Fix `scripts/rotate-credentials.sh`** (the CRITICAL smoking gun) → write-local-`.ps1`-then-SCP for the server TOML; for pods, write a `.ps1` and invoke via rc-sentry `/exec` `-File` (no inline `Set-Content`); add `--canary <pod>`; behavioral read-back verify.
2. **Add `scripts/check-no-inline-ps-ssh.sh`** — a mechanical grep gate (the §3 lesson). Detects Tier-A `ssh …"powershell …(Set-Content|Out-File|Remove-Item|Rename-Item)` + `/exec` JSON inline-PS-write. Baseline-aware (records the 5 known-remaining Tier-A files so it BLOCKS *new* ones and reports the baseline as P2 debt). Wire into `test/run-all.sh` security suite + pre-commit.

**P2 (tracked, follow-up — gate prevents regression meanwhile):** convert the 5 remaining Tier-A files (`deploy-sentry.sh`, `deploy-nextjs.sh`, `cleanup-pod-edge-stale-ps1.sh`, `cleanup-pos-edge-backups.sh`; `venue-down.sh` is Tier-B read-only). Each: write-local-`.ps1`→SCP→`-File`.

**P3 (registry, James-recon-gated):** authoritative `\Run`-key writer+repair + Session-1 assert in the deploy audit, once Phase-1 recon confirms current per-pod registry/Session state.

**Mechanism-trust-check (rc-agent OTA surface):** cached at `.planning/specs/v2/MECHANISM-TRUST/deploy-registry-powershell-2026-06-01.json` — see that file. Summary: the **re-rollout** uses the rc-sentry `/exec` atomic-swap (NOT rotate-credentials.sh's inline-PS), which is 4/5 YES with canary mitigating the one PARTIAL → **re-rollout may proceed canary-first**; the rotate-credentials inline-PS path is the FAIL surface this RCA fixes.

---
**Cross-refs:** `HANDOFF-rc-agent-fleet-rerollout-20260601.md` (gated consumer) · `racecontrol/CLAUDE.md` Code Quality "NEVER edit remote files via inline PowerShell over SSH" (the unenforced rule) · §S-146 + mechanism-trust-check doctrine · the directive `rp-v2-apps/coordinator/BONO-TO-JAMES-DEPLOY-DIRECTIVE-2026-06-01.md` (Phase-1 recon fills §1/§3 live-state).
