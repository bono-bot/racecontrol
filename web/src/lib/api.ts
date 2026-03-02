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
  activeGames: () => fetchApi<{ games: GameLaunchInfo[] }>("/games/active"),
  gameHistory: (pod_id?: string) => {
    const qs = pod_id ? `?pod_id=${pod_id}` : "";
    return fetchApi<{ events: GameLaunchEvent[] }>(`/games/history${qs}`);
  },

  // Auth
  assignCustomer: (data: {
    pod_id: string;
    driver_id: string;
    pricing_tier_id: string;
    auth_type: string;
    custom_price_paise?: number;
    custom_duration_minutes?: number;
  }) =>
    fetchApi<{ token?: AuthTokenInfo; error?: string }>("/auth/assign", {
      method: "POST",
      body: JSON.stringify(data),
    }),
  cancelAssignment: (id: string) =>
    fetchApi<{ status?: string; error?: string }>(`/auth/cancel/${id}`, {
      method: "POST",
    }),
  pendingAuthTokens: () =>
    fetchApi<{ tokens: AuthTokenInfo[] }>("/auth/pending"),
  pendingAuthTokenForPod: (podId: string) =>
    fetchApi<{ token: AuthTokenInfo | null }>(`/auth/pending/${podId}`),

  // AC LAN
  listAcPresets: () => fetchApi<{ presets: AcPresetSummary[] }>("/ac/presets"),
  saveAcPreset: (name: string, config: AcLanSessionConfig) =>
    fetchApi<{ id: string; name: string }>("/ac/presets", {
      method: "POST",
      body: JSON.stringify({ name, config }),
    }),
  getAcPreset: (id: string) =>
    fetchApi<{ id: string; name: string; config: AcLanSessionConfig }>(`/ac/presets/${id}`),
  updateAcPreset: (id: string, data: { name?: string; config?: AcLanSessionConfig }) =>
    fetchApi<{ ok: boolean }>(`/ac/presets/${id}`, { method: "PUT", body: JSON.stringify(data) }),
  deleteAcPreset: (id: string) =>
    fetchApi<{ ok: boolean }>(`/ac/presets/${id}`, { method: "DELETE" }),
  startAcSession: (config: AcLanSessionConfig, pod_ids: string[]) =>
    fetchApi<{ session_id: string }>("/ac/session/start", {
      method: "POST",
      body: JSON.stringify({ config, pod_ids }),
    }),
  stopAcSession: (session_id: string) =>
    fetchApi<{ ok: boolean }>("/ac/session/stop", {
      method: "POST",
      body: JSON.stringify({ session_id }),
    }),
  activeAcSessions: () => fetchApi<{ sessions: AcServerInfo[] }>("/ac/session/active"),
  listAcSessions: (params?: { status?: string; limit?: number }) => {
    const qs = params ? new URLSearchParams(params as Record<string, string>).toString() : "";
    return fetchApi<{ sessions: AcSessionRecord[] }>(`/ac/sessions${qs ? `?${qs}` : ""}`);
  },
  acTracks: () => fetchApi<{ tracks: AcTrack[] }>("/ac/content/tracks"),
  acCars: () => fetchApi<{ cars: AcCar[] }>("/ac/content/cars"),
};

interface GameLaunchEvent {
  id: string;
  pod_id: string;
  sim_type: string;
  event_type: string;
  pid?: number;
  error_message?: string;
  created_at: string;
}

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
  game_state?: GameState;
  current_game?: string;
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

// ─── Auth Token Types ──────────────────────────────────────────────────────

export interface AuthTokenInfo {
  id: string;
  pod_id: string;
  driver_id: string;
  driver_name: string;
  pricing_tier_id: string;
  pricing_tier_name: string;
  auth_type: "pin" | "qr";
  token: string;
  status: "pending" | "consumed" | "expired" | "cancelled";
  allocated_seconds: number;
  custom_price_paise?: number;
  custom_duration_minutes?: number;
  created_at: string;
  expires_at: string;
}

// ─── Game Launcher Types ───────────────────────────────────────────────────

export type GameState = "idle" | "launching" | "running" | "stopping" | "error";

export interface GameLaunchInfo {
  pod_id: string;
  sim_type: string;
  game_state: GameState;
  pid?: number;
  launched_at?: string;
  error_message?: string;
}

