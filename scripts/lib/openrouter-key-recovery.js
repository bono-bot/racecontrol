/**
 * OpenRouter API Key Auto-Recovery Module
 *
 * Provisions new child API keys via the management key when a 401 is encountered.
 * Falls back to Bono relay if local management key is unavailable.
 *
 * Usage:
 *   const { recoverKey, checkKeyValid, is401Error, loadSavedKey } = require('./lib/openrouter-key-recovery');
 *   const newKey = await recoverKey();  // tries local mgmt key, then Bono relay
 */

'use strict';

const https = require('https');
const http = require('http');
const fs = require('fs');
const path = require('path');

const KEY_FILE = path.join(__dirname, '..', '..', 'data', 'openrouter-mma-key.txt');

// Serialize concurrent recovery attempts
let _recoveryInFlight = null;

/**
 * Provision a new child key using the OpenRouter management key.
 * @param {string} mgmtKey - The management API key
 * @returns {Promise<string>} The new child API key
 */
function _provisionViaManagementKey(mgmtKey) {
  return new Promise((resolve, reject) => {
    const dateLabel = new Date().toISOString().split('T')[0];
    const body = JSON.stringify({ name: `mma-auto-${dateLabel}-${Date.now().toString(36)}` });

    const req = https.request({
      hostname: 'openrouter.ai',
      path: '/api/v1/keys',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${mgmtKey}`,
      },
    }, (res) => {
      let data = '';
      res.on('data', chunk => { data += chunk; });
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.key) {
            resolve(parsed.key);
          } else {
            reject(new Error(`Key provisioning failed: ${JSON.stringify(parsed)}`));
          }
        } catch (e) {
          reject(new Error(`Key provisioning parse error: ${e.message}`));
        }
      });
    });
    req.on('error', reject);
    req.setTimeout(15000, () => { req.destroy(); reject(new Error('Key provisioning timeout')); });
    req.write(body);
    req.end();
  });
}

/**
 * Provision a key via Bono relay (fallback when James has no management key).
 * Sends exec request to comms-link relay which forwards to Bono.
 * @returns {Promise<string>} The new child API key
 */
function _provisionViaBono() {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      command: 'provision_openrouter_key',
      reason: 'MMA audit 401 recovery — local mgmt key unavailable',
    });

    const req = http.request({
      hostname: 'localhost',
      port: 8766,
      path: '/relay/exec/run',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
    }, (res) => {
      let data = '';
      res.on('data', chunk => { data += chunk; });
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.ok && parsed.result) {
            // Extract key from result output
            const keyMatch = parsed.result.match(/sk-or-v1-[a-f0-9]{64}/);
            if (keyMatch) {
              resolve(keyMatch[0]);
            } else {
              reject(new Error(`Bono provisioning succeeded but no key in output: ${parsed.result.slice(0, 200)}`));
            }
          } else {
            reject(new Error(`Bono relay failed: ${parsed.error || JSON.stringify(parsed)}`));
          }
        } catch (e) {
          reject(new Error(`Bono relay parse error: ${e.message}`));
        }
      });
    });
    req.on('error', (e) => reject(new Error(`Bono relay unreachable: ${e.message}`)));
    req.setTimeout(30000, () => { req.destroy(); reject(new Error('Bono relay timeout (30s)')); });
    req.write(body);
    req.end();
  });
}

/**
 * Save a recovered key to disk for persistence across MMA runs.
 * @param {string} key - The new API key
 */
function _saveKey(key) {
  try {
    fs.mkdirSync(path.dirname(KEY_FILE), { recursive: true });
    fs.writeFileSync(KEY_FILE, key + '\n', { mode: 0o600 });
    console.log(`[key-recovery] New key saved to ${KEY_FILE}`);
  } catch (e) {
    console.error(`[key-recovery] WARNING: could not save key to ${KEY_FILE}: ${e.message}`);
  }
}

/**
 * Load a previously saved key from disk.
 * @returns {string|null} The saved key, or null if not found
 */
function loadSavedKey() {
  try {
    const key = fs.readFileSync(KEY_FILE, 'utf-8').trim();
    if (key && key.startsWith('sk-or-')) return key;
  } catch { /* no file */ }
  return null;
}

/**
 * Check if an API key is valid by hitting the auth endpoint.
 * @param {string} apiKey - The key to check
 * @returns {Promise<{valid: boolean, error?: string, label?: string, remaining?: number}>}
 */
function checkKeyValid(apiKey) {
  return new Promise((resolve) => {
    const req = https.request({
      hostname: 'openrouter.ai',
      path: '/api/v1/auth/key',
      method: 'GET',
      headers: { 'Authorization': `Bearer ${apiKey}` },
    }, (res) => {
      let data = '';
      res.on('data', chunk => { data += chunk; });
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.error) {
            resolve({ valid: false, error: parsed.error.message });
          } else {
            resolve({
              valid: true,
              label: parsed.data?.label,
              remaining: parsed.data?.limit_remaining,
            });
          }
        } catch { resolve({ valid: false, error: 'Parse error' }); }
      });
    });
    req.on('error', (e) => resolve({ valid: false, error: e.message }));
    req.setTimeout(10000, () => { req.destroy(); resolve({ valid: false, error: 'Timeout' }); });
    req.end();
  });
}

/**
 * Recover from a 401 error by provisioning a new child key.
 * Tries local management key first, then falls back to Bono relay.
 * Serializes concurrent recovery attempts.
 *
 * @returns {Promise<string>} The new API key
 * @throws {Error} If both local and Bono provisioning fail
 */
async function recoverKey() {
  // Serialize: if another call is already recovering, wait for it
  if (_recoveryInFlight) {
    return _recoveryInFlight;
  }

  _recoveryInFlight = (async () => {
    // Try 1: Local management key
    const mgmtKey = process.env.OPENROUTER_MGMT_KEY;
    if (mgmtKey) {
      try {
        console.log('[key-recovery] Attempting local provisioning via management key...');
        const newKey = await _provisionViaManagementKey(mgmtKey);
        _saveKey(newKey);
        console.log('[key-recovery] New key provisioned successfully (local).');
        return newKey;
      } catch (e) {
        console.error(`[key-recovery] Local provisioning failed: ${e.message}`);
      }
    } else {
      console.log('[key-recovery] No OPENROUTER_MGMT_KEY set — skipping local provisioning.');
    }

    // Try 2: Bono relay
    try {
      console.log('[key-recovery] Attempting provisioning via Bono relay...');
      const newKey = await _provisionViaBono();
      _saveKey(newKey);
      console.log('[key-recovery] New key provisioned successfully (via Bono).');
      return newKey;
    } catch (e) {
      console.error(`[key-recovery] Bono provisioning failed: ${e.message}`);
    }

    throw new Error(
      'Key recovery failed — both local management key and Bono relay failed. ' +
      'Get a new key manually from openrouter.ai/settings/keys'
    );
  })();

  try {
    return await _recoveryInFlight;
  } finally {
    _recoveryInFlight = null;
  }
}

/**
 * Check if an API error response indicates a 401 (dead key).
 * @param {object} parsedError - The parsed error object from OpenRouter
 * @returns {boolean}
 */
function is401Error(parsedError) {
  if (!parsedError) return false;
  const code = parsedError.code || parsedError.status;
  const msg = parsedError.message || '';
  return code === 401 || code === '401' || msg.includes('401') || msg.includes('Unauthorized') || msg.includes('User not found');
}

module.exports = { recoverKey, checkKeyValid, is401Error, loadSavedKey };
