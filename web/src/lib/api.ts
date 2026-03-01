const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

export async function fetchApi<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}/api/v1${path}`, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  return res.json();
}

export const api = {
  health: () => fetchApi<{ status: string; version: string }>("/health"),
  venue: () => fetchApi<{ name: string; location: string; timezone: string; pods: number }>("/venue"),

  // Pods
  listPods: () => fetchApi<{ pods: Pod[] }>("/pods"),
  getPod: (id: string) => fetchApi<{ pod: Pod }>(`/pods/${id}`),

  // Drivers
  listDrivers: () => fetchApi<{ drivers: Driver[] }>("/drivers"),
  createDriver: (data: Partial<Driver>) =>
    fetchApi<{ id: string; name: string }>("/drivers", {
      method: "POST",
      body: JSON.stringify(data),
    }),
  getDriver: (id: string) => fetchApi<Driver>(`/drivers/${id}`),

  // Sessions
  listSessions: () => fetchApi<{ sessions: Session[] }>("/sessions"),
  createSession: (data: Partial<Session>) =>
    fetchApi<{ id: string }>("/sessions", {
      method: "POST",
      body: JSON.stringify(data),
    }),

  // Laps
  listLaps: () => fetchApi<{ laps: Lap[] }>("/laps"),
  sessionLaps: (id: string) => fetchApi<{ laps: Lap[] }>(`/sessions/${id}/laps`),

  // Leaderboard
  trackLeaderboard: (track: string) => fetchApi<{ track: string; records: LeaderboardEntry[] }>(`/leaderboard/${track}`),

  // Events
  listEvents: () => fetchApi<{ events: RaceEvent[] }>("/events"),

  // Bookings
  listBookings: () => fetchApi<{ bookings: Booking[] }>("/bookings"),

  // Pricing
  listPricingTiers: () => fetchApi<{ tiers: PricingTier[] }>("/pricing"),
  createPricingTier: (data: Partial<PricingTier>) =>
    fetchApi<{ id: string }>("/pricing", { method: "POST", body: JSON.stringify(data) }),
  updatePricingTier: (id: string, data: Partial<PricingTier>) =>
    fetchApi<{ ok: boolean }>(`/pricing/${id}`, { method: "PUT", body: JSON.stringify(data) }),
  deletePricingTier: (id: string) =>
    fetchApi<{ ok: boolean }>(`/pricing/${id}`, { method: "DELETE" }),

  // Billing
  startBilling: (data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    custom_price_paise?: number;
    custom_duration_minutes?: number;
  }) => fetchApi<{ ok: boolean }>("/billing/start", { method: "POST", body: JSON.stringify(data) }),
  activeBillingSessions: () => fetchApi<{ sessions: BillingSession[] }>("/billing/active"),
  listBillingSessions: (params?: { date?: string; status?: string }) => {
    const qs = new URLSearchParams(params as Record<string, string>).toString();
    return fetchApi<{ sessions: BillingSessionRecord[] }>(`/billing/sessions${qs ? `?${qs}` : ""}`);
  },
  getBillingSession: (id: string) => fetchApi<BillingSessionRecord>(`/billing/sessions/${id}`),
  billingSessionEvents: (id: string) =>
    fetchApi<{ events: BillingEvent[] }>(`/billing/sessions/${id}/events`),
  stopBilling: (id: string) =>
    fetchApi<{ ok: boolean }>(`/billing/${id}/stop`, { method: "POST" }),
  pauseBilling: (id: string) =>
    fetchApi<{ ok: boolean }>(`/billing/${id}/pause`, { method: "POST" }),
  resumeBilling: (id: string) =>
    fetchApi<{ ok: boolean }>(`/billing/${id}/resume`, { method: "POST" }),
  extendBilling: (id: string, additional_seconds: number) =>
    fetchApi<{ ok: boolean }>(`/billing/${id}/extend`, {
      method: "POST",
      body: JSON.stringify({ additional_seconds }),
    }),
  dailyBillingReport: (date?: string) => {
    const qs = date ? `?date=${date}` : "";
    return fetchApi<DailyReport>(`/billing/report/daily${qs}`);
  },
};

// Types
export interface Pod {
  id: string;
  number: number;
  name: string;
  ip_address: string;
  sim_type: string;
  status: "offline" | "idle" | "in_session" | "error";
  current_driver?: string;
  current_session_id?: string;
  last_seen?: string;
  driving_state?: "active" | "idle" | "no_device";
  billing_session_id?: string;
}

export interface Driver {
  id: string;
  name: string;
  email?: string;
  phone?: string;
  steam_guid?: string;
  total_laps: number;
  total_time_ms: number;
}

export interface Session {
  id: string;
  type: string;
  sim_type: string;
  track: string;
  car_class?: string;
  status: string;
  started_at?: string;
}

export interface Lap {
  id: string;
  driver_id: string;
  track: string;
  car: string;
  lap_number?: number;
  lap_time_ms: number;
  sector1_ms?: number;
  sector2_ms?: number;
  sector3_ms?: number;
  valid: boolean;
}

export interface LeaderboardEntry {
  position: number;
  track: string;
  car: string;
  driver: string;
  best_lap_ms: number;
  achieved_at: string;
}

export interface RaceEvent {
  id: string;
  name: string;
  type: string;
  status: string;
}

export interface Booking {
  id: string;
  driver_id: string;
  start_time: string;
  end_time: string;
  status: string;
}

export interface TelemetryFrame {
  pod_id: string;
  driver_name: string;
  car: string;
  track: string;
  lap_number: number;
  lap_time_ms: number;
  speed_kmh: number;
  throttle: number;
  brake: number;
  gear: number;
  rpm: number;
}

// ─── Billing Types ──────────────────────────────────────────────────────────

export interface BillingSession {
  id: string;
  driver_id: string;
  driver_name: string;
  pod_id: string;
  pricing_tier_name: string;
  allocated_seconds: number;
  driving_seconds: number;
  remaining_seconds: number;
  status: "pending" | "active" | "paused_manual" | "completed" | "ended_early" | "cancelled";
  driving_state: "active" | "idle" | "no_device";
  started_at?: string;
}

export interface PricingTier {
  id: string;
  name: string;
  duration_minutes: number;
  price_paise: number;
  is_trial: boolean;
  is_active: boolean;
  sort_order?: number;
}

export interface BillingEvent {
  id: string;
  event_type: string;
  driving_seconds_at_event: number;
  metadata?: string;
  created_at: string;
}

export interface DailyReport {
  date: string;
  total_sessions: number;
  total_revenue_paise: number;
  total_driving_seconds: number;
  sessions: BillingSessionRecord[];
}

export interface BillingSessionRecord {
  id: string;
  driver_id: string;
  driver_name: string;
  pod_id: string;
  pricing_tier_name: string;
  allocated_seconds: number;
  driving_seconds: number;
  status: string;
  price_paise: number;
  started_at?: string;
  ended_at?: string;
}
