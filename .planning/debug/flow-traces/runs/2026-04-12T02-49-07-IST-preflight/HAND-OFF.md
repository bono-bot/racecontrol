# Hand-off — F1 25 Staff-Launch Trace, Pre-flight Complete

**Written:** 2026-04-12 ~03:00 IST
**Written by:** James (autonomous, while user asleep)
**Read this first when you wake up.**

---

## TL;DR in 5 lines

1. ✅ Pre-flight is clean. Pod 4 idle, blanked state visible, observation channels validated (log tail, DXGI screenshot, tasklist).
2. ⚠️ **P0 secret leak found in Pod 4 rc-agent.toml** — rotate the OpenRouter API key NOW before touching anything else. Details: `FINDINGS.md` §"Non-trace findings".
3. 🎯 **Strong Bug 2 lead found without firing a single launch**: F1 25 game_id namespace mismatch between `rc-agent.toml` (`f1_25`) and `STEAM_APP_IDS` (`f1_25_ac`). See `FINDINGS.md` §"Candidate 1".
4. 🎯 Second lead: Assetto Corsa EVO + LMU are installed and TOML-configured but **missing from `STEAM_APP_IDS`** entirely. Same bug class as Candidate 1. See `FINDINGS.md` §"Candidate 2".
5. 🚫 No code committed. No state changed on any pod/server. Phase 5 gate honoured.

---

## What to do in what order when you wake up

### Step 0 — Rotate the OpenRouter key (3 minutes)

**Do this before anything else.** The key was in Pod 4's `rc-agent.toml` and was pulled into my conversation context when I ran `Get-Content rc-agent.toml` via `/exec`. OpenRouter's leaked-key scanner may auto-revoke it.

1. Open `openrouter.ai/settings/keys`, rotate the key
2. Decide target location for the new key:
   - **Recommended:** `[mma]` section in `C:\RacingPoint\racecontrol.toml` on the server (per MMA-21 standing rule, read by rc-agent via WS config push)
   - **Alternative:** `RCAGENT_OPENROUTER_KEY` env var in the pod's `start-rcagent.bat`
3. Strip `openrouter_api_key` from `deploy/configs/rc-agent-pod{1-8}.toml` in the repo
4. Deploy config to all 8 pods (existing deploy-pod config pipeline)
5. Add `scripts/security/check-pod-configs.sh` that greps all `rc-agent-pod*.toml` for `sk-or-` and fails CI if found — plug into `stage-release.sh` pre-build
6. Commit with `fix(security): move openrouter key out of pod TOML`

### Step 1 — Decide: fix-first or trace-first for Bug 2?

