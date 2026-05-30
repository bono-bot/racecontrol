# rc-installer V2 (web-distributed) — §S-146 RCA + Build Design

**Date:** 2026-05-31 IST · **Branch:** `feat/rc-installer-v2-web` (from scope-freeze base `8ff1e4f5`)
**Author:** bono · **Authority:** Captain this session — *"let's build RC Installer"* + *"deployable via a website URL link instead of USB Drive"* + AskUserQuestion answers (payload model = **runtime CDN fetch (spec §6/§7)**; crate shape = **new Tauri GUI crate from scratch**) + 2026-05-31 reconcile decision (*"Reconcile now"*).
**Supersession recorded:** the earlier audit's **Rule 3** ("payloads swapped at *package time*, not runtime fetch") is **superseded by the Captain's explicit direction this session** → runtime CDN fetch + web-URL distribution. This is the ratified R-133 spec's model. No silent override — Captain-chosen. **NOTE (MMA flag, see Part C / Q1):** 5/5 MMA models recommend a *hybrid* (embed signed baseline + CDN for updates) over pure runtime-fetch for offline-venue resilience — flagged for the later fetch-increment design; does NOT affect this trust-core increment.

---

## Part A — §S-146 RCA (V1 pendrive installer → V2 web bootstrapper)

Required before H1 PLAN for a deployed-section change that shares lineage with V1. The V1 artifact is the committed pendrive crate (`crates/rc-installer/src/main.rs` @ base, 1531 lines, Windows-only via `#[cfg(not(windows))] compile_error!`). It is the V1-shaped pod installer; V2 replaces it. **Foundational boundary (`pod-state-channel`) → MMA Step 1 DIAGNOSE ran before this RCA was finalized (Part C).**

### §1 — Boundary map (paths + lines)

| V1 boundary | Location | What it does |
|---|---|---|
| Distribution = USB | `main.rs:83-90` `get_source_dir()` → exe's own dir = pendrive | Reads payloads from the drive the .exe sits on. **Deleted in V2** (web download). |
| Hardcoded topology | `main.rs:39-43` `DEST_DIR`, `CORE_URL=ws://192.168.31.23:8080/ws/agent`, `CORE_IP`, `HEARTBEAT_PORT=9999` | Single-venue constants baked in binary. **V2: no per-venue constants** (spec §4.12). |
| Pod-only | `main.rs:1422` `get_pod_number()` 1–8 | No Server profile. **V2: Server\|Pod profile picker** (spec §5 step 2). |
| Payload copy | `main.rs:469` `copy_files()` `fs::copy` from pendrive | Raw byte copy, **no signature check**. **V2: ed25519+sha256 gate before any execution** (spec §7). |
| Windows hardening | `main.rs:361` kill/mutex, `:691` edge, `:723` registry, `:774` legacy cleanup, `:1054` start | Defender/mutex/service-install logic. **Reusable** as the V2 Pod-install backend (later increment). |
| Self-identity | none | V1 binary cannot verify itself. **V2: bootstrapper self-sha against `installer_artifact.sha256`** (§7 step 6). |

### §2 — Inherited-issue catalogue
I do **not** have the §S-61 V1 failure-mode doc or categories A–J loaded this session (honest gap). Catalogue derived from the V1 code directly:
- **I-1 No supply-chain trust** — V1 executes whatever sits on the USB. Swapped pendrive runs arbitrary code. (trust-chain.)
- **I-2 Single-venue baked-in** — `192.168.31.23` in the binary → one binary per venue; productization-blocking.
- **I-3 No upgrade/idempotency contract** — re-run semantics implicit.
- **I-4 No uninstall/--purge** — (audit GI-12, "highest blast-radius") absent.
- **I-5 No resume** — interrupted install leaves partial state, no `.partial` recovery.
- **I-6 Defender/Authenticode trust is implicit** — `install.bat` excludes Defender; no Authenticode publisher gate.

### §3 — Past-bug disposition
| Issue | Disposition |
|---|---|
| I-1 supply-chain | **root-caused in V2** — signature chain §7 is structural (`signature_verifier`), not optional. |
| I-2 single-venue | **root-caused** — `config.rs` has one tier-1 base; zero per-venue constants. |
| I-3 idempotency | **designed** — install-marker → upgrade UX (spec §4.11); SEAM until install module. |
| I-4 uninstall | **not-applicable this increment** — tracked as a V2 GI; not in trust core. |
| I-5 resume | **designed** — `.partial` + prefix re-verify (spec §4.10); SEAM until fetch module. |
| I-6 Authenticode | **partly-applicable** — Windows shell owns Authenticode (spec §4.6); we own the ed25519 release-trust gate. **MMA Q4: Authenticode-only is insufficient; step-6 self-sha is the second factor (implemented this increment).** |

