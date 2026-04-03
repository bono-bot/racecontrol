/**
 * Typed API client for metrics endpoints.
 * Maps to Rust handlers in crates/racecontrol/src/api/metrics.rs
 * and crates/racecontrol/src/api/games.rs (alternatives).
 *
 * Uses the same API_BASE + fetch pattern as web/src/lib/api.ts.
 */

const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

// ─── Local type declarations matching packages/shared-types/src/metrics.ts ──
// (Declared locally to avoid adding a build-time dependency on @racingpoint/types
// in the web package; kept in sync via check-billing-status-parity.js)

export interface FailureMode {
  mode: string;
  count: number;
}

export interface LaunchStatsResponse {
  success_rate: number;
  avg_time_to_track_ms: number | null;
  p95_time_to_track_ms: number | null;
  total_launches: number;
  common_failure_modes: FailureMode[];
  last_30d_trend: string;
}

export interface BillingAccuracyResponse {
  avg_delta_ms: number | null;
  max_delta_ms: number | null;
  sessions_with_zero_delta: number;
  sessions_where_billing_never_started: number;
  false_playable_signals: number;
}

export interface AlternativeCombo {
  car: string | null;
  track: string | null;
  success_rate: number;
  avg_time_ms: number | null;
  total_launches: number;
}

export interface LaunchMatrixRow {
  pod_id: string;
  total_launches: number;
  success_rate: number;
  avg_time_ms: number | null;
  top_3_failure_modes: FailureMode[];
  flagged: boolean;
}

// ─── API Client Functions ─────────────────────────────────────────────────────

async function metricsGet<T>(path: string): Promise<T> {
  const res = await fetch(`${API_BASE}/api/v1${path}`, {
    headers: { "Content-Type": "application/json" },
  });
  if (!res.ok) {
    throw new Error(`Metrics API error ${res.status}: ${path}`);
  }
  return res.json() as Promise<T>;
}

/**
 * GET /api/v1/metrics/launch-stats
 * Returns overall launch success rate and failure mode breakdown.
 * Optional filters: game (sim_type) and pod (pod_id).
 */
export function getLaunchStats(params?: {
  game?: string;
  pod?: string;
}): Promise<LaunchStatsResponse> {
  const qs = params
    ? new URLSearchParams(
        Object.fromEntries(
          Object.entries(params).filter(([, v]) => v !== undefined && v !== "")
        ) as Record<string, string>
      ).toString()
    : "";
  return metricsGet<LaunchStatsResponse>(
    `/metrics/launch-stats${qs ? `?${qs}` : ""}`
  );
}

/**
 * GET /api/v1/metrics/billing-accuracy
 * Returns billing delta accuracy statistics.
 */
export function getBillingAccuracy(): Promise<BillingAccuracyResponse> {
  return metricsGet<BillingAccuracyResponse>("/metrics/billing-accuracy");
}

/**
 * GET /api/v1/games/alternatives?game=&car=&track=&pod=
 * Returns alternative car/track combos with high success rates.
 */
export function getAlternatives(params: {
  game: string;
  car: string;
  track: string;
  pod: string;
}): Promise<AlternativeCombo[]> {
  const qs = new URLSearchParams(params as Record<string, string>).toString();
  return metricsGet<AlternativeCombo[]>(`/games/alternatives?${qs}`);
}

/**
 * GET /api/v1/admin/launch-matrix?game={game}
 * Returns per-pod launch reliability matrix for the given game.
 */
export function getLaunchMatrix(game: string): Promise<LaunchMatrixRow[]> {
  const qs = new URLSearchParams({ game }).toString();
  return metricsGet<LaunchMatrixRow[]>(`/admin/launch-matrix?${qs}`);
}

// ─── Fleet Game Matrix (DASH-01) ──────────────────────────────────────────────

export interface GameMatrixPodEntry {
  installed: boolean;
  launchable: boolean;
  scanned_at: string;
}

export interface GameMatrixGame {
  game_id: string;
  display_name: string;
  sim_type: string;
  pods: Record<string, GameMatrixPodEntry>;
}

export interface GameMatrixResponse {
  games: GameMatrixGame[];
}

/**
 * GET /api/v1/fleet/game-matrix
 * Returns which games are installed on which pods.
 */
export function getGameMatrix(): Promise<GameMatrixResponse> {
  return metricsGet<GameMatrixResponse>("/fleet/game-matrix");
}

// ─── Combo Reliability List (DASH-02) ─────────────────────────────────────────

export interface ComboListRow {
  pod_id: string;
  sim_type: string;
  car: string | null;
  track: string | null;
  success_rate: number;
  avg_time_ms: number | null;
  total_launches: number;
  flagged: boolean;
}

/**
 * GET /api/v1/admin/combo-list?game=&sort_by=&order=
 * Returns per-combo reliability rows, sortable, flagged if success_rate < 70%.
 */
export function getComboList(params?: {
  game?: string;
  sort_by?: string;
  order?: string;
}): Promise<ComboListRow[]> {
  const qs = params
    ? new URLSearchParams(
        Object.fromEntries(
          Object.entries(params).filter(([, v]) => v !== undefined && v !== "")
        ) as Record<string, string>
      ).toString()
    : "";
  return metricsGet<ComboListRow[]>(`/admin/combo-list${qs ? `?${qs}` : ""}`);
}
