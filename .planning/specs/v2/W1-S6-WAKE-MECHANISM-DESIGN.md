# W1-S6 Wake-Mechanism Design Skeleton

**Status:** DESIGN-SKELETON (Captain explicit-verb authored 2026-05-15 ~07:30 IST · Captain quote *"author wake-mechanism design-skeleton at racecontrol/.planning/specs/v2/W1-S6-WAKE-MECHANISM-DESIGN.md"*)
**Anchor:** §S-352 (this commit-cycle)
**Composes-with:** §S-350 wake-mechanism SCP probe 3/3 PASS · §S-347 G33v7 4-attempt model · `briefings/bono/memory/reference_james_wake_mechanism.md` (Phase 1 SSH-launch · pre-V2 substrate) · `scripts/bono/james-ctl.sh` (ON/OFF/STATUS/RESTART idempotent)

**Scope:** Design contract — NOT implementation. Each phase enable requires separate Captain auth verb.

---

## §1 — Motivation + non-goals

### Motivation

bono (Linux VPS · 100.70.177.44) needs an authenticated path to wake james (Windows AI-SERVER · 100.82.33.94) when bono detects an event class that requires james AI-pilot attention. Examples:
- Halo-doorbell V.2 pod-rc-agent-dead state (current carry-forward · §S-350 carry · §S-351 N=2 confirmed) — james needs to investigate venue side
- Substrate-bridge V1↔V2 RCA blocker — bono detects but can't unblock without james-side venue knowledge
- §14.4 DEPRECATE-trigger surface where bilateral consensus is needed within hours, not days
- Cross-pilot AMPLIFIER round-trip <30min target per V-LBAC-3

Today's gap: bono notifies via `comms.db + WebSocket`, but james-side AI session is poll-on-prompt (per comms-link/CLAUDE.md "Cross-pilot notification latency floor"). Worst-case: hours/days latency if james session not invoked. Wake-mechanism = push-on-event substrate that does **NOT** require bono to assume james session-readiness.

### Non-goals

- **NOT a replacement for comms.db + WebSocket** — message-delivery substrate continues to be the canonical text-relay path. Wake mechanism only triggers james session-active state; messages still flow via comms.db.
- **NOT a unilateral pilot-mutation channel** — wake mechanism does NOT give bono ability to execute arbitrary commands on james-side. The receive-side handshake constrains the wake action to: (a) launch the Claude Code session attached to james-side terminal, (b) write a wake-event audit-log entry. Nothing else.
- **NOT for fleet/pod targets** — pods are managed via rc-watchdog + rc-sentry. This wake-mechanism is specific to the james AI-pilot session.
- **NOT for Server .23 racecontrol** — that has its own supervision chain (`.bat-respawn` + watchdog).
- **NOT a substitute for §S-146 V1↔V2 RCA** — wake mechanism enables james to respond faster; it does not relax doctrine gates.

---

## §2 — Path-discovery results (from §S-350 MAO-3 SCP probe · 3/3 PASS)

3-layer probe set per [[capability-claim-without-probe]] doctrine:

| Layer | Probe | Result | Evidence |
|---|---|---|---|
| L1 Network | `tailscale status \| grep 100.82.33.94` | PASS | `ai-server james@ windows active; relay "blr"; tx 264MB rx 834MB` (active relay; data flowing) |
| L2 Transport | `curl http://100.82.33.94:8766/health` | PASS | 200 OK in ~80ms (per §S-350) |
| L3 Auth | `ssh -o BatchMode=yes bono@100.82.33.94 'echo OK'` | PASS | Returns `OK` cleanly; `hostname` returns `AI-SERVER` |

**Conclusion:** SSH BatchMode path bono→james-side AI-SERVER is operational. No new infrastructure needed for the transport layer. Design work focuses on the **wake action** + **audit-log contract** + **handshake protocol**.

---

## §3 — Protocol spec (DRAFT · sub-phase ratify required)

### §3.1 — Wake action surface

bono-side wake invocation:

```
$ /root/bin/wake-james.sh "<event_class>" "<rationale_50char>"
```

Underlying call:

```
ssh -o BatchMode=yes -o ConnectTimeout=10 bono@100.82.33.94 \
    "C:\Users\bono\racingpoint\scripts\wake-receive.bat" \
    "<event_class>" "<rationale_50char>" "<bono_audit_id>"
```

