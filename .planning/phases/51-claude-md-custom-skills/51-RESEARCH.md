# Phase 51: CLAUDE.md + Custom Skills — Research

**Researched:** 2026-03-20 IST
**Domain:** Claude Code project context files and custom skill system
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- CLAUDE.md replaces MEMORY.md as the primary context source for Racing Point operations
- CLAUDE.md includes everything: full network map (IPs + MACs), crate names, binary naming, deploy rules, billing rates, 4-tier debug order, brand identity, constraints, security cameras, server services
- CLAUDE.md includes full tables — pod IPs with MACs, camera IPs, server ports
- CLAUDE.md includes standing process rules — Refactor Second, No Fake Data, Cross-Process Updates, Prompt Quality Check, Learn From Past Fixes, Bono comms protocol, deploy rules
- MEMORY.md shrinks to: James Vowles identity, Bono relationship, Uday info, timezone preference, feedback memories, current milestones, open issues, recent commits (~60 lines)
- CLAUDE.md lives at repo root: `racecontrol/CLAUDE.md` (auto-loaded by Claude Code on session start)
- `/rp:deploy`: `disable-model-invocation: true`, user-only. Sequence: cargo build --release --bin rc-agent → size check → copy to `C:\Users\bono\racingpoint\deploy-staging\rc-agent.exe` → verify. Outputs pendrive deploy command. Does NOT push to any pod.
- `/rp:deploy-server`: `disable-model-invocation: true`, user-only. Full pipeline: cargo build --release --bin racecontrol → kill old process automatically → swap binary → start new → verify :8080 returns 200 → git commit → notify Bono via comms-link INBOX.md
- `/rp:pod-status`: model-invocable (read-only, safe to auto-trigger). Queries `/api/v1/fleet/health`, extracts specific pod data. Dynamic IP injection from pod number.
- `/rp:incident`: model-invocable. Auto-query + auto-fix, confirm destructive only, auto-logs to LOGBOOK after fix confirmed. Falls back to guide-only if server unreachable.

### Claude's Discretion
- Exact CLAUDE.md section ordering and formatting
- How to handle MEMORY.md migration (which memories to keep vs move)
- Skill file naming conventions within `.claude/skills/`
- Error handling patterns within skills (what to show on failure)

### Deferred Ideas (OUT OF SCOPE)
- HOOK-01 (SessionStart context re-injection after compaction) — v9.x future
- HOOK-02 (PostToolUse auto-notify Bono on git commits) — v9.x future
- /rp:logbook as a standalone skill — partially absorbed into /rp:incident auto-logging
- /rp:fleet-health (summarize all pod states) — v9.x future
- /rp:new-pod-config (generate pod TOML) — v9.x future
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SKILL-01 | James's Claude Code sessions auto-load Racing Point project context (pod IPs, crate names, naming conventions, constraints) from a project-level CLAUDE.md | CLAUDE.md format, auto-load mechanism, content inventory from MEMORY.md |
| SKILL-02 | James can invoke `/rp:deploy` to build rc-agent and stage the binary, with `disable-model-invocation: true` | Skill file format, frontmatter fields, bash command composition, staging path |
| SKILL-03 | James can invoke `/rp:deploy-server` to build racecontrol, stop old process, swap binary, verify :8080 | Kill pattern, process verification, git commit pattern, Bono notification |
| SKILL-04 | James can invoke `/rp:pod-status <pod>` to query any pod's rc-agent status via dynamic IP injection | fleet/health endpoint shape, pod number → IP mapping table |
| SKILL-05 | James can invoke `/rp:incident <description>` to get structured incident response following the 4-tier debug order | 4-tier debug order, LOGBOOK.md format, destructive action gates, fallback pattern |
</phase_requirements>

---

## Summary

Phase 51 is a pure content-authoring phase. No Rust code changes. No pod deploys. Everything lives in two new directories at repo root: `CLAUDE.md` (single file) and `.claude/skills/` (four markdown skill files). The technical surface area is entirely within Claude Code's project file system — a well-documented, stable feature.

The central challenge is not technical but editorial: the CLAUDE.md must be comprehensive enough that a fresh Claude session with zero MEMORY.md can operate the venue, yet structured so Claude can scan it quickly under context pressure. The skills must be precise enough to execute deterministic build pipelines without user typing, yet explicit about confirmation gates for destructive actions.

