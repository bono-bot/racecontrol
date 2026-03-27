import type { KioskExperience, KioskSettings, Driver, PricingTier, Pod, BillingSession, WalletInfo, WalletTransaction, AcCatalog, DebugActivityData, DebugPlaybook, DebugIncident, DebugDiagnosis, PodActivityEntry, FleetHealthResponse, KioskMultiplayerResult, CafeMenuResponse, CafeOrderItem, CafeOrderResponse, ActivePromo, RecentSession, VenueShutdownResponse } from "./types";
import type { RedeemPinResponse, AlternativeCombo } from "@racingpoint/types";

export type { ActivePromo, RedeemPinResponse };

const API_BASE =
  process.env.NEXT_PUBLIC_API_URL ||
  (typeof window !== "undefined"
    ? `${window.location.protocol}//${window.location.host}`
    : "http://localhost:8080");

const MAX_RETRIES = 2;
const RETRY_BASE_MS = 500;

export async function fetchApi<T>(path: string, options?: RequestInit): Promise<T> {
  const token = typeof window !== "undefined" ? sessionStorage.getItem("kiosk_staff_token") : null;
  const headers: Record<string, string> = { "Content-Type": "application/json" };
  if (token) headers["Authorization"] = `Bearer ${token}`;

  let lastError: Error | null = null;

  for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 30_000);
    try {
      const res = await fetch(`${API_BASE}/api/v1${path}`, {
        headers,
        signal: controller.signal,
        ...options,
      });
      if (!res.ok) {
        const text = await res.text().catch(() => "");
        throw new Error(`API ${res.status}: ${path} — ${text.slice(0, 200)}`);
      }
      return res.json();
    } catch (err) {
      lastError = err instanceof Error ? err : new Error(String(err));
      // Only retry on network errors (TypeError from fetch / AbortError from timeout), not HTTP errors (4xx/5xx)
      const isNetworkError = err instanceof TypeError || (err instanceof DOMException && err.name === 'AbortError');
      if (!isNetworkError || attempt >= MAX_RETRIES) break;
      // Exponential backoff: 500ms, 1000ms
      await new Promise((r) => setTimeout(r, RETRY_BASE_MS * Math.pow(2, attempt)));
    } finally {
      clearTimeout(timeoutId);
    }
  }

  throw lastError!;
}

