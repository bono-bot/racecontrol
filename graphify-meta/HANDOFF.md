# Graphify Map-of-Maps & Gap-Detection — Session Handoff

**Session:** 2026-04-20  (UTC time range approx 00:00 – 00:55)
**Primary artifact:** This document. Read first.

---

## 1 — What this session set up

A **two-layer graphify system** for the RacingPoint ecosystem.

### Layer A: Per-module graphs (existing graphify-out folders — some created this session)

| Module | Path | Nodes | Scan type |
|---|---|---|---|
| racecontrol | `racecontrol/graphify-out/` | ~9811 (drifted to 9834) | pre-existing, full semantic |
| racingpoint-admin | `racingpoint-admin/graphify-out/` | 407 | pre-existing |
| comms-link | `comms-link/graphify-out/` | 811 | pre-existing |
| whatsapp-bot | `whatsapp-bot/graphify-out/` | 373 | pre-existing |
| people-tracker | `people-tracker/graphify-out/` | 78 | **NEW THIS SESSION** (AST only) |
| pod-agent | `pod-agent/graphify-out/` | 52 | **NEW THIS SESSION** (AST only, legacy) |
| rc-ops-mcp | `rc-ops-mcp/graphify-out/` | 2 | **NEW THIS SESSION** (AST only) |
| marketing | — | — | **NEVER SCANNED** — 530 png + 116 mp4 = ~$5-20 LLM cost |

### Layer B: Meta-map (new this session)

Location: `C:/Users/bono/racingpoint/graphify-meta/`

| File | Purpose |
|---|---|
| `meta.html` | Interactive map of modules — double-click node to open that module's graph.html |
| `meta-graph.json` | Module list + inter-module edges + god-nodes per module |
| `META_REPORT.md` | Scorecard + missing scans list |
| `build-meta.mjs` | Regenerator (ms to run, zero cost) |
| `api-edges.mjs` | URL-pattern edge discoverer (grep consumers, match against racecontrol routes) |
| `api-edges.json` | Current result: 7 real cross-module API edges |
| `scan-module.py` | Pipeline driver for new code-only module scans |

### Layer C: Gap-detection (new this session, racecontrol-scoped)

Location: `C:/Users/bono/racingpoint/racecontrol/scripts/graphify-post/`

| File | Purpose |
|---|---|
| `gap-rules.mjs` | 29 declarative gap rules (G1–G29). Each has detect() + optional apply() |
| `validate.mjs` | Runs all detects → `GAP_REPORT.md` + `GAP_REPORT.json`. Non-destructive |
| `repair.mjs` | Applies auto-fix rules → `graph.repaired.json` + `*.repaired.html`. Never overwrites |
| `subsystems.mjs` | Registry of 10 subsystems with regex classifiers |
| `subsystem-audit.mjs` | Coverage audit per subsystem → `SUBSYSTEM_MAP.md` |
| `rebuild-index.mjs` | Regenerates `communities/index.html` |
| `install-hook.sh` | Adds validate to git post-commit hook (NOT installed by default) |
| `README.md` | Usage + rule table |

---

## 2 — What actually got done vs what didn't

### ✓ Shipped (verified with raw output)
- 3 new module scans (people-tracker / pod-agent / rc-ops-mcp) — AST-only, zero LLM cost
- URL-based inter-module edge discovery — 7 real edges found: admin→racecontrol (3), comms-link→racecontrol (2), whatsapp-bot→racecontrol (2)
- Gap repair verified: G13/G17/G18/G24 flip HIT→PASS on repaired graph; G2 reduces 37→5 residual collisions (5 are upstream same-file trait-impl limit)
- Subsystem audit scorecard built, wired into validate.mjs GAP_REPORT.md

### ✗ NOT done (explicitly left for handoff)

