"use client";

import { useEffect, useRef, useState, useCallback } from "react";
import { AnimatePresence, motion } from "motion/react";
import { SkeletonRow, EmptyState } from "./Skeleton";

// --- Types ---

interface LeaderboardEntry {
  rank: number;
  driver_id: string;
  driver_name: string;
  best_lap_ms: number;
  gap_ms: number | null;
  is_personal_best: boolean;
  is_session_best: boolean;
  laps_completed: number;
}

interface LeaderboardTableProps {
  simType?: string;
  gameId?: string;
  limit?: number;
}

// --- Helpers ---

function formatLapTime(ms: number): string {
  const totalSecs = ms / 1000;
  const mins = Math.floor(totalSecs / 60);
  const secs = Math.floor(totalSecs % 60);
  const millis = ms % 1000;
  return `${mins}:${String(secs).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
}

function formatGap(gapMs: number | null): string {
  if (gapMs == null) return "\u2014";
  return `+${(gapMs / 1000).toFixed(3)}`;
}

function rankColor(rank: number): string {
  if (rank === 1) return "text-rp-red";
  if (rank === 2) return "text-neutral-200";
  if (rank === 3) return "text-rp-yellow";
  return "text-neutral-400";
}

function rowAccent(entry: LeaderboardEntry): string {
  // Session best takes precedence over personal best
  if (entry.is_session_best) return "border-l-2 border-l-rp-green bg-rp-green/5";
  if (entry.is_personal_best) return "border-l-2 border-l-rp-purple bg-rp-purple/5";
  return "";
}

// --- Trophy Icon ---

function TrophyIcon() {
  return (
    <svg
      width="48"
      height="48"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M6 9H4.5a2.5 2.5 0 0 1 0-5H6" />
      <path d="M18 9h1.5a2.5 2.5 0 0 0 0-5H18" />
      <path d="M4 22h16" />
      <path d="M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22" />
      <path d="M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20.24 17 22" />
      <path d="M18 2H6v7a6 6 0 0 0 12 0V2Z" />
    </svg>
  );
}

// --- Component ---

export function LeaderboardTable({
  simType,
  gameId,
  limit = 20,
}: LeaderboardTableProps) {
  const [entries, setEntries] = useState<LeaderboardEntry[]>([]);
  const [loading, setLoading] = useState(true);

  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isMountedRef = useRef(true);

  // Build query params for REST fallback
  const buildQuery = useCallback(() => {
    const params = new URLSearchParams();
    if (simType) params.set("sim_type", simType);
    if (gameId) params.set("game_id", gameId);
    params.set("limit", String(limit));
    const qs = params.toString();
    return qs ? `?${qs}` : "";
  }, [simType, gameId, limit]);

  useEffect(() => {
    isMountedRef.current = true;

    // Initial REST fetch so we show data immediately before WS first push
    const apiBase = process.env.NEXT_PUBLIC_API_URL || "http://192.168.31.23:8080";
    fetch(`${apiBase}/api/v1/leaderboards${buildQuery()}`)
      .then((res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        return res.json() as Promise<{ entries?: LeaderboardEntry[] }>;
      })
      .then((data) => {
        if (!isMountedRef.current) return;
        if (data.entries && Array.isArray(data.entries)) {
          setEntries(data.entries.slice(0, limit));
        }
        setLoading(false);
      })
      .catch(() => {
        // REST failed — WS will provide data
        if (isMountedRef.current) setLoading(false);
      });

    // WS connection with reconnect
    function connect() {
      if (!isMountedRef.current) return;

      const wsUrl =
        (process.env.NEXT_PUBLIC_WS_URL || "ws://192.168.31.23:8080") + "/ws";
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      ws.onopen = () => {
        if (!isMountedRef.current) {
          ws.close();
          return;
        }
        // Could send subscription message here if needed
      };

      ws.onmessage = (event) => {
        if (!isMountedRef.current) return;
        try {
          const msg = JSON.parse(event.data as string) as {
            type?: string;
            entries?: LeaderboardEntry[];
          };
          if (
            msg.type === "LeaderboardUpdate" &&
            Array.isArray(msg.entries)
          ) {
            setEntries(msg.entries.slice(0, limit));
            setLoading(false);
          }
        } catch {
          /* ignore malformed messages */
        }
      };

      ws.onclose = () => {
        if (!isMountedRef.current) return;
        // Reconnect with minimum 1s delay (prevents reconnect storm)
        reconnectTimeoutRef.current = setTimeout(connect, 1000);
      };

      ws.onerror = () => {
        ws.close(); // triggers onclose -> reconnect
      };
    }

    connect();

    return () => {
      isMountedRef.current = false;
      // Clear pending reconnect timer first
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
        reconnectTimeoutRef.current = null;
      }
      // Close WS — prevents onclose from triggering reconnect (isMountedRef.current=false)
      if (wsRef.current) {
        wsRef.current.close();
        wsRef.current = null;
      }
    };
  }, [limit, buildQuery]);

  // Loading state: 8 skeleton rows
  if (loading) {
    return (
      <div className="overflow-auto rounded-lg border border-rp-border">
        <table className="w-full text-sm">
          <thead className="border-b border-rp-border bg-rp-black sticky top-0 z-10">
            <tr>
              <th className="w-10 text-center text-xs text-rp-grey px-3 py-2">
                #
              </th>
              <th className="text-left text-xs text-rp-grey px-4 py-2 uppercase tracking-wider">
                Driver
              </th>
              <th className="text-left text-xs text-rp-grey px-4 py-2 uppercase tracking-wider">
                Best Lap
              </th>
              <th className="text-left text-xs text-rp-grey px-4 py-2 uppercase tracking-wider">
                Gap
              </th>
              <th className="text-right text-xs text-rp-grey px-3 py-2 uppercase tracking-wider">
                Laps
              </th>
            </tr>
          </thead>
          <tbody>
            {Array.from({ length: 8 }).map((_, i) => (
              <tr key={i}>
                <td colSpan={5}>
                  <SkeletonRow />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    );
  }

  // Empty state
  if (entries.length === 0) {
    return (
      <div className="overflow-auto rounded-lg border border-rp-border">
        <EmptyState
          icon={<TrophyIcon />}
          headline="No lap times yet"
          hint="Lap times will appear here as drivers complete laps"
        />
      </div>
    );
  }

  return (
    <div className="overflow-auto rounded-lg border border-rp-border">
      <table className="w-full text-sm">
        <thead className="border-b border-rp-border bg-rp-black sticky top-0 z-10">
          <tr>
            <th className="w-10 text-center text-xs text-rp-grey px-3 py-2">
              #
            </th>
            <th className="text-left text-xs text-rp-grey px-4 py-2 uppercase tracking-wider">
              Driver
            </th>
            <th className="text-left text-xs text-rp-grey px-4 py-2 uppercase tracking-wider">
              Best Lap
            </th>
            <th className="text-left text-xs text-rp-grey px-4 py-2 uppercase tracking-wider">
              Gap
            </th>
            <th className="text-right text-xs text-rp-grey px-3 py-2 uppercase tracking-wider">
              Laps
            </th>
          </tr>
        </thead>
        <tbody>
          <AnimatePresence mode="popLayout">
            {entries.map((entry) => (
              <motion.tr
                key={entry.driver_id}
                layout
                initial={{ opacity: 0, x: -8 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 8 }}
                transition={{ duration: 0.2 }}
                className={`border-b border-rp-border/50 transition-colors ${rowAccent(entry)}`}
              >
                <td
                  className={`w-10 text-center text-sm font-mono font-bold px-3 py-3 ${rankColor(entry.rank)}`}
                >
                  {entry.rank}
                </td>
                <td className="text-sm font-medium text-white px-4 py-3">
                  {entry.driver_name}
                </td>
                <td className="font-mono text-sm text-neutral-200 px-4 py-3">
                  {formatLapTime(entry.best_lap_ms)}
                </td>
                <td className="font-mono text-sm text-rp-grey px-4 py-3">
                  {formatGap(entry.gap_ms)}
                </td>
                <td className="text-sm text-rp-grey text-right px-3 py-3">
                  {entry.laps_completed}
                </td>
              </motion.tr>
            ))}
          </AnimatePresence>
        </tbody>
      </table>
    </div>
  );
}
