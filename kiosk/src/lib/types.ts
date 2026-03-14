// ── Deploy Types ────────────────────────────────────────────────────────────

export type DeployState =
  | { state: 'idle' }
  | { state: 'killing' }
  | { state: 'waiting_dead' }
  | { state: 'downloading'; detail: { progress_pct: number } }
  | { state: 'size_check' }
  | { state: 'starting' }
  | { state: 'verifying_health' }
  | { state: 'complete' }
  | { state: 'failed'; detail: { reason: string } }
  | { state: 'waiting_session' };

export interface DeployPodStatus {
  pod_id: string;
  state: DeployState;
  last_updated: string;
}

export interface DeployProgressEvent {
  pod_id: string;
  state: DeployState;
  message: string;
  timestamp: string;
}

// ─── Pod Types ────────────────────────────────────────────────────────────

export type PodStatus = "offline" | "idle" | "in_session" | "error" | "disabled";
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
  split_count?: number;
  split_duration_minutes?: number;
  current_split_number?: number;
}

export interface PendingSplitContinuation {
  pod_id: string;
  driver_id: string;
  driver_name: string;
  split_count: number;
  current_split_number: number;
  split_duration_minutes: number;
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
  has_used_trial?: boolean;
}

// ─── Wallet ──────────────────────────────────────────────────────────────

export interface WalletInfo {
  driver_id: string;
  balance_paise: number;
  total_credited_paise: number;
  total_debited_paise: number;
  updated_at: string | null;
}

export interface WalletTransaction {
  id: string;
  driver_id: string;
  amount_paise: number;
  balance_after_paise: number;
  txn_type: string;
  reference_id: string | null;
  notes: string | null;
  staff_id: string | null;
  created_at: string;
}

// ─── Billing Warning ──────────────────────────────────────────────────────

export interface BillingWarning {
  sessionId: string;
  podId: string;
  remaining: number;
  timestamp: number;
}

// ─── AC Catalog ──────────────────────────────────────────────────────────

export interface CatalogItem {
  id: string;
  name: string;
  category: string;
  country?: string;
}

export interface PresetEntry {
  id: string;
  name: string;
  tagline: string;
  car_id: string;
  car_name: string;
  track_id: string;
  track_name: string;
  session_type: string;
  difficulty: string;
  category: string;
  duration_hint: string;
  featured: boolean;
}

export interface AcCatalog {
  tracks: {
    featured: CatalogItem[];
    all: CatalogItem[];
  };
  cars: {
    featured: CatalogItem[];
    all: CatalogItem[];
  };
  categories: {
    tracks: string[];
    cars: string[];
  };
  presets?: PresetEntry[];
}

// ─── Side Panel ──────────────────────────────────────────────────────

export type PanelMode = "setup" | "live_session" | "waiting" | "wallet_topup" | null;

export type SetupStep =
  | "register_driver"
  | "select_plan"
  | "select_game"
  | "session_splits"
  | "player_mode"
  | "session_type"
  | "ai_config"
  | "multiplayer_lobby"
  | "select_experience"
  | "select_track"
  | "select_car"
  | "driving_settings"
  | "review";

export type SessionType = "practice" | "qualification" | "race";
export type PlayerMode = "single" | "multi";
export type ExperienceMode = "preset" | "custom";
export type AiDifficulty = "easy" | "medium" | "hard";

// ─── Kiosk Pod Card State ─────────────────────────────────────────────────

export type KioskPodState =
  | "idle"
  | "registering"
  | "waiting"
  | "selecting"
  | "on_track"
  | "ending";

// ─── Pod Activity Log ────────────────────────────────────────────────────

export type ActivityCategory = "system" | "game" | "billing" | "auth" | "race_engineer";
export type ActivitySource = "agent" | "core" | "race_engineer" | "staff";

export interface PodActivityEntry {
  id: string;
  pod_id: string;
  pod_number: number;
  timestamp: string;
  category: ActivityCategory;
  action: string;
  details: string;
  source: ActivitySource;
}

// ─── Debug System ────────────────────────────────────────────────────────

export type DebugHealthColor = "green" | "yellow" | "orange" | "red" | "grey";

export interface PodHealth {
  pod_id: string;
  pod_number: number;
  seconds_since_heartbeat: number;
  health: DebugHealthColor;
  status: string;
}

export interface PlaybookStep {
  step_number: number;
  action: string;
  expected_result: string;
  timeout_seconds: number;
}

export interface DebugPlaybook {
  id: string;
  category: string;
  title: string;
  steps: PlaybookStep[];
}

export interface DebugIncident {
  id: string;
  pod_id?: string;
  category: string;
  description: string;
  status: string;
  playbook_id?: string;
  created_at: string;
}

export interface DebugDiagnosis {
  diagnosis: string;
  model: string;
  incident_id: string;
  playbook?: DebugPlaybook;
  past_resolutions: {
    resolution_text: string;
    effectiveness: number;
    created_at: string;
  }[];
}

export interface DebugActivityData {
  pod_health: PodHealth[];
  billing_events: {
    id: string;
    session_id: string;
    event_type: string;
    created_at: string;
    pod_id?: string;
  }[];
  game_events: {
    id: string;
    pod_id: string;
    event_type: string;
    created_at: string;
    error_message?: string;
  }[];
}
