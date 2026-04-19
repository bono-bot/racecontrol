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
  const gitShaFull = execSync('git rev-parse HEAD', { encoding: 'utf8', cwd: REPO_ROOT }).trim();
  let dirty = false;
  try {
    execSync('git diff --quiet HEAD --', { stdio: 'ignore', cwd: REPO_ROOT });
  } catch {
    dirty = true;
  }
  const commitLabel = dirty ? `${gitCommit}-dirty` : gitCommit;

  const nowUtc = new Date();
  const istOffsetMs = (5 * 60 + 30) * 60 * 1000;
  const nowIst = new Date(nowUtc.getTime() + istOffsetMs);
  const buildInfo = JSON.stringify(
    {
      git_commit: commitLabel,
      git_sha_full: gitShaFull,
      dirty,
      build_time_utc: nowUtc.toISOString(),
      build_time_ist: nowIst.toISOString().replace('Z', '+05:30'),
    },
    null,
    2
  ) + '\n';

  const standalonePublic = path.join(standaloneDir, 'public');
  fs.mkdirSync(standalonePublic, { recursive: true });

  // git-commit.txt — three-location ledger (standalone/, standalone/public/, repo public/)
  writeVerified(path.join(standaloneDir, 'git-commit.txt'), commitLabel);
  writeVerified(path.join(standalonePublic, 'git-commit.txt'), commitLabel);
  writeVerified(path.join('public', 'git-commit.txt'), commitLabel);

  // build-info.json — drives useBuildIdRefresh + humans reading deploy state.
  // Must be regenerated post-build so its git_commit matches the binary the
  // deploy ships. Relying on prebuild alone was observed to fail: the Bono VPS
  // 2026-04-19 incident + James 2026-04-20 01:20 build both shipped a stale
  // build-info.json from a prior build because prebuild hooks were skipped or
  // validate-frontend-env.sh errored out before inject-git-commit.js ran.
  // Writing here guarantees the value matches HEAD for every build that
  // reaches postbuild — which is every build that ships.
  writeVerified(path.join(standaloneDir, 'build-info.json'), buildInfo);
  writeVerified(path.join(standalonePublic, 'build-info.json'), buildInfo);
  writeVerified(path.join('public', 'build-info.json'), buildInfo);

  console.log(`postbuild: wrote git-commit.txt + build-info.json (${commitLabel}) to standalone/, standalone/public/, and repo public/ (readback-verified)`);
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
