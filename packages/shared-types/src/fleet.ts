/** Maps to Rust PodFleetStatus struct in crates/racecontrol/src/fleet_health.rs */
export interface PodFleetStatus {
  pod_number: number;
  pod_id?: string;
  name?: string;
  node_type?: string;
  ws_connected: boolean;
  http_reachable: boolean;
  version?: string;
  build_id?: string;
  uptime_secs?: number;
  crash_recovery?: boolean;
  ip_address?: string;
  last_seen?: string;
  last_http_check?: string;
  in_maintenance: boolean;
  maintenance_failures: string[];
  violation_count_24h: number;
  last_violation_at?: string;
  idle_health_fail_count: number;
  idle_health_failures: string[];
  /** Whether freedom mode is active (lock screen dismissed, no restrictions). */
  freedom_mode?: boolean;
  /** Whether the pod screen is blanked (black screen between sessions). */
  screen_blanked?: boolean;
  /** Current game state: "idle", "loading", "running", "error". */
  game_state?: string;
}

export interface FleetHealthResponse {
  pods: PodFleetStatus[];
  timestamp: string;
}

/**
 * Phase 362: Config mismatch detection WS event.
 * Fired when a pod's running game config differs from the kiosk-requested config.
 * Maps to Rust AgentMessage::ConfigMismatchDetected in rc-common/src/protocol.rs.
 */
export interface ConfigMismatchDetected {
  type: 'ConfigMismatchDetected';
  pod_id: string;
  sim_type: string;
  /** Array of [field_name, expected_value, actual_value] tuples */
  mismatches: [string, string, string][];
  timestamp: string;
}
