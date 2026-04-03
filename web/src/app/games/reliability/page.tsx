"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import {
  getLaunchMatrix,
  getGameMatrix,
  getComboList,
} from "@/lib/api/metrics";
import type {
  LaunchMatrixRow,
  GameMatrixResponse,
  ComboListRow,
} from "@/lib/api/metrics";

const SUPPORTED_GAMES = [
  { value: "assetto_corsa", label: "Assetto Corsa" },
  { value: "assetto_corsa_evo", label: "AC EVO" },
  { value: "f1_24", label: "F1 24" },
  { value: "forza_motorsport", label: "Forza Motorsport" },
  { value: "iracing", label: "iRacing" },
  { value: "le_mans_ultimate", label: "Le Mans Ultimate" },
];

function rowBgClass(successRate: number): string {
  if (successRate < 0.7) return "bg-red-900/20";
  if (successRate < 0.9) return "bg-amber-900/20";
  return "bg-green-900/20";
}

function fmtRate(rate: number): string {
  return `${(rate * 100).toFixed(1)}%`;
}

function fmtMs(ms: number | null): string {
  if (ms === null) return "--";
  return `${Math.round(ms).toLocaleString()} ms`;
}

function truncate(s: string | null, len: number): string {
  if (s === null) return "--";
  return s.length > len ? `${s.slice(0, len)}…` : s;
}

function StatusBadge({ flagged }: { flagged: boolean }) {
  if (flagged) {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-red-900/50 text-red-400">
        <span className="w-1.5 h-1.5 rounded-full bg-red-400 animate-pulse" />
        Flagged
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-emerald-900/50 text-emerald-400">
      <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
      OK
    </span>
  );
}

