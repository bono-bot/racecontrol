// ═══════════════════════════════════════════════════════════════
// RC API Client — wraps all API endpoints for E2E test automation
// ═══════════════════════════════════════════════════════════════

import { API_BASE, STAFF_PIN, ADMIN_PIN } from './test-data';
import * as fs from 'fs';
import * as path from 'path';

const TOKEN_FILE = path.join(__dirname, '..', '.e2e-token');

export class RCApiClient {
  private token: string | null = null;

  // ─── Auth ────────────────────────────────────────────────
  // Shared token cache — persisted to file to survive across test suites
  private static cachedToken: string | null = null;

  private static loadPersistedToken(): string | null {
    try {
      if (fs.existsSync(TOKEN_FILE)) {
        const data = JSON.parse(fs.readFileSync(TOKEN_FILE, 'utf8'));
        // Token valid for 12h — check if still fresh (within 11h)
        if (data.token && data.timestamp && Date.now() - data.timestamp < 11 * 3600 * 1000) {
          return data.token;
        }
      }
    } catch { /* ignore */ }
    return null;
  }

  private static persistToken(token: string): void {
    try {
      fs.writeFileSync(TOKEN_FILE, JSON.stringify({ token, timestamp: Date.now() }));
    } catch { /* ignore */ }
  }