`wake-receive.bat` on james-side:
1. Parse args (`event_class`, `rationale`, `bono_audit_id`)
2. Validate `event_class` against allowlist (see §3.4)
3. Write entry to `C:\Users\bono\racingpoint\state\wake-audit.jsonl`
4. Check if Claude Code session for james is already active (poll for `~/.claude/state/james-session-active.flag` within 60s)
5. If not active: launch (`start "" cmd /c "<launch_claude_cmd>"`)
6. Return exit code: `0` = wake delivered, `1` = wake rejected (allowlist), `2` = wake errored (launch fail)

### §3.2 — Audit-log contract

Both sides write to append-only JSONL:

**bono-side** `/root/.claude/state/wake-dispatch-audit.jsonl`:
```json
{"ts":"<ISO>","direction":"out","target":"james","event_class":"<allowlisted>","rationale":"<50char>","ssh_exit":<int>,"audit_id":"<uuid>"}
```

**james-side** `C:\Users\bono\racingpoint\state\wake-audit.jsonl`:
```json
{"ts":"<ISO>","direction":"in","from":"bono","event_class":"<allowlisted>","rationale":"<50char>","action":"launched|already-active|rejected|errored","bono_audit_id":"<uuid>"}
```

Both sides match on `audit_id` ↔ `bono_audit_id` for cross-host traceability.

### §3.3 — Handshake protocol

- bono generates `audit_id` (UUID v4) BEFORE invoking SSH
- bono appends `direction=out` entry FIRST, then invokes SSH
- james-side `wake-receive.bat` MUST consume `audit_id` and echo it back in its audit entry
- If SSH timeout >10s OR exit≠0: bono appends `ssh_exit` to its entry and does NOT retry automatically (per §3.5 — explicit Captain re-auth required for retry-loop class)

### §3.4 — Allowlist (initial)

| event_class | Use case | Rate-limit |
|---|---|---|
| `halo-rc-agent-dead` | Pod rc-agent dead despite pods alive | 1/30min |
| `v1-v2-rca-blocker` | §S-146 boundary detected by bono, needs james venue knowledge | 3/24h |
| `amplifier-rt-30min-target` | V-LBAC-3 30min round-trip target threatened | 5/24h |
| `captain-explicit-dispatch` | Captain instructed bono to wake james | unlimited |

james-side validates `event_class` against this list; unrecognized class = rejection (exit 1) + audit entry with `action=rejected`.

### §3.5 — Failure modes

- SSH timeout >10s — log + give up (NO retry loop). bono falls back to comms.db notify with delayed-delivery acknowledgement.
- james-side launch error — james AI-pilot may already be in unrecoverable state; surface to Captain via WhatsApp escalation path
- Allowlist rejection — bono should not have invoked with that class; this is a design bug (bono code-path validates before invoking, BUT defense-in-depth on james-side)
- Rate-limit exceeded — soft-block, log, bono backs off to default poll-on-prompt cadence

---

## §4 — Windows cmd.exe shell constraints (empirical anchor §S-350)

james-side AI-SERVER's default SSH-receive shell is **Windows cmd.exe** (not WSL bash, not PowerShell). Verified via §S-350 probe 3: `ssh bono@100.82.33.94 'echo OK; whoami; date; uname -a'` returned the literal string `OK; whoami; date; uname -a` — Unix `;`-chains are NOT interpreted as command separators.

**Constraints flowing into design:**

1. **One command per SSH invocation** — chain via batch file, NOT via shell separators
2. **Batch file uses Windows quoting** (`"..."`) — single-quote does not delimit on Windows
3. **Path separators** — Windows backslash; bono-side script must use cmd-compatible paths
4. **No `&&`/`||`/`|`** in inline SSH; use batch file `if errorlevel` instead
5. **Environment variables** — `%VAR%` not `$VAR`; bono should pass values as positional args, not via env propagation
6. **Output line-endings** — CRLF on Windows; bono parsing must tolerate both

PowerShell alternative considered: would enable Unix-shell-like syntax, but adds startup latency (~1-2s) and changes the auth surface. **Decision:** stick with cmd.exe for v1; PowerShell wrapper deferred to v2 if shell ergonomics become a blocker.

---

## §5 — Enable phases (each requires separate Captain auth verb)