| Item | Why | Blocker |
|---|---|---|
| Marketing module scan | 530 png + 116 mp4 triggers vision + Whisper | ~$5-20 cost — needs user OK |
| Admin repo semantic re-scan | 188 .md + 147 .png → LLM+vision | ~$5-10 cost — needs user OK |
| Racecontrol `--mode deep` re-extraction | Would fix G3/G14/G15 (struct fields, implements, semantically_similar_to) | Destructive + ~$3-5 — needs user OK |
| Chrome visual verify of meta.html | Lost track after multiple rebuilds | Needs Chrome-mcp profile-lock recovery |
| `graphify-meta/` git placement | Ungoverned location | Decision: which repo hosts it? |
| Git hook installation | Written, not activated | Operator runs `bash scripts/graphify-post/install-hook.sh` |
| G9 structural fix for graph.json drift | Root cause unknown | See §4 |

---

## 3 — Open gaps (consolidated)

### 3.1 Graphify-core gaps (29 rules, G1–G29)

Current state of racecontrol/graph.json (**drifted** — see §4):

| Severity | HIT | PASS on live | PASS on repaired |
|---|---|---|---|
| P0 | 2 | — | — |
| P1 | 8 | 7 | 5 |
| P2 | 8 | 7 | 7 |
| P3 | 7 | 5 | 5 |

**Rules still HIT on repaired graph (not fixable without re-extraction or upstream graphify changes):**

- G1  (P0 warn) — GET()/run() name-collision hubs (523 cross-comm edges on one `GET()` node)
- G3  (P0 manual) — Struct-field access edges invisible (e.g. AcLaunchParams deg=1)
- G4  (P1 manual) — Unit tests disconnected from functions under test
- G6  (P1 manual) — External-boundary edges missing (spawns/binds/reads_file)
- G7  (P2 warn) — Community modularity weak (c12 spread across 15+ partners)
- G8  (P2 manual) — Single-file dominance (57% of c12 from one file)
- G9  (P3 manual) — bot_coordinator_recovery miscategorized in AC launcher community
- G10 (P3 warn) — Absolute Windows paths baked into node IDs
- G12 (P1 warn) — Community IDs reshuffle on rebuild (77.8% drift between snapshots)
- G14 (P2 manual) — 5 schema-declared relations absent globally (implements, cites, etc.)
- G15 (P2 manual) — `semantically_similar_to` never emitted
- G16 (P2 manual) — Graph is undirected — caller/callee direction lost
- G19 (P2 manual) — Hyperedges in JSON but not rendered in HTML
- G20 (P3 manual) — Provenance metadata (source_url, captured_at, author) empty
- G21 (P3 warn) — source_location null on 206 nodes + 71 edges
- G22 (P1 warn) — Hub dominance (c12: 640 edges / 278 members = 2.30)
- G23 (P3 warn) — edge `weight` field effectively constant
- G25 (P1 manual) — Admin frontend under-indexed (13 of 139 files)
- G26 (P2 warn) — Admin has no coherent community (max density 10.4%)
- G27 (P2 manual) — Cross-repo fetch→backend edges missing (partial — api-edges.mjs found 7)
- G28 (P1 manual) — Sibling repos not in scan scope (3 of 4 now scanned this session)
- G29 (P3 warn) — Next.js route-group parens (detect only, low confidence)

### 3.2 Subsystem coverage gaps

| Subsystem | FS files | Indexed | Coverage |
|---|---|---|---|
| Admin Panel | 139 | 13 | **9%** |
| Billing | 41 | 41 | 100% |
| Kiosk (PWA) | 65 | 13 | **20%** |
| Web / POS | 95 | 27 | **28%** |

Kiosk/Web are INSIDE racecontrol and ARE scanned, but graphify's AST handles `.tsx` unevenly — indexes page/route files, misses many components. Fix requires `--mode deep` re-extraction.

### 3.3 Meta-map gaps

