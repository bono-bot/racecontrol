/**
 * Shared auth helper — reads the JWT acquired by global-setup.ts.
 *
 * Usage in any test file:
 *   import { getSharedJwt } from '../shared-auth';
 *   const { token } = getSharedJwt();
 *
 * This replaces per-file validate-pin calls. One JWT for all tests.
 */
import fs from 'fs';
import path from 'path';

const AUTH_STATE_PATH = path.join(__dirname, '.auth-state.json');

interface AuthState {
  token: string;
  staffId: string;
  staffName: string;
  role: string;
}

export function getSharedJwt(): AuthState {
  try {
    return JSON.parse(fs.readFileSync(AUTH_STATE_PATH, 'utf-8'));
  } catch {
    return { token: '', staffId: '', staffName: '', role: '' };
  }
}
