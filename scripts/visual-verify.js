#!/usr/bin/env node
// visual-verify.js — Programmatic screenshot capture + visual analysis for CGP G1 proof
//
// Usage:
//   node scripts/visual-verify.js                    # All 8 pods, capture + analyze
//   node scripts/visual-verify.js --pods 1,3,5       # Specific pods
//   node scripts/visual-verify.js --compare <dir>    # Compare against previous capture
//   node scripts/visual-verify.js --json              # JSON output for scripting
//
// Evidence is saved to: scripts/visual-evidence/<timestamp>/
//   pod1.png, pod2.png, ..., analysis.json, summary.txt
//
// Exit codes: 0 = all pass, 1 = issues found, 2 = pods unreachable

const http = require('http');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Pod network map (from CLAUDE.md)
const POD_MAP = {
  1: '192.168.31.89',
  2: '192.168.31.33',
  3: '192.168.31.28',
  4: '192.168.31.88',
  5: '192.168.31.86',
  6: '192.168.31.87',
  7: '192.168.31.38',
  8: '192.168.31.91',
};

const DEBUG_PORT = 18924;
const SCREENSHOT_TIMEOUT = 15000; // 15s — PowerShell CopyFromScreen can be slow

// --- PNG minimal parser (extract dimensions + sample pixels without sharp) ---

function parsePngHeader(buf) {
  // PNG signature: 89 50 4E 47 0D 0A 1A 0A
  if (buf.length < 24 || buf[0] !== 0x89 || buf[1] !== 0x50) {
    return null;
  }
  // IHDR chunk starts at byte 8, data at 16
  const width = buf.readUInt32BE(16);
  const height = buf.readUInt32BE(20);
  const bitDepth = buf[24];
  const colorType = buf[25];
  return { width, height, bitDepth, colorType };
}