  async login(pin: string = STAFF_PIN): Promise<string> {
    // Return in-memory cached token if available
    if (RCApiClient.cachedToken) {
      this.token = RCApiClient.cachedToken;
      return this.token;
    }

    // Try file-persisted token (survives across test files)
    const persisted = RCApiClient.loadPersistedToken();
    if (persisted) {
      this.token = persisted;
      RCApiClient.cachedToken = persisted;
      return persisted;
    }

    // Retry with backoff to handle 429 rate limiting
    for (let attempt = 0; attempt < 5; attempt++) {
      if (attempt > 0) {
        const delay = Math.pow(2, attempt) * 1000;
        await new Promise(r => setTimeout(r, delay));
      }

      const resp = await fetch(`${API_BASE}/staff/validate-pin`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin }),
      });

      if (resp.status === 429) {
        console.log(`  Rate limited (429), retrying in ${Math.pow(2, attempt + 1)}s...`);
        continue;
      }

      if (resp.ok) {
        const data = await resp.json();
        if (data.token) {
          this.token = data.token;
          RCApiClient.cachedToken = data.token;
          RCApiClient.persistToken(data.token);
          return data.token;
        }
      }

      // Fallback to admin-login (uses admin PIN, not the staff PIN)
      const resp2 = await fetch(`${API_BASE}/auth/admin-login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ pin: ADMIN_PIN }),
      });

      if (resp2.status === 429) {
        console.log(`  Rate limited on admin-login (429), retrying...`);
        continue;
      }

      if (resp2.ok) {
        const data2 = await resp2.json();
        this.token = data2.token;
        RCApiClient.cachedToken = data2.token;
        RCApiClient.persistToken(data2.token);
        return data2.token;
      }

      throw new Error(`Login failed: ${resp2.status} ${await resp2.text()}`);
    }

    throw new Error('Login failed after 5 retries (rate limited)');
  }

  private headers(): Record<string, string> {
    const h: Record<string, string> = { 'Content-Type': 'application/json' };
    if (this.token) h['Authorization'] = `Bearer ${this.token}`;
    return h;
  }

  private async get<T>(path: string): Promise<T> {
    for (let attempt = 0; attempt < 3; attempt++) {
      const resp = await fetch(`${API_BASE}${path}`, { headers: this.headers() });
      if (resp.status === 429) {
        await new Promise(r => setTimeout(r, (attempt + 1) * 2000));
        continue;
      }
      if (!resp.ok) throw new Error(`GET ${path} failed: ${resp.status} ${await resp.text()}`);
      return resp.json();
    }
    throw new Error(`GET ${path} failed after retries (rate limited)`);
  }

  private async post<T>(path: string, body?: unknown): Promise<T> {
    for (let attempt = 0; attempt < 3; attempt++) {
      const resp = await fetch(`${API_BASE}${path}`, {
        method: 'POST',
        headers: this.headers(),
        body: body ? JSON.stringify(body) : undefined,
      });
      if (resp.status === 429) {
        await new Promise(r => setTimeout(r, (attempt + 1) * 2000));
        continue;
      }
      if (!resp.ok) throw new Error(`POST ${path} failed: ${resp.status} ${await resp.text()}`);
      return resp.json();
    }
    throw new Error(`POST ${path} failed after retries (rate limited)`);
  }

  // ─── Health ──────────────────────────────────────────────
  async health(): Promise<{ status: string; build_id: string; service: string; version: string; whatsapp: string }> {
    return this.get('/health');
  }

  async fleetHealth(): Promise<FleetPod[]> {
    const data: any = await this.get('/fleet/health');
    // Response is { pods: [...], dashboard_clients, ... }
    return data.pods || data || [];
  }

  // ─── Drivers ─────────────────────────────────────────────
  async registerDriver(data: {
    name: string;
    phone?: string;
    email?: string;
    dob?: string;
  }): Promise<{ id: string }> {
    return this.post('/drivers', data);
  }

  async venueRegister(data: {
    name: string;
    dob: string;
    waiver_consent: boolean;
    guardian_name?: string;
  }): Promise<{ driver_id?: string; id?: string; status?: string; error?: string }> {
    // venue/register is public (no auth), creates driver with waiver_signed=1
    const resp = await fetch(`${API_BASE}/venue/register`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    });
    return resp.json();
  }

  async getDriver(id: string): Promise<Driver> {
    return this.get(`/drivers/${id}`);
  }

  async listDrivers(): Promise<Driver[]> {
    return this.get('/drivers');
  }

  // ─── Wallet ──────────────────────────────────────────────
  async getWallet(driverId: string): Promise<Wallet> {
    const resp = await this.get(`/wallet/${driverId}`);
    // API wraps in { wallet: {...} } — unwrap if present
    return resp.wallet || resp;
  }

  async topupWallet(driverId: string, data: {
    amount_paise: number;
    method: string;
    staff_id?: string;
    notes?: string;
  }): Promise<{ new_balance_paise: number; balance_paise?: number; txn_id?: string; status: string }> {
    return this.post(`/wallet/${driverId}/topup`, data);
  }

  async walletTransactions(driverId: string): Promise<WalletTransaction[]> {
    const resp = await this.get(`/wallet/${driverId}/transactions`);
    return resp.transactions || resp;
  }

  // ─── Billing ─────────────────────────────────────────────
  async startBilling(data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id?: string;
    sim_type?: string;
    track?: string;
    car?: string;
    session_type?: string;
    coupon_code?: string;
    staff_discount_paise?: number;
    discount_reason?: string;
    custom_price_paise?: number;
    idempotency_key?: string;
  }): Promise<BillingStartResponse> {
    // billing/start can hang if pod has a stuck session — use fetch with timeout
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 30000); // 30s max
    try {
      const resp = await fetch(`${API_BASE}/billing/start`, {
        method: 'POST',
        headers: this.headers(),
        body: JSON.stringify(data),
        signal: controller.signal,
      });
      clearTimeout(timeout);
      if (!resp.ok) throw new Error(`POST /billing/start failed: ${resp.status} ${await resp.text()}`);
      const result = await resp.json();
      // Alias billing_session_id as id for convenience
      result.id = result.billing_session_id;
      return result;
    } catch (e: any) {
      clearTimeout(timeout);
      if (e.name === 'AbortError') {
        throw new Error(`billing/start timed out after 30s on ${data.pod_id} — pod may have stuck session`);
      }
      throw e;
    }
  }

  async stopBilling(sessionId: string): Promise<BillingSession> {
    // Use timeout — stopBilling can hang if per-pod lock is held
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 15000);
    try {
      const resp = await fetch(`${API_BASE}/billing/${sessionId}/stop`, {
        method: 'POST',
        headers: this.headers(),
        signal: controller.signal,
      });
      clearTimeout(timeout);
      if (!resp.ok) throw new Error(`POST /billing/${sessionId}/stop failed: ${resp.status} ${await resp.text()}`);
      return resp.json();
    } catch (e: any) {
      clearTimeout(timeout);
      if (e.name === 'AbortError') throw new Error(`stopBilling timed out after 15s`);
      throw e;
    }
  }

  async pauseBilling(sessionId: string, reason?: string): Promise<BillingSession> {
    return this.post(`/billing/${sessionId}/pause`, { reason });
  }

  async resumeBilling(sessionId: string): Promise<BillingSession> {
    return this.post(`/billing/${sessionId}/resume`);
  }

  async extendBilling(sessionId: string, extraMinutes: number): Promise<BillingSession> {
    return this.post(`/billing/${sessionId}/extend`, { extra_minutes: extraMinutes });
  }

  async upgradeBilling(sessionId: string, newTierId: string): Promise<BillingSession> {
    return this.post(`/billing/${sessionId}/upgrade`, { pricing_tier_id: newTierId });
  }

  async refundBilling(sessionId: string, data: {
    method: string;
    amount_paise?: number;
    reason?: string;
    idempotency_key?: string;
  }): Promise<unknown> {
    return this.post(`/billing/${sessionId}/refund`, data);
  }

  async getBillingSession(id: string): Promise<BillingSession> {
    return this.get(`/billing/sessions/${id}`);
  }

  async billingSessionEvents(id: string): Promise<BillingEvent[]> {
    return this.get(`/billing/sessions/${id}/events`);
  }

  async billingSessionSummary(id: string): Promise<unknown> {
    return this.get(`/billing/sessions/${id}/summary`);
  }

  async activeBillingSessions(): Promise<BillingSession[]> {
    const resp = await this.get('/billing/active');
    return resp.sessions || resp;
  }

  async listBillingSessions(): Promise<BillingSession[]> {
    const resp = await this.get('/billing/sessions');
    return resp.sessions || resp;
  }

  async applyBillingDiscount(sessionId: string, data: {
    discount_paise: number;
    reason: string;
    approval_code?: string;
  }): Promise<unknown> {
    return this.post(`/billing/${sessionId}/discount`, data);
  }

  // ─── Games ───────────────────────────────────────────────
  async launchGame(data: {
    pod_id: string;
    sim_type: string;
    track?: string;
    car?: string;
    launch_args?: {
      track?: string;
      car?: string;
      session_type?: string;
      ai_level?: number;
      ai_count?: number;
      ai_cars?: unknown[];
    };
  }): Promise<{ ok: boolean; verified?: boolean }> {
    // launch_args must be a JSON STRING, not nested object
    const args = data.launch_args || {};
    if (data.track && !args.track) args.track = data.track;
    if (data.car && !args.car) args.car = data.car;
    const payload: Record<string, unknown> = {
      pod_id: data.pod_id,
      sim_type: data.sim_type,
    };
    if (Object.keys(args).length > 0) {
      payload.launch_args = JSON.stringify(args);
    }
    // Use timeout — game launch can hang if pod is unresponsive
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 30000);
    try {
      const resp = await fetch(`${API_BASE}/games/launch`, {
        method: 'POST',
        headers: this.headers(),
        body: JSON.stringify(payload),
        signal: controller.signal,
      });
      clearTimeout(timeout);
      if (!resp.ok) throw new Error(`POST /games/launch failed: ${resp.status} ${await resp.text()}`);
      return resp.json();
    } catch (e: any) {
      clearTimeout(timeout);
      if (e.name === 'AbortError') throw new Error(`games/launch timed out after 30s on ${data.pod_id}`);
      throw e;
    }
  }

  async stopGame(data: { pod_id: string }): Promise<{ ok: boolean }> {
    return this.post('/games/stop', data);
  }

  async podGameState(podId: string): Promise<PodGameState> {
    return this.get(`/games/pod/${podId}`);
  }

  async activeGames(): Promise<unknown[]> {
    return this.get('/games/active');
  }

  async gamesCatalog(): Promise<unknown> {
    return this.get('/games/catalog');
  }

  // ─── Pricing ─────────────────────────────────────────────
  async listPricingTiers(): Promise<PricingTier[]> {
    const data: any = await this.get('/pricing');
    return data.tiers || data || [];
  }

  async createPricingTier(data: {
    name: string;
    duration_minutes: number;
    price_paise: number;
    is_trial?: boolean;
    is_active?: boolean;
  }): Promise<PricingTier> {
    return this.post('/pricing', data);
  }

  async pricingDisplay(): Promise<unknown> {
    return this.get('/pricing/display');
  }

  // ─── Coupons ─────────────────────────────────────────────
  async createCoupon(data: {
    code: string;
    coupon_type: string;
    value: number;
    max_uses?: number;
    valid_from?: string;
    valid_until?: string;
    min_spend_paise?: number;
    first_session_only?: boolean;
    is_active?: boolean;
  }): Promise<{ id: string }> {
    return this.post('/coupons', data);
  }

  // ─── Pods ────────────────────────────────────────────────
  async listPods(): Promise<Pod[]> {
    return this.get('/pods');
  }

  async getPod(id: string): Promise<Pod> {
    return this.get(`/pods/${id}`);
  }

  async podInventory(podId: string): Promise<PodInventory> {
    return this.get(`/fleet/pod-inventory/${podId}`);
  }

  // Control blanking screen on a pod
  // blank=false → dismiss blanking (show game/desktop)
  // blank=true → restore blanking screen
  async setPodScreen(podId: string, mode: 'game' | 'blank'): Promise<{ ok: boolean }> {
    return this.post(`/pods/${podId}/screen`, { blank: mode === 'blank' });
  }

  // ─── Presets ─────────────────────────────────────────────
  async listPresets(): Promise<GamePreset[]> {
    return this.get('/presets');
  }

  // ─── Kiosk Experiences ───────────────────────────────────
  async listKioskExperiences(): Promise<KioskExperience[]> {
    return this.get('/kiosk/experiences');
  }

  async launchKioskExperience(data: {
    pod_id: string;
    experience_id?: string;
    driver_id: string;
  }): Promise<unknown> {
    return this.post('/kiosk/pod-launch-experience', data);
  }

  // ─── Multiplayer ─────────────────────────────────────────
  async bookMultiplayer(data: {
    host_driver_id: string;
    friend_ids: string[];
    experience_id?: string;
    pricing_tier_id?: string;
    sim_type?: string;
    track?: string;
    car?: string;
  }): Promise<unknown> {
    return this.post('/kiosk/book-multiplayer', data);
  }

  // ─── Utility: wait for billing status ────────────────────
  async waitForBillingStatus(
    sessionId: string,
    targetStatuses: string[],
    timeoutMs = 120_000,
    pollMs = 2000,
  ): Promise<BillingSession> {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      const session = await this.getBillingSession(sessionId);
      if (targetStatuses.includes(session.status)) return session;
      await new Promise(r => setTimeout(r, pollMs));
    }
    throw new Error(`Billing session ${sessionId} did not reach ${targetStatuses} within ${timeoutMs}ms`);
  }
}

// ─── Types ─────────────────────────────────────────────────
export interface FleetPod {
  pod_number: number;
  ws_connected: boolean;
  http_reachable: boolean;
  version: string;
  build_id: string;
  uptime_secs: number;
  last_seen: string;
  ip?: string;
  billing_session_id?: string;
  game_state?: string;
}

export interface Driver {
  id: string;
  name: string;
  phone: string;
  email?: string;
  dob?: string;
  has_used_trial: boolean;
  status: string;
  linked_to?: string;
}

export interface Wallet {
  driver_id: string;
  balance_paise: number;
  total_credited_paise: number;
  total_debited_paise: number;
}

export interface WalletTransaction {
  id: string;
  driver_id: string;
  amount_paise: number;
  txn_type: string;
  reference_id?: string;
  notes?: string;
  created_at: string;
}

export interface BillingStartResponse {
  ok: boolean;
  id: string; // alias for billing_session_id
  billing_session_id: string;
  wallet_debit_paise: number;
  original_price_paise: number;
  discount_paise: number;
  discount_reason?: string;
  allocated_seconds: number;
  nonce: string;
}

export interface BillingSession {
  id: string;
  driver_id: string;
  pod_id: string;
  status: string;
  pricing_tier_id?: string;
  allocated_seconds: number;
  driving_seconds: number;
  wallet_debit_paise: number;
  wallet_owner_id?: string;
  discount_paise?: number;
  started_at?: string;
  ended_at?: string;
  end_reason?: string;
  recovery_pause_seconds?: number;
  custom_price_paise?: number;
}

export interface BillingEvent {
  id: string;
  billing_session_id: string;
  event_type: string;
  driving_seconds_at_event: number;
  created_at: string;
}

export interface PodGameState {
  pod_id: string;
  game_state: string;
  sim_type?: string;
  session_id?: string;
}

export interface PricingTier {
  id: string;
  name: string;
  duration_minutes: number;
  price_paise: number;
  is_trial: boolean;
  is_active: boolean;
}

export interface Pod {
  id: string;
  pod_number: number;
  status: string;
  ip?: string;
}

export interface PodInventory {
  pod_id: string;
  installed_games: string[];
}

export interface GamePreset {
  id: string;
  name: string;
  game: string;
  car?: string;
  track?: string;
  session_type?: string;
  enabled: boolean;
  reliability_score?: number;
}

export interface KioskExperience {
  id: string;
  name: string;
  game: string;
  track?: string;
  car?: string;
  pricing_tier_id: string;
  is_active: boolean;
}
