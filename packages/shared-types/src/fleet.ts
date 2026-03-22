/** Maps to Rust PodFleetStatus struct in crates/racecontrol/src/fleet_health.rs */
export interface PodFleetStatus {
  pod_number: number;
  pod_id?: string;
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
}

export interface FleetHealthResponse {
  pods: PodFleetStatus[];
  timestamp: string;
}