The existing `tdd-debug` skill in `~/.claude/skills/tdd-debug/SKILL.md` is the definitive reference for skill file format in this installation. It uses pure markdown with no frontmatter — just a `## Trigger` section defining invocation conditions and structured workflow sections. The GSD system uses a different format with YAML frontmatter (including `disable-model-invocation`); the racecontrol skills will follow the GSD convention since that is what the planner expects.

**Primary recommendation:** Author CLAUDE.md as a reference manual (tables, rules, facts) not a narrative. Author skills as deterministic runbooks (step-by-step bash sequences) not conversation starters. Both formats are read by the same model — dense structured content is more reliable than prose.

---

## Standard Stack

### Core

| File | Location | Purpose | Why |
|------|----------|---------|-----|
| `CLAUDE.md` | `racecontrol/CLAUDE.md` | Auto-loaded project context for every Claude Code session | Claude Code reads this file on session start when CWD is racecontrol |
| `SKILL.md` files | `racecontrol/.claude/skills/{skill-name}/SKILL.md` | One file per skill, invoked by `/rp:{name}` slash command | Claude Code skill system discovers `SKILL.md` in `.claude/skills/` subdirectories |

### Claude Code File Discovery (VERIFIED from settings.json + existing skills)

Claude Code loads context files in this order on session start:

1. **Global `~/.claude/CLAUDE.md`** — applies to all sessions (none exists currently)
2. **Project `{repo-root}/CLAUDE.md`** — auto-loaded when CWD is inside the repo (THIS is what we create)
3. **Skills** — discovered from `.claude/skills/{name}/SKILL.md` in both global `~/.claude/skills/` and project `.claude/skills/`

The existing global skill (`~/.claude/skills/tdd-debug/SKILL.md`) confirms the discovery pattern. Project-level `.claude/skills/` is a subdirectory of the repo root — same level as `.planning/`.

### Skill Frontmatter Fields (from GSD conventions + CONTEXT.md decisions)

```yaml
---
name: rp-deploy
description: Build rc-agent release binary and stage for pod deployment
disable-model-invocation: true
---
```

| Field | Values | Effect |
|-------|--------|--------|
| `name` | `rp-deploy`, `rp-deploy-server`, `rp-pod-status`, `rp-incident` | Slash command name (without namespace) |
| `description` | One-line summary | Shown in skill picker UI |
| `disable-model-invocation` | `true` or omit | When `true`: skill is never auto-triggered by Claude; only James can invoke explicitly. Required for SKILL-02, SKILL-03. |

### No New Dependencies

This phase installs zero new software. All commands used by skills are:
- `cargo build` — already on PATH via `export PATH="$PATH:/c/Users/bono/.cargo/bin"`
- `bash`/shell built-ins — `cp`, file size checks, `grep`, `curl`
- `git` — already installed and configured
- `python` — for INBOX.md comms-link notification (already exists)

---

## Architecture Patterns

### Recommended File Structure

```
racecontrol/
├── CLAUDE.md                        # Project context (auto-loaded by Claude Code)
└── .claude/
    └── skills/
        ├── rp-deploy/
        │   └── SKILL.md             # /rp:deploy — rc-agent build + stage
        ├── rp-deploy-server/
        │   └── SKILL.md             # /rp:deploy-server — full server pipeline
        ├── rp-pod-status/
        │   └── SKILL.md             # /rp:pod-status — query pod health
        └── rp-incident/
            └── SKILL.md             # /rp:incident — structured incident response
```

### Pattern 1: CLAUDE.md as Operator Reference

**What:** A dense reference document with tables and rules — not a narrative document.