export default function ReliabilityPage() {
  // ── Section A: Fleet Game Matrix ───────────────────────────────────────────
  const [gameMatrix, setGameMatrix] = useState<GameMatrixResponse | null>(null);
  const [matrixLoading, setMatrixLoading] = useState(true);

  useEffect(() => {
    function loadMatrix() {
      getGameMatrix()
        .then((data) => {
          setGameMatrix(data);
          setMatrixLoading(false);
        })
        .catch(() => {
          setMatrixLoading(false);
        });
    }
    loadMatrix();
    const id = setInterval(loadMatrix, 30_000);
    return () => clearInterval(id);
  }, []);

  // ── Section B: Combo Reliability ──────────────────────────────────────────
  const [comboRows, setComboRows] = useState<ComboListRow[]>([]);
  const [comboLoading, setComboLoading] = useState(true);
  const [comboGame, setComboGame] = useState("");
  const [sortBy, setSortBy] = useState("success_rate");
  const [sortOrder, setSortOrder] = useState("asc");

  useEffect(() => {
    function loadCombos() {
      setComboLoading(true);
      getComboList({ game: comboGame, sort_by: sortBy, order: sortOrder })
        .then((data) => {
          setComboRows(data);
          setComboLoading(false);
        })
        .catch(() => {
          setComboLoading(false);
        });
    }
    loadCombos();
    const id = setInterval(loadCombos, 30_000);
    return () => clearInterval(id);
  }, [comboGame, sortBy, sortOrder]);

  // ── Section C: Launch Matrix by Game (existing) ────────────────────────────
  const [game, setGame] = useState("assetto_corsa");
  const [rows, setRows] = useState<LaunchMatrixRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setLoading(true);
    setError(null);
    getLaunchMatrix(game)
      .then((data) => {
        setRows(data);
        setLoading(false);
      })
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : "Failed to load launch matrix");
        setLoading(false);
      });
  }, [game]);

  return (
    <DashboardLayout>
      <div className="mb-8">
        <h1 className="text-2xl font-bold text-white">Reliability Dashboard</h1>
        <p className="text-sm text-rp-grey mt-1">
          Fleet game availability, combo reliability, and per-pod launch analysis
        </p>
      </div>

      {/* ── Section A: Fleet Game Matrix ────────────────────────────────── */}
      <section className="mb-10">
        <h2 className="text-lg font-semibold text-white mb-3">Fleet Game Matrix</h2>
        <p className="text-xs text-rp-grey mb-4">
          Which games are installed and launchable per pod. Auto-refreshes every 30s.
        </p>

        {matrixLoading ? (
          <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
            <div className="text-rp-grey text-sm animate-pulse">Loading game matrix...</div>
          </div>
        ) : !gameMatrix || gameMatrix.games.length === 0 ? (
          <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
            <p className="text-neutral-400 mb-2">No game inventory data</p>
            <p className="text-rp-grey text-sm">
              Agents must connect and report inventory before this section populates.
            </p>
          </div>
        ) : (
          <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border">
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Game
                  </th>
                  {/* Pod columns — using last 4 chars of pod_id as label.
                      TODO: map to pod numbers when pod registry is available */}
                  {Array.from(
                    new Set(
                      gameMatrix.games.flatMap((g) => Object.keys(g.pods))
                    )
                  ).sort().map((podId) => (
                    <th
                      key={podId}
                      className="text-center px-3 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider"
                    >
                      …{podId.slice(-4)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-rp-border/50">
                {gameMatrix.games.map((g) => {
                  const allPodIds = Array.from(
                    new Set(
                      gameMatrix.games.flatMap((gm) => Object.keys(gm.pods))
                    )
                  ).sort();
                  return (
                    <tr key={g.game_id} className="hover:bg-white/5 transition-colors">
                      <td className="px-4 py-3 text-neutral-200 font-medium">
                        <div>{g.display_name}</div>
                        {g.sim_type && (
                          <div className="text-xs text-rp-grey">{g.sim_type}</div>
                        )}
                      </td>
                      {allPodIds.map((podId) => {
                        const entry = g.pods[podId];
                        return (
                          <td key={podId} className="px-3 py-3 text-center">
                            {entry ? (
                              entry.launchable ? (
                                <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-emerald-900/50 text-emerald-400">
                                  Installed
                                </span>
                              ) : (
                                <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-amber-900/50 text-amber-400">
                                  No Exec
                                </span>
                              )
                            ) : (
                              <span className="w-2 h-2 rounded-full bg-rp-grey/40 inline-block" />
                            )}
                          </td>
                        );
                      })}
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* ── Section B: Combo Reliability ────────────────────────────────── */}
      <section className="mb-10">
        <h2 className="text-lg font-semibold text-white mb-3">Combo Reliability</h2>

        {/* Controls */}
        <div className="flex flex-wrap gap-3 mb-4">
          <select
            value={comboGame}
            onChange={(e) => setComboGame(e.target.value)}
            className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
          >
            <option value="">All Games</option>
            {SUPPORTED_GAMES.map((g) => (
              <option key={g.value} value={g.value}>
                {g.label}
              </option>
            ))}
          </select>

          <select
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value)}
            className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
          >
            <option value="success_rate">Success Rate</option>
            <option value="total_launches">Total Launches</option>
            <option value="avg_time_ms">Avg Time</option>
          </select>

          <select
            value={sortOrder}
            onChange={(e) => setSortOrder(e.target.value)}
            className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
          >
            <option value="asc">Ascending</option>
            <option value="desc">Descending</option>
          </select>
        </div>

        {comboLoading ? (
          <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
            <div className="text-rp-grey text-sm animate-pulse">Loading combo data...</div>
          </div>
        ) : comboRows.length === 0 ? (
          <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
            <p className="text-neutral-400">No combo reliability data yet</p>
          </div>
        ) : (
          <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border">
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Pod</th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Game</th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Car</th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Track</th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Total</th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Success Rate</th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Avg Time</th>
                  <th className="text-center px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Status</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-rp-border/50">
                {comboRows.map((row, idx) => (
                  <tr
                    key={`${row.pod_id}-${row.sim_type}-${row.car ?? ""}-${row.track ?? ""}-${idx}`}
                    className={`transition-colors ${rowBgClass(row.success_rate)}`}
                  >
                    <td className="px-4 py-3 text-neutral-200 font-mono text-xs">
                      {row.pod_id.slice(0, 8)}
                    </td>
                    <td className="px-4 py-3 text-neutral-300 text-xs">
                      {row.sim_type}
                    </td>
                    <td className="px-4 py-3 text-neutral-400 text-xs">
                      {truncate(row.car, 30)}
                    </td>
                    <td className="px-4 py-3 text-neutral-400 text-xs">
                      {truncate(row.track, 30)}
                    </td>
                    <td className="px-4 py-3 text-neutral-300 text-right font-mono">
                      {row.total_launches.toLocaleString()}
                    </td>
                    <td
                      className={`px-4 py-3 text-right font-mono font-semibold ${
                        row.success_rate < 0.7
                          ? "text-red-400"
                          : row.success_rate < 0.9
                          ? "text-amber-400"
                          : "text-emerald-400"
                      }`}
                    >
                      {fmtRate(row.success_rate)}
                    </td>
                    <td className="px-4 py-3 text-neutral-400 text-right font-mono">
                      {fmtMs(row.avg_time_ms)}
                    </td>
                    <td className="px-4 py-3 text-center">
                      <StatusBadge flagged={row.flagged} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>

      {/* ── Section C: Launch Matrix by Pod (existing) ───────────────────── */}
      <section>
        <div className="flex items-center justify-between mb-3">
          <div>
            <h2 className="text-lg font-semibold text-white">Launch Matrix by Pod</h2>
            <p className="text-xs text-rp-grey">Per-pod launch success rates and failure analysis</p>
          </div>
          <select
            value={game}
            onChange={(e) => setGame(e.target.value)}
            className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
          >
            {SUPPORTED_GAMES.map((g) => (
              <option key={g.value} value={g.value}>
                {g.label}
              </option>
            ))}
          </select>
        </div>

        {/* Color legend */}
        <div className="flex gap-4 mb-4 text-xs text-rp-grey">
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-3 rounded bg-green-900/60 border border-green-700/50" />
            &ge;90% reliable
          </span>
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-3 rounded bg-amber-900/60 border border-amber-700/50" />
            70-90% acceptable
          </span>
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-3 rounded bg-red-900/60 border border-red-700/50" />
            &lt;70% flagged
          </span>
        </div>

        {loading ? (
          <div className="bg-rp-card border border-rp-border rounded-lg p-12 text-center">
            <div className="text-rp-grey text-sm animate-pulse">Loading launch matrix...</div>
          </div>
        ) : error ? (
          <div className="bg-red-900/20 border border-red-800/50 rounded-lg p-8 text-center">
            <p className="text-red-400 text-sm">{error}</p>
          </div>
        ) : rows.length === 0 ? (
          <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
            <p className="text-neutral-400 mb-2">No launch data available</p>
            <p className="text-rp-grey text-sm">
              Launch data will appear after pods have attempted game launches.
            </p>
          </div>
        ) : (
          <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border">
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Pod
                  </th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Total Launches
                  </th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Success Rate
                  </th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Avg Time
                  </th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Top Failures
                  </th>
                  <th className="text-center px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Status
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-rp-border/50">
                {rows.map((row) => (
                  <tr
                    key={row.pod_id}
                    className={`transition-colors ${rowBgClass(row.success_rate)}`}
                  >
                    <td className="px-4 py-3 text-neutral-200 font-mono text-xs">
                      {row.pod_id.slice(0, 8)}
                    </td>
                    <td className="px-4 py-3 text-neutral-300 text-right font-mono">
                      {row.total_launches.toLocaleString()}
                    </td>
                    <td
                      className={`px-4 py-3 text-right font-mono font-semibold ${
                        row.success_rate < 0.7
                          ? "text-red-400"
                          : row.success_rate < 0.9
                          ? "text-amber-400"
                          : "text-emerald-400"
                      }`}
                    >
                      {fmtRate(row.success_rate)}
                    </td>
                    <td className="px-4 py-3 text-neutral-400 text-right font-mono">
                      {fmtMs(row.avg_time_ms)}
                    </td>
                    <td className="px-4 py-3 text-neutral-400 text-xs">
                      {row.top_3_failure_modes.length > 0
                        ? row.top_3_failure_modes
                            .map((f) => `${f.mode} (${f.count})`)
                            .join(", ")
                        : "--"}
                    </td>
                    <td className="px-4 py-3 text-center">
                      <StatusBadge flagged={row.flagged} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </DashboardLayout>
  );
}
