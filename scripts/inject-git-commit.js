#!/usr/bin/env node
// inject-git-commit.js — write git SHA + dirty flag + build timestamp into public/ before next build.
//
// Called from each Next.js app's `prebuild` hook:
//   node ../scripts/inject-git-commit.js
//
// Outputs (relative to the app's cwd):
//   public/git-commit.txt   — short SHA with optional "-dirty" suffix (matches racecontrol/build.rs)
//   public/build-info.json  — { git_commit, git_sha_full, dirty, build_time_utc, build_time_ist }
//
// Why public/ and not .next/standalone/: health route reads from public/git-commit.txt FIRST,
// public/ works in dev/standalone/webpack/turbopack uniformly, and pwa has no postbuild step.
//
// Graceful fallback: if git is unavailable, writes "unknown" rather than failing the build.

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Run git from the racecontrol repo root (this script's parent dir), NOT the caller's cwd.
// kiosk/, web/, pwa/ each contain stray .git/ scaffolding dirs with no commits —
// running git from there resolves to the wrong repo and returns "unknown".
const REPO_ROOT = path.resolve(__dirname, '..');

function tryGit(args) {
  try {
    return execSync(`git ${args}`, {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'ignore'],
      cwd: REPO_ROOT,
    }).trim();
  } catch {
    return null;
  }
}

const shortSha = tryGit('rev-parse --short HEAD') || 'unknown';
const fullSha = tryGit('rev-parse HEAD') || 'unknown';

let dirty = false;
if (shortSha !== 'unknown') {
  try {
    execSync('git diff --quiet HEAD --', { stdio: 'ignore', cwd: REPO_ROOT });
  } catch {
    dirty = true;
  }
}

const commit = dirty && shortSha !== 'unknown' ? `${shortSha}-dirty` : shortSha;
const nowUtc = new Date();
const istOffsetMs = (5 * 60 + 30) * 60 * 1000;
const nowIst = new Date(nowUtc.getTime() + istOffsetMs);

const buildInfo = {
  git_commit: commit,
  git_sha_full: fullSha,
  dirty,
  build_time_utc: nowUtc.toISOString(),
  build_time_ist: nowIst.toISOString().replace('Z', '+05:30'),
};

const publicDir = path.join(process.cwd(), 'public');
fs.mkdirSync(publicDir, { recursive: true });

fs.writeFileSync(path.join(publicDir, 'git-commit.txt'), commit + '\n');
fs.writeFileSync(path.join(publicDir, 'build-info.json'), JSON.stringify(buildInfo, null, 2) + '\n');

console.log(`inject-git-commit: wrote public/git-commit.txt = ${commit}`);