export interface AiDebugSuggestion {
  pod_id: string;
  sim_type: string;
  error_context: string;
  suggestion: string;
  model: string;
  created_at: string;
}

// ─── AC LAN Types ─────────────────────────────────────────────────────────

export type AcServerStatus = "starting" | "running" | "stopping" | "stopped" | "error";

export interface AcSessionBlock {
  name: string;
  session_type: "practice" | "qualifying" | "race" | "booking";
  duration_minutes: number;
  laps: number;
  wait_time_secs: number;
}

export interface AcWeatherConfig {
  graphics: string;
  base_temperature_ambient: number;
  base_temperature_road: number;
  variation_ambient: number;
  variation_road: number;
  wind_base_speed_min: number;
  wind_base_speed_max: number;
  wind_base_direction: number;
  wind_variation_direction: number;
}

export interface AcDynamicTrackConfig {
  session_start: number;
  randomness: number;
  session_transfer: number;
  lap_gain: number;
}

export interface AcEntrySlot {
  car_model: string;
  skin: string;
  driver_name: string;
  guid: string;
  ballast: number;
  restrictor: number;
  pod_id?: string;
}

export interface AcLanSessionConfig {
  name: string;
  track: string;
  track_config: string;
  cars: string[];
  max_clients: number;
  password: string;
  sessions: AcSessionBlock[];
  entries: AcEntrySlot[];
  weather: AcWeatherConfig[];
  dynamic_track: AcDynamicTrackConfig;
  pickup_mode: boolean;
  udp_port: number;
  tcp_port: number;
  http_port: number;
  abs_allowed: number;
  tc_allowed: number;
  autoclutch_allowed: boolean;
  tyre_blankets_allowed: boolean;
  stability_allowed: boolean;
  force_virtual_mirror: boolean;
  damage_multiplier: number;
  fuel_rate: number;
  tyre_wear_rate: number;
  min_csp_version: number;
  csp_extra_options?: string;
}

export interface AcServerInfo {
  session_id: string;
  config: AcLanSessionConfig;
  status: AcServerStatus;
  pid?: number;
  started_at?: string;
  join_url: string;
  connected_pods: string[];
  error_message?: string;
}

export interface AcPresetSummary {
  id: string;
  name: string;
  track: string;
  track_config: string;
  cars: string[];
  max_clients: number;
  created_at: string;
  updated_at?: string;
}

export interface AcSessionRecord {
  id: string;
  preset_id?: string;
  status: string;
  pod_ids?: string;
  pid?: number;
  join_url?: string;
  error_message?: string;
  started_at?: string;
  ended_at?: string;
  created_at: string;
}

export interface AcTrack {
  id: string;
  name: string;
  configs: string[];
}

export interface AcCar {
  id: string;
  name: string;
  class: string;
}

export function defaultAcConfig(): AcLanSessionConfig {
  return {
    name: "RacingPoint LAN Race",
    track: "monza",
    track_config: "",
    cars: ["ks_ferrari_488_gt3"],
    max_clients: 16,
    password: "",
    sessions: [
      { name: "Practice", session_type: "practice", duration_minutes: 10, laps: 0, wait_time_secs: 30 },
      { name: "Qualifying", session_type: "qualifying", duration_minutes: 10, laps: 0, wait_time_secs: 60 },
      { name: "Race", session_type: "race", duration_minutes: 0, laps: 10, wait_time_secs: 60 },
    ],
    entries: [],
    weather: [{
      graphics: "3_clear",
      base_temperature_ambient: 26,
      base_temperature_road: 32,
      variation_ambient: 1,
      variation_road: 1,
      wind_base_speed_min: 0,
      wind_base_speed_max: 5,
      wind_base_direction: 0,
      wind_variation_direction: 15,
    }],
    dynamic_track: { session_start: 100, randomness: 0, session_transfer: 100, lap_gain: 0 },
    pickup_mode: true,
    udp_port: 9600,
    tcp_port: 9600,
    http_port: 8081,
    abs_allowed: 1,
    tc_allowed: 1,
    autoclutch_allowed: true,
    tyre_blankets_allowed: true,
    stability_allowed: false,
    force_virtual_mirror: false,
    damage_multiplier: 100,
    fuel_rate: 100,
    tyre_wear_rate: 100,
    min_csp_version: 2144,
  };
}