export const api = {
  // Health
  health: () => fetchApi<{ status: string; version: string }>("/health"),
  fleetHealth: () => fetchApi<FleetHealthResponse>("/fleet/health"),

  // Pods
  listPods: () => fetchApi<{ pods: Pod[] }>("/pods"),

  // Drivers
  listDrivers: () => fetchApi<{ drivers: Driver[] }>("/drivers"),
  createDriver: (data: Partial<Driver>) =>
    fetchApi<{ id: string; name: string }>("/drivers", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  // Pricing
  listPricingTiers: () => fetchApi<{ tiers: PricingTier[] }>("/pricing"),

  // Billing
  activeBillingSessions: () => fetchApi<{ sessions: BillingSession[] }>("/billing/active"),
  startBilling: (data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    staff_id?: string;
    split_count?: number;
    split_duration_minutes?: number;
  }) =>
    fetchApi<{ ok?: boolean; error?: string; billing_session_id?: string }>("/billing/start", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  getSplitOptions: (durationMinutes: number) =>
    fetchApi<{ duration_minutes: number; options: { count: number; duration_minutes: number; label: string }[] }>(
      `/billing/split-options/${durationMinutes}`
    ),

  continueSplit: (data: { pod_id: string; sim_type: string; launch_args: string }) =>
    fetchApi<{ ok?: boolean; error?: string; billing_session_id?: string; current_split_number?: number; total_splits?: number }>(
      "/billing/continue-split",
      { method: "POST", body: JSON.stringify(data) }
    ),

  refundSession: (billingSessionId: string, data: {
    amount_paise: number;
    method: "wallet" | "cash" | "upi";
    reason: string;
  }) =>
    fetchApi<{ ok?: boolean; error?: string; refund_id?: string }>(
      `/billing/${billingSessionId}/refund`,
      { method: "POST", body: JSON.stringify(data) }
    ),

  recentSessions: (limit = 10) =>
    fetchApi<{ sessions: RecentSession[] }>(
      `/billing/sessions?limit=${limit}&status=completed`
    ),

  // Auth
  assignCustomer: (data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    auth_type: string;
    custom_price_paise?: number;
    custom_duration_minutes?: number;
  }) =>
    fetchApi<{ token?: unknown; error?: string }>("/auth/assign", {
      method: "POST",
      body: JSON.stringify(data),
    }),
  cancelAssignment: (id: string) =>
    fetchApi<{ status?: string; error?: string }>(`/auth/cancel/${id}`, {
      method: "POST",
    }),
  startNow: (tokenId: string) =>
    fetchApi<{ status?: string; billing_session_id?: string; error?: string }>("/auth/start-now", {
      method: "POST",
      body: JSON.stringify({ token_id: tokenId }),
    }),

  // Game Catalog (authoritative source — replaces hardcoded GAMES list)
  gamesCatalog: () =>
    fetchApi<{ games: { id: string; name: string; abbr: string; installed_pod_count: number }[] }>("/games/catalog"),

  // Game Launcher
  launchGame: (pod_id: string, sim_type: string, launch_args?: string) =>
    fetchApi<{ ok: boolean }>("/games/launch", {
      method: "POST",
      body: JSON.stringify({ pod_id, sim_type, launch_args }),
    }),
  relaunchGame: (pod_id: string) =>
    fetchApi<{ ok: boolean }>(`/games/relaunch/${pod_id}`, {
      method: "POST",
    }),
  retryPodJoin: (session_id: string, pod_id: string) =>
    fetchApi<{ status: string }>("/ac/session/retry-pod", {
      method: "POST",
      body: JSON.stringify({ session_id, pod_id }),
    }),
  updateAcSessionConfig: (session_id: string, config: { track?: string; track_config?: string; cars?: string[] }) =>
    fetchApi<{ status: string }>("/ac/session/update-config", {
      method: "POST",
      body: JSON.stringify({ session_id, ...config }),
    }),
  setAcContinuousMode: (session_id: string, enabled: boolean) =>
    fetchApi<{ status: string }>(`/ac/session/${session_id}/continuous`, {
      method: "POST",
      body: JSON.stringify({ enabled }),
    }),
  stopGame: (pod_id: string) =>
    fetchApi<{ ok: boolean }>("/games/stop", {
      method: "POST",
      body: JSON.stringify({ pod_id }),
    }),
  setTransmission: (pod_id: string, transmission: string) =>
    fetchApi<{ ok: boolean; transmission: string }>(`/pods/${pod_id}/transmission`, {
      method: "POST",
      body: JSON.stringify({ transmission }),
    }),
  setFfb: (pod_id: string, preset: string) =>
    fetchApi<{ ok: boolean; preset: string }>(`/pods/${pod_id}/ffb`, {
      method: "POST",
      body: JSON.stringify({ preset }),
    }),

  // Kiosk Experiences
  listExperiences: () => fetchApi<{ experiences: KioskExperience[] }>("/kiosk/experiences"),
  createExperience: (data: Partial<KioskExperience>) =>
    fetchApi<{ id: string; name: string }>("/kiosk/experiences", {
      method: "POST",
      body: JSON.stringify(data),
    }),
  getExperience: (id: string) => fetchApi<KioskExperience>(`/kiosk/experiences/${id}`),
  updateExperience: (id: string, data: Partial<KioskExperience>) =>
    fetchApi<{ ok: boolean }>(`/kiosk/experiences/${id}`, {
      method: "PUT",
      body: JSON.stringify(data),
    }),
  deleteExperience: (id: string) =>
    fetchApi<{ ok: boolean }>(`/kiosk/experiences/${id}`, { method: "DELETE" }),

  // AC Catalog
  getAcCatalog: () => fetchApi<AcCatalog>("/customer/ac/catalog"),

  // Kiosk PIN Validation
  validateKioskPin: (pin: string, pod_id?: string) =>
    fetchApi<{
      status?: string;
      error?: string;
      billing_session_id?: string;
      pod_id?: string;
      pod_number?: number;
      driver_name?: string;
      pricing_tier_name?: string;
      allocated_seconds?: number;
    }>("/auth/kiosk/validate-pin", {
      method: "POST",
      body: JSON.stringify({ pin, pod_id }),
    }),

  // Staff Auth
  validateStaffPin: (pin: string) =>
    fetchApi<{
      status?: string;
      error?: string;
      staff_id?: string;
      staff_name?: string;
      token?: string;
    }>("/staff/validate-pin", {
      method: "POST",
      body: JSON.stringify({ pin }),
    }),

  // Kiosk Settings
  getSettings: () => fetchApi<{ settings: KioskSettings }>("/kiosk/settings"),
  updateSettings: (data: Partial<KioskSettings>) =>
    fetchApi<{ ok: boolean; updated: number }>("/kiosk/settings", {
      method: "PUT",
      body: JSON.stringify(data),
    }),

  // Wallet (staff-facing)
  getWallet: (driverId: string) =>
    fetchApi<{ wallet: WalletInfo | null }>(`/wallet/${driverId}`),
  topupWallet: (
    driverId: string,
    amount_paise: number,
    method: string,
    notes?: string
  ) =>
    fetchApi<{ status?: string; new_balance_paise?: number; error?: string }>(
      `/wallet/${driverId}/topup`,
      {
        method: "POST",
        body: JSON.stringify({ amount_paise, method, notes }),
      }
    ),
  walletTransactions: (driverId: string, limit = 20) =>
    fetchApi<{ transactions: WalletTransaction[] }>(
      `/wallet/${driverId}/transactions?limit=${limit}`
    ),
  refundWallet: (
    driverId: string,
    amount_paise: number,
    notes?: string,
    reference_id?: string
  ) =>
    fetchApi<{ status?: string; new_balance_paise?: number; error?: string }>(
      `/wallet/${driverId}/refund`,
      {
        method: "POST",
        body: JSON.stringify({ amount_paise, notes, reference_id }),
      }
    ),

  // Pod Activity Log
  globalActivity: (limit = 100) =>
    fetchApi<PodActivityEntry[]>(`/activity?limit=${limit}`),
  podActivity: (podId: string, limit = 100) =>
    fetchApi<PodActivityEntry[]>(`/pods/${podId}/activity?limit=${limit}`),

  // Server Logs
  serverLogs: (lines = 200, level?: string) =>
    fetchApi<{ lines: string[]; file: string | null; total: number; filtered: number }>(
      `/logs?lines=${lines}${level ? `&level=${level}` : ""}`
    ),

  // Debug System
  debugActivity: (hours?: number) =>
    fetchApi<DebugActivityData>(`/debug/activity${hours ? `?hours=${hours}` : ""}`),

  debugPlaybooks: () =>
    fetchApi<{ playbooks: DebugPlaybook[] }>("/debug/playbooks"),

  createDebugIncident: (description: string, pod_id?: string) =>
    fetchApi<{ incident: DebugIncident; playbook?: DebugPlaybook }>("/debug/incidents", {
      method: "POST",
      body: JSON.stringify({ description, pod_id }),
    }),

  listDebugIncidents: (status?: string) =>
    fetchApi<{ incidents: DebugIncident[] }>(`/debug/incidents${status ? `?status=${status}` : ""}`),

  diagnoseIncident: (incident_id: string) =>
    fetchApi<DebugDiagnosis>("/debug/diagnose", {
      method: "POST",
      body: JSON.stringify({ incident_id }),
    }),

  resolveDebugIncident: (id: string, status: string, resolution_text?: string, effectiveness?: number) =>
    fetchApi<{ ok: boolean }>(`/debug/incidents/${id}`, {
      method: "PUT",
      body: JSON.stringify({ status, resolution_text, effectiveness }),
    }),

  applyDebugFix: (incidentId: string, action: string, podId?: string) =>
    fetchApi<{ ok: boolean; action?: string; output?: string; error?: string }>(
      `/debug/incidents/${incidentId}/apply-fix`,
      {
        method: "POST",
        body: JSON.stringify({ action, pod_id: podId }),
      }
    ),

  // Customer Self-Service (phone auth + booking)
  customerLogin: (phone: string) =>
    fetchApi<{ status?: string; error?: string }>("/customer/login", {
      method: "POST",
      body: JSON.stringify({ phone }),
    }),
  customerVerifyOtp: (phone: string, otp: string) =>
    fetchApi<{ token?: string; driver_id?: string; driver_name?: string; error?: string }>(
      "/customer/verify-otp",
      {
        method: "POST",
        body: JSON.stringify({ phone, otp }),
      }
    ),
  customerBook: async (
    token: string,
    data: {
      pricing_tier_id: string;
      experience_id?: string;
      custom?: Record<string, unknown>;
    }
  ): Promise<{
    pin?: string;
    pod_number?: number;
    allocated_seconds?: number;
    error?: string;
  }> => {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 30_000);
    try {
      const res = await fetch(`${API_BASE}/api/v1/customer/book`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify(data),
        signal: controller.signal,
      });
      if (!res.ok) {
        const text = await res.text().catch(() => "");
        return { error: `API ${res.status}: ${text.slice(0, 200)}` };
      }
      return res.json();
    } finally {
      clearTimeout(timeoutId);
    }
  },

  // Kiosk Multiplayer Booking
  kioskBookMultiplayer: async (
    token: string,
    data: {
      pricing_tier_id: string;
      pod_count: number;
      experience_id?: string;
      custom?: Record<string, unknown>;
    }
  ): Promise<KioskMultiplayerResult & { error?: string }> => {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 30_000);
    try {
      const res = await fetch(`${API_BASE}/api/v1/kiosk/book-multiplayer`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify(data),
        signal: controller.signal,
      });
      if (!res.ok) {
        const text = await res.text().catch(() => "");
        return { error: `API ${res.status}: ${text.slice(0, 200)}` } as KioskMultiplayerResult & { error: string };
      }
      return res.json();
    } finally {
      clearTimeout(timeoutId);
    }
  },

  // Pod Screen Blanking
  setBlankScreen: (pod_id: string, blank: boolean) =>
    fetchApi<{ ok: boolean; pod_id: string; blank: boolean }>(`/pods/${pod_id}/screen`, {
      method: "POST",
      body: JSON.stringify({ blank }),
    }),

  // Pod Unrestrict (employee training/maintenance — disables kiosk enforcement + clears lock screen)
  unrestrictPod: (pod_id: string, unrestrict: boolean) =>
    fetchApi<{ ok: boolean; pod_id: string; unrestricted: boolean }>(`/pods/${pod_id}/unrestrict`, {
      method: "POST",
      body: JSON.stringify({ unrestrict }),
    }),

  // Pod Freedom Mode (all restrictions lifted, but passive monitoring stays active)
  setFreedomMode: (pod_id: string, enabled: boolean) =>
    fetchApi<{ ok: boolean; pod_id: string; freedom_mode: boolean }>(`/pods/${pod_id}/freedom`, {
      method: "POST",
      body: JSON.stringify({ enabled }),
    }),

  // Pod Power Management
  wakePod: (id: string) =>
    fetchApi<{ status: string; pod_id: string }>(`/pods/${id}/wake`, { method: "POST" }),
  shutdownPod: (id: string) =>
    fetchApi<{ status: string; pod_id: string }>(`/pods/${id}/shutdown`, { method: "POST" }),
  restartPod: (id: string) =>
    fetchApi<{ status: string; pod_id: string }>(`/pods/${id}/restart`, { method: "POST" }),
  wakeAllPods: () =>
    fetchApi<{ status: string; results: unknown[] }>("/pods/wake-all", { method: "POST" }),
  shutdownAllPods: () =>
    fetchApi<{ status: string; results: unknown[] }>("/pods/shutdown-all", { method: "POST" }),
  restartAllPods: () =>
    fetchApi<{ status: string; results: unknown[] }>("/pods/restart-all", { method: "POST" }),
  lockdownPod: (id: string, locked: boolean) =>
    fetchApi<{ ok: boolean; pod_id: string; locked: boolean }>(
      `/pods/${id}/lockdown`,
      { method: "POST", body: JSON.stringify({ locked }) }
    ),
  lockdownAllPods: (locked: boolean) =>
    fetchApi<{ ok: boolean; results: unknown[] }>(
      "/pods/lockdown-all",
      { method: "POST", body: JSON.stringify({ locked }) }
    ),

  // Pod Enable/Disable
  enablePod: (id: string) =>
    fetchApi<{ ok: boolean }>(`/pods/${id}/enable`, { method: "POST" }),
  disablePod: (id: string) =>
    fetchApi<{ ok: boolean }>(`/pods/${id}/disable`, { method: "POST" }),

  // Venue Shutdown (150s timeout — audit can take up to 120s)
  venueShutdown: () => {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), 150_000);
    return fetchApi<VenueShutdownResponse>("/venue/shutdown", {
      method: "POST",
      signal: controller.signal,
    }).finally(() => clearTimeout(timeoutId));
  },

  // Maintenance
  clearMaintenance: (podId: string) =>
    fetchApi<{ ok: boolean; pod_id: string }>(`/pods/${podId}/clear-maintenance`, { method: "POST" }),

  // Kiosk Pod Launch Experience
  podLaunchExperience: (pod_id: string, experience_id: string) =>
    fetchApi<{ ok: boolean; billing_session_id?: string }>("/kiosk/pod-launch-experience", {
      method: "POST",
      body: JSON.stringify({ pod_id, experience_id }),
    }),

  // Kiosk Allowlist
  listKioskAllowlist: () =>
    fetchApi<{ allowlist: { id: string; process_name: string; added_by: string; notes: string | null; created_at: string }[]; hardcoded_count: number }>("/config/kiosk-allowlist"),

  addKioskAllowlistEntry: (processName: string, notes?: string) =>
    fetchApi<{ id?: string; process_name?: string; status?: string; message?: string }>("/config/kiosk-allowlist", {
      method: "POST",
      body: JSON.stringify({ process_name: processName, notes }),
    }),

  deleteKioskAllowlistEntry: (processName: string) =>
    fetchApi<void>(`/config/kiosk-allowlist/${encodeURIComponent(processName)}`, {
      method: "DELETE",
    }),

  // Cafe Menu (public, no auth required)
  publicCafeMenu: () => fetchApi<CafeMenuResponse>("/cafe/menu"),
  publicCafePromos: () => fetchApi<ActivePromo[]>("/cafe/promos/active"),

  // Cafe Orders (staff auth required)
  placeCafeOrder: (driverId: string, items: CafeOrderItem[]) =>
    fetchApi<CafeOrderResponse | { error: string }>("/cafe/orders", {
      method: "POST",
      body: JSON.stringify({ driver_id: driverId, items }),
    }),

  // Kiosk PIN Redemption (remote booking flow)
  redeemPin: (pin: string) =>
    fetchApi<RedeemPinResponse>("/kiosk/redeem-pin", {
      method: "POST",
      body: JSON.stringify({ pin }),
    }),

  // Combo reliability — fetch alternatives when success_rate < 70%
  getAlternatives: (params: { game: string; car: string; track: string; pod_id: string }) =>
    fetchApi<{ combo_success_rate: number; alternatives: AlternativeCombo[] }>(
      `/games/alternatives?game=${encodeURIComponent(params.game)}&car=${encodeURIComponent(params.car)}&track=${encodeURIComponent(params.track)}&pod_id=${encodeURIComponent(params.pod_id)}`
    ),
};