I found what looks like the root cause of Bug 2 (F1 25 not spawning) **without firing a real launch**. The question is: do you want to **verify the hypothesis with a real trace first** (the methodology's intent), or **apply the fix first then verify both the fix and the methodology at once**?

**Methodology-strict path (recommended — it's what you asked for):**
   1. Wake up, read this
   2. Fire ONE F1 25 launch from `kiosk/staff` (hops 0-50 per flow-trace MAP)
   3. I run the 5-channel capture while you watch
   4. We see the actual divergence happen in real-time — confirms which code path the TOML `f1_25` → `3059520` mismatch breaks
   5. THEN apply the fix (single commit at identified divergence layer)
   6. Re-run the trace, confirm the bug is closed
   7. Close the loop per CLD Step 4

**Fix-first path (faster, methodology-loose):**
   1. Apply the STEAM_APP_IDS fix (add ACE, LMU, collapse f1_25/f1_25_ac to just f1_25)
   2. Build + deploy rc-agent to Pod 4 only (canary)
   3. Fire F1 25 launch from kiosk staff
   4. If it works → deploy fleet-wide + regression test
   5. If it doesn't → we've learned the divergence isn't here, go back to methodology-strict

**My recommendation: methodology-strict.** You explicitly asked for PoE + regression + close-the-loop discipline and this is exactly the shape of problem that benefits from it. Firing one real launch with the observation stack running will produce data that either confirms my hypothesis OR reveals I'm wrong — either outcome is a net win. Fix-first is tempting but it's the exact "jump from symptom to fix" pattern the Cause Elimination Process standing rule warns against.

### Step 2 — Run the F1 25 trace

This is what my autonomous window stopped short of doing, because firing a real launch while you sleep was too much state risk unsupervised.

**Prereqs verified (Phase 1 MAP hops 0-9):**
- Pod 4 idle, blanked state ✅
- rc-agent b1fc9484 running in Session 1 ✅
- Steam client up on Pod 4 ✅
- Pod `/exec` + `/screenshot` reachable unauthenticated ✅
- Comms-link REALTIME ✅
- Venue closed (no customer collision risk) ✅ at 02:49 IST when I checked

**Fire sequence (yours to execute):**

1. Tell me: "Starting F1 25 trace" so I flip instrumentation to RUNNING
2. I capture hop-0 baseline screenshot + log offset marker on Pod 4
3. You open `http://192.168.31.23:3300/kiosk/staff` in a browser (James PC or phone — whichever you physically have)
4. Log in as staff
5. Select Pod 4, F1 25, 30-min tier, hit launch
6. I capture continuously into `runs/<timestamp>-f1_25-attempt-01/` during hops 10-50
7. Within ~3 minutes, I produce a divergence report

### Step 3 — Run the AC baseline if F1 25 trace is ambiguous

**Only run this if the F1 25 trace result is unclear.** If F1 25 cleanly shows `game_id mismatch → dispatch failure`, we don't need the AC control case — the methodology says the baseline is for when you can't explain the failure, not when it's self-evident.

---

## What I verified for you (so you don't have to re-check)

**Channels (read `FINDINGS.md` §"Observation Channel Validation" for the evidence):**

| Channel | Status | Command that works |
|---|---|---|
| P1 — rc-agent log tail | ✅ | `powershell -NoProfile Get-Content C:\RacingPoint\rc-agent-.2026-04-11.jsonl -Tail N` via `/exec` |
| P2 — DXGI screenshot | ✅ | `GET http://192.168.31.88:8090/screenshot?method=dxgi&quality=80&scale=50` |
| P3 — tasklist | ✅ | `tasklist /V /FO CSV` via `/exec` (full list, filter client-side) |
| P4 — server log tail | ⏳ not yet tested | Server runs on .23, needs SSH or /exec on racecontrol's exec endpoint — deferred |
| P5 — go2rtc snapshot | ⏳ backup only | Demoted from primary — DXGI is pixel-perfect, no need |

**Pod 4 state at 02:49 IST:**
- Screen: blanked (Racing Point eSports logo on black), IDLE-01 fix visibly working
- Processes: rc-agent alive Session 1, Steam idle, NO games running
- Inventory: 4 games found (bug — see `FINDINGS.md`)
- Pre-flight + process_guard: DISABLED (pre-release testing toggle)
- `/exec` and `/screenshot`: unauthenticated (service key is unset in pod env)
- Parallel F-01/F-02 session: active but noise-only (probing with blocked commands every 2 min, no state change)

**Commands that will NOT work on Pod 4 /exec (learned the hard way):**
- Any command with shell metachars `& | > < \` ; ^` — rejected by filter
- PowerShell `-Command "..."` with embedded quotes — outer quotes mangled
- Bash inline JSON with backslashes — Bash mangles `\\`, write to file instead
- Windows `tasklist /FI "..."` — quoting ruined by cmd.exe layer, use raw tasklist + client-side filter

---

## What I intentionally did NOT do

Per the CLD methodology and your explicit "no code changes until Phase 5 + user approval" rule:

- ❌ Did not write any code
- ❌ Did not commit anything
- ❌ Did not fire a launch (state risk unsupervised)
- ❌ Did not drive Playwright against kiosk staff
- ❌ Did not restart rc-agent
- ❌ Did not touch any pod config
- ❌ Did not deploy anything
- ❌ Did not send comms-link messages to the parallel session (no clean channel, and contamination is already low)

---

## Files to read when you wake up

1. This file — `HAND-OFF.md`
2. `FINDINGS.md` — full evidence + divergence candidates + fix sketches
3. `observations/pod4-hop0-idle-dxgi.jpg` — visual proof of IDLE-01 fix working right now

---

## Open questions from the flow-trace doc that are now ANSWERED

From `f1_25-staff-launch.md` §13:
1. ✅ `runs/` location — using `.planning/debug/flow-traces/runs/<ts>-<scope>/`
2. ✅ Background capture loops — deferred, on-demand capture worked fine for pre-flight scope
3. ✅ User-eye log format — replaced with deterministic pod-side DXGI screenshots + log tail (camera demoted)
4. ⏳ AC baseline — you decide per HAND-OFF §"Step 3"
5. ✅ Parallel F-01/F-02 — contamination verified low (noise only), no hold needed
6. ✅ Verification target — Pod 4 via DXGI, not NVR

---

## If you want me to keep going while you sleep

Unlikely by the time you read this, but: if you wake up, skim this, decide "yes keep tracing", leave me a prompt saying "keep going, launch from my phone at <time>", I can capture the trace on command.

Otherwise: read, decide step 0 rotate → step 1 pick path → step 2 fire → step 3 conditional. I'll be waiting to execute the capture side.