// Sample pixel colors from a PNG by decoding via PowerShell (Windows-only, fast)
// Uses a temp .ps1 file to avoid shell escaping issues with $() interpolation
// Returns array of {x, y, r, g, b} samples
async function samplePixels(pngPath, points) {
  const escapedPath = pngPath.replace(/\\/g, '\\\\').replace(/'/g, "''");
  const psScript = [
    'Add-Type -AssemblyName System.Drawing',
    `$img = [System.Drawing.Image]::FromFile('${pngPath.replace(/'/g, "''")}')`,
    '$bmp = New-Object System.Drawing.Bitmap($img)',
    ...points.map(
      (p) =>
        `try { $c = $bmp.GetPixel(${p.x}, ${p.y}); Write-Output "${p.x},${p.y},$($c.R),$($c.G),$($c.B)" } catch { Write-Output "${p.x},${p.y},ERR" }`
    ),
    '$bmp.Dispose()',
    '$img.Dispose()',
  ].join('\r\n');

  const tmpPs1 = path.join(path.dirname(pngPath), '_pixel-sample.ps1');
  try {
    fs.writeFileSync(tmpPs1, psScript);
    const out = execSync(
      `powershell -NoProfile -ExecutionPolicy Bypass -File "${tmpPs1}"`,
      { timeout: 15000, encoding: 'utf-8' }
    );
    return out
      .trim()
      .split('\n')
      .map((line) => {
        const parts = line.trim().split(',');
        if (parts.length < 5 || parts[2] === 'ERR') return null;
        return {
          x: parseInt(parts[0]),
          y: parseInt(parts[1]),
          r: parseInt(parts[2]),
          g: parseInt(parts[3]),
          b: parseInt(parts[4]),
        };
      })
      .filter(Boolean);
  } catch {
    return [];
  } finally {
    try { fs.unlinkSync(tmpPs1); } catch {}
  }
}

// Analyze pixel samples to determine screen state
function analyzeScreenState(samples, width, height) {
  if (samples.length === 0) return { state: 'unknown', confidence: 0, reason: 'no pixel data' };

  const totalBrightness = samples.reduce((s, p) => s + (p.r + p.g + p.b) / 3, 0);
  const avgBrightness = totalBrightness / samples.length;

  // Count dark pixels (blanking screen uses #1A1A1A = RGB 26,26,26 — threshold at 40)
  const darkPixels = samples.filter((p) => p.r < 40 && p.g < 40 && p.b < 40).length;
  const darkRatio = darkPixels / samples.length;

  // Count very bright pixels (desktop/white backgrounds)
  const brightPixels = samples.filter((p) => p.r > 200 && p.g > 200 && p.b > 200).length;
  const brightRatio = brightPixels / samples.length;

  // Check for Racing Red (#E10600) — logo/branding indicator
  const redPixels = samples.filter((p) => p.r > 180 && p.g < 30 && p.b < 30).length;
  const hasRacingRed = redPixels > 0;

  // Desktop blue (Windows default) — taskbar or desktop background
  const bluePixels = samples.filter((p) => p.b > 150 && p.r < 80 && p.g < 80).length;

  // Determine state
  if (darkRatio > 0.7) {
    if (hasRacingRed) {
      return {
        state: 'blanking_with_logo',
        confidence: 0.95,
        reason: `${Math.round(darkRatio * 100)}% dark pixels + Racing Red detected`,
      };
    }
    return {
      state: 'blanking_dark',
      confidence: 0.85,
      reason: `${Math.round(darkRatio * 100)}% dark pixels, avg brightness ${Math.round(avgBrightness)}`,
    };
  }

  if (brightRatio > 0.5) {
    return {
      state: 'desktop_or_app',
      confidence: 0.8,
      reason: `${Math.round(brightRatio * 100)}% bright pixels — likely desktop or app window`,
    };
  }

  if (bluePixels > samples.length * 0.1) {
    return {
      state: 'desktop_visible',
      confidence: 0.7,
      reason: `Blue pixels detected (${bluePixels}) — possible Windows desktop/taskbar`,
    };
  }

  // Mixed colors — likely a game or complex UI
  return {
    state: 'game_or_ui',
    confidence: 0.6,
    reason: `Mixed colors, avg brightness ${Math.round(avgBrightness)} — likely game running or UI active`,
  };
}

// --- HTTP helpers ---

function fetchScreenshot(ip) {
  return new Promise((resolve, reject) => {
    const req = http.get(
      { hostname: ip, port: DEBUG_PORT, path: '/screenshot', timeout: SCREENSHOT_TIMEOUT },
      (res) => {
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode}`));
          return;
        }
        const chunks = [];
        res.on('data', (c) => chunks.push(c));
        res.on('end', () => resolve(Buffer.concat(chunks)));
      }
    );
    req.on('error', reject);
    req.on('timeout', () => {
      req.destroy();
      reject(new Error('timeout'));
    });
  });
}

function fetchDebugStatus(ip) {
  return new Promise((resolve, reject) => {
    const req = http.get(
      { hostname: ip, port: DEBUG_PORT, path: '/status', timeout: 5000 },
      (res) => {
        let body = '';
        res.on('data', (c) => (body += c));
        res.on('end', () => {
          try {
            resolve(JSON.parse(body));
          } catch {
            reject(new Error('invalid JSON'));
          }
        });
      }
    );
    req.on('error', reject);
    req.on('timeout', () => {
      req.destroy();
      reject(new Error('timeout'));
    });
  });
}

// --- Image comparison (pixel diff between two PNGs) ---

async function compareScreenshots(currentPath, previousPath) {
  const currentInfo = parsePngHeader(fs.readFileSync(currentPath));
  const previousInfo = parsePngHeader(fs.readFileSync(previousPath));

  if (!currentInfo || !previousInfo) return { match: false, reason: 'invalid PNG' };
  if (currentInfo.width !== previousInfo.width || currentInfo.height !== previousInfo.height) {
    return {
      match: false,
      reason: `resolution changed: ${previousInfo.width}x${previousInfo.height} → ${currentInfo.width}x${currentInfo.height}`,
    };
  }

  // Sample grid of points from both images
  const w = currentInfo.width;
  const h = currentInfo.height;
  const gridPoints = [];
  for (let gx = 0; gx < 8; gx++) {
    for (let gy = 0; gy < 6; gy++) {
      gridPoints.push({
        x: Math.floor((gx + 0.5) * (w / 8)),
        y: Math.floor((gy + 0.5) * (h / 6)),
      });
    }
  }

  const [currentSamples, previousSamples] = await Promise.all([
    samplePixels(currentPath, gridPoints),
    samplePixels(previousPath, gridPoints),
  ]);

  if (currentSamples.length === 0 || previousSamples.length === 0) {
    return { match: false, reason: 'could not sample pixels' };
  }

  // Compare matching sample points
  let totalDiff = 0;
  let compared = 0;
  for (let i = 0; i < Math.min(currentSamples.length, previousSamples.length); i++) {
    const c = currentSamples[i];
    const p = previousSamples[i];
    if (c && p) {
      totalDiff += Math.abs(c.r - p.r) + Math.abs(c.g - p.g) + Math.abs(c.b - p.b);
      compared++;
    }
  }

  const avgDiff = compared > 0 ? totalDiff / compared / 3 : 255; // per-channel avg
  const similarity = Math.max(0, 100 - (avgDiff / 255) * 100);

  return {
    match: similarity > 90,
    similarity: Math.round(similarity * 10) / 10,
    avgPixelDiff: Math.round(avgDiff * 10) / 10,
    samplesCompared: compared,
    reason: similarity > 90 ? 'screens match' : `${Math.round(100 - similarity)}% pixel difference detected`,
  };
}

// --- Main ---

async function verifyPod(podNum, ip, evidenceDir) {
  const result = {
    pod: podNum,
    ip,
    timestamp: new Date().toISOString(),
    screenshot: null,
    debugStatus: null,
    analysis: null,
    verdict: 'UNKNOWN',
    error: null,
  };

  // Fetch debug status first (fast, validates connectivity)
  try {
    result.debugStatus = await fetchDebugStatus(ip);
  } catch (e) {
    result.error = `debug endpoint unreachable: ${e.message}`;
    result.verdict = 'UNREACHABLE';
    return result;
  }

  // Capture screenshot
  const screenshotPath = path.join(evidenceDir, `pod${podNum}.png`);
  try {
    const png = await fetchScreenshot(ip);
    fs.writeFileSync(screenshotPath, png);
    result.screenshot = screenshotPath;

    const header = parsePngHeader(png);
    if (!header) {
      result.error = 'invalid PNG data received';
      result.verdict = 'FAIL';
      return result;
    }

    result.resolution = `${header.width}x${header.height}`;

    // Sample pixels at strategic locations:
    // - 4 corners (detect desktop/taskbar)
    // - center (detect logo/game)
    // - center-left, center-right (detect blanking coverage on triple monitors)
    const w = header.width;
    const h = header.height;
    const samplePoints = [
      { x: 10, y: 10, label: 'top-left' },
      { x: w - 11, y: 10, label: 'top-right' },
      { x: 10, y: h - 11, label: 'bottom-left' },
      { x: w - 11, y: h - 11, label: 'bottom-right' },
      { x: Math.floor(w / 2), y: Math.floor(h / 2), label: 'center' },
      { x: Math.floor(w / 6), y: Math.floor(h / 2), label: 'left-monitor-center' },
      { x: Math.floor((5 * w) / 6), y: Math.floor(h / 2), label: 'right-monitor-center' },
      { x: Math.floor(w / 2), y: h - 5, label: 'bottom-center-taskbar' },
      // Grid fill for better coverage
      { x: Math.floor(w / 4), y: Math.floor(h / 4), label: 'q1' },
      { x: Math.floor((3 * w) / 4), y: Math.floor(h / 4), label: 'q2' },
      { x: Math.floor(w / 4), y: Math.floor((3 * h) / 4), label: 'q3' },
      { x: Math.floor((3 * w) / 4), y: Math.floor((3 * h) / 4), label: 'q4' },
    ];

    const samples = await samplePixels(screenshotPath, samplePoints);
    result.pixelSamples = samples;
    result.analysis = analyzeScreenState(samples, w, h);

    // Cross-reference with debug status
    const lockState = result.debugStatus?.lock_screen_state;
    const edgeCount = result.debugStatus?.edge_process_count ?? 0;

    if (lockState === 'screen_blanked' && edgeCount > 0 && result.analysis.state.startsWith('blanking')) {
      result.verdict = 'PASS';
      result.verdictReason = `Blanking confirmed: ${lockState}, edge=${edgeCount}, visual=${result.analysis.state}`;
    } else if (lockState === 'screen_blanked' && edgeCount === 0) {
      result.verdict = 'FAIL';
      result.verdictReason = `BROKEN BLANKING: state=${lockState} but edge_process_count=0`;
    } else if (lockState === 'hidden' && result.analysis.state === 'game_or_ui') {
      result.verdict = 'PASS';
      result.verdictReason = `Game/UI active: lock=${lockState}, visual=${result.analysis.state}`;
    } else if (result.analysis.state === 'desktop_visible' || result.analysis.state === 'desktop_or_app') {
      result.verdict = 'WARN';
      result.verdictReason = `Desktop/app visible: lock=${lockState}, visual=${result.analysis.state}`;
    } else {
      result.verdict = 'INFO';
      result.verdictReason = `lock=${lockState}, edge=${edgeCount}, visual=${result.analysis.state} (${result.analysis.confidence})`;
    }
  } catch (e) {
    result.error = `screenshot failed: ${e.message}`;
    result.verdict = 'FAIL';
  }

  return result;
}

async function main() {
  const args = process.argv.slice(2);
  const jsonMode = args.includes('--json');
  const compareDir = args.includes('--compare') ? args[args.indexOf('--compare') + 1] : null;

  // Parse --pods flag
  let podNums = [1, 2, 3, 4, 5, 6, 7, 8];
  if (args.includes('--pods')) {
    const podArg = args[args.indexOf('--pods') + 1];
    podNums = podArg.split(',').map(Number).filter((n) => n >= 1 && n <= 8);
  }

  // Create evidence directory
  const now = new Date();
  const pad = (n) => String(n).padStart(2, '0');
  // Compute IST manually (UTC+5:30) — Git Bash TZ= silently fails
  const istMs = now.getTime() + (5 * 60 + 30) * 60 * 1000;
  const ist = new Date(istMs);
  const timestamp = `${ist.getUTCFullYear()}-${pad(ist.getUTCMonth() + 1)}-${pad(ist.getUTCDate())}_${pad(ist.getUTCHours())}${pad(ist.getUTCMinutes())}${pad(ist.getUTCSeconds())}`;

  const evidenceDir = path.join(__dirname, 'visual-evidence', timestamp);
  fs.mkdirSync(evidenceDir, { recursive: true });

  if (!jsonMode) {
    console.log(`=== VISUAL VERIFICATION (${timestamp} IST) ===`);
    console.log(`Evidence dir: ${evidenceDir}`);
    console.log(`Pods: ${podNums.join(', ')}\n`);
  }

  // Capture all pods in parallel
  const results = await Promise.all(
    podNums.map((num) => verifyPod(num, POD_MAP[num], evidenceDir))
  );

  // Print results
  let passCount = 0;
  let failCount = 0;
  let warnCount = 0;

  for (const r of results) {
    if (r.verdict === 'PASS') passCount++;
    else if (r.verdict === 'FAIL' || r.verdict === 'UNREACHABLE') failCount++;
    else if (r.verdict === 'WARN') warnCount++;

    if (!jsonMode) {
      const icon = r.verdict === 'PASS' ? '[PASS]' : r.verdict === 'FAIL' ? '[FAIL]' : r.verdict === 'UNREACHABLE' ? '[FAIL]' : r.verdict === 'WARN' ? '[WARN]' : '[INFO]';
      console.log(`${icon} Pod ${r.pod} (${r.ip})`);
      if (r.resolution) console.log(`      Resolution: ${r.resolution}`);
      if (r.verdictReason) console.log(`      ${r.verdictReason}`);
      if (r.error) console.log(`      Error: ${r.error}`);
      if (r.screenshot) console.log(`      Screenshot: ${r.screenshot}`);
      console.log('');
    }
  }

  // Compare with previous capture if requested
  if (compareDir && fs.existsSync(compareDir)) {
    if (!jsonMode) console.log(`\n=== COMPARISON vs ${compareDir} ===\n`);
    for (const r of results) {
      if (!r.screenshot) continue;
      const prevFile = path.join(compareDir, `pod${r.pod}.png`);
      if (!fs.existsSync(prevFile)) {
        if (!jsonMode) console.log(`[SKIP] Pod ${r.pod}: no previous screenshot`);
        continue;
      }
      const cmp = await compareScreenshots(r.screenshot, prevFile);
      r.comparison = cmp;
      if (!jsonMode) {
        const icon = cmp.match ? '[SAME]' : '[DIFF]';
        console.log(`${icon} Pod ${r.pod}: ${cmp.reason} (similarity: ${cmp.similarity}%)`);
      }
    }
  }

  // Save analysis JSON
  const analysisPath = path.join(evidenceDir, 'analysis.json');
  fs.writeFileSync(analysisPath, JSON.stringify(results, null, 2));

  // Save human-readable summary
  const summary = [
    `Visual Verification — ${timestamp} IST`,
    `PASS: ${passCount}  FAIL: ${failCount}  WARN: ${warnCount}`,
    '',
    ...results.map((r) => `Pod ${r.pod}: ${r.verdict} — ${r.verdictReason || r.error || 'no data'}`),
    '',
    `Evidence: ${evidenceDir}`,
  ].join('\n');
  fs.writeFileSync(path.join(evidenceDir, 'summary.txt'), summary);

  if (jsonMode) {
    console.log(JSON.stringify({ timestamp, evidenceDir, passCount, failCount, warnCount, results }, null, 2));
  } else {
    console.log(`\n=== SUMMARY: PASS=${passCount} FAIL=${failCount} WARN=${warnCount} ===`);
    console.log(`Evidence saved: ${evidenceDir}`);
    if (failCount > 0) {
      console.log('STATUS: FAIL — visual issues detected');
    } else if (warnCount > 0) {
      console.log('STATUS: WARN — review screenshots manually');
    } else {
      console.log('STATUS: PASS — visual verification complete');
    }
  }

  // Symlink latest for easy access
  const latestLink = path.join(__dirname, 'visual-evidence', 'latest');
  try {
    if (fs.existsSync(latestLink)) fs.rmSync(latestLink, { recursive: true });
    // On Windows, use junction instead of symlink (no admin required)
    fs.symlinkSync(evidenceDir, latestLink, 'junction');
  } catch {
    // Symlink may fail on some Windows configs — not critical
  }

  process.exit(failCount > 0 ? 1 : 0);
}

main().catch((e) => {
  console.error('Fatal:', e.message);
  process.exit(2);
});
