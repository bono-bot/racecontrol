import type { BillingSessionStatus } from './billing';
import type { GameState } from './pod';

/** v22.0: Server -> Agent flag sync payload (CoreToAgentMessage::FlagSync) */
export interface FlagSyncPayload {
  flags: Record<string, boolean>;
  version: number;
}

/** v22.0: Server -> Agent config push payload (CoreToAgentMessage::ConfigPush) */
export interface WsConfigPushPayload {
  fields: Record<string, unknown>;
  schema_version: number;
  sequence: number;
}

/** v22.0: Server -> Agent OTA download command (CoreToAgentMessage::OtaDownload) */
export interface OtaDownloadPayload {
  manifest_url: string;
  binary_sha256: string;
  version: string;
}

/** v22.0: Server -> Agent kill switch (CoreToAgentMessage::KillSwitch) */
export interface KillSwitchPayload {
  flag_name: string;
  active: boolean;
  reason?: string;
}

/** v22.0: Agent -> Server config ack (AgentMessage::ConfigAck) */
export interface ConfigAckPayload {
  pod_id: string;
  sequence: number;
  accepted: boolean;
}

/** v22.0: Agent -> Server OTA ack (AgentMessage::OtaAck) */
export interface OtaAckPayload {
  pod_id: string;
  version: string;
  success: boolean;
  error?: string;
}

/** v22.0: Agent -> Server flag cache sync request (AgentMessage::FlagCacheSync) */
export interface FlagCacheSyncPayload {
  pod_id: string;
  cached_version: number;
}

/**
 * Structured diagnostics from a game launch attempt.
 * Maps to Rust LaunchDiagnostics in crates/rc-common/src/types.rs
 */
export interface LaunchDiagnostics {
  /** Whether Content Manager was attempted (multiplayer only) */
  cm_attempted: boolean;
  /** CM process exit code (null if CM wasn't used or is still running) */
  cm_exit_code?: number | null;
  /** CM log error excerpts (null if no errors found) */
  cm_log_errors?: string | null;
  /** Whether direct acs.exe fallback was used after CM failure */
  fallback_used: boolean;
  /** acs.exe exit code if it exited immediately (null if still running) */
  direct_exit_code?: number | null;
}

/**
 * Billing timer tick — sent every 1s for active/waiting billing sessions.
 * Maps to DashboardEvent::BillingTick(BillingSessionInfo) in protocol.rs
 */
export interface BillingTick {
  /** Pod this session is on */
  pod_id: string;
  /** Unique billing session identifier */
  session_id: string;
  /** Current billing session status */
  status: BillingSessionStatus;
  /** Remaining seconds in the session */
  remaining_seconds: number;
  /** Elapsed driving seconds (count-up model) */
  elapsed_seconds: number;
}

/**
 * Game state changed on a pod.
 * Maps to DashboardEvent::GameStateChanged(GameLaunchInfo) in protocol.rs
 */
export interface GameStateChanged {
  /** Pod this game state change is for */
  pod_id: string;
  /** New game state */
  game_state: GameState;
  /** Game name/sim_type if available */
  game_name?: string;
  /** Error message if game state is "error" */
  error_message?: string | null;
  /** Process exit code if game exited abnormally */
  exit_code?: number | null;
  /** Structured diagnostics from the launch attempt */
  diagnostics?: LaunchDiagnostics | null;
}