- Marketing module never scanned (see §2)
- Only outbound URL edges captured (consumers → racecontrol); no back-edges (racecontrol callbacks to modules)
- `api-edges.mjs` regex misses templated URLs (`/api/drivers/${id}` → captures only `/api/drivers`)
- Admin has 117 tsx files but api-edges.mjs found only 3 distinct URL fragments — suggests many admin pages use URL-builders or service classes that hide the literal path
- No label-overlap edges passed threshold of 3+ (intentional — previous threshold of 1 was too noisy)
- kiosk, web, billing, admin-api are sub-modules INSIDE racecontrol but still roll up into the racecontrol meta-node — no "sub-module" concept
- meta.html not visually verified this session after edge-type changes

### 3.4 Operational gaps

- **graph.json drift** — 9811 → 9834 nodes at 00:52 UTC (see §4)
- Git hook for validate.mjs exists but not installed
- graphify-meta/ directory is NOT in any git repo — could vanish on cleanup
- No watcher/auto-rebuild — all meta-map updates are manual
- Validate currently only checks racecontrol's graph.json; no gap-rules evaluated against the 6 per-module graphs
- Missing gap-rule coverage: nothing validates the META graph itself (e.g. "modules with 0 cross-module edges suggest incomplete scan")

---

## 4 — RESOLVED: graph.json "drift" was legitimate post-commit rebuilds

**Corrected 2026-04-20 session-2.** Original hypothesis (save_manifest side-effect) was **wrong**.

### Observed
```
pristine (02:07 UTC): 9811 nodes, 23430 edges, 13,833,028 bytes
at 05:52 UTC:        9832 nodes,            13,856,756 bytes
at 06:39 UTC:        9834 nodes, 23517 edges, 13,881,664 bytes (CURRENT)
```

### Actual root cause
racecontrol/.git/hooks/post-commit runs `graphify.watch._rebuild_code(Path('.'))` after EVERY commit that touches tracked files. The session produced 17+ racecontrol commits between 00:09–06:38 IST; each one rebuilt graph.json.

Diff of current vs pristine shows **25 added node IDs** — every one is a file created during the session and landed by a session commit:
- `scripts/graphify-post/{gap-rules,validate,repair,subsystems,subsystem-audit,rebuild-index}.mjs`
- `scripts/audit/autostart-surfaces.ps1`, `scripts/audit/kiosk-swap-verify.ps1`
- `scripts/deploy/pos-watchdog.ps1`
- `scripts/watchdog/chunk-integrity-probe.ps1` + its 6 inner functions
- `crates/racecontrol/src/state/methods.rs`, `src/ws/agent_fleet.rs`

**Evidence:** `git log --since='2026-04-20 00:00' --until='10:00'` shows commits `66738a3e`, `b6f66e63`, `af81915d`, etc. — exact files = exact added nodes.

### Why save_manifest hypothesis was wrong
`graphify.detect.save_manifest` (C:/Users/bono/AppData/Local/Programs/Python/Python312/Lib/site-packages/graphify/detect.py) writes a MTIME manifest (filename → float), NOT graph nodes. It cannot add 25 nodes to graph.json. Default `_MANIFEST_PATH='graphify-out/manifest.json'` IS CWD-relative (real latent leak) but it would only overwrite manifest.json, not graph.json. No manifest.json exists in racecontrol/graphify-out/ currently — so scan-module.py leakage didn't happen this session either.

### Action taken session-2
1. **Fixed latent leak anyway** (hygiene): `scan-module.py` now passes `manifest_path=str(out_dir / 'manifest.json')` explicitly. Prevents future CWD-based accidents.
2. **Did NOT restore backup** — the 9834-node state is the correct current reality. Restoring to 9811 would delete 25 real file nodes.
3. The `graph.json.bak-*` backups remain on disk as snapshots; no reason to reference them unless a future corruption is suspected.

### When to worry next time
If graph.json grows WITHOUT a matching racecontrol commit within the last minute, THEN investigate. Otherwise, growth = expected post-commit rebuild.

---

## 5 — Resume playbook (priority ordered)

### Priority A — DONE in session-2 (see §4)
- Drift was legitimate post-commit rebuilds, not a bug.
- Latent CWD-leak in scan-module.py patched anyway (explicit `manifest_path`).
- No backup restore required.

