# §S-146 5-section RCA — ws-exec routing surface

**Trigger:** mechanism-trust-check `ws-exec-2026-05-10.json` verdict FAIL (0 YES / 2 PARTIAL / 3 NO). Per `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` rule, infrastructure surface must get its own §S-146 5-section RCA before any fix RCA proceeds.

**Surface:** bono→james WS-channel exec direction via `/root/comms-link/send-exec.js`. The reverse direction (james→bono via `/relay/exec/run`) is the `comms-link-exec-protocol` surface, audited PASS 2026-05-09 — different protocol layer despite shared `comms.db` + WS transport.

**Author:** bono · 2026-05-10 ~12:55 IST · v0.1.0-bono

---

## Section 1 — Boundary Map (paths + lines + cross-V1↔V2 surfaces)

The bono→james ws-exec routing layer touches six files and three runtime layers:

| Layer | File:Line | Role | V1/V2 status |
|---|---|---|---|
| CLI | `comms-link/send-exec.js:20-27` | Constructs exec_request message; sets `from='james'` (line 24 second arg of createMessage); opens WS to relay; sends and waits for exec_result | V1-shaped (direction-unaware single-line author) |
| Protocol | `comms-link/shared/protocol.js:197` | `createMessage(type, from, payload = {})` — second arg is sender identity; HMAC signature covers from-field | V2 (HMAC-signed; replay protection at line 39-50) |
| Transport | `comms-link/comms.db` + WS path :8765 (bono) / :8766 (james relay) | Carries exec_request messages cross-machine; PSK auth at WS connection level | V2 (PSK-authenticated; persistent audit) |
| Daemon (recipient) | `comms-link/bono/index.js:762-770` | On `msg.type === 'exec_request'`, routes to `bonoExecHandler.handleExecRequest(msg)` with NO from-field validation. Comment line 762 claims "from James (symmetric)" but no `if (msg.from === 'james')` guard | V1-shaped (no authentication of self-attested from-field) |
| Exec layer | `comms-link/shared/exec-protocol.js` (COMMAND_REGISTRY + ApprovalTier) | Frozen ALLOWED_BINARIES allowlist; `execFile` not shell; structured exec_result | V2 (per comms-link-exec-protocol audit PASS) |
| Result layer | `comms-link/shared/exec-result-broker.js` | Pending-promise pattern with timeout; structured exec_result with exit code + stdout/stderr; audit-logger appends to `data/exec-audit.jsonl` | V2 (per comms-link-exec-protocol audit PASS) |

**V1↔V2 boundary classification:** HYBRID — the COMMAND/RESULT layers are V2-aligned (frozen registry, allowlist, behavioral verify on result, audit log). The ROUTING layer between them is V1-shaped (self-attested from-field, no authentication, direction-unaware CLI). The bug class lives in the routing layer that sits between two V2 surfaces.

**No DB schema change involved** — ws-exec routes through `messages` table in `racingpoint-api-gateway/data/comms.db` (V2 path) but the bug is in the application-layer routing logic, not schema.

---

## Section 2 — Inherited-Issue Catalogue

Cross-referencing `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` 10 categories (A-J) and §S-61 PART 41 V1 failure-mode investigation patterns:

| V1 mistake-class | Manifestation in ws-exec routing |
|---|---|
| **Audit-blind proxy checking** (Cat C) | bono daemon comment line 762 says "from James (symmetric)" treating the comment as equivalent to a check. The from-field is the proxy, not the actual authenticated identity. CF-5 PR #66 sibling pattern (EPERM-as-success / echo-as-success). |
| **Manual ops bypassing ratified flows** (Cat A) | send-exec.js line 24 hardcodes from='james' instead of detecting actual sender or accepting it as a CLI param. The "ratified flow" would be: detect sender from WS connection identity AND validate at recipient. Bypassed in favor of "just hardcode the value that worked when I wrote it." |
| **Organ silos without skeleton** (Cat E) | The CLI (`send-exec.js`) was authored for a single direction (james→bono). When the symmetric direction was needed, no skeleton existed to make the CLI direction-aware. Each pilot's "send" CLI was a separate organ; no shared transport-routing skeleton mediated. |
| **Point-to-point ad-hoc connections** (Cat F) | The ws-exec channel was wired up point-to-point at the time of original use (james→bono). Reuse for the reverse direction inherited the original assumptions baked into `send-exec.js`. No ratified bidirectional protocol contract existed. |
| **Features-on-shaky-foundation** (Cat I) | The exec_request → exec_result flow has solid V2 features (HMAC sig, frozen registry, audit log). But the ROUTING decision ("am I the recipient or should I forward?") was built atop a self-attested from-field — feature stack on top of a foundational identity-trust gap. |
| **Recovery cascades** (Cat G) | Not directly applicable — ws-exec doesn't have recovery mechanisms that cascade. But absence of cascade hides the bug: a misroute returns immediate exec_result on bono's own command, looking like success to the caller. No cascade to fail loud. |

