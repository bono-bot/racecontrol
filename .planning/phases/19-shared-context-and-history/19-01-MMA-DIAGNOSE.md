# MMA Step 1 (DIAGNOSE) — Phase 19 Shared Mesh WAL + Dialogue Primitive

**Date:** 2026-04-22 (IST)
**Target:** comms-link `feat/phase-19-shared-wal` @ `6298619` (PR #1)
**Files audited:** `bono/mesh-wal-store.js`, `bono/comms-server.js` lines 207-372 (new routes), `scripts/mesh-wal-inject.js`
**Run by:** James (OpenRouter direct curl, key from `racecontrol/data/openrouter-mma-key.txt`)
**Script:** `racecontrol/scripts/mma-phase19-diagnose.sh`

## Models (5 — 4 vendor families, ≥1 reasoner + ≥1 code expert + ≥1 SRE)

| Slot | Model | Vendor | Elapsed | Status |
|---|---|---|---|---|
| Reasoner | `deepseek/deepseek-r1-0528` | deepseek | 82.7s | OK (reasoning field; content empty — findings in reasoning trace) |
| Code expert | `deepseek/deepseek-chat-v3-0324` | deepseek | 24.1s | OK (3852 chars) |
| Code expert | `x-ai/grok-code-fast-1` | xai | 22.3s | OK (5155 chars) |
| SRE | `xiaomi/mimo-v2-pro` | xiaomi | 82.6s | OK (1298 chars — partial, cut mid-entry) |
| Generalist | `google/gemini-2.5-flash` | google | 23.8s | OK (15534 chars — most detailed) |

**Vendor diversity:** deepseek×2, xai×1, xiaomi×1, google×1 = 4 families (≥3 required ✓, max 2 per vendor ✓)
**Role coverage:** reasoner ✓, code expert ×2 ✓, SRE ✓, generalist ✓

**Raw responses:** `racecontrol/.planning/phases/19-shared-context-and-history/mma-diagnose-output/*.json`

## Consensus (3/5 majority per UNIFIED-MMA-PROTOCOL.md Step 1)

### P0 — Must fix before deploy

#### P0-1: JSONL injection via unescaped newlines — **5/5 models**

**Location:** `bono/mesh-wal-store.js` `validateEntry()` (lines 23-36) + all fields that accept user strings (`summary`, `question`, `answer`, `context_refs[]`, `ref`, `repo`, `branch`, `metadata.*`).

**Issue:** `validateEntry` caps length but does NOT strip `\r\n\0` or other control characters. Entries are appended as JSONL (one JSON object per line). A newline inside a string field breaks JSONL parsing AND lets a malicious agent inject a fully-forged entry on the next line.

**Evidence:** Trivial PoC — `{"agent":"james","kind":"commit","summary":"real\n{\"agent\":\"bono\",\"kind\":\"decision\",\"summary\":\"injected\"}"}` writes two lines to disk. The injected line looks native to any reader.

**Fix:** In `validateEntry`, reject or sanitize strings containing `/[\r\n\x00]/` across every string field. Recommend reject (throw) over strip to fail loudly. Also apply to `metadata` values if they contain strings.

**Cited by:** gemini (P1), grok (P0), mimo (P1), deepseek-v3 (P1), deepseek-r1 (implied in reasoning).

#### P0-2: Agent field forgeable — **4/5 models** (gemini/grok/deepseek-v3/deepseek-r1)

**Location:** `bono/comms-server.js` `/mesh/*` routes + `bono/mesh-wal-store.js` append path.

**Issue:** Client supplies `agent: 'james'|'bono'` in request body. `VALID_AGENTS` enum check passes for any legal value. There is NO cryptographic binding between the PSK used for Bearer auth and the `agent` field. A PSK holder (single shared secret today) can write entries claiming to be from the other agent.

**Threat model consideration:** If the threat model is "PSK is a shared secret both agents hold," impersonation is already implied. If the threat model is "each agent has its own PSK," the current code regresses it.

**Fix options (smallest first):**
- (a) Short-term: document the limitation in the WAL schema; audit trail is per-PSK not per-agent.
- (b) Medium: derive `agent` server-side from PSK lookup (requires per-agent PSKs).
- (c) Long: HMAC the entry with an agent-private key; server verifies signature.

**Recommendation:** (a) + plan (b) as v4.0 Phase work. Do NOT block Phase 19 ship on this — the current ONE-PSK model means the attack is already available via mesh-append.js directly.

**Cited by:** gemini (P1), grok (P0), deepseek-v3 (P2), deepseek-r1 (P1).

### P1 — Strongly recommend fix before deploy

#### P1-1: Thread_id mutation after append — **4/5 models** (gemini/grok/deepseek-v3/deepseek-r1)

**Location:** `bono/mesh-wal-store.js` `ask()` method (line ~218-220).

**Issue:** `ask()` calls `this.append({... thread_id: req.thread_id || null})` first. Server assigns an ID at append time. Then after append returns, `ask()` mutates `askEntry.metadata.thread_id = askEntry.id` — but ONLY on the in-memory object, NOT on disk. Disk version for root asks has `thread_id: null` permanently. Projectors work via `|| entry.id` fallback, but this is a latent consistency hole — any future code that reads the on-disk entry (e.g., recovery tool, auditor, log-shipper) sees null and can't tell it's a thread root vs a bug.

**Evidence:** Confirmed in my own probe list (flagged before MMA).

**Fix (simplest):** Compute the id BEFORE the append. Pre-generate `id = randomUUID()`, pass it through to `append`, set `thread_id: req.thread_id || id` in the same object, THEN append. Requires small refactor of `append` to accept a client-supplied id OR expose a new private path.

**Cited by:** gemini (P1), grok (P1), deepseek-v3 (P1), deepseek-r1 (P1).

#### P1-2: Archive corruption + double-read on crash between writeFile/unlink — **5/5 models**

**Location:** `bono/mesh-wal-store.js` `_rotateNoLock()` (lines 147-154).

**Issue:** `_rotateNoLock` does `writeFile(archivePath, gzip) → unlink(filePath)`. If process crashes between those two ops, BOTH the archive AND the original full-size file exist. Next startup: `_readAllAvailable` reads the original + the archive, returning duplicate entries for whatever was in the original.

**NOTE on mutex scope:** `_rotateNoLock` IS called from within `_withLock` via `append()`, so a concurrent-append race is NOT possible (contra gemini's first framing). The real issue is crash resilience, not concurrency.

**Fix:** Replace `writeFile(archive) + unlink(original)` with an atomic `rename(filePath, tmpPath) → gzip tmp → rename tmpPath.gz → archivePath`. Recovery path on startup: if `*.jsonl.tmp` or uncompressed duplicates exist, reconcile before accepting writes.

**Minimal patch:** rename first, then compress the renamed file (crash-safe: rename is atomic on most FS).

**Cited by:** all 5.

#### P1-3: gzip decompression bomb — **deepseek-v3 unique** (flagged P1)

**Location:** `bono/mesh-wal-store.js` `_readAllAvailable()` (lines 117-124).

**Issue:** `gunzipSync(compressed)` has no decompressed-size cap. A maliciously-crafted 10 MB gzip can expand to 1+ GB ("gzip bomb"). Since the archive file is written by MeshWalStore itself today (not externally supplied), this is currently low-risk — but if the threat model ever includes "someone writes to /root/comms-link/data/ directly" (e.g., via another service or a compromised archive), this is a server-wide DoS.

**Fix:** Stream decompression via `zlib.createGunzip()` with a byte counter that aborts past `MAX_DECOMPRESSED_BYTES` (e.g., 50 MB). Or: gate archive ingestion by signed manifest.

**Priority:** P2 given current threat model (only MeshWalStore writes archives).

**Cited by:** deepseek-v3 only (unique). gemini alluded to it in tail-amplification finding but didn't call it a bomb.

### P2 — Should fix eventually

#### P2-1: No rate limiting on /mesh/wal/append — **4/5 models**

**Location:** `bono/comms-server.js` all `/mesh/*` routes.

**Issue:** A client with a valid PSK can flood append at line rate, causing rapid rotation, disk I/O exhaustion, and CPU burn (gzip of every 10 MB rotation).

**Fix:** Add simple token-bucket per IP (or per PSK if per-agent PSKs exist).

**Priority:** P2 because the current threat model (two trusted agents sharing a PSK) makes abuse unlikely. But a cheap guard.

#### P2-2: Error messages may leak internal paths — **3/5 models**

**Location:** Most `catch` blocks in `comms-server.js` dialogue routes.

**Issue:** `jsonResponse(res, 400, { error: walErr.message })` — if `walErr.message` is something like "ENOENT: no such file or directory, open '/root/comms-link/data/mesh-wal.jsonl'", filesystem paths leak to the caller. Also: `correlation_id not found: <id>` confirms existence/non-existence, enabling probing.

**Fix:** Normalize error messages — generic strings client-side, full detail in server logs only.

#### P2-3: Promise.all can wait up to 3s for slow fetches in SessionStart hook — **3/5 models**

**Location:** `scripts/mesh-wal-inject.js` `main()`.

**Issue:** Each `fetchJson` has a 3s timeout, but `Promise.all` waits for ALL four. Worst case: SessionStart hook delays session start by ~3s if Bono is unresponsive. Acceptable for a hook-at-startup (one-time cost), but not ideal.

**Fix:** `Promise.allSettled` or wrap `Promise.all` in `Promise.race` against a hard 3s timeout.

### Cleared by code review (NOT issues)

| Probe | Verdict | Rationale |
|---|---|---|
| PSK timing side-channel | SAFE | Uses `timingSafeEqual` after SHA-256 pre-hash — constant-length compare, no early-return side channel. 4/5 models confirmed. |
| DoS via parseBody before JSON.parse | SAFE | `parseBody` rejects IN-STREAM via `req.destroy()` when `size > maxBytes`; JSON.parse runs only after `end` event. 4/5 models confirmed. |
| openDialogues O(N) scan every call | ACCEPTABLE | Bounded by MAX_TAIL_N=500 per read. Few-ms cost at current scale. All 5 models agreed acceptable with "optimize later if needed". |
| Auth bypass on /mesh/* routes | SAFE | Every route validates Bearer PSK via `validatePsk`. No unauthed path into mesh state. |
| Hook blocking session on slow Bono | MITIGATED | 3s timeout enforced per fetch via `req.on('timeout')` + `req.destroy()`. Main catch-all ensures exit 0 on any failure. P2-3 above is the residual edge case. |

### Additional finding — gemini unique observation

**`/relay/fallback-inbox` does NOT pass maxBytes to parseBody → defaults to 5 MB.** Pre-existing, NOT introduced by Phase 19. But worth surfacing: dialogue routes correctly pass MESH_WAL_MAX_BODY (64 KB), legacy routes don't. Consider a follow-up PR to apply size caps to all legacy routes.

## Budget

5 model calls × ~6K tokens input + ~3K tokens output. OpenRouter billing not yet reported in response metadata. **Estimated total: ~$0.15-0.40** (well under $5 session budget).

## Recommendation — Next step

### MUST FIX before merge + deploy

1. **P0-1** (JSONL injection) — 20-line addition to `validateEntry`. Small, safe, landable in one commit.
2. **P1-1** (thread_id mutation) — modest refactor of `ask()` to pre-generate id. Keep `append()` signature stable by allowing an optional pre-set id.
3. **P1-2** (archive/unlink crash window) — swap `writeFile + unlink` for `rename + background gzip`. Straightforward.

### SHOULD FIX (can be Plan 19-04.1 follow-up)

4. **P1-3** (decompression bomb) — streaming gunzip with size cap.
5. **P2-1** (rate limiting) — token bucket middleware.
6. **P2-2** (error sanitization) — generic error strings client-side.
7. **P2-3** (Promise.race wrap) — tiny hook patch.

### ACCEPT as-is (document in WAL schema)

- **P0-2** (agent forgeable) — acknowledge in schema: "agent field is client-asserted; audit trail is per-PSK-holder, not cryptographically bound." Per-agent PSKs = v4.0 Phase 22.

### No action needed

- PSK timing safety, parseBody early-reject, openDialogues N=500, hook per-request timeout.

## Gate for Step 2 (PLAN)

Per UNIFIED-MMA-PROTOCOL.md, Step 2 (PLAN) proposes concrete fix plans for the consensus P0/P1 items. Step 2 uses 5 different-role models to design fix plans with actions/risk/rollback. Budget remains within $5.

**James needs user sign-off to proceed to Step 2.** Alternative: ship Phase 19 with fixes for P0-1, P1-1, P1-2 directly (smaller patch, no Step 2 ceremony) — since these are all local code changes with obvious fixes and we've already converged.

---

**Raw model outputs:** `mma-diagnose-output/{deepseek-r1,deepseek-v3,grok-code,mimo-v2-pro,gemini-flash}.json`
**Prompt:** `mma-diagnose-output/prompt.txt`
