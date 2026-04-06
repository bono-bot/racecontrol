/**
 * Auth setup module for the page crawler.
 *
 * Generates and caches Playwright storageState files via staff PIN
 * authentication. Sessions are reused for 1 hour to avoid
 * re-authenticating on every crawl run.
 */

import { chromium } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const AUTH_DIR = path.join(__dirname, '.auth');
const SESSION_TTL_MS = 60 * 60 * 1000; // 1 hour

/**
 * Derive a filesystem-safe key from a base URL.
 * e.g. "http://192.168.31.23:3200" -> "192.168.31.23-3200"
 */
function storageKey(baseUrl: string): string {
  const url = new URL(baseUrl);
  const host = url.hostname.replace(/\./g, '.');
  const port = url.port || (url.protocol === 'https:' ? '443' : '80');
  return `${host}-${port}`;
}

/**
 * Ensure a valid authenticated storageState file exists for the given
 * base URL. Returns the absolute path to the JSON file.
 *
 * If a cached file exists and is less than 1 hour old it is reused.
 * Otherwise a fresh session is created by POSTing to the staff PIN
 * validation endpoint.
 */
export async function ensureAuth(baseUrl: string): Promise<string> {
  // Create .auth directory if missing
  if (!fs.existsSync(AUTH_DIR)) {
    fs.mkdirSync(AUTH_DIR, { recursive: true });
  }

  const key = storageKey(baseUrl);
  const filePath = path.join(AUTH_DIR, `${key}.json`);

  // Reuse cached session if fresh enough
  if (fs.existsSync(filePath)) {
    const stat = fs.statSync(filePath);
    const ageMs = Date.now() - stat.mtimeMs;
    if (ageMs < SESSION_TTL_MS) {
      return filePath;
    }
  }

  // Authenticate via staff PIN
  const browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();

  try {
    const response = await context.request.post(
      `${baseUrl}/api/auth/validate-pin`,
      {
        data: { pin: '1234' },
        headers: { 'Content-Type': 'application/json' },
      },
    );

    if (!response.ok()) {
      throw new Error(
        `PIN auth failed: ${response.status()} ${response.statusText()}`,
      );
    }

    const body = await response.json();
    const token: string = body.token ?? body.jwt ?? body.access_token ?? '';

    if (!token) {
      throw new Error(
        'PIN auth response did not contain a token field (tried: token, jwt, access_token)',
      );
    }

    // Parse the URL to derive cookie domain/properties
    const url = new URL(baseUrl);

    // Build a storageState JSON with the JWT as a cookie
    const storageState = {
      cookies: [
        {
          name: 'token',
          value: token,
          domain: url.hostname,
          path: '/',
          expires: -1, // session cookie
          httpOnly: true,
          secure: false,
          sameSite: 'Lax' as const,
        },
      ],
      origins: [],
    };

    fs.writeFileSync(filePath, JSON.stringify(storageState, null, 2));
    return filePath;
  } finally {
    await browser.close();
  }
}

/**
 * Ensure .gitignore contains an entry for the auth cache directory.
 */
export function ensureGitignore(repoRoot: string): void {
  const gitignorePath = path.join(repoRoot, '.gitignore');
  const entry = 'tests/page-crawler/.auth/';

  if (!fs.existsSync(gitignorePath)) {
    fs.writeFileSync(gitignorePath, `${entry}\n`);
    return;
  }

  const content = fs.readFileSync(gitignorePath, 'utf-8');
  if (!content.includes(entry)) {
    const nl = content.endsWith('\n') ? '' : '\n';
    fs.appendFileSync(gitignorePath, `${nl}${entry}\n`);
  }
}