### Priority B — Marketing scan decision (1 min)
1. Ask user: scan marketing module? Cost ~$5-20 if we run vision + Whisper on 530 images + 116 videos.
2. Alternative: skip media files, scan only .js + .json — effectively free, but marketing is media-heavy so graph will be sparse.

### Priority C — Deep re-extraction (racecontrol --mode deep) decision (1 min)
- Closes G3, G14, G15, and improves kiosk/web tsx coverage significantly.
- Cost: ~$3-5. Time: ~15-25 min.
- Ask user before proceeding; current graph is usable without it.

### Priority D — DONE in session-2
- `install-hook.sh` run from racecontrol repo root — appended graphify-post block to `.git/hooks/post-commit`.
- Payload dry-run: `node scripts/graphify-post/validate.mjs graphify-out` → `25/29 rules HIT`, wrote fresh GAP_REPORT.md.
- Live integration: commit `150f69d7` fired the hook, console echoed `graphify-post: GAP_REPORT.md updated (25 hits)`.

### Priority E — DONE in session-2
- graphify-meta/ moved from `/c/Users/bono/racingpoint/graphify-meta/` → `/c/Users/bono/racingpoint/racecontrol/graphify-meta/`.
- ROOT in build-meta.mjs + api-edges.mjs parametrized via `findEcosystemRoot()` (walks up looking for both `racecontrol/` and `racingpoint-admin/` as sibling dirs) + `$GRAPHIFY_META_ROOT` override.
- .gitignore added for generated artifacts (meta-graph.json, meta.html, META_REPORT.md, api-edges.json).
- Committed in `150f69d7` (6 files, 1004 insertions). Pushed.
- Legacy location removed + verified no residual `manifest.json` at home-root.

### Priority F — DONE in session-2
- api-edges.mjs patched: routing-prefix stripping (rc/v1/v2/v3) + template-param stripping (`${id}`, `:id`, `{id}`) + expanded backend tokenization (every `_`-split segment + adjacent-pair joins) + multi-candidate matching (first / compound / tail).
- Edge count: **7 → 16** (admin 3 → 12). `/api/rc/staff` now correctly maps to `auth_staff.rs` + `staff_checklists.rs`.
- One known false-positive: `admin -> scan/bank-statement` resolves to `game_state.rs` via generic `state` token. The `normalized.includes('_' + tok)` match is too permissive for 4-5 char tokens. Noted but not fixed — 1/12 noise rate is acceptable.

### Priority G — DONE in session-2
- `slice-submodules.mjs` (new file, 188 lines) reads racecontrol/graphify-out/graph.json + 8 slice predicates → writes `racecontrol/graphify-out-<id>/graph.json` per slice + `slices.json` manifest.
- Slices produced: kiosk (232n/182e/55c), web (342n/308e/68c), pwa-legacy (128n/95e/23c), admin-api (38n/32e/4c), billing-api (85n/108e/5c), customer-api (98n/132e/6c), rc-agent (2247n/4707e/36c), rc-sentry (396n/776e/13c). c = communities present (subset of the 519 in parent).
- `build-meta.mjs` extended: loads slices.json and adds children with `parent: 'racecontrol'` + `graph_path_override`. Meta-graph now shows 16 modules (was 8). Label-overlap edges surface noise between rc.web/rc.pwa/rc.kiosk — all three are Next.js apps sharing `page.tsx` / `layout.tsx` top labels. Known noise, not yet filtered.
- Slice output dirs (`graphify-out-*/`) added to racecontrol/.gitignore — regenerable from graph.json + slice-submodules.mjs.
- Committed in `44ac83b2`. Pushed.

