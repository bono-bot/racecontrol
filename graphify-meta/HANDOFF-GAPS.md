# Graphify — Remaining Gaps (Condensed)

One-page checklist for next session. For context read HANDOFF.md.

## Quick-close (no cost, under 30 min each)
- [x] Priority A: Fix `scan-module.py` save_manifest CWD-leak — PATCHED + VERIFIED session-2. Post-patch scan of people-tracker from home CWD left racecontrol/graph.json sha256 unchanged, manifest.json landed at people-tracker/graphify-out/. Legacy pre-patch leak at `/c/Users/bono/graphify-out/` cleaned.
- [x] Priority D: `bash scripts/graphify-post/install-hook.sh` — INSTALLED + LIVE-FIRED session-2. Hook block visible in racecontrol/.git/hooks/post-commit. Commit `150f69d7` produced `graphify-post: GAP_REPORT.md updated (25 hits)`.
- [x] Priority E: Move to `racecontrol/graphify-meta/` — DONE session-2. ROOT parametrized via findEcosystemRoot() + $GRAPHIFY_META_ROOT env override. Committed `150f69d7`, pushed to origin/main.
- [x] Priority F: Improve api-edges.mjs URL resolution — DONE session-2. Routing-prefix strip (rc/v1/v2/v3) + template-param strip + per-segment tokenization. Edges 7 → 16.
- [~] ~~Restore graph.json from backup~~ — REJECTED session-2: 9834-node state is correct; added nodes are real session-commit files.
- [ ] Visually verify meta.html in Chrome (kill MCP profile-lock first per memory recipe)
- [ ] Run subsystem-audit against each per-module graph (currently only racecontrol audited)
- [x] Fix false-positive in api-edges.mjs — DONE session-2 `44ac83b2`. Replaced substring-contains with segment-boundary match. scan/bank-statement false edge gone, hr/attendance true positive preserved, edge count 16 → 17.
- [x] Priority G: sub-module slicer — DONE session-2 `44ac83b2`. 8 slices emitted (kiosk/web/pwa/admin-api/billing-api/customer-api/rc-agent/rc-sentry). Meta-map now shows 16 modules including sliced children.
- [x] Label-overlap stoplist — DONE session-2 (this message). `FRAMEWORK_STOPLIST` in build-meta.mjs excludes Next.js + Rust + tooling filenames. Noise collapsed; rc.web/rc.pwa/rc.kiosk edges removed, genuine overlaps (racecontrol↔rc-agent via ac_launcher.rs) preserved.
- [x] Billing route precision — DONE session-2 (this message). api-edges matcher rewritten to score by orderless segment-overlap count; top matches ranked by score. `/api/billing/report/daily` now correctly maps to `billing_daily_report.rs` (score 3) over `billing_coupon.rs` (score 1).
- [x] Per-slice gap-rules — DONE session-2 (this message). `scripts/graphify-post/validate-slices.mjs` iterates all 8 slice dirs, runs validate.mjs each, writes per-slice GAP_REPORT.md + combined SLICE_GAP_SUMMARY.json. All 8 slices: 12-19 rules hit each.
- [~] Visual Chrome verify — skipped per MCP profile-lock risk. Structural HTML parse confirms: 16786 bytes, vis-network CDN present, all 8 slice IDs embedded. Full visual check still pending next session.

## User-decision gates (cost ~$1-20)
- [ ] Scan marketing module — $5-20 (530 png + 116 mp4 = vision + Whisper)
- [ ] Re-extract admin with `--mode deep` — $5-10 (188 md + 147 png)
- [ ] Re-extract racecontrol with `--mode deep` — $3-5 (closes G3, G14, G15, improves kiosk/web coverage)

## Semantic gaps requiring re-extraction (Manual)
- [ ] G3  Struct-field access invisible (AcLaunchParams deg=1 etc.)
- [ ] G4  Tests disconnected from functions under test
- [ ] G6  External-boundary edges (spawns/binds/reads_file) missing
- [ ] G14 implements, cites, conceptually_related_to, shares_data_with, semantically_similar_to all globally absent
- [ ] G15 semantically_similar_to — cross-cutting similarity feature never fires
- [ ] G20 Provenance metadata (source_url/captured_at/author) empty — needs YAML-frontmatter tagged sources

## Algo / upstream-graphify gaps
- [ ] G12 Community IDs non-stable across rebuilds (77.8% drift hour-to-hour). Needs content-hash-derived labels, not algo change.
- [ ] G16 Graph is undirected (`graph.directed=false`) — re-run graphify with `--directed`.
- [ ] G19 Hyperedges in JSON but not rendered in HTML template.

## Still-HIT auto-rules (partial upstream limit)
- [ ] G2  5 residual same-file `.default()` / `.paint()` collisions — need struct-scope info graphify doesn't capture.

## Subsystem scan-scope gaps
- [ ] Admin Panel only 9% indexed (13/139). Needs `/graphify racingpoint-admin/` run.
- [ ] Kiosk 20% (13/65). Needs `/graphify .` inside `racecontrol/kiosk/` OR `--mode deep` on parent racecontrol.
- [ ] Web 28% (27/95). Same fix as Kiosk.

## Meta-map gaps
- [ ] api-edges.mjs misses templated URLs (`/api/drivers/${id}`, `/api/:pod/state`). Extend regex.
- [ ] api-edges.mjs missing back-edges (racecontrol → module callbacks)
- [ ] Kiosk/Web/Admin-API not split out as sub-modules under racecontrol meta-node
- [ ] Admin scraped only 3 URL fragments — suggests hidden URL-builders; grep Admin's service-class files for route strings
- [ ] No gap-rules evaluated against per-module graphs (admin/comms-link/whatsapp-bot etc.)
- [ ] No watcher auto-rebuilds meta-map when module graphs change

## G9 entries to investigate
- [x] (1) Root cause of graph.json drift — RESOLVED session-2: racecontrol post-commit hook runs `graphify.watch._rebuild_code` on every commit; 17+ session commits caused the 9811→9834 growth, and all 25 added node IDs match session-committed files. Not a bug.
- [ ] (2) Why community-detection is hyper-sensitive to graph-content changes — is Louvain seeded?
- [ ] (3) Why `group` field injection didn't enable the per-community legend (retrospective — rule retracted, but useful lesson re: proxy tests)

## Files created this session (DO NOT DELETE blindly)
Scripts (in git-trackable location):
- `racecontrol/scripts/graphify-post/` — 8 files, 60 KB total

Meta infrastructure (UNGOVERNED location, needs moving):
- `graphify-meta/` at `/c/Users/bono/racingpoint/graphify-meta/` — 7 files, 50 KB

New module graphs:
- `people-tracker/graphify-out/` (85 KB)
- `pod-agent/graphify-out/` (50 KB)
- `rc-ops-mcp/graphify-out/` (1 KB)

Throwaway analysis scripts (safe to delete):
- `/c/Users/bono/racingpoint/graphify-out/analyze_c12.mjs`
- `...graphify-out/analyze_c12_global.mjs`
- `...graphify-out/close_gaps.mjs`
- `...graphify-out/deep_gaps.mjs`
- `...graphify-out/verify_repair.mjs`
- `...graphify-out/admin_analysis.mjs`
- `...graphify-out/debug_kiosk.mjs`
- `...graphify-out/debug_audit.mjs`
- `...graphify-out/list_admin_in_graph.mjs`
