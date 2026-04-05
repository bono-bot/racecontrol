// ═══════════════════════════════════════════════════════════════
// Pod Screen Capture — capture screenshots from pod displays
// via rc-agent :8090 /exec endpoint
// ═══════════════════════════════════════════════════════════════

import * as http from 'http';
import * as fs from 'fs';
import * as path from 'path';

const EVIDENCE_DIR = path.resolve(__dirname, '..', 'evidence');

// Ensure evidence directory exists
if (!fs.existsSync(EVIDENCE_DIR)) {
  fs.mkdirSync(EVIDENCE_DIR, { recursive: true });
}

// Execute a command on a pod via rc-agent :8090 /exec
async function execOnPod(podIp: string, cmd: string, timeoutMs = 30000): Promise<{ stdout: string; stderr: string; exit_code: number }> {
  return new Promise((resolve, reject) => {
    const data = JSON.stringify({ cmd, timeout_ms: timeoutMs });
    const req = http.request({
      hostname: podIp,
      port: 8090,
      path: '/exec',
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(data) },
      timeout: timeoutMs + 10000,
    }, (res) => {
      let body = '';
      res.on('data', (c) => body += c);
      res.on('end', () => {
        try {
          resolve(JSON.parse(body));
        } catch (e) {
          reject(new Error(`Failed to parse response from pod ${podIp}: ${body}`));
        }
      });
    });
    req.on('error', reject);
    req.on('timeout', () => { req.destroy(); reject(new Error(`Timeout executing on pod ${podIp}`)); });
    req.write(data);
    req.end();
  });
}

// Write a file to a pod via rc-agent :8090 /write
async function writeOnPod(podIp: string, filePath: string, content: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const data = JSON.stringify({ path: filePath, content });
    const req = http.request({
      hostname: podIp,
      port: 8090,
      path: '/write',
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'Content-Length': Buffer.byteLength(data) },
      timeout: 10000,
    }, (res) => {
      let body = '';
      res.on('data', (c) => body += c);
      res.on('end', () => { resolve(); });
    });
    req.on('error', reject);
    req.write(data);
    req.end();
  });
}

// Read a file from a pod (base64 encoded for binary files)
async function readFromPod(podIp: string, filePath: string): Promise<Buffer> {
  const result = await execOnPod(podIp, `powershell -Command "[Convert]::ToBase64String([IO.File]::ReadAllBytes('${filePath}'))"`, 30000);
  if (result.stdout) {
    return Buffer.from(result.stdout.trim(), 'base64');
  }
  throw new Error(`Failed to read ${filePath} from pod ${podIp}`);
}

// ─── Screenshot capture ────────────────────────────────────

// Deploy screenshot PS1 script to a pod (one-time setup per test run)
async function deployScreenshotScript(podIp: string): Promise<void> {
  const script = `
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
param([string]$OutputPath)
$bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
$bmp.Save($OutputPath, [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose()
$bmp.Dispose()
Write-Output "OK"
`.trim();

  await writeOnPod(podIp, 'C:\\RacingPoint\\e2e-screenshot.ps1', script);
}

// Capture a single screenshot from a pod and save locally
export async function capturePodScreenshot(
  podIp: string,
  outputName: string,
): Promise<string> {
  const remoteFile = `C:\\RacingPoint\\e2e-${outputName}.png`;
  const localFile = path.join(EVIDENCE_DIR, `${outputName}.png`);

  // Deploy script if not already there
  await deployScreenshotScript(podIp);

  // Execute screenshot
  const result = await execOnPod(
    podIp,
    `powershell -ExecutionPolicy Bypass -File C:\\RacingPoint\\e2e-screenshot.ps1 -OutputPath "${remoteFile}"`,
    15000,
  );

  if (!result.stdout?.includes('OK')) {
    console.warn(`Screenshot may have failed on ${podIp}: ${result.stderr || result.stdout}`);
  }

  // Download the screenshot
  try {
    const imageData = await readFromPod(podIp, remoteFile);
    fs.writeFileSync(localFile, imageData);
    return localFile;
  } catch (e) {
    console.warn(`Failed to download screenshot from ${podIp}: ${e}`);
    return '';
  }
}

// Start periodic screenshot recording on a pod (every intervalSec seconds)
export async function startPodRecording(
  podIp: string,
  testId: string,
  durationSec = 60,
  intervalSec = 5,
): Promise<void> {
  const frameCount = Math.ceil(durationSec / intervalSec);
  const script = `
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
for ($i = 0; $i -lt ${frameCount}; $i++) {
  try {
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $bmp.Save("C:\\RacingPoint\\e2e-frame-$i.png", [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose()
    $bmp.Dispose()
  } catch { }
  Start-Sleep -Seconds ${intervalSec}
}
`.trim();

  await writeOnPod(podIp, 'C:\\RacingPoint\\e2e-record.ps1', script);

  // Fire and forget — recording runs in background on pod
  execOnPod(
    podIp,
    'powershell -ExecutionPolicy Bypass -File C:\\RacingPoint\\e2e-record.ps1',
    (durationSec + 10) * 1000,
  ).catch(() => { /* recording might timeout — that's OK */ });
}

// Download recorded frames from pod
export async function downloadRecordingFrames(
  podIp: string,
  testId: string,
  maxFrames = 12,
): Promise<string[]> {
  const files: string[] = [];

  for (let i = 0; i < maxFrames; i++) {
    try {
      const remoteFile = `C:\\RacingPoint\\e2e-frame-${i}.png`;
      const localFile = path.join(EVIDENCE_DIR, `${testId}-frame-${i}.png`);
      const data = await readFromPod(podIp, remoteFile);
      fs.writeFileSync(localFile, data);
      files.push(localFile);
    } catch {
      break; // No more frames
    }
  }

  return files;
}

// Clean up recording files on pod
export async function cleanupPodRecording(podIp: string): Promise<void> {
  try {
    await execOnPod(podIp, 'del C:\\RacingPoint\\e2e-frame-*.png C:\\RacingPoint\\e2e-*.png 2>nul', 5000);
  } catch {
    // Ignore cleanup failures
  }
}

// ─── High-level convenience functions ──────────────────────

// Full game launch verification sequence
export async function verifyGameLaunchVisually(
  podIp: string,
  testId: string,
  podNumber: number,
): Promise<{
  beforeLaunch: string;
  gameLaunching: string;
  gameRunning: string;
}> {
  const prefix = `${testId}-pod${podNumber}`;

  // 1. Screenshot before launch (should show lock screen)
  const beforeLaunch = await capturePodScreenshot(podIp, `${prefix}-01-before-launch`);

  // 2. Start recording for the launch sequence
  await startPodRecording(podIp, prefix, 120, 10);

  return {
    beforeLaunch,
    gameLaunching: '', // Will be captured during recording
    gameRunning: '', // Will be captured after playable signal
  };
}

// Capture "game is running" screenshot (call after PlayableSignal)
export async function captureGameRunning(
  podIp: string,
  testId: string,
  podNumber: number,
): Promise<string> {
  const prefix = `${testId}-pod${podNumber}`;
  return await capturePodScreenshot(podIp, `${prefix}-03-game-running`);
}

// Capture "after session end" screenshot (should show lock screen again)
export async function captureAfterEnd(
  podIp: string,
  testId: string,
  podNumber: number,
): Promise<string> {
  const prefix = `${testId}-pod${podNumber}`;
  return await capturePodScreenshot(podIp, `${prefix}-04-after-end`);
}
