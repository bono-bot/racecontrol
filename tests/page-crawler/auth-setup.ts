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
    // Auth goes to racecontrol API (:8080), not the frontend app
    const rcUrl = process.env.RC_API_URL ?? 'http://192.168.31.23:8080';
    const staffPin = process.env.STAFF_PIN ?? '7080'; // TEST_ONLY_E2E
    const response = await context.request.post(
      `${rcUrl}/api/v1/staff/validate-pin`,
      {
        data: { pin: staffPin },
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

    const url = new URL(baseUrl);
    const port = url.port || (url.protocol === 'https:' ? '443' : '80');
    const isKiosk = port === '3300';

    // Build storageState matching what each frontend actually reads:
    // - Web (:3200) and Admin (:3201): localStorage key "rp_staff_jwt"
    // - Kiosk (:3300): cookie "kiosk_staff_jwt" + localStorage "kiosk_staff_token"
    const storageState: {
      cookies: Array<{
        name: string;
        value: string;
        domain: string;
        path: string;
        expires: number;
        httpOnly: boolean;
        secure: boolean;
        sameSite: 'Lax' | 'Strict' | 'None';
      }>;
      origins: Array<{
        origin: string;
        localStorage: Array<{ name: string; value: string }>;
      }>;
    } = {
      cookies: isKiosk
        ? [
            {
              name: 'kiosk_staff_jwt',
              value: token,
              domain: url.hostname,
              path: '/',
              expires: Math.floor(Date.now() / 1000) + 1800,
              httpOnly: false,
              secure: false,
              sameSite: 'Strict' as const,
            },
          ]
        : [],
      origins: [
        {
          origin: `${url.protocol}//${url.host}`,
          localStorage: isKiosk
            ? [{ name: 'kiosk_staff_token', value: token }]
            : [{ name: 'rp_staff_jwt', value: token }],
        },
      ],
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