**Sections (in recommended order):**
1. **Project Identity** — repo name, purpose, James's role, key contacts
2. **Network Map** — all 8 pods (pod number, IP, MAC), server (.23 MAC + services), James (.27), POS (.20), router (.1)
3. **Crate Names & Binary Naming** — workspace layout, binary names, server naming rules
4. **Deployment Rules** — the 6-step kill→delete→download→size check→start→connect sequence, "never run pod binaries on James's PC" rule, test-before-upload definition
5. **4-Tier Debug Order** — deterministic → memory → local Ollama → cloud. Full tier definitions.
6. **Standing Process Rules** — Refactor Second, Cross-Process Updates, No Fake Data, Prompt Quality Check, Learn From Past Fixes, Bono comms protocol
7. **Billing & Rates** — 30min/₹700, 60min/₹900, 5min free trial, 10s idle threshold
8. **Fleet Endpoints** — `/api/v1/fleet/health` shape, rc-agent remote_ops :8090, webterm :9999
9. **Key File Paths** — racecontrol.toml on server, deploy-staging/, comms-link INBOX.md
10. **Build Commands** — cargo PATH export, crate-specific build commands
11. **Brand Identity** — colors, fonts (for front-end work)
12. **Security Cameras** — IPs, auth (for camera queries)
13. **Open Issues** — current blockers (keep short, update regularly)

**What NOT to include in CLAUDE.md:**
- Long prose narratives — use bullet points and tables
- Anything that changes weekly (recent commits, session notes) — those stay in MEMORY.md
- Duplicate content from MEMORY.md — pick one home for each fact

### Pattern 2: Deterministic Skill (disable-model-invocation: true)

**What:** A skill file that describes an exact sequence of bash commands. Claude executes each step, reports the output, and stops at a human approval gate before any irreversible action.

**When to use:** Build + deploy pipelines where the wrong action has permanent consequences (bad binary on server = downtime, bad binary on pod = brick).

**Example structure for /rp:deploy:**

```markdown
---
name: rp-deploy
description: Build rc-agent release binary and stage for pendrive deployment
disable-model-invocation: true
---

# /rp:deploy — RC-Agent Build + Stage

## When to Use
James explicitly runs `/rp:deploy` to prepare a new rc-agent binary for pod deployment.
Never auto-triggered.

## Steps

### Step 1: Export PATH
```bash
export PATH="$PATH:/c/Users/bono/.cargo/bin"
```

### Step 2: Build rc-agent release binary
```bash
cd /c/Users/bono/racingpoint/racecontrol && cargo build --release --bin rc-agent 2>&1
```
STOP if exit code != 0. Show full error output.

### Step 3: Verify binary size
```bash
ls -la /c/Users/bono/racingpoint/racecontrol/target/release/rc-agent.exe
```
Expected: > 8,000,000 bytes. STOP if smaller (truncated build).

### Step 4: Stage to deploy-staging
```bash
cp /c/Users/bono/racingpoint/racecontrol/target/release/rc-agent.exe \
   /c/Users/bono/racingpoint/deploy-staging/rc-agent.exe
```

### Step 5: Verify staged binary
```bash
ls -la /c/Users/bono/racingpoint/deploy-staging/rc-agent.exe
```

## Output
After success, show:
- Binary size in bytes
- The pendrive deploy command: `D:\pod-deploy\install.bat <pod_number>`
- Reminder: deploy to Pod 8 first, verify, then remaining pods
```

### Pattern 3: Model-Invocable Skill (read-only query)

**What:** A skill that Claude can invoke automatically when the context warrants it (e.g., James says "is pod 3 okay?"). Safe because it only reads state, never modifies it.

**Pod number → IP mapping** (embedded in skill, used for dynamic injection):

| Pod | IP |
|-----|----|
| 1 | 192.168.31.89 |
| 2 | 192.168.31.33 |
| 3 | 192.168.31.28 |
| 4 | 192.168.31.88 |
| 5 | 192.168.31.86 |
| 6 | 192.168.31.87 |
| 7 | 192.168.31.38 |
| 8 | 192.168.31.91 |

**Fleet health endpoint:** `http://192.168.31.23:8080/api/v1/fleet/health`

**Response shape (from fleet_health.rs):** Returns array of `PodFleetStatus` objects. Each has:
- `pod_number`: u32
- `pod_id`: Option<String>
- `ws_connected`: bool
- `http_reachable`: bool
- `version`: Option<String>
- `build_id`: Option<String>
- `uptime_secs`: Option<i64>
- `crash_recovery`: Option<bool>
- `ip_address`: Option<String>
- `last_seen`: Option<String>
- `last_http_check`: Option<String>

The skill uses `curl` to fetch the endpoint, then extracts the specific pod's entry by filtering on `pod_number`.

### Pattern 4: Incident Skill with Tiered Auto-Actions

**What:** A skill that accepts a free-text incident description, runs a structured diagnostic sequence, and proposes a fix — only asking for confirmation on destructive actions.

