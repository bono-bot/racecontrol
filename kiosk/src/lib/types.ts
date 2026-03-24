// ── Shared types from @racingpoint/types ─────────────────────────────────
// Single source of truth — derived from Rust structs in rc-common
import type { GameState } from '@racingpoint/types';
export type {
  SimType,
  PodStatus,
  DrivingState,
  GameState,
  Pod,
  BillingSessionStatus,
  BillingSession,
  PricingTier,
  Driver,
  PodFleetStatus,
  FleetHealthResponse,
} from '@racingpoint/types';

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

// ─── Kiosk-local Pod Types ────────────────────────────────────────────────
// Note: Pod, PodStatus, DrivingState, GameState re-exported from @racingpoint/types above

export type BillingStatus = "pending" | "active" | "paused_manual" | "completed" | "ended_early" | "cancelled";
export type AuthType = "pin" | "qr";
export type AuthTokenStatus = "pending" | "consumed" | "expired" | "cancelled";

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
// BillingSession, BillingSessionStatus, PricingTier re-exported from @racingpoint/types above

export interface RecentSession {
  id: string;
  driver_id: string;
  driver_name: string;
  pod_id: string;
  pod_number: number;
  pricing_tier_name: string;
  allocated_seconds: number;
  driving_seconds: number;
  cost_paise?: number;
  status: BillingStatus;
  started_at?: string;
  ended_at?: string;
}

export interface PendingSplitContinuation {
  pod_id: string;
  driver_id: string;
  driver_name: string;
  split_count: number;
  current_split_number: number;
  split_duration_minutes: number;
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
  diagnostics?: LaunchDiagnostics;
}

export interface LaunchDiagnostics {
  cm_attempted: boolean;
  cm_exit_code?: number;
  cm_log_errors?: string;
  fallback_used: boolean;
  direct_exit_code?: number;
}

// ─── Kiosk Multiplayer ──────────────────────────────────────────────────

export interface KioskMultiplayerAssignment {
  pin: string;
  pod_id: string;
  pod_number: number;
  role: string; // "host" | "invitee"
}

export interface KioskMultiplayerResult {
  group_session_id: string;
  experience_name: string;
  tier_name: string;
  allocated_seconds: number;
  assignments: KioskMultiplayerAssignment[];
}

// ─── AC Server (Dashboard) ──────────────────────────────────────────────────

export interface AcServerInfo {
  session_id: string;
  config: {
    name: string;
    track: string;
    track_config: string;
    cars: string[];
    max_clients: number;
    password: string;
  };
  status: "starting" | "running" | "stopping" | "stopped" | "error";
  pid?: number;
  started_at?: string;
  join_url: string;
  connected_pods: string[];
  error_message?: string;
  continuous_mode: boolean;
}

export interface MultiplayerGroupStatus {
  group_session_id: string;
  ac_session_id: string;
  pod_ids: string[];
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
// Driver re-exported from @racingpoint/types above (includes optional has_used_trial)

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

export type PanelMode = "setup" | "live_session" | "waiting" | "wallet_topup" | "game_picker" | "refund" | null;

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

export type SessionType = "practice" | "hotlap" | "race" | "trackday" | "race_weekend";
export type PlayerMode = "single" | "multi";
export type ExperienceMode = "preset" | "custom";
export type AiDifficulty = "easy" | "medium" | "hard";

// ─── Kiosk Pod Card State ─────────────────────────────────────────────────

export type KioskPodState =
  | "idle"
  | "registering"
  | "waiting"
  | "selecting"
  | "loading"      // game process detected, billing not yet started
  | "on_track"
  | "crashed"
  | "join_failed"
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

// ─── Fleet Health Types ──────────────────────────────────────────────────
// PodFleetStatus, FleetHealthResponse re-exported from @racingpoint/types above

// ─── Cafe Menu ────────────────────────────────────────────────────────────────

export interface CafeMenuItem {
  id: string;
  name: string;
  description: string | null;
  category_id: string;
  category_name: string;
  selling_price_paise: number;
  cost_price_paise: number;
  is_available: boolean;
  is_countable: boolean;
  stock_quantity: number;
  out_of_stock: boolean;
  created_at: string | null;
  updated_at: string | null;
  image_path: string | null;
}

export interface CafeMenuResponse {
  items: CafeMenuItem[];
  total: number;
  page: number;
}

export interface CafeOrderItem {
  item_id: string;
  quantity: number;
}

export interface CafeOrderItemDetail {
  item_id: string;
  name: string;
  quantity: number;
  unit_price_paise: number;
  line_total_paise: number;
}

export interface ActivePromo {
  id: string;
  name: string;
  promo_type: "combo" | "happy_hour" | "gaming_bundle";
  config: Record<string, unknown>;
  stacking_group: string | null;
  time_label: string | null;
}

export interface CafeOrderResponse {
  order_id: string;
  receipt_number: string;
  wallet_txn_id: string;
  total_paise: number;
  discount_paise: number;
  applied_promo_id: string | null;
  applied_promo_name: string | null;
  new_balance_paise: number;
  items: CafeOrderItemDetail[];
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
