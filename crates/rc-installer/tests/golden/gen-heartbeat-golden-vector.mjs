#!/usr/bin/env node
// PROPOSED license_heartbeat cross-language canonicalization golden-vector
// generator. PENDING Replit confirmation of the recipe — see
// /root/.bono-staging/SPEC-PROPOSAL-license-heartbeat-canonical-signing-20260601.md
// (Q1). This SCAFFOLD lets us flip "proposed" → "pinned" the moment Replit
// confirms the field-set/ordering; until then it proves TS<->Rust agree on the
// PROPOSED recipe and freezes it so a later drift is caught.
//
// OWNERSHIP (Replit reply 2026-06-01): there is NO LicenseHeartbeat schema in
// packages/contracts yet (release.yaml Phase L §7 is "SPEC ONLY"). The canonical
// source of truth WILL be the Replit-authored packages/contracts LicenseHeartbeat
// (YAML + zod + canonical serializer + parity gate), gated on an AUTH-MEMO-1 rule
// #5 ratify + Captain sign-off (tier-auth / B9 key surface). THIS generator
// MIRRORS that contract + is a test-vector source — NOT the reference (same
// precedent as ReleaseManifest <- release.ts). Post-ratify, the contract serializer
// emits the canonical bytes and the Rust test diffs against them.
//
// Recipe (mirrors crates/rc-installer/src/manifest.rs `canonical_signed_bytes`):
//   - compact JSON (no spaces),
//   - field order = struct DECLARATION order (NOT alphabetical),
//   - the `signature` field is EXCLUDED from the signed bytes,
//   - `feature_opt_in` is an object with keys SORTED lexically (so it is
//     deterministic across languages; Rust BTreeMap<String,_> sorts for free),
//   - `issued_at` / `valid_until` are Unix-millisecond integers,
//   - `next_refresh_after` lives OUTSIDE license_heartbeat (200-body sibling) →
//     NOT part of the signed bytes, so it is absent here.
//
// node:crypto ed25519 (no new dep). Raw 32-byte pubkey = JWK `x` (base64url) =
// ed25519-dalek VerifyingKey::to_bytes(); raw 64-byte sig = Signature::to_bytes().
//
// Heartbeat DATA is fixed (machine_fingerprint is a fixed 64-hex literal, NOT a
// runtime sha) → canonical_bytes_hex is deterministic across runs; only keys +
// signatures are random. Run once, commit license-heartbeat-golden.json. The
// Rust test (canonical_heartbeat_golden_vector.rs) also pins the production
// canonical bytes to a HARDCODED constant the generator never touches, so a
// silent recipe change is caught even after a regenerate. Regenerate:
//   node crates/rc-installer/tests/golden/gen-heartbeat-golden-vector.mjs

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// feature_opt_in must serialize with SORTED keys for cross-language determinism
// (Rust BTreeMap does this implicitly; JS objects preserve insertion order, so
// we sort explicitly).
function sortedObj(o) {
  const out = {};
  for (const k of Object.keys(o).sort()) out[k] = o[k];
  return out;
}

// Canonical signed payload in EXACT declaration order (signature excluded).
function canonicalOf(hb) {
  return {
    tenant_id: hb.tenant_id,
    machine_fingerprint: hb.machine_fingerprint,
    issued_at: hb.issued_at,
    valid_until: hb.valid_until,
    signing_key_id: hb.signing_key_id,
    license_class: hb.license_class,
    feature_opt_in: sortedObj(hb.feature_opt_in),
  };
}

function buildVector(label, hb) {
  const canonicalJson = JSON.stringify(canonicalOf(hb));
  const canonicalBytes = Buffer.from(canonicalJson, "utf8");
  const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
  const signature = crypto.sign(null, canonicalBytes, privateKey);
  const pubRaw = Buffer.from(publicKey.export({ format: "jwk" }).x, "base64url");
  return {
    label,
    kid: hb.signing_key_id,
    public_key_hex: pubRaw.toString("hex"),
    canonical_bytes_hex: canonicalBytes.toString("hex"),
    canonical_json: canonicalJson,
    signature_hex: signature.toString("hex"),
    heartbeat: { ...hb, signature: signature.toString("hex") },
  };
}

// Fixed 64-hex machine fingerprints (composite sha256-hex shape per Q3) — fixed
// literals, NOT runtime shas, so the canonical bytes (and the Rust frozen
// anchor) are hand-computable.
const FP_PROD = "f0e1d2c3b4a5968778695a4b3c2d1e0f00112233445566778899aabbccddeeff";
const FP_TRIAL = "1a2b3c4d5e6f70819293a4b5c6d7e8f9000102030405060708090a0b0c0d0e0f";

const ISSUED_AT = 1748649600000; // Unix ms
const VALID_UNTIL = ISSUED_AT + 3600000; // +1h TTL

// production class with a NON-EMPTY feature_opt_in (sorted: multiplayer < telemetry).
const production = {
  tenant_id: "rp-hyd",
  machine_fingerprint: FP_PROD,
  issued_at: ISSUED_AT,
  valid_until: VALID_UNTIL,
  signing_key_id: "rc-hb-test-001",
  license_class: "production",
  feature_opt_in: { telemetry: false, multiplayer: true },
};

// trial class with an EMPTY feature_opt_in (proves {} canonicalizes identically).
const trial = {
  tenant_id: "rp-hyd",
  machine_fingerprint: FP_TRIAL,
  issued_at: ISSUED_AT,
  valid_until: VALID_UNTIL,
  signing_key_id: "rc-hb-test-002",
  license_class: "trial",
  feature_opt_in: {},
};

// A valid-but-unrelated ed25519 public key (signed nothing here) for the
// wrong-key negative test.
const wrongPubHex = Buffer.from(
  crypto.generateKeyPairSync("ed25519").publicKey.export({ format: "jwk" }).x,
  "base64url",
).toString("hex");

const fixture = {
  _comment:
    "PROPOSED license_heartbeat golden vectors (pending Replit recipe confirm). For each vector, the Rust test-local canonical_signed_bytes MUST equal canonical_bytes_hex and ed25519 verify_strict MUST accept the signature. wrong_public_key_hex is a valid-but-unrelated key for the negative test. Generated by gen-heartbeat-golden-vector.mjs.",
  recipe_status: "PROPOSED-pending-replit-confirm",
  wrong_public_key_hex: wrongPubHex,
  vectors: [
    buildVector("production-non-empty-feature-opt-in", production),
    buildVector("trial-empty-feature-opt-in", trial),
  ],
};

const outPath = path.join(__dirname, "license-heartbeat-golden.json");
fs.writeFileSync(outPath, JSON.stringify(fixture, null, 2) + "\n");
console.log(`wrote ${outPath}`);
for (const v of fixture.vectors) {
  console.log(`  [${v.label}] canonical (${Buffer.from(v.canonical_json, "utf8").length}B): ${v.canonical_json}`);
}
console.log(`  wrong_public_key_hex=${wrongPubHex}`);
