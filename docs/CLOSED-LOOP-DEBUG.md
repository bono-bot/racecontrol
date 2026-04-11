# Closed-Loop Debug (CLD) v1.0

**Every investigation starts AND ends at the layer closest to the user.**

Designed from 25 real incidents at Racing Point. Catches 22/25 bug classes.
The 3 it misses (silent backend degradation) need monitoring, not debugging.

---

## The 5 Steps

### Step 1: OPEN — Reproduce the specific symptom

Observe **exactly what the user sees**, not a proxy.

| Symptom type | OPEN action |
|-------------|-------------|
| "looks wrong / no UI / broken page" | `npx playwright screenshot <URL> screenshot.png` then Read the image |
| "access denied / can't login" | `curl -X POST <endpoint> -d '<exact payload user sends>'` |
| "crash / won't start / offline" | `tasklist /V /FO CSV \| findstr <process>` + check session context |
| "wrong result / wrong calculation" | Reproduce the exact user flow, check the output value |
| "deployed but old behavior" | Check `build_id` + compare against `git rev-parse --short HEAD` |

**Rule:** "Does the page load" is NOT "does the feature work."
Test the **specific behavior** the user reported, not a general health check.

### Step 2: DESCEND — Find root cause through layers

Work through layers until root cause is found. Stop at the first layer that fails.

```
Layer 1: SMOKE      — Does the page/endpoint respond? CSS/JS load?
Layer 2: FUNCTION   — Does the specific feature work end-to-end?
Layer 3: BOUNDARY   — Do systems connect? Field names match? Auth passes?
Layer 4: INFRA      — Right binary, right session, right config, right machine?
Layer 5: DATA       — Right values in DB? Right rows exist? Active flags correct?
Layer 6: CODE       — Logic correct in source?
```

**Per-layer quick checks:**

| Layer | Command | What a failure looks like |
|-------|---------|--------------------------|
| 1 SMOKE | `curl -o /dev/null -w "%{http_code} %{size_download}" <url>` | 404, 500, size=0 |
| 1 SMOKE (CSS) | `curl -o /dev/null -w "%{http_code}" <css_chunk_url>` | 404 = stale build |
| 2 FUNCTION | `curl -X POST <api> -d '<user payload>'` | error response, wrong data |
| 3 BOUNDARY | grep field names in sender vs receiver code | name mismatch (Serde drops silently) |
| 4 INFRA | `tasklist /V /FO CSV \| findstr <proc>` | Session=Services (should be Console) |
| 4 INFRA | `ssh server "type C:\RacingPoint\<config>"` | wrong values, stale config |
| 5 DATA | `sqlite3 <db> "SELECT ..."` or API query | missing row, wrong value, is_active=0 |
| 6 CODE | `git log -S "<term>" -- "*.rs"` + read the function | logic error in source |

### Step 3: FIX — Apply the smallest change at the right layer

Fix at the layer where root cause lives. Don't fix Layer 6 (code) when the bug is Layer 4 (wrong binary deployed).

### Step 4: CLOSE — Verify at the SAME layer as Step 1

**Re-run the exact same test from Step 1.** Not a health check — the same screenshot, the same curl, the same user flow.

| Step 1 was | Step 4 must be |
|-----------|----------------|
| Screenshot showing unstyled HTML | Screenshot showing styled page |
| curl returning "Invalid PIN" | curl returning `{"status":"ok"}` |
| tasklist showing Session=Services | tasklist showing Session=Console |
| race.ini showing AI_LEVEL=3 | race.ini showing AI_LEVEL=1 |

**If Step 4 doesn't match Step 1's format, the loop is not closed.**

### Step 5: SWEEP — Check all deploy targets

Every fix that touches a deployed system must be verified on ALL targets where it runs.

```
Frontend apps:  Venue (.23) AND Cloud (Bono VPS)
Rust binaries:  Server (.23) AND Pods 1-8 AND POS (.20)
Config files:   Server + all pods + cloud
Test fixtures:  Committed + pushed (git status clean)
```

**One machine fixed ≠ all machines fixed.** Check each.

---

## When NOT to use CLD

- **Silent degradation** (no user symptom): Use monitoring/alerting instead
- **Cargo test failures**: Standard test debugging, no need for layers
- **Build errors**: Compiler tells you the layer — just fix it

## Relationship to CGP

CLD is the **investigation method**. CGP is the **claim discipline**.

- CLD tells you HOW to investigate (5 steps, 6 layers)
- CGP tells you WHEN you can claim done (H1-H5 gates)
- CLD Step 4 (CLOSE) produces the evidence that CGP H3 requires
- CLD Step 5 (SWEEP) produces the enumeration that CGP H4 requires

## Origin

Built from 25 real incidents (2026-03 to 2026-04). Backtested: catches 22/25.
Designed by James Vowles and Uday Singh, 2026-04-11.
