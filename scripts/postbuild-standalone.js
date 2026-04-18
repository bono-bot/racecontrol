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
try {
  const gitCommit = execSync('git rev-parse --short HEAD', { encoding: 'utf8', cwd: REPO_ROOT }).trim();
  fs.writeFileSync(path.join(standaloneDir, 'git-commit.txt'), gitCommit);
  const standalonePublic = path.join(standaloneDir, 'public');
  fs.mkdirSync(standalonePublic, { recursive: true });
  fs.writeFileSync(path.join(standalonePublic, 'git-commit.txt'), gitCommit);
  console.log(`postbuild: wrote git-commit.txt (${gitCommit}) to standalone/ and standalone/public/`);
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
