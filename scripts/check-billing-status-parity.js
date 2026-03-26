#!/usr/bin/env node
// check-billing-status-parity.js
// Cross-language drift prevention. Exit 0=ok, Exit 1=mismatch.
// Usage: node scripts/check-billing-status-parity.js

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.join(__dirname, "..");
let exitCode = 0;

function countRustEnumVariants(rustSrc, enumName) {
  const idx = rustSrc.indexOf("pub enum " + enumName);
  if (idx === -1) return null;
  const braceStart = rustSrc.indexOf("{", idx);
  if (braceStart === -1) return null;
  let depth = 0;
  let enumBody = "";
  for (let i = braceStart; i < rustSrc.length; i++) {
    const ch = rustSrc[i];
    if (ch === "{") depth++;
    else if (ch === "}") {
      depth--;
      if (depth === 0) { enumBody = rustSrc.slice(braceStart + 1, i); break; }
    }
  }
  if (!enumBody) return null;
  const lines = enumBody.split('\n');
  let count = 0;
  for (const line of lines) {
    const t = line.trim();
    if (!t || t.startsWith("//") || t.startsWith("///") || t.startsWith("#")) continue;
    if (/^[A-Z][A-Za-z0-9_]*/.test(t)) count++;
  }
  return count;
}

function countTsUnionVariants(tsSrc, typeName) {
  const typeIdx = tsSrc.indexOf("export type " + typeName);
  if (typeIdx === -1) return null;
  const afterType = tsSrc.slice(typeIdx);
  const semiIdx = afterType.indexOf(";");
  if (semiIdx === -1) return null;
  const block = afterType.slice(0, semiIdx + 1);
  // Match both | "..." (subsequent variants) and = "..." or = \n  | "..." (first variant may follow =)
  const pipeMatches = block.match(/\| "[^"]+"/g) || [];
  // Also count first variant after = sign (e.g. = "idle" | "launching" ...)
  const firstVariantMatch = block.match(/=\s*"([^"]+)"/);
  const firstCount = firstVariantMatch ? 1 : 0;
  return pipeMatches.length + firstCount;
}

function checkParity(enumName, rustFile, tsFile, tsTypeName) {
  console.log('\nChecking ' + enumName + ' parity...');
  let rustSrc, tsSrc;
  try { rustSrc = fs.readFileSync(rustFile, "utf-8"); }
  catch (e) { console.error("  ERROR reading " + rustFile); exitCode = 1; return; }
  try { tsSrc = fs.readFileSync(tsFile, "utf-8"); }
  catch (e) { console.error("  ERROR reading " + tsFile); exitCode = 1; return; }
  const rustCount = countRustEnumVariants(rustSrc, enumName);
  const tsCount = countTsUnionVariants(tsSrc, tsTypeName);
  if (rustCount === null) { console.error("  ERROR: enum " + enumName + " not found in Rust"); exitCode = 1; return; }
  if (tsCount === null) { console.error("  ERROR: type " + tsTypeName + " not found in TS"); exitCode = 1; return; }
  if (rustCount !== tsCount) {
    console.error("  MISMATCH: BillingSessionStatus drift detected!");
    console.error("    Rust " + enumName + ": " + rustCount + " variants");
    console.error("    TS " + tsTypeName + ": " + tsCount + " variants");
    exitCode = 1;
  } else {
    console.log("  OK: " + enumName + " -- " + rustCount + " variants match");
  }
}

checkParity(
  "BillingSessionStatus",
  path.join(ROOT, "crates/rc-common/src/types.rs"),
  path.join(ROOT, "packages/shared-types/src/billing.ts"),
  "BillingSessionStatus"
);

checkParity(
  "GameState",
  path.join(ROOT, "crates/rc-common/src/types.rs"),
  path.join(ROOT, "packages/shared-types/src/pod.ts"),
  "GameState"
);

console.log("");
if (exitCode === 0) { console.log("All parity checks passed."); }
else { console.error("Drift detected -- see errors above."); }
process.exit(exitCode);