### §4 — V2-alignment delta
V2 keeps the V1 Windows-hardening *mechanics* (hard-won, correct for the install step) but inverts the **trust and distribution** posture: USB→web, copy→verify-then-stage, baked-venue→tenant-token-at-runtime, pod-only→profile-driven. The signature chain is the new load-bearing addition with no V1 analogue.

### §5 — V2-framed proposal
Build the spec's web-distributed Tauri bootstrapper, **verification-first** (spec §13.3). Layer order: (1) trust core [this increment] → (2) CDN fetch → (3) Tauri GUI + frontend wizard → (4) download website/endpoint → (5) agent provisioning + capability seed. Foundational gates (signing-key custody / rule #5 / B8 ceremony) stay **Captain-physical**; this increment ships **placeholder kids that fail closed**. **Data model reconciled to the canonical contract `packages/contracts/src/release.ts` this increment (Captain "Reconcile now" 2026-05-31)** — see Part C / Q3.

---

## Part B — Build design

### Distribution model (the "website URL" directive)
Operator browses to `https://console.racecontrol.app/install/bootstrapper` (spec §2) → downloads a small **signed** bootstrapper → runs it → it fetches per-profile payloads from the tier-1 CDN at runtime and **verifies every one** against the embedded ed25519 trust set before execution. No USB path. (MMA-flagged hybrid variant deferred to fetch-increment.)

### Crate factoring
One crate, two layers:
- **Trust core (`src/lib.rs` + modules)** — pure, platform-independent Rust. Compiles + unit-tests on Linux, cross-compiles to `x86_64-pc-windows-gnu`. Spec §7 + doctrine non-negotiables live here. **Built + tested this increment.**
- **Tauri GUI shell (`src/main.rs` V1 + later Tauri runtime, feature `gui`)** — Windows/CI-built (this host lacks `webkit2gtk`/Tauri CLI; the V1 `main.rs` is `#[cfg(not(windows))] compile_error!`-guarded → host builds MUST scope to `--lib --tests`). **Authored next increment.** V1 `main.rs` left untouched this increment (reversible).

### Canonical data model (reconciled to `release.ts` this increment)
| Type | Fields (declaration order = canonical signed order) |
|---|---|
| `ReleaseManifest` | release_id, release_class, release_ring, artifacts[], installer_artifact, previous_release_id (Option), cut_at (**u64 Unix-ms**), signing_key_id, signature (**excluded from signed bytes**) |
| `ReleaseArtifact` | **artifact_id**, sha256, size_bytes (u64), target  *(NO download_url — URL derived from CDN base + id)* |
| `InstallerArtifact` | sha256, size_bytes (u64), download_url (child of `console.racecontrol.app/install/`), signing_key_id |

`canonical_signed_bytes()` serializes all `ReleaseManifest` fields **except `signature`**, compact (`serde_json::to_vec` on a borrowing `#[derive(Serialize)]` struct in declaration order — NOT `Value` [alphabetizes], NOT `serde(skip)` [breaks round-trip]). `#[serde(deny_unknown_fields)]` on all three (matches TS `.strict()`).

### Module status (spec §3 layout)
| Module | This increment | Notes |
|---|---|---|
| `error.rs` | ✅ `VerifyError` = spec §8 codes (Debug+Clone+PartialEq+thiserror) | |
| `config.rs` | ✅ one tier-1 base `https://console.racecontrol.app` (§4.5) | |
| `profile.rs` | ✅ Server\|Pod + `as_str()`=server/pod | D-INSTALLER-1: no flavor enum |
| `manifest.rs` | ✅ canonical types + `canonical_signed_bytes` + `from_json` + `installer_artifact_count` | reconciled to release.ts |
| `trusted_keys.rs` | ✅ fail-closed lookup (§4.4) | |
| `signature_verifier.rs` | ✅ §7 verify_manifest (5 ordered gates) + verify_installer_sha + verify_artifact_sha; ed25519 **verify_strict** | |
| `manifest_fetcher.rs` / `velopack_fetcher.rs` | ⏭ next | HTTP GET + fetch_verify_and_stage (no raw-fetch path, §4.2) |
| `agent_provisioner.rs` / `capability_seed.rs` / `post_install_log.rs` | ⏭ SEAM | |
| `main.rs` / `installer_wizard.rs` / GUI | ⏭ Tauri increment | V1 main.rs untouched |

