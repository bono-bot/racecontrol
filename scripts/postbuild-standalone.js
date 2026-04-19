#!/usr/bin/env node
// postbuild-standalone.js — Cross-platform Next.js standalone postbuild
// Copies .next/static and public/ into .next/standalone/ for standalone deploys.
// Replaces the bash-only commands in package.json build scripts that fail on Windows cmd.exe.
//
// Usage: node ../scripts/postbuild-standalone.js

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

function copyDirSync(src, dest) {
  if (!fs.existsSync(src)) return;
  fs.mkdirSync(dest, { recursive: true });
  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const srcPath = path.join(src, entry.name);
    const destPath = path.join(dest, entry.name);
    if (entry.isDirectory()) {
      copyDirSync(srcPath, destPath);
    } else {
      fs.copyFileSync(srcPath, destPath);
    }
  }
}

const standaloneDir = path.join('.next', 'standalone');
if (!fs.existsSync(standaloneDir)) {
  console.error('ERROR: .next/standalone/ does not exist — is output: "standalone" set in next.config?');
  process.exit(1);
}

// Copy .next/static -> .next/standalone/.next/static
const staticSrc = path.join('.next', 'static');
const staticDest = path.join(standaloneDir, '.next', 'static');
if (fs.existsSync(staticSrc)) {
  copyDirSync(staticSrc, staticDest);
  console.log('postbuild: copied .next/static -> standalone');
} else {
  console.error('ERROR: .next/static/ not found');
  process.exit(1);
}

// Copy public/ -> .next/standalone/public (if exists)
const publicSrc = 'public';
const publicDest = path.join(standaloneDir, 'public');
if (fs.existsSync(publicSrc)) {
  copyDirSync(publicSrc, publicDest);
  console.log('postbuild: copied public/ -> standalone');
}

// Write git commit to standalone for /api/health reporting. Belt-and-suspenders:
// write to BOTH standalone/public/git-commit.txt (health route's first fallback
// path at process.cwd()+public/) AND standalone/git-commit.txt (third fallback).
// This ensures the truth-ledger survives even when the prebuild inject was
// skipped or failed on the host — observed on Bono VPS 2026-04-19 where
// validate-frontend-env.sh in the prebuild chain prevented inject from running.
//
// Run git from the racecontrol repo root (scripts/..), NOT kiosk/web cwd —
// those dirs contain stray .git/ scaffolding (no commits) from shadcn/Next
// scaffolds that hijacks git resolution and fails with "Needed a single revision".
const REPO_ROOT = path.resolve(__dirname, '..');

/**
 * Write a value to a file, then read back to verify. Retries on mismatch.
 * Hard-fails after 3 attempts — post-deploy freshness gates depend on this.
 *
 * Why: empirically, a prior write to standalone/public/git-commit.txt during
 * the full `npm run build` pipeline silently reverted to a stale value,
 * causing deploy freshness-gate rollbacks. Root cause unidentified (possibly
 * Next.js internal static-copy phase racing with postbuild). This read-back
 * loop makes the write observably correct or fails LOUD.
 */
function writeVerified(filePath, value) {
  for (let attempt = 1; attempt <= 3; attempt++) {
    fs.writeFileSync(filePath, value);
    const readback = fs.readFileSync(filePath, 'utf8');
    if (readback === value) {
      if (attempt > 1) {
        console.warn(`postbuild: write to ${filePath} succeeded on retry ${attempt}`);
      }
      return;
    }
    console.warn(`postbuild: WARN attempt ${attempt}: wrote ${JSON.stringify(value)}, read back ${JSON.stringify(readback)} — retrying`);
  }
  console.error(`postbuild: FATAL: ${filePath} readback mismatch after 3 attempts — freshness gate will fail`);
  process.exit(2);
}

try {
  const gitCommit = execSync('git rev-parse --short HEAD', { encoding: 'utf8', cwd: REPO_ROOT }).trim();
  const standalonePublic = path.join(standaloneDir, 'public');
  fs.mkdirSync(standalonePublic, { recursive: true });
  writeVerified(path.join(standaloneDir, 'git-commit.txt'), gitCommit);
  writeVerified(path.join(standalonePublic, 'git-commit.txt'), gitCommit);
  console.log(`postbuild: wrote git-commit.txt (${gitCommit}) to standalone/ and standalone/public/ (readback-verified)`);
} catch (e) {
  console.warn('postbuild: could not determine git commit:', e.message);
}

// Verify static chunks exist
const chunksDir = path.join(staticDest, 'chunks');
if (fs.existsSync(chunksDir)) {
  const jsFiles = fs.readdirSync(chunksDir).filter(f => f.endsWith('.js'));
  if (jsFiles.length > 0) {
    console.log(`postbuild: verified ${jsFiles.length} JS chunks in standalone`);
  } else {
    console.error('ERROR: no .js files in standalone static/chunks/');
    process.exit(1);
  }
} else {
  console.error('ERROR: static/chunks/ directory missing from standalone');
  process.exit(1);
}