**4-Tier Debug Order (from MEMORY.md / to be in CLAUDE.md):**
1. **Deterministic fixes** — stale sockets, game cleanup, temp files, WerFault. No LLM needed. Auto-apply.
2. **Memory-based fixes** — check LOGBOOK for similar past incidents. Apply proven fix.
3. **Local Ollama** — query qwen3:0.6b on James's machine (:11434) for diagnosis.
4. **Cloud Claude** — escalate if all else fails. Not auto-triggered from skill.

**Confirmation gate rule:** Actions that read state (curl, tasklist) run automatically. Actions that modify state (process kill, billing end, rc-agent restart) require explicit confirmation from James before execution.

**LOGBOOK.md append format** (from LOGBOOK.md inspection):
```
| 2026-03-20 15:30 IST | James | {commit_hash} | {incident description + resolution} |
```

**Fallback pattern:** If `curl http://192.168.31.23:8080/api/v1/fleet/health` fails (server unreachable), the skill switches to guide-only mode: presents the 4-tier checklist without running any queries.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Pod number → IP lookup | A Rust lookup table or separate config file | Hardcoded table in each skill's markdown | Skills are self-contained markdown files — no runtime lookup needed, just a table in the skill body |
| Fleet health polling | A custom monitoring loop in the skill | One `curl` call to the existing `/api/v1/fleet/health` endpoint | The endpoint already returns all pod data in one response; filter client-side |
| Bono notification | A new comms system | Append to `C:\Users\bono\racingpoint\comms-link\INBOX.md` + git commit + push | comms-link is the established James→Bono channel. Already working. |
| Process kill on server | Remote exec via rc-agent :8090 | Direct `taskkill /F /IM racecontrol.exe` or equivalent in the deploy-server skill — runs on James's machine after copying binary | The skill runs on James's machine (.27), not on the server. Server binary swap happens via file copy to a shared path or the skill SSH's to server. Need to clarify the swap path. |
| LOGBOOK append | A separate tool | `echo "| timestamp | James | hash | summary |" >> LOGBOOK.md` in bash | LOGBOOK.md is plain markdown, bash append is sufficient |

**Key insight:** Skills are markdown documents that Claude reads and interprets as instructions — they do not run code directly. All bash commands in a skill are executed by Claude's built-in Bash tool. Keep skills as simple sequences of commands with clear human gates.

---

## Common Pitfalls

### Pitfall 1: Binary Not in Expected Location After cargo build
**What goes wrong:** `cargo build --release --bin rc-agent` succeeds but binary is at `target/release/rc-agent` (no `.exe`) when running in WSL, or at a different path in Windows Git Bash vs cmd.exe.
**Why it happens:** Windows paths differ depending on shell environment. The skill always runs in Claude Code's bash environment (Git Bash on Windows).
**How to avoid:** Use `/c/Users/bono/racingpoint/racecontrol/target/release/rc-agent.exe` as the absolute path. Always verify with `ls -la` after build before proceeding.
**Warning signs:** Binary size < 1MB (indicates build failure that appeared to succeed).

### Pitfall 2: racecontrol.exe Kill Leaves Server Port Occupied
**What goes wrong:** `/rp:deploy-server` kills the old racecontrol process, starts the new one, but `:8080` doesn't respond because the old process took ~3-5 seconds to fully exit and release the port.
**Why it happens:** Windows process termination is asynchronous — `taskkill` returns before the process fully exits.
**How to avoid:** After kill, poll for port release: `curl -sf http://192.168.31.23:8080/api/v1/health` should fail before starting new binary. Add a 3-second sleep between kill and start. Then poll for `:8080` up (max 30s, 5s intervals).
**Warning signs:** New process fails to start with "address already in use" error.

### Pitfall 3: disable-model-invocation Not Honored
**What goes wrong:** Claude auto-triggers `/rp:deploy-server` mid-conversation when James describes a deployment problem.
**Why it happens:** If the frontmatter `disable-model-invocation: true` is missing or malformed, Claude treats the skill as callable.
**How to avoid:** Verify the frontmatter is the very first content in the skill file, properly formatted with `---` delimiters. Test by describing a deploy scenario in conversation and confirming Claude only suggests the skill, not executes it.
**Warning signs:** Claude starts a build mid-conversation without James typing `/rp:deploy-server`.

