#!/usr/bin/env node
// L1-5 cross-language canonicalization golden-vector generator (before-real-key
// gate #6). Produces TWO ReleaseManifest vectors (first-release: null prev / 1
// artifact; upgrade: Some(prev) / 2 artifacts) + their canonical signed bytes +
// ed25519 signatures, using the SAME canonicalization recipe as the Rust trust
// core (crates/rc-installer/src/manifest.rs `canonical_signed_bytes`):
//   - compact JSON (no spaces),
//   - field order = struct DECLARATION order (NOT alphabetical key-sort),
//   - the `signature` field is EXCLUDED from the signed bytes.
//
// Also emits `wrong_public_key_hex` (a valid-but-unrelated ed25519 key) for the
// Rust negative test that proves the signature check is real (not just payload).
//
// node:crypto ed25519 (no new dep). Raw 32-byte pubkey = JWK `x` (base64url) =
// ed25519-dalek VerifyingKey::to_bytes(); raw 64-byte sig = Signature::to_bytes().
//
// Manifest DATA is fixed → canonical_bytes_hex is deterministic across runs (only
// keys + signatures are random). Run once, commit release-manifest-golden.json.
// The Rust test (canonical_golden_vector.rs) also pins the first-release canonical
// bytes to a HARDCODED constant the generator never touches, so a silent recipe
// change is caught even after a regenerate. Regenerate:
//   node crates/rc-installer/tests/golden/gen-golden-vector.mjs
//
// NOTE: all string data is ASCII. Non-ASCII / float / unicode-escaping
// canonicalization parity (serde_json vs JSON.stringify) is the documented
// before-real-key follow-up (rc-installer MMA Q3 spec-gap).

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const sha = (s) => crypto.createHash("sha256").update(s).digest("hex");

// Canonical signed payload in EXACT CanonicalManifest declaration order
// (signature excluded). Single-sourced so every vector uses one recipe.
function canonicalOf(m) {
  return {
    release_id: m.release_id,
    release_class: m.release_class,
    release_ring: m.release_ring,
    artifacts: m.artifacts.map((a) => ({
      artifact_id: a.artifact_id,
      sha256: a.sha256,
      size_bytes: a.size_bytes,
      target: a.target,
    })),
    installer_artifact: {
      sha256: m.installer_artifact.sha256,
      size_bytes: m.installer_artifact.size_bytes,
      download_url: m.installer_artifact.download_url,
      signing_key_id: m.installer_artifact.signing_key_id,
    },
    previous_release_id: m.previous_release_id,
    cut_at: m.cut_at,
    signing_key_id: m.signing_key_id,
  };
}

function buildVector(label, m) {
  const canonicalJson = JSON.stringify(canonicalOf(m));
  const canonicalBytes = Buffer.from(canonicalJson, "utf8");
  const { publicKey, privateKey } = crypto.generateKeyPairSync("ed25519");
  const signature = crypto.sign(null, canonicalBytes, privateKey);
  const pubRaw = Buffer.from(publicKey.export({ format: "jwk" }).x, "base64url");
  return {
    label,
    kid: m.signing_key_id,
    public_key_hex: pubRaw.toString("hex"),
    canonical_bytes_hex: canonicalBytes.toString("hex"),
    canonical_json: canonicalJson,
    signature_hex: signature.toString("hex"),
    manifest: { ...m, signature: signature.toString("hex") },
  };
}

const firstRelease = {
  release_id: "rel-2026Q2-golden",
  release_class: "feature",
  release_ring: "general",
  artifacts: [
    { artifact_id: "rc-agent-2026Q2-001", sha256: sha("rc-agent-payload"), size_bytes: 16, target: "rc-agent" },
  ],
  installer_artifact: {
    sha256: sha("installer-binary-bytes"),
    size_bytes: 21,
    download_url: "https://console.racecontrol.in/install/bootstrapper",
    signing_key_id: "rc-golden-test-001",
  },
  previous_release_id: null,
  cut_at: 1748649600000,
  signing_key_id: "rc-golden-test-001",
};

// Production UPGRADE shape: non-null previous_release_id + 2 artifacts.
const upgrade = {
  release_id: "rel-2026Q2-upgrade",
  release_class: "critical-security",
  release_ring: "canary",
  artifacts: [
    { artifact_id: "rc-agent-2026Q2-002", sha256: sha("rc-agent-payload-v2"), size_bytes: 32, target: "rc-agent" },
    { artifact_id: "racecontrol-2026Q2-002", sha256: sha("racecontrol-payload-v2"), size_bytes: 64, target: "racecontrol" },
  ],
  installer_artifact: {
    sha256: sha("installer-binary-bytes-v2"),
    size_bytes: 42,
    download_url: "https://console.racecontrol.in/install/bootstrapper-v2",
    signing_key_id: "rc-golden-test-002",
  },
  previous_release_id: "rel-2026Q1-prior",
  cut_at: 1751328000000,
  signing_key_id: "rc-golden-test-002",
};

// A valid-but-unrelated ed25519 public key (signed nothing here) for the
// wrong-key negative test.
const wrongPubHex = Buffer.from(
  crypto.generateKeyPairSync("ed25519").publicKey.export({ format: "jwk" }).x,
  "base64url",
).toString("hex");

const fixture = {
  _comment:
    "L1-5 golden vectors: TS-signed ReleaseManifests. For each vector, Rust canonical_signed_bytes MUST equal canonical_bytes_hex and verify_manifest MUST accept it. wrong_public_key_hex is a valid-but-unrelated key for the negative test. Generated by gen-golden-vector.mjs.",
  wrong_public_key_hex: wrongPubHex,
  vectors: [
    buildVector("first-release-null-prev-1-artifact", firstRelease),
    buildVector("upgrade-some-prev-2-artifacts", upgrade),
  ],
};

const outPath = path.join(__dirname, "release-manifest-golden.json");
fs.writeFileSync(outPath, JSON.stringify(fixture, null, 2) + "\n");
console.log(`wrote ${outPath}`);
for (const v of fixture.vectors) {
  console.log(`  [${v.label}] canonical (${Buffer.from(v.canonical_json, "utf8").length}B): ${v.canonical_json}`);
}
console.log(`  wrong_public_key_hex=${wrongPubHex}`);