### What is NOT done / NOT verified this increment
- No Tauri build, no GUI, no frontend, no actual HTTP fetch, no Windows .exe produced here.
- No real signing keys (placeholder kids only; B8 ceremony is Captain-physical).
- **No real-key signature parity exercised** (placeholder keys fail-closed). Data-model SHAPE reconciled to `release.ts`; the canonicalization-FORMAT pinning + cross-language golden vector remain (Part C / Q3).
- The download website + `getInstallerBootstrapper` endpoint not built (next increments, partly rp-v2-apps).

---

## Part C — MMA Step 1 DIAGNOSE findings + dispositions

**Surface:** `MMA-RC-INSTALLER-bono-2026-05-31` · **5 models, 5 vendor families** (deepseek-r1-0528, qwen3-coder, nemotron-3-super, gemini-2.5-pro, kimi-k2.5) · **$0.06014** · spend → `comms-link/data/openrouter-spend-bono.jsonl` · raw → `/tmp/mma-rcinstaller-results/`. (gemini truncated at A2.CHAIN-HOLES on max_tokens; other 4 complete.)

### Q1 — runtime-CDN-fetch vs offline reality → **5/5 HYBRID**
All five recommend switching pure runtime-fetch → **hybrid** (embed a signed baseline payload for offline cold-start; CDN for updates). Gaps: TLS/CA MITM (corporate proxy), CDN/BGP compromise, DNS hijack, captive-portal-HTML-as-payload, partial-download TOCTOU. **Disposition:** architecture finding for the **fetch increment** (contradicts the Captain-locked runtime-fetch choice → revisit at fetch-increment design). Does NOT affect the trust core (signatures verified either way). **Recorded; no action this increment.**

### Q2 — replay/downgrade/rollback → **5/5 NO protection exists**
A validly-signed *old* manifest can be replayed to force a vulnerable release; `previous_release_id` + `cut_at` present but **unused**. **Must-add-before-real-key (ranked consensus):**
1. Persist a release_id / cut_at **high-water mark** (protected registry/ACL); reject manifest ≤ stored (anti-rollback).
2. **Bind `trusted-keys.json` into the manifest signature** — currently embedded *unsigned* relative to the manifest (key-substitution risk if the binary is tampered). (Contract change.)
3. `cut_at` **freshness window** (reject manifests older than N days).
4. **TOCTOU-resistant** verify→stage (verify in-memory stream, not the on-disk file).
5. Online key **revocation** check / next-key-fingerprint commitment in the manifest.
**Disposition:** all are fetch/runtime/contract-layer concerns — none are in the pure verify core, none are tested by the staged tests. **Recorded as before-real-key gates** (security-debt ledger). The trust core this increment is signature+sha verification only.

### Q3 — reconcile data model NOW vs defer → **3/5 RECONCILE-NOW** (deepseek, nvidia, moonshot; qwen defer; gemini no-answer) → **Captain decision 2026-05-31: RECONCILE NOW**
Divergences (all in signed fields) reconciled to `release.ts` this increment: cut_at→u64 Unix-ms · release_ring uses {canary,early,general} · ReleaseArtifact→{artifact_id,sha256,size_bytes,target} (dropped download_url) · artifacts non-empty in realistic fixtures. The two staged test fixtures were rewritten to the canonical shape. **Still open after reconcile (5/5 flagged):** "canonical-JSON" underspecifies number formatting, unicode escaping, nested-object key ordering, null handling → these silently break byte-parity even with shapes aligned. **Before-real-key gate:** pin the canonicalization FORMAT spec + a golden cross-vector (TS signs → Rust verifies → assert byte-identical `canonical_signed_bytes`). **Recorded.**

### Q4 — fail-closed + Authenticode → **5/5**
Placeholder-key fail-closed (`PlaceholderKid`) is correct; add a loud, actionable error (`"run B8 key provisioning"`) + a dev-mode cfg flag to avoid training operators to ignore failures (later GUI increment). Authenticode-only is **insufficient** (stolen-cert threat) → step-6 self-sha (`InstallerArtifactShaMismatch`) is the second factor and IS implemented in this increment's verify core. Build-trust-core-first sequencing endorsed *provided* the golden vector follows (before-real-key gate above).

### Before-real-key gate (consolidated — logged to security-debt ledger)
1. Anti-rollback high-water mark · 2. trusted-keys bound into manifest signature · 3. cut_at freshness · 4. TOCTOU-resistant staging · 5. online revocation / next-key commitment · 6. canonicalization-format spec + golden cross-vector · 7. (design) hybrid offline-baseline payload. All gate the FIRST real signing key; none block this placeholder-key trust-core increment.