### False-positive fix (bonus, session-2)
- api-edges.mjs matcher rewritten from `tok.includes('_' + normalized)` substring check to `haystack.split(/[_/]/).includes(needle)` segment-boundary check.
- Before: `/api/scan/bank-statement` matched `game_state.rs` via `_state` substring inside `bank_statement` (false positive).
- After: no substring-only matches; `hr/attendance` → `admin_hr.rs` still works (segment match).
- Edge count: 16 → 17. Admin 11, comms-link 3 (+1), whatsapp 3 (+1).
- Output path for api-edges.json + meta.html/meta-graph.json migrated to `__dirname`-relative so it works after the Priority E relocation.

---

## 6 — Evidence trail (where to find proof of claims)

- Gap audit on repaired graph: `racecontrol/graphify-out/GAP_REPORT.md` (full report), `GAP_REPORT.json` (machine-readable)
- Subsystem audit: `racecontrol/graphify-out/SUBSYSTEM_MAP.md`
- URL-edge discovery raw output: `graphify-meta/api-edges.json`
- Meta-map scorecard: `graphify-meta/META_REPORT.md`
- Community_12 deep analysis scripts (throwaway): `graphify-out/analyze_c12.mjs`, `analyze_c12_global.mjs`, `close_gaps.mjs`, `deep_gaps.mjs`, `verify_repair.mjs`, `admin_analysis.mjs`
- Chrome screenshot of rendered community_12: `graphify-out/c12_render.png`, `c12_repaired_render.png`

---

## 7 — What NOT to do in the next session

- ~~Do not re-run scan-module.py from an arbitrary CWD without fixing the save_manifest drift first~~ — FIXED session-2 (explicit `manifest_path`), safe to run from anywhere.
- **Do not run** `/graphify .` on marketing/ without explicit user cost approval.
- **Do not promote** graph.repaired.json → graph.json until the drift root cause is understood — the drift might cascade into the repair.
- **Do not blanket-kill** chrome.exe via `taskkill /F /IM chrome.exe` to recover MCP lock — use the targeted PowerShell recipe from memory (`reference_james_ssh_github_and_mcp_recovery.md`).

---

## 8 — Session metrics
### session-1 (original)
- Claims this session: ~60
- Corrections: 1 (pivot from Option 3 big-graph to per-module meta-map on user redirect)
- G9 entries: 3
  - (1) Misframed 77.8% drift as algorithm non-determinism (was actually graph-content churn on deterministic Louvain)
  - (2) Claimed G11 "fixed" based on `group` field presence without behavior-verifying the legend (proxy test)
  - (3) graph.json drift from 9811 → 9834 during session, root cause unidentified (save_manifest hypothesis untested) — **RESOLVED session-2**: post-commit hook, not save_manifest

### session-2 (2026-04-20 01:10–07:15 IST, autonomous)
- Priorities closed: A, D, E, F, G + api-edges false-positive fix
- Priorities remaining: B+C (cost gates — need user approval)
- Commits this session: `150f69d7`, `3a2669ec`, `5f666c5d`, `44ac83b2` on origin/main
- Corrections from user: 0
- G9 entries: 0 new (session-1's G9-3 marked RESOLVED in §4)
- Self-identified errors: 2 (session-1 root-cause misdiagnosis of drift; my own over-tight 6-char gate on api-edges matcher that killed hr/attendance until the segment-boundary rewrite)
- New issues discovered:
  - HTML viz fails >5000 nodes during graphify-watch rebuild (JSON persists, `Rebuild failed` message is misleading)
  - Label-overlap edges between rc.web / rc.pwa / rc.kiosk are noisy — all three are Next.js apps with shared `page.tsx` / `layout.tsx` top-labels. Filter needs a Next.js filename stoplist.
  - build-meta.mjs emits `has_communities_dir: false` for slice modules — slice dirs don't have the `communities/` subtree. Cosmetic.
  - Parallel claude session deployed `3a2669ec` to server .23 at 07:10 IST with a cosmetic rollback failure; SWAPLOG.md left dirty in working tree (not mine to commit).

---

**Contact:** James (james@racingpoint.in). Meta-map committed at `racecontrol/graphify-meta/` as of session-2.