### Pitfall 4: CLAUDE.md Context Too Large for Effective Use
**What goes wrong:** CLAUDE.md grows to 500+ lines and Claude starts truncating or de-prioritizing its content under context pressure.
**Why it happens:** Auto-loading happens once per session start; very large files compete with the working context for the session.
**How to avoid:** Keep CLAUDE.md under 300 lines. Use tables for dense data (pod IPs fit in 10 lines as a table). Move narrative content (like long incident history) to linked reference files. MEMORY.md's 280-line current state was already flagged as too large in the system reminder — the migration is a compression exercise, not just a copy.
**Warning signs:** Claude says "based on my training" for facts that should be in CLAUDE.md.

### Pitfall 5: Fleet Health Endpoint Returns Wrong Pod Data
**What goes wrong:** `/rp:pod-status pod-3` queries the fleet health endpoint but returns data for a different pod due to incorrect filtering.
**Why it happens:** The API response is an array indexed by registration order, not necessarily pod_number order. Filtering on array index instead of the `pod_number` field returns wrong data.
**How to avoid:** Always filter the JSON response by `pod_number` field, not array index. Use `jq '.[] | select(.pod_number == 3)'` or equivalent.
**Warning signs:** Pod status reports wrong IP or wrong connection state for the queried pod.

### Pitfall 6: Bono Notification Missing Git Push
**What goes wrong:** `/rp:deploy-server` appends to INBOX.md and commits but forgets to push, so Bono never sees the message.
**Why it happens:** The standing rule "always git push after committing" applies here too but is easy to omit from an automated sequence.
**How to avoid:** The skill's Bono notification step must be: append INBOX.md → `git add INBOX.md` → `git commit` → `git push`. All four steps as one sequence. Missing the push = Bono never gets the message.
**Warning signs:** comms-link has local commits ahead of origin after running the skill.

---

## Code Examples

Verified patterns for skill construction:

### Fleet Health Query + Pod Filter
```bash
# Source: crates/racecontrol/src/fleet_health.rs (PodFleetStatus struct)
# Fetch fleet health and extract Pod 3's data
curl -sf http://192.168.31.23:8080/api/v1/fleet/health | \
  python3 -c "
import json, sys
data = json.load(sys.stdin)
pod = next((p for p in data if p.get('pod_number') == 3), None)
if pod:
    print(f'Pod 3 — ws_connected: {pod[\"ws_connected\"]}, http_reachable: {pod[\"http_reachable\"]}')
    print(f'  version: {pod.get(\"version\", \"unknown\")}, uptime_secs: {pod.get(\"uptime_secs\", \"unknown\")}')
    print(f'  last_seen: {pod.get(\"last_seen\", \"never\")}')
else:
    print('Pod 3 not found in fleet health response')
"
```

### LOGBOOK.md Append (IST Timestamp)
```bash
# Source: LOGBOOK.md convention — | timestamp | author | commit | summary |
TIMESTAMP=$(python3 -c "from datetime import datetime, timezone, timedelta; ist=timezone(timedelta(hours=5,minutes=30)); print(datetime.now(ist).strftime('%Y-%m-%d %H:%M IST'))")
COMMIT=$(cd /c/Users/bono/racingpoint/racecontrol && git rev-parse --short HEAD)
echo "| $TIMESTAMP | James | \`$COMMIT\` | INCIDENT: {description} — RESOLVED: {resolution} |" >> /c/Users/bono/racingpoint/racecontrol/LOGBOOK.md
```

### Bono Notification via comms-link
```bash
# Source: MEMORY.md comms method — append to INBOX.md, commit, push
INBOX="/c/Users/bono/racingpoint/comms-link/INBOX.md"
TIMESTAMP=$(python3 -c "from datetime import datetime, timezone, timedelta; ist=timezone(timedelta(hours=5,minutes=30)); print(datetime.now(ist).strftime('%Y-%m-%d %H:%M IST'))")
echo "" >> "$INBOX"
echo "## $TIMESTAMP — from james" >> "$INBOX"
echo "" >> "$INBOX"
echo "Deployed racecontrol to server .23. New binary: {commit_hash}. Verify :8080 on your side." >> "$INBOX"
cd /c/Users/bono/racingpoint/comms-link && git add INBOX.md && git commit -m "james: deploy notification $(date +%Y-%m-%d)" && git push
```

