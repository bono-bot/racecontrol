// ─── Pod Types ────────────────────────────────────────────────────────────

export type PodStatus = "offline" | "idle" | "in_session" | "error";
export type DrivingState = "active" | "idle" | "no_device";
export type GameState = "idle" | "launching" | "running" | "stopping" | "error";
export type BillingStatus = "pending" | "active" | "paused_manual" | "completed" | "ended_early" | "cancelled";
export type AuthType = "pin" | "qr";
export type AuthTokenStatus = "pending" | "consumed" | "expired" | "cancelled";

export interface Pod {
  id: string;
  number: number;
  name: string;
  ip_address: string;
  sim_type: string;
  status: PodStatus;
  current_driver?: string;
  current_session_id?: string;
  last_seen?: string;
  driving_state?: DrivingState;
  billing_session_id?: string;
  game_state?: GameState;
  current_game?: string;
}

// ─── Telemetry ────────────────────────────────────────────────────────────

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

// ─── Laps ─────────────────────────────────────────────────────────────────

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

// ─── Billing ──────────────────────────────────────────────────────────────

export interface BillingSession {
  id: string;
  driver_id: string;
  driver_name: string;
  pod_id: string;
  pricing_tier_name: string;
  allocated_seconds: number;
  driving_seconds: number;
  remaining_seconds: number;
  status: BillingStatus;
  driving_state: DrivingState;
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

// ─── Auth Tokens ──────────────────────────────────────────────────────────

export interface AuthTokenInfo {
  id: string;
  pod_id: string;
  driver_id: string;
  driver_name: string;
  pricing_tier_id: string;
  pricing_tier_name: string;
  auth_type: AuthType;
  token: string;
  status: AuthTokenStatus;
  allocated_seconds: number;
  custom_price_paise?: number;
  custom_duration_minutes?: number;
  created_at: string;
  expires_at: string;
}

// ─── Game Launcher ────────────────────────────────────────────────────────

export interface GameLaunchInfo {
  pod_id: string;
  sim_type: string;
  game_state: GameState;
  pid?: number;
  launched_at?: string;
  error_message?: string;
}

// ─── Kiosk Experiences ────────────────────────────────────────────────────

export interface KioskExperience {
  id: string;
  name: string;
  game: string;
  track: string;
  car: string;
  car_class?: string;
  duration_minutes: number;
  start_type: string;
  ac_preset_id?: string;
  sort_order: number;
  is_active: boolean;
}

// ─── Kiosk Settings ───────────────────────────────────────────────────────

export interface KioskSettings {
  venue_name: string;
  tagline: string;
  business_hours_start: string;
  business_hours_end: string;
  spectator_auto_rotate: string;
  spectator_show_leaderboard: string;
  [key: string]: string;
}

// ─── Driver ───────────────────────────────────────────────────────────────

export interface Driver {
  id: string;
  name: string;
  email?: string;
  phone?: string;
  total_laps: number;
  total_time_ms: number;
}

// ─── Billing Warning ──────────────────────────────────────────────────────

export interface BillingWarning {
  sessionId: string;
  podId: string;
  remaining: number;
  timestamp: number;
}

// ─── Kiosk Pod Card State ─────────────────────────────────────────────────

export type KioskPodState =
  | "idle"
  | "registering"
  | "waiting"
  | "selecting"
  | "on_track"
  | "ending";