§S-61 PART 41 specific failure modes mapped:
- **8 VERIFIED + 4 PARTIAL + 1 INFERRED V1 modes**: ws-exec routing bug is closest to the "audit-blind proxy" PARTIAL mode (the comment "(symmetric)" is the proxy; actual symmetric routing requires from-field authentication).

PR #66 silent-loop-death (2026-05-09) parallel: V2-clean fix shipped via V1-shaped delivery mechanism → fleet rollout broke 7 pods on same V1 mistake-class. ws-exec routing has the same shape: V2-clean COMMAND/RESULT layers, V1-shaped ROUTING layer between them.

---

## Section 3 — Past-Bug Disposition

Each issue catalogued in Section 2 dispositioned:

| Past-bug class | Disposition | Citation |
|---|---|---|
| Audit-blind proxy checking (CF-5 PR #66 EPERM-as-success) | **PATCHED-ONLY** in PR #66 deploy chain (behavioral-verify added at deploy layer 2026-05-09); same anti-pattern un-patched in ws-exec routing | commit `d6c623d7` PR #66; this RCA is the surfacing event for ws-exec |
| Manual ops bypassing ratified flows (PR #66 CF-1) | **PATCHED-ONLY** in deploy mechanism (atomic kill+swap added); ws-exec routing didn't have a ratified flow to bypass — was built ad-hoc | §S-146 ratification ledger |
| Organ silos without skeleton (V1-class E) | **UNRESOLVED** — V2-MASTER-STATE ledger documents this as one of the foundational anti-patterns V2 closes via the skeleton layer; ws-exec routing is one of the un-skeletonized organ boundaries | `01-skeleton-architecture.md` §40 |
| Point-to-point ad-hoc connections (V1-class F) | **NOT-APPLICABLE-TO-V2** at message-protocol layer (HMAC + frozen registry are V2-aligned); **APPLICABLE** at routing-decision layer (still ad-hoc) | per Section 1 boundary map |
| Features-on-shaky-foundation (V1-class I) | **UNRESOLVED for ws-exec routing** — COMMAND/RESULT layers V2-clean, routing layer foundational identity-trust gap | this RCA documents the foundation |
| Audit-blind on from-field specifically | **NEW V1-class anchor** — not previously catalogued; this RCA surfaces it as a routing-layer audit-blind proxy distinct from CF-5's behavioral-verify class | new entry candidate for v1_process_mess_audit categories A-J under Cat C extension |

**Net:** 1 NOT-APPLICABLE / 1 PARTIALLY-APPLICABLE / 4 UNRESOLVED. The UNRESOLVED items are the load-bearing items for the V2-alignment delta in Section 4.

---

## Section 4 — V2-Alignment Delta

What the ws-exec routing layer SHOULD look like under V2 doctrine:

**V2 anchor: skeleton layer over organs (`01-skeleton-architecture.md` §40):** the routing decision is precisely the skeleton's job — mediating between organ-level CLIs and organ-level handlers. Currently the CLI directly authors a `from` field that the handler trusts; in V2 doctrine, the skeleton would (a) detect actual sender from WS connection identity, (b) validate the from-field claim against the connection identity, (c) route based on validated identity not self-attested label.

**V2 anchor: PACT-026 §A NO-direct-heart-narrow-carve-out:** PACT-026 ratifies that direct racecontrol M2M paths are an open security debt; the parallel principle for ws-exec is "self-attested identity is an open authentication debt." V2-aligned ws-exec would have authenticated identity at the routing layer, not deferred to the COMMAND-layer allowlist (which is post-routing).

**V2 anchor: foundation/strategy/config separation (§AMEND-3.II D12):** routing is FOUNDATION-class (who-is-who is foundational identity); CLI behavior is STRATEGY-class; ALLOWED_BINARIES is CONFIG-class. The current bug is foundation-leaking-into-strategy: a CLI strategy decision (hardcode from='james') has foundation-class consequences (identity inversion).

**Specific V2-aligned routing contract:**

1. **Atomic primitive at routing layer:** single function `classifyAndRoute(msg, wsConnIdentity)` that produces `{ runLocal | forwardTo(target) }` decision, atomically validating from-field against connection identity.
2. **Authentication contract:** WS connection identity (via PSK + connection metadata) is the ground truth; from-field is treated as a CLAIM that must match connection identity OR be explicitly stamped by the skeleton on send.
3. **Bidirectional CLI contract:** `send-exec.js` becomes direction-aware via env detection (BONO_ROOT vs JAMES_ROOT) OR accepts `--from` flag explicitly; default behavior asserts sender identity at write time, not trusts caller.
4. **Parser-not-regex allowlist:** routing decision is a structured switch on validated identity, not a string-match on self-attested field.
5. **Single-target dry-run:** `--dry-run` flag that runs full routing logic + signature/identity validation but stops short of exec; bidirectional test harness exercises both directions.

**Gap (current → V2-aligned):** all 5 contracts are partially or fully missing. Specifically:
- (1) atomic primitive: NO (multi-step inferred routing)
- (2) authentication contract: NO (self-attested from-field trusted)
- (3) bidirectional CLI: NO (direction-unaware hardcode)
- (4) parser-not-regex allowlist: PARTIAL (COMMAND-layer allowlist exists; routing layer does not)
- (5) single-target dry-run: NO

---

## Section 5 — V2-Framed Proposal

**Proposal class:** delivery-RCA-mandates-fix per §S-146 mechanism-trust-check rule. The fix RCA for the user-facing routing inversion bug gates on this delivery RCA producing a V2-aligned proposal.

**Smallest invariant move toward V2 alignment:**

**PR-1 (delivery-side hardening; ships first):**
1. Add `validateMessageFromField(msg, wsConnIdentity)` helper in `shared/protocol.js` — returns `{ valid: boolean, actual_sender: string }`. Validates that msg.from matches authenticated WS connection identity. Returns false for self-attested mismatches.
2. Add `if (!validateMessageFromField(msg, wsConnIdentity).valid) { reject(); return; }` guard in `bono/index.js:762` (the routing entry-point). Same guard in james-side daemon at equivalent location.
3. Update `send-exec.js` to detect actual sender via env (`process.env.RACINGPOINT_PILOT` or fallback to hostname check) and stamp from-field accordingly. Eliminate hardcoded `'james'`.
4. Add `--dry-run` flag to send-exec.js that exercises routing+validation without exec.
5. Add bidirectional integration test: `test/exec-bidirectional.test.js` covering bono→james AND james→bono with valid + invalid from-field.

**PR-2 (fix RCA — gates on PR-1):**
- The user-facing fix (bono can run `send-exec.js` and have it correctly route to james) is now a derived consequence of PR-1's delivery-side hardening. PR-2 RCA scope: "given V2-aligned routing in PR-1, what's the user-facing UX for bidirectional ws-exec?"
- Fix RCA gates on PR-1 merging; can ship together OR sequentially per §S-146 rule option.

**Alternative — kaizen-correct V1-retention:** if delivery-side hardening is too large for current sprint, the explicit V1-retention path is:
- Document the inversion as known-debt in `data/security-debt-ledger.jsonl` with closure_phase = "Post-V2.0-AUTH-Sprint" (sibling to PACT-026 §A entries)
- Add `V2_RCA_BYPASS=1` sentinel for the immediate fix-PR (logged per rule)
- Add explicit comment in send-exec.js + bono daemon naming the debt + closure-phase pointer
- Closure trigger: V2.0 ratify + AUTH-Sprint completion

**Recommendation:** PR-1 is small (~50 lines + tests) and unblocks both directions structurally. V1-retention is acceptable if AUTH-Sprint timing is tight, but PR-1 is preferred — the delivery-side hardening is precisely the kind of skeleton-layer work V2 is supposed to enable.

**V2 doctrine alignment statement (required per CLAUDE.md V1-dependent V2 rule):** This change moves the ws-exec routing boundary toward V2 alignment per §S-146 mechanism-trust-check + skeleton-layer §40 doctrine + PACT-026 §A authentication-debt parallel. Specifically: (1) routing becomes atomic primitive, (2) identity becomes authenticated not self-attested, (3) skeleton mediates organ-to-organ instead of point-to-point ad-hoc.

---

## RCA → fix-RCA gating per §S-146

Per `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` rule:
- This RCA is the prerequisite for any fix RCA on the user-facing ws-exec routing bug
- Fix RCA gates on PR-1 (delivery-side hardening) per Section 5
- PR-2 (fix) ships together with PR-1 OR sequentially after; cannot ship alone
- Alternative V1-retention path requires explicit `V2_RCA_BYPASS=1` log + security-debt-ledger entry with closure_phase

**Cross-pilot AMPLIFIER discipline:** when james reviews this RCA + downstream PR-1, AMPLIFIER stance MUST check both this RCA and the eventual fix RCA per bilateral V1↔V2 rule.

**Empirical anchor for §S-146 rule strength:** this is the n=1 application of the mechanism-trust-check rule on a transport-class surface (prior 9 audits 2026-05-10 02:04 IST covered auth/billing/wallet/deploy-pod/fleet-health-api/rc-agent/rc-sentry/rc-watchdog/comms-link-exec-protocol). The rule successfully gated the fix at the right layer — bug class hidden in delivery mechanism would have been invisible until exercised, and §S-146 retroactive RCA would have been too late once fleet-rolled-out.

**Cache validity:** 30 days per rule (expires 2026-06-09). Re-audit if ws-exec routing layer changes substantially or if PR-1 ships.