### rc-agent Build + Stage
```bash
# Source: MEMORY.md deploy rules — staging path
export PATH="$PATH:/c/Users/bono/.cargo/bin"
cd /c/Users/bono/racingpoint/racecontrol
cargo build --release --bin rc-agent
ls -la target/release/rc-agent.exe
cp target/release/rc-agent.exe /c/Users/bono/racingpoint/deploy-staging/rc-agent.exe
ls -la /c/Users/bono/racingpoint/deploy-staging/rc-agent.exe
```

### racecontrol Kill + Verify + Start (Server .23)
```bash
# Note: racecontrol.exe runs on server .23 (Windows user: ADMIN)
# The binary swap requires copying to server path.
# Current deploy method: the deploy-racecontrol.bat script on server + HTTP trigger via :8090 or webterm
# The skill should: build locally, copy to staging, then provide the server-side command
# Server config: C:\RacingPoint\racecontrol.toml, binary starts via start-racecontrol.bat

# Kill command (to run on server via webterm :9999 or rc-agent :8090):
# taskkill /F /IM racecontrol.exe

# Start command (to run on server):
# cd C:\RacingPoint && start-racecontrol.bat

# Verify :8080 from James's machine:
curl -sf http://192.168.31.23:8080/api/v1/health && echo "OK" || echo "FAIL"
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Context in `~/.claude/projects/.../MEMORY.md` (global) | `racecontrol/CLAUDE.md` (project-scoped) | Phase 51 | Session starts with full Racing Point context without MEMORY.md needing to be read |
| No slash commands for deploy | `/rp:deploy`, `/rp:deploy-server` skills | Phase 51 | One command replaces 6-step manual sequence |
| James must remember pod IPs | IP table in CLAUDE.md + dynamic injection in skills | Phase 51 | Claude knows all 8 pod IPs without James typing them |
| Incident diagnosis is ad-hoc | `/rp:incident` skill enforces 4-tier order | Phase 51 | Consistent diagnostic approach, auto-logged to LOGBOOK |

---

## Open Questions

1. **How does /rp:deploy-server swap the binary on server .23?**
   - What we know: racecontrol.exe runs on server .23 (192.168.31.23), started via `start-racecontrol.bat` via HKLM Run key. The skill runs on James's machine (.27).
   - What's unclear: The skill needs to (a) kill the old process on .23, (b) copy the new binary to .23, (c) start it. The deploy method is currently "pendrive or webterm." There's no confirmed remote exec path from James's machine to the server that works.
   - Recommendation: The skill should (1) build locally, (2) copy binary to `deploy-staging/`, (3) use webterm (:9999) to trigger the server-side kill + swap if webterm is running, OR provide manual instructions if webterm is unavailable. The skill description in CONTEXT.md says "kill old racecontrol process automatically" — this implies webterm is the execution channel. The planner should make this explicit in PLAN.md.

2. **CLAUDE.md section for "open issues" — how fresh should it be?**
   - What we know: Open issues change weekly. CLAUDE.md is a static file.
   - What's unclear: Should CLAUDE.md include a live "open issues" section, or reference MEMORY.md for current issues?
   - Recommendation: Include a short "Current Blockers" section in CLAUDE.md (3-5 items max) updated at the same time as MEMORY.md. Accept some staleness — this is an operational trade-off.

3. **Skill naming convention — kebab-case directory vs namespace**
   - What we know: GSD skills use `tdd-debug` as directory name. The slash command is `/tdd-debug`.
   - What's unclear: The CONTEXT.md uses `/rp:deploy` (colon namespace separator). Does the skill file `name:` frontmatter support namespacing?
   - Recommendation: Use `rp-deploy` as directory name and `rp:deploy` as the skill name in frontmatter. The colon in the invocation (`/rp:deploy`) is the Claude Code namespace syntax — skills in `.claude/skills/` within the racecontrol project would be in the `rp` namespace if the skill name field contains the colon prefix. Verify this against actual Claude Code skill system behavior — if namespacing is not supported, use `/rp-deploy` as the invocation.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Manual verification + curl (no automated tests for markdown files) |
| Config file | n/a |
| Quick run command | `curl -sf http://192.168.31.23:8080/api/v1/fleet/health` |
| Full suite command | Open Claude Code session in racecontrol, verify CLAUDE.md loaded, invoke each skill |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SKILL-01 | Claude session opens knowing pod IPs without MEMORY.md | Manual | Open new session, ask "what is pod 3's IP?" | ❌ Wave 0 |
| SKILL-02 | `/rp:deploy` builds rc-agent and stages binary | Manual + bash | Invoke skill, verify `deploy-staging/rc-agent.exe` size | ❌ Wave 0 |
| SKILL-03 | `/rp:deploy-server` builds + kills + swaps + verifies :8080 | Manual + curl | Invoke skill, verify `curl http://192.168.31.23:8080/api/v1/health` returns 200 | ❌ Wave 0 |
| SKILL-04 | `/rp:pod-status pod-8` returns Pod 8 state | Manual + curl | Invoke skill, verify pod_number: 8 in response | ❌ Wave 0 |
| SKILL-05 | `/rp:incident "Pod 3 lock screen blank"` returns structured response | Manual | Invoke skill, verify 4-tier structure in output | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** Verify skill file is syntactically valid markdown (no broken tables/code blocks)
- **Per wave merge:** Open fresh Claude Code session, verify CLAUDE.md loaded (ask a pod IP question), invoke at least one skill
- **Phase gate:** All 5 requirement behaviors confirmed before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `racecontrol/CLAUDE.md` — covers SKILL-01
- [ ] `racecontrol/.claude/skills/rp-deploy/SKILL.md` — covers SKILL-02
- [ ] `racecontrol/.claude/skills/rp-deploy-server/SKILL.md` — covers SKILL-03
- [ ] `racecontrol/.claude/skills/rp-pod-status/SKILL.md` — covers SKILL-04
- [ ] `racecontrol/.claude/skills/rp-incident/SKILL.md` — covers SKILL-05
- [ ] `racecontrol/.claude/` directory — must exist first

