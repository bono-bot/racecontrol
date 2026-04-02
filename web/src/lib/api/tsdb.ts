/**
 * TSDB metrics API client — Phase 291 (DASH-01)
 *
 * Calls Phase 286 REST endpoints:
 *   GET /api/v1/metrics/names
 *   GET /api/v1/metrics/snapshot
 *   GET /api/v1/metrics/query
 *
 * TypeScript interfaces match Rust response structs in
 * crates/racecontrol/src/api/metrics_query.rs exactly.
 */

import { fetchApi } from "@/lib/api";

// ─── Interfaces matching Rust response structs ────────────────────────────────

export interface TimePoint {
  ts: number;       // unix epoch seconds (i64 in Rust)
  value: number;    // f64 in Rust
}

export interface QueryResponse {
  metric: string;
  pod: number | null;
  resolution: string;
  points: TimePoint[];
}

export interface SnapshotEntry {
  name: string;           // matches Rust field `name` (NOT metric_name)
  pod: number | null;     // matches Rust field `pod: Option<u32>` (NOT pod_id string)
  value: number;
  updated_at: number;     // unix epoch seconds (i64 in Rust, NOT ISO string)
}

export interface SnapshotResponse {
  metrics: SnapshotEntry[];
}

export interface NamesResponse {
  names: string[];        // wrapped object (NOT bare array)
}

// ─── API client functions ─────────────────────────────────────────────────────

/**
 * Fetch all distinct metric names.
 * GET /api/v1/metrics/names
 */
export async function fetchMetricNames(): Promise<string[]> {
  const resp = await fetchApi<NamesResponse>("/metrics/names");
  return resp.names;
}

/**
 * Fetch latest value per metric+pod combination.
 * GET /api/v1/metrics/snapshot[?pod=N]
 */
export async function fetchMetricSnapshot(pod?: number): Promise<SnapshotEntry[]> {
  const qs = pod !== undefined ? `?pod=${pod}` : "";
  const resp = await fetchApi<SnapshotResponse>(`/metrics/snapshot${qs}`);
  return resp.metrics;
}

/**
 * Query time-series data for a specific metric.
 * GET /api/v1/metrics/query?metric=X&from=Y&to=Z[&pod=N][&resolution=raw|hourly|daily]
 */
export async function fetchMetricQuery(
  metric: string,
  from: number,
  to: number,
  pod?: number,
  resolution?: string,
): Promise<QueryResponse> {
  const params = new URLSearchParams({ metric, from: String(from), to: String(to) });
  if (pod !== undefined) params.set("pod", String(pod));
  if (resolution !== undefined) params.set("resolution", resolution);
  return fetchApi<QueryResponse>(`/metrics/query?${params.toString()}`);
}
