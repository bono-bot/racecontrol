"use client";

import { useState, useEffect, useRef, useCallback } from "react";
import { AnimatePresence, motion } from "motion/react";

// ── Types ────────────────────────────────────────────────────────────────────

interface LeaderboardRecord {
  track: string;
  car: string;
  driver: string;
  best_lap_ms: number;
  best_lap_display: string;
  achieved_at: string;
}

interface LeaderboardResponse {
  records: LeaderboardRecord[];
  tracks: { track: string; laps: number }[];
  top_drivers: { position: number; name: string; total_laps: number; fastest_lap_ms: number | null }[];
}

// ── Sim type tab config ─────────────────────────────────────────────────────

const SIM_TABS: { label: string; value: string }[] = [
  { label: "AC", value: "assetto_corsa" },
  { label: "F1", value: "f1_25" },
  { label: "iRacing", value: "iracing" },
  { label: "LMU", value: "le_mans_ultimate" },
  { label: "Forza", value: "forza" },
];

// ── Helpers ──────────────────────────────────────────────────────────────────

function formatLapTime(ms: number): string {
  if (!ms || ms <= 0) return "--:--.---";
  const totalSec = ms / 1000;
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toFixed(3).padStart(6, "0")}`;
}

function prettyName(raw: string): string {
  if (!raw) return "";
  const segment = raw.split(/[/\\]/).pop() || raw;
  return segment
    .replace(/[-_]+/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase())
    .trim();
}

// ── API base ─────────────────────────────────────────────────────────────────

const API_BASE =
  typeof window !== "undefined"
    ? process.env.NEXT_PUBLIC_API_URL || `${window.location.protocol}//${window.location.host}`
    : "";

// ── Component ────────────────────────────────────────────────────────────────

export function KioskLeaderboard() {
  const [records, setRecords] = useState<LeaderboardRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeSim, setActiveSim] = useState("assetto_corsa");
  const wsRef = useRef<WebSocket | null>(null);

  // Fetch leaderboard data
  const fetchRecords = useCallback(async (simType: string) => {
    try {
      const res = await fetch(
        `${API_BASE}/api/v1/public/leaderboard?sim_type=${encodeURIComponent(simType)}`
      );
      if (!res.ok) return;
      const data: LeaderboardResponse = await res.json();
      setRecords(data.records || []);
    } catch {
      // Silently fail — kiosk is ambient display, no error toasts
    } finally {
      setLoading(false);
    }
  }, []);

  // Initial fetch + polling
  useEffect(() => {
    setLoading(true);
    fetchRecords(activeSim);

    const interval = setInterval(() => {
      fetchRecords(activeSim);
    }, 10_000);

    return () => clearInterval(interval);
  }, [activeSim, fetchRecords]);

  // WebSocket subscription for live updates
  useEffect(() => {
    const wsUrl = (process.env.NEXT_PUBLIC_WS_URL || "").replace("http", "ws");
    if (!wsUrl) return;

    let stopped = false;

    const connect = () => {
      if (stopped) return;
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;

      ws.onmessage = (e) => {
        try {
          const msg = JSON.parse(e.data);
          if (msg.type === "LeaderboardUpdate" && Array.isArray(msg.records)) {
            setRecords(msg.records);
          }
        } catch {
          // ignore malformed messages
        }
      };

      ws.onclose = () => {
        if (!stopped) {
          setTimeout(connect, 2000); // 2s minimum reconnect delay
        }
      };

      ws.onerror = () => {
        // onclose will fire after onerror
      };
    };

    connect();

    return () => {
      stopped = true;
      wsRef.current?.close();
    };
  }, []);

  // Tab change handler
  const handleTabChange = (simType: string) => {
    setActiveSim(simType);
    setRecords([]);
    setLoading(true);
  };

  return (
    <div className="flex flex-col h-full">
      {/* Sim type filter tabs */}
      <div className="flex gap-2 px-4 py-3 border-b border-rp-border overflow-x-auto">
        {SIM_TABS.map((tab) => (
          <button
            key={tab.value}
            onClick={() => handleTabChange(tab.value)}
            className={`min-h-[44px] px-5 py-2 rounded-lg text-sm font-bold uppercase tracking-wider transition-all active:scale-95 flex-shrink-0 ${
              activeSim === tab.value
                ? "bg-rp-red text-white"
                : "border border-rp-border text-rp-grey hover:text-white"
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Content area */}
      <div className="flex-1 overflow-y-auto">
        {/* Loading skeleton */}
        {loading && records.length === 0 && (
          <div>
            {[...Array(5)].map((_, i) => (
              <div key={i} className="flex items-center gap-4 px-4 py-3 border-b border-rp-border animate-pulse">
                <div className="w-8 h-5 bg-rp-surface rounded" />
                <div className="flex-1 h-5 bg-rp-surface rounded" />
                <div className="w-24 h-5 bg-rp-surface rounded" />
              </div>
            ))}
          </div>
        )}

        {/* Empty state */}
        {!loading && records.length === 0 && (
          <div className="flex flex-col items-center justify-center py-16 text-rp-grey gap-2">
            <span className="text-4xl">&#127937;</span>
            <p className="text-sm">No lap times yet -- be the first!</p>
          </div>
        )}

        {/* Leaderboard rows with AnimatePresence */}
        {records.length > 0 && (
          <AnimatePresence mode="popLayout">
            {records.slice(0, 10).map((rec, idx) => (
              <motion.div
                key={`${rec.driver}-${rec.track}-${activeSim}`}
                layout
                initial={{ opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: 10 }}
                transition={{ duration: 0.3, ease: "easeOut" }}
                className="flex items-center gap-4 px-4 py-3 border-b border-rp-border"
              >
                {/* Rank */}
                <span
                  className={`w-8 text-center font-bold text-lg font-[family-name:var(--font-mono-jb)] ${
                    idx === 0
                      ? "text-yellow-400"
                      : idx === 1
                      ? "text-zinc-300"
                      : idx === 2
                      ? "text-amber-600"
                      : "text-rp-grey"
                  }`}
                >
                  {idx + 1}
                </span>

                {/* Driver + track/car info */}
                <div className="flex-1 min-w-0">
                  <p className="text-white font-semibold truncate">{rec.driver}</p>
                  <p className="text-[10px] text-zinc-500 truncate">
                    {prettyName(rec.track)} &middot; {prettyName(rec.car)}
                  </p>
                </div>

                {/* Lap time */}
                <span
                  className={`font-[family-name:var(--font-mono-jb)] text-sm font-bold ${
                    idx === 0 ? "text-purple-400" : "text-white"
                  }`}
                >
                  {rec.best_lap_display || formatLapTime(rec.best_lap_ms)}
                </span>

                {/* Record badge for P1 */}
                {idx === 0 && (
                  <span className="px-1.5 py-0.5 rounded text-xs bg-purple-500/20 text-purple-300 font-bold">
                    REC
                  </span>
                )}
              </motion.div>
            ))}
          </AnimatePresence>
        )}
      </div>
    </div>
  );
}
