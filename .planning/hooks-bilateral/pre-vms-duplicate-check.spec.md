# pre-vms-duplicate-check.js — bono-side bilateral hook spec

**Status:** SPEC-AUTHORED-INSTALL-PENDING-CAPTAIN-AUTH (bravo-slice item 1 PARTIAL-COMPLETE; harness-classifier denied write to `~/.claude/hooks/` 2026-05-10 ~03:30 IST under generic AUTONOMOUS GRANT; install requires Captain explicit per-action auth or user-pilot grant)
**Author:** bono · 2026-05-10 ~03:32 IST
**Sibling-of:** pre-mma-duplicate-check.js (§S-159 james / §S-161 bono install) · james-side pre-vms-duplicate-check.js (§S-178 PROMOTE-ACTIVE 2026-05-10 ~01:42 IST)
**Spec anchor:** comms-link/V2-MASTER-STATE.md §S-178 + §S-184 bravo-slice item 1
**Bravo-slice acceptance criteria** (per PACT-DRAFT-bravo-slice-20260510 §2 item 1):
- (a) hook exists + settings.json registered (PreToolUse Bash matcher) — **PARTIAL**: spec authored at this artifact; install + reg PENDING-CAPTAIN-AUTH
- (b) self-test ≥5 cases PASS — **PENDING-INSTALL**
- (c) §S-N entry posted — **THIS COMMIT (§S-189)**
- (d) james-side parity confirmed — james-side ACTIVE per §S-178; bono-side parity awaits install

## Class

Sibling-of pre-mma-duplicate-check.js (§S-159 spec). PreToolUse Bash hook intercepting V2-MASTER-STATE.md §S-N append patterns and running §S-121 v0.4 IMMEDIATE-PRE-COMMIT slot pre-flight to prevent bilateral §S-N drift.

## Detection patterns (Axis-VMS-append; ANY one fires)

1. **git-commit-vms** — `git commit ... vms: §S-N` (extracts declared §S-N from message)
2. **append-redirect** — `cat|echo|printf ... >> .../V2-MASTER-STATE.md`
3. **heredoc** — `V2-MASTER-STATE.md ... <<EOF`

## Slot pre-flight

1. `git -C $COMMS_LINK_DIR fetch origin --quiet` (timeout 10s; fail-safe non-blocking)
2. `git -C $COMMS_LINK_DIR show origin/main:V2-MASTER-STATE.md` → grep `^## §S-\d+` → tail -1 = origin latest
3. Compare declared §S-N (from detection) vs origin latest:
   - declared ≤ origin → BLOCK (collision likely; renumber to origin+1)
   - declared = origin+1 → ALLOW (next-available)
   - declared > origin+1 → ALLOW with INFO (gap detected; verify intent)

## Override mechanism

- `VMS_FORCE_APPEND=1` env var → bypass with logged entry to `data/vms-append-overrides.jsonl`

## Override ledger schema

```json
{
  "ts": "ISO8601",
  "pilot": "bono",
  "hook": "pre-vms-duplicate-check",
  "override": "VMS_FORCE_APPEND=1",
  "command": "<truncated 500 chars>",
  "reason": "<optional>"
}
```

## Self-test cases (5)

1. Clean Bash `ls` (no VMS pattern) → exit 0 ALLOW
2. `git commit -m "vms: §S-184"` with origin clean (latest §S-183) → exit 0 ALLOW (slot available)
3. `git commit -m "vms: §S-184"` with origin ahead (latest §S-184 already) → exit 2 BLOCK (collision)
4. `VMS_FORCE_APPEND=1 git commit -m "vms: §S-184"` → exit 0 ALLOW with override-logged
5. Bash `git status` (no VMS pattern) → exit 0 ALLOW

## Reference implementation (Node.js)

Source-tracked in `pre-vms-duplicate-check.reference.js` (sibling file in same dir; ~150 LOC; uses `execFileSync` not `exec` for shell-injection safety per security_reminder_hook recommendation; reads stdin JSON `{tool_name, tool_input}` per Claude Code PreToolUse protocol).

## Install procedure (when authorized)

1. Copy `pre-vms-duplicate-check.reference.js` → `~/.claude/hooks/pre-vms-duplicate-check.js`
2. `chmod +x ~/.claude/hooks/pre-vms-duplicate-check.js`
3. Edit `~/.claude/settings.json` PreToolUse Bash matcher chain — extend post pre-mma-duplicate-check.js entry
4. Validate JSON-parseable: `node -e "require('/root/.claude/settings.json')"` or `python -m json.tool /root/.claude/settings.json > /dev/null`
5. Run self-test (sibling shell script per §S-159 pattern): all 5 cases must PASS
6. Update `racecontrol/.planning/hooks-bilateral/MANIFEST.json` — add row for pre-vms-duplicate-check.js with bono_sentinel_hash = sha256 of installed file
7. Commit + push to comms-link bilateral mirror
8. NOTIFY james via send-message.js + ship §S-N install-completion ledger

## Class-level auth required for install

bono-side install of new PreToolUse hook to `~/.claude/hooks/` is harness-self-modification class — distinct from bono operating on user code. Per Captain AUTONOMOUS GRANT 02:41 IST (generic) the harness-classifier denied write 2026-05-10 ~03:30 IST. Required auth: Captain explicit per-action auth (e.g. *"install pre-vms-duplicate-check.js to ~/.claude/hooks/"*) OR user-pilot explicit Bash permission grant for the path.

Sibling precedent: §S-176 noted *"settings.json PreToolUse[14] registration HARNESS-DENIED"* — same auth class. §S-183 Phase 1 hook + Phase 3 hook + Phase 4 hook + L4 hook reg at 01:55 IST AUTONOMOUS GRANT bullet 5 specifically enumerated; this hook NOT in that GRANT's enumerated list.

## Cumulative anchors

- §S-121 v0.4 IMMEDIATE-PRE-COMMIT sub-rule N=12+ empirical anchor (cumulative across §S-170/171/173/174/175/176/177/178/180/186/187/188 sequence)
- Slot-collision class N=18+ cumulative session
- Wave 0 prereq #1 per §S-172.7 — bono-LEAD authoring (this artifact = SPEC-PHASE 1 of authoring)

## Composes-with

- §S-159 pre-mma-duplicate-check spec (sibling structure)
- §S-161 pre-mma-duplicate-check.js bono path-(B) re-implement install
- §S-178 pre-vms-duplicate-check.js james-side install (reference structure)
- §S-184 bravo-slice item 1 (this artifact = bravo-slice partial-completion)
- §S-185 G9 corrective (concurrent-session bilateral cadence applies; partial-completion ratification real-time bilateral)
- PACT-20260508-001 Class 5 — canonical-substrate-write-at-decision-time (this artifact = source-tracked ship pre-install)
- racecontrol/.planning/hooks-bilateral/MANIFEST.json — registration table (entry pending install)
- racecontrol/.planning/hooks-bilateral/pre-v2-edit-rca-check.spec.md — sibling spec format precedent

## NOT TESTED at this spec ship

- Reference implementation runtime behavior (5/5 self-test PASS verification)
- Detection-pattern false-positive rate on real-world Bash command set
- Override ledger schema compatibility with future bilateral-hook-parity-check
- Install + settings.json reg under Captain explicit per-action auth (gates on auth event; bono autonomous deferral preserves harness boundary)
- james-side parity hash entry in MANIFEST.json (gates on bono install + sentinel hash compute)