| Phase | Scope | Captain auth required |
|---|---|---|
| **Phase 0 — design ratify** | This document ratified bilaterally | *"ratify wake-mechanism design-skeleton"* OR AMPLIFIER round-trip |
| **Phase 1 — bono-side staging** | Write `/root/bin/wake-james.sh` + state-file + audit-log skeleton (no SSH invocation yet) | *"enable wake-mechanism on bono · staging"* |
| **Phase 2 — james-side install** | Author `wake-receive.bat` + state-dir on james-side; bilateral commit | *"authorize wake-receive.bat install on james-side"* + james-side harness auth |
| **Phase 3 — wire bono→james** | Enable SSH invocation on `halo-rc-agent-dead` class only · soak 7d | *"enable wake-mechanism live · halo-rc-agent-dead class only"* |
| **Phase 4 — expand allowlist** | Add other event_classes per §3.4 · per-class ratify | per-class verb |
| **Phase 5 — bidirectional** | james→bono wake symmetry (if needed) | TBD pending Phase 3 metrics |

**Phase-gate criteria:** each phase requires (a) zero unauthorized wakes during prior phase, (b) audit-log parity bono↔james ±0% (no missing entries), (c) Captain ratify before advance.

---

## §6 — Not-yet-decided gates (open Q-DECs requiring Captain disposition before Phase 1)

| Q | Question | Bono-rec |
|---|---|---|
| Q-WAKE-1 | Wake script path: `/root/bin/wake-james.sh` (in PATH) vs `/root/comms-link/scripts/bono/wake-james.sh` (canonical with james-ctl.sh sibling)? | Latter — sibling to existing `james-ctl.sh` keeps wake-mechanism in `comms-link/scripts/bono/` tree with symlink to `/root/bin/` |
| Q-WAKE-2 | Audit-log location james-side: `C:\Users\bono\racingpoint\state\` vs PowerShell-managed appdata? | Former — symmetric with bono path structure + visible to james AI-pilot via existing memory-paths |
| Q-WAKE-3 | Should bono auto-invoke wake on `halo-rc-agent-dead` once detected, OR require Captain dispatch verb each time? | Auto-invoke after Phase 3 soak — Captain can revoke via halt verb; manual-only Phase 3 |
| Q-WAKE-4 | Rate-limit enforcement layer: bono-side caller (before SSH) vs james-side receiver (in `.bat`) vs both? | Both — defense-in-depth; bono prevents wasted SSH calls; james prevents allowlist bypass via direct james-side spoofing |
| Q-WAKE-5 | Should `wake-audit.jsonl` rotate? Size cap? | Yes — 10MB rotate, 30-day retention; bilateral parity |
| Q-WAKE-6 | Bilateral verification: should both sides cross-check audit entries via comms.db sync? | Yes Phase 3+ — bono periodic-poll `wake-audit.jsonl` from james-side via existing relay; raises alert on missing-entry class |

---

## §7 — Composes-with + cross-refs

- **§S-350** — MAO-3 SCP probe 3/3 PASS path-discovery (this design builds on those reach results)
- **§S-351** — cross-host parity probe (composes for verification probe class · same primitives)
- **§S-347** — G33v7 4-attempt model (W1-S6 dispatch.rs · this design is sibling to that work but independent surface)
- **`briefings/bono/memory/reference_james_wake_mechanism.md`** — pre-V2 SSH-launch substrate · this design supersedes that for V2-era
- **`scripts/bono/james-ctl.sh`** — existing on/off/status/restart pattern · wake-mechanism is the EVENT-DRIVEN variant; sibling
- **comms-link/CLAUDE.md "Cross-pilot notification latency floor"** — wake mechanism IS Option B from `feedback_doorbell_pollmodel_not_pushmodel_gap.md` post-V2.0-ratify
- **`pre-bash-destructive-git-branch-check.js`** + **`pre-push-maor-check.js`** — branch + MAOR gates on bono-side commits authoring this design (independent of harness self-mod chain)

---

## §8 — Verify-by (this design)

- V-WAKE-1: Phase 0 ratify by Captain explicit verb OR AMPLIFIER round-trip within 48h
- V-WAKE-2: Phase 1 staging file exists + audit-log entry written for first invocation (dry-run mode · no SSH yet)
- V-WAKE-3: Phase 2 bilateral install commit lands with bono↔james both sides
- V-WAKE-4: Phase 3 first live wake delivers `halo-rc-agent-dead` event within 60s end-to-end · audit entries match on `audit_id`
- V-WAKE-5: Phase 3 7d soak with zero unauthorized invocations + zero audit-log drift bono↔james

**Stale-at:** 2026-06-15 (1 month from authoring; design refresh required if any phase not yet at ratify state)

---

End of W1-S6 Wake-Mechanism Design Skeleton.
