import type { KioskExperience, KioskSettings, Driver, PricingTier, Pod, BillingSession, WalletInfo, WalletTransaction } from "./types";

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

export async function fetchApi<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}/api/v1${path}`, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  return res.json();
}

export const api = {
  // Health
  health: () => fetchApi<{ status: string; version: string }>("/health"),

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
  }) =>
    fetchApi<{ ok: boolean }>("/billing/start", {
      method: "POST",
      body: JSON.stringify(data),
    }),

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

  // Game Launcher
  launchGame: (pod_id: string, sim_type: string, launch_args?: string) =>
    fetchApi<{ ok: boolean }>("/games/launch", {
      method: "POST",
      body: JSON.stringify({ pod_id, sim_type, launch_args }),
    }),
  stopGame: (pod_id: string) =>
    fetchApi<{ ok: boolean }>("/games/stop", {
      method: "POST",
      body: JSON.stringify({ pod_id }),
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
};