---

## Sources

### Primary (HIGH confidence)
- `~/.claude/skills/tdd-debug/SKILL.md` — definitive local reference for skill file format in this Claude Code installation
- `~/.claude/settings.json` — confirms existing hooks pattern, MCP config, skill-creator plugin enabled
- `C:\Users\bono\.claude\projects\C--Users-bono\memory\MEMORY.md` — source material for CLAUDE.md content (all 280 lines reviewed)
- `crates/racecontrol/src/fleet_health.rs` — `PodFleetStatus` struct fields (ws_connected, http_reachable, version, build_id, uptime_secs, last_seen)
- `crates/racecontrol/src/api/routes.rs` + `main.rs` — confirmed `/api/v1/fleet/health` is the correct endpoint path (nested under `/api/v1`)
- `racecontrol/LOGBOOK.md` — confirmed LOGBOOK format: `| Timestamp | Author | Commit | Summary |`
- `.planning/phases/51-claude-md-custom-skills/51-CONTEXT.md` — all locked decisions

### Secondary (MEDIUM confidence)
- `.planning/codebase/STRUCTURE.md` — workspace layout, binary names, path conventions
- `.planning/codebase/CONVENTIONS.md` — confirmed binary naming rules (racecontrol.exe, rc-agent.exe)
- `.planning/STATE.md` — confirmed fleet/health endpoint and v9.0 constraints

### Tertiary (LOW confidence)
- Skill frontmatter `disable-model-invocation` field behavior — confirmed as the decision in CONTEXT.md but exact Claude Code runtime behavior for this field was not verified against official docs. If namespacing (`rp:deploy` vs `rp-deploy`) behaves differently than expected, skill invocation may require adjustment.

---

## Metadata

**Confidence breakdown:**
- CLAUDE.md content: HIGH — source material is MEMORY.md (reviewed in full), supplemented by codebase files
- Skill file format: HIGH — tdd-debug skill is a live working reference in this exact installation
- Fleet health endpoint shape: HIGH — read directly from fleet_health.rs source
- Server binary swap mechanism in /rp:deploy-server: MEDIUM — execution channel (webterm vs manual) needs clarification in plan
- Skill namespace (`rp:deploy` vs `rp-deploy`): MEDIUM — convention documented in CONTEXT.md but Claude Code skill namespace behavior not independently verified

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (30 days — stable Claude Code skill system, no expected breaking changes)
