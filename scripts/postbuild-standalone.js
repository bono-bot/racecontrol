#!/usr/bin/env node
// postbuild-standalone.js — Cross-platform Next.js standalone postbuild
// Copies .next/static and public/ into .next/standalone/ for standalone deploys.
// Replaces the bash-only commands in package.json build scripts that fail on Windows cmd.exe.
//
// Usage: node ../scripts/postbuild-standalone.js

const fs = require('fs');
const path = require('path');

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
