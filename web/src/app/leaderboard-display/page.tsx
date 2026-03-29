"use client";

import { useEffect, useState, useRef, useCallback } from "react";
import { fetchPublic } from "@/lib/api";
import type {
  PublicTrackRecord,
  PublicTopDriver,
  PublicTrackLeaderboardEntry,
} from "@/lib/api";

// ─── Types ──────────────────────────────────────────────────────────────

interface TimeTrial {
  id: string;
  track: string;
  car: string;
  week_start: string;
  week_end: string;
}

interface TimeTrialEntry {
  position: number;
  driver: string;
  best_lap_ms: number;
  best_lap_display: string;
  attempts: number;
}

interface RecordBrokenEvent {
  record_type: string;
  track: string;
  car: string;
  sim_type: string;
  driver_name: string;
  lap_time_ms: number;
  previous_time_ms: number | null;
  driver_id: string;
}

interface CachedData {
  records: PublicTrackRecord[];
  topDrivers: PublicTopDriver[];
  timeTrial: TimeTrial | null;
  timeTrialEntries: TimeTrialEntry[];
  tracks: TrackInfo[];
  trackRecords: Record<string, PublicTrackLeaderboardEntry[]>;
  timestamp: number;
}

interface TrackInfo {
  name: string;
  total_laps: number;
}

// ─── Helpers ────────────────────────────────────────────────────────────

const CACHE_KEY = "rp-leaderboard-display-cache";
const MAX_CACHE_AGE_MS = 60 * 60 * 1000; // 1 hour
const PANEL_COUNT = 4;

function formatLapTime(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  if (minutes > 0) {
    return `${minutes}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
  }
  return `${seconds}.${String(millis).padStart(3, "0")}`;
}

function medalColor(position: number): string {
  if (position === 1) return "#FFD700";
  if (position === 2) return "#C0C0C0";
  if (position === 3) return "#CD7F32";
  return "#888888";
}

function ratingBadge(ratingClass: string | null | undefined): string {
  if (!ratingClass) return "";
  const map: Record<string, string> = {
    bronze: "bg-amber-800 text-amber-200",
    silver: "bg-neutral-400 text-neutral-900",
    gold: "bg-yellow-500 text-yellow-900",
    platinum: "bg-cyan-400 text-cyan-900",
    diamond: "bg-purple-400 text-purple-900",
  };
  return map[ratingClass.toLowerCase()] || "bg-neutral-600 text-neutral-200";
}

function timeAgo(ts: number): string {
  const diff = Math.floor((Date.now() - ts) / 1000);
  if (diff < 60) return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return `${Math.floor(diff / 3600)}h ago`;
}

async function fetchWithTimeout<T>(promise: Promise<T>, timeoutMs: number): Promise<T> {
  return Promise.race([
    promise,
    new Promise<never>((_, reject) =>
      setTimeout(() => reject(new Error("timeout")), timeoutMs)
    ),
  ]);
}

// ─── Component ──────────────────────────────────────────────────────────

export default function LeaderboardDisplayPage() {
  // URL params — read in useEffect (hydration-safe)
  const [simType, setSimType] = useState<string | null>(null);
  const [rotationSpeed, setRotationSpeed] = useState(15);
  const [kioskMode, setKioskMode] = useState(false);

  // Panel state
  const [activePanel, setActivePanel] = useState(0);
  const [visible, setVisible] = useState(true);

  // Data
  const [records, setRecords] = useState<PublicTrackRecord[]>([]);
  const [topDrivers, setTopDrivers] = useState<(PublicTopDriver & { composite_rating?: number; rating_class?: string })[]>([]);
  const [timeTrial, setTimeTrial] = useState<TimeTrial | null>(null);
  const [timeTrialEntries, setTimeTrialEntries] = useState<TimeTrialEntry[]>([]);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [trackRecords, setTrackRecords] = useState<Record<string, PublicTrackLeaderboardEntry[]>>({});
  const [trackCycleIndex, setTrackCycleIndex] = useState(0);

  // Record broken overlay
  const [recordOverlay, setRecordOverlay] = useState<RecordBrokenEvent | null>(null);

  // Connection state
  const [lastUpdated, setLastUpdated] = useState<number>(0);
  const [wsDisconnectedSince, setWsDisconnectedSince] = useState<number | null>(null);
  const [dataUnavailable, setDataUnavailable] = useState(false);

  // Heartbeat
  const heartbeatRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const startTimeRef = useRef(Date.now());

  // ─── Read URL params (useEffect for hydration safety) ──────────────

  useEffect(() => {
    const params = new URLSearchParams(window.location.search);
    const st = params.get("sim_type");
    if (st) setSimType(st);
    const rs = params.get("rotation_speed");
    if (rs) setRotationSpeed(Math.max(5, parseInt(rs, 10) || 15));
    setKioskMode(params.get("kiosk") === "true");
  }, []);

  // ─── Kiosk mode: hide cursor, disable right-click ─────────────────

  useEffect(() => {
    if (!kioskMode) return;
    document.body.style.cursor = "none";
    const handler = (e: MouseEvent) => e.preventDefault();
    document.addEventListener("contextmenu", handler);
    return () => {
      document.body.style.cursor = "";
      document.removeEventListener("contextmenu", handler);
    };
  }, [kioskMode]);

  // ─── Load cached data from localStorage ────────────────────────────

  useEffect(() => {
    try {
      const raw = localStorage.getItem(CACHE_KEY);
      if (!raw) return;
      const cached: CachedData = JSON.parse(raw);
      if (Date.now() - cached.timestamp > MAX_CACHE_AGE_MS) {
        localStorage.removeItem(CACHE_KEY);
        return;
      }
      setRecords(cached.records || []);
      setTopDrivers(cached.topDrivers || []);
      setTimeTrial(cached.timeTrial);
      setTimeTrialEntries(cached.timeTrialEntries || []);
      setTracks(cached.tracks || []);
      setTrackRecords(cached.trackRecords || {});
      setLastUpdated(cached.timestamp);
    } catch {
      // Corrupted cache — ignore
    }
  }, []);

  // ─── Save to localStorage when data changes ────────────────────────

  const saveCache = useCallback(() => {
    if (!lastUpdated) return;
    const data: CachedData = {
      records,
      topDrivers,
      timeTrial,
      timeTrialEntries,
      tracks,
      trackRecords,
      timestamp: lastUpdated,
    };
    try {
      localStorage.setItem(CACHE_KEY, JSON.stringify(data));
    } catch {
      // Storage full — ignore
    }
  }, [records, topDrivers, timeTrial, timeTrialEntries, tracks, trackRecords, lastUpdated]);

  useEffect(() => {
    saveCache();
  }, [saveCache]);

  // ─── Fetch data ────────────────────────────────────────────────────

  const fetchData = useCallback(async () => {
    try {
      const qs = simType ? `?sim_type=${simType}` : "";

      // Fetch leaderboard overview
      const overview = await fetchWithTimeout(
        fetchPublic<{
          records: PublicTrackRecord[];
          tracks: TrackInfo[];
          top_drivers: (PublicTopDriver & { composite_rating?: number; rating_class?: string })[];
          time_trial: TimeTrial | null;
        }>(`/public/leaderboard${qs}`),
        5000
      );
      setRecords(overview.records || []);
      setTopDrivers(overview.top_drivers || []);
      setTracks(overview.tracks || []);
      if (overview.time_trial) {
        setTimeTrial(overview.time_trial);
      }

      // Fetch time trial details
      try {
        const ttData = await fetchWithTimeout(
          fetchPublic<{
            time_trial: TimeTrial | null;
            leaderboard: TimeTrialEntry[];
            message?: string;
          }>("/public/time-trial"),
          5000
        );
        if (ttData.time_trial) {
          setTimeTrial(ttData.time_trial);
          setTimeTrialEntries(ttData.leaderboard || []);
        } else {
          setTimeTrial(null);
          setTimeTrialEntries([]);
        }
      } catch {
        // Time trial fetch failed — keep existing data
      }

      // Fetch per-track records (top 5 tracks by laps)
      const topTracks = (overview.tracks || [])
        .sort((a: TrackInfo, b: TrackInfo) => b.total_laps - a.total_laps)
        .slice(0, 8);

      const trackResults: Record<string, PublicTrackLeaderboardEntry[]> = {};
      for (const track of topTracks) {
        try {
          const trackData = await fetchWithTimeout(
            fetchPublic<{
              leaderboard: PublicTrackLeaderboardEntry[];
            }>(`/public/leaderboard/${encodeURIComponent(track.name)}${qs}`),
            5000
          );
          trackResults[track.name] = (trackData.leaderboard || []).slice(0, 5);
        } catch {
          // Skip this track on timeout
        }
      }
      setTrackRecords(trackResults);

      setLastUpdated(Date.now());
      setDataUnavailable(false);
    } catch {
      // Check if cached data is too old
      if (lastUpdated && Date.now() - lastUpdated > MAX_CACHE_AGE_MS) {
        setDataUnavailable(true);
      }
    }
  }, [simType, lastUpdated]);

  // Initial fetch + periodic refresh every 60s
  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 60_000);
    return () => clearInterval(interval);
  }, [fetchData]);

  // ─── WebSocket for record_broken events ────────────────────────────

  useEffect(() => {
    const WS_BASE = process.env.NEXT_PUBLIC_WS_URL || "ws://localhost:8080/ws/dashboard";
    const WS_TOKEN = process.env.NEXT_PUBLIC_WS_TOKEN || "";
    const wsUrl = WS_TOKEN ? `${WS_BASE}?token=${WS_TOKEN}` : WS_BASE;
    let ws: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

    function connect() {
      ws = new WebSocket(wsUrl);
      ws.onopen = () => {
        setWsDisconnectedSince(null);
      };
      ws.onmessage = (e) => {
        try {
          const msg = JSON.parse(e.data);
          if (msg.event === "record_broken" && msg.data) {
            const evt = msg.data as RecordBrokenEvent;
            // Show overlay
            setRecordOverlay(evt);
            setTimeout(() => setRecordOverlay(null), 5000);
            // Refresh data
            fetchData();
          }
        } catch {
          // Ignore parse errors
        }
      };
      ws.onclose = () => {
        setWsDisconnectedSince((prev) => prev ?? Date.now());
        if (reconnectTimer) clearTimeout(reconnectTimer);
        reconnectTimer = setTimeout(connect, 5000);
      };
      ws.onerror = () => {
        ws?.close();
      };
    }

    connect();
    return () => {
      if (reconnectTimer) clearTimeout(reconnectTimer);
      ws?.close();
    };
  }, [fetchData]);

  // ─── Heartbeat ping ────────────────────────────────────────────────

  useEffect(() => {
    const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";

    // Try to get a display ID from URL or generate one
    const params = new URLSearchParams(window.location.search);
    const displayId = params.get("display_id") || `display-${Math.random().toString(36).slice(2, 10)}`;

    const sendPing = () => {
      const uptimeS = Math.floor((Date.now() - startTimeRef.current) / 1000);
      fetch(`${API_BASE}/api/v1/kiosk/ping`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ display_id: displayId, uptime_s: uptimeS }),
      }).catch(() => {
        // Ping failure is non-critical
      });
    };

    sendPing();
    heartbeatRef.current = setInterval(sendPing, 60_000);
    return () => {
      if (heartbeatRef.current) clearInterval(heartbeatRef.current);
    };
  }, []);

  // ─── Panel rotation ────────────────────────────────────────────────

  useEffect(() => {
    const interval = setInterval(() => {
      // Fade out
      setVisible(false);
      setTimeout(() => {
        setActivePanel((prev) => {
          const next = (prev + 1) % PANEL_COUNT;
          // Cycle track index when entering per-track panel
          if (next === 3) {
            setTrackCycleIndex((ti) => {
              const trackNames = Object.keys(trackRecords);
              return trackNames.length > 0 ? (ti + 1) % trackNames.length : 0;
            });
          }
          return next;
        });
        // Fade in
        setVisible(true);
      }, 500);
    }, rotationSpeed * 1000);
    return () => clearInterval(interval);
  }, [rotationSpeed, trackRecords]);

  // ─── Offline warning ───────────────────────────────────────────────

  const showOfflineWarning = wsDisconnectedSince !== null &&
    Date.now() - wsDisconnectedSince > 30_000 &&
    lastUpdated > 0;

  // ─── Render ────────────────────────────────────────────────────────

  if (dataUnavailable) {
    return (
      <div className="fixed inset-0 flex items-center justify-center" style={{ background: "#1A1A1A" }}>
        <div className="text-center">
          <div className="text-6xl mb-6" style={{ color: "#E10600" }}>RACING POINT</div>
          <div className="text-2xl text-neutral-500">Leaderboard data unavailable</div>
          <div className="text-lg text-neutral-600 mt-2">Please check server connection</div>
        </div>
      </div>
    );
  }

  const trackNames = Object.keys(trackRecords);
  const currentTrackName = trackNames[trackCycleIndex % Math.max(1, trackNames.length)] || "";
  const currentTrackEntries = trackRecords[currentTrackName] || [];

  return (
    <div
      className="fixed inset-0 overflow-hidden select-none"
      style={{ background: "#1A1A1A", fontFamily: "'Montserrat', sans-serif" }}
    >
      {/* Record Broken Overlay */}
      {recordOverlay && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center"
          style={{
            background: "rgba(0, 0, 0, 0.85)",
            animation: "fadeIn 0.3s ease-out",
          }}
        >
          <div className="text-center animate-pulse">
            <div className="text-8xl mb-4">&#127942;</div>
            <div className="text-5xl font-black mb-4" style={{ color: "#E10600" }}>
              NEW RECORD!
            </div>
            <div className="text-3xl text-white font-bold mb-2">
              {recordOverlay.driver_name}
            </div>
            <div className="text-2xl text-neutral-400 mb-2">{recordOverlay.track}</div>
            <div className="text-4xl font-mono font-bold" style={{ color: "#4ade80" }}>
              {formatLapTime(recordOverlay.lap_time_ms)}
            </div>
            {recordOverlay.previous_time_ms && (
              <div className="text-lg text-neutral-500 mt-2">
                Previous: {formatLapTime(recordOverlay.previous_time_ms)}
              </div>
            )}
          </div>
        </div>
      )}

      {/* Offline warning */}
      {showOfflineWarning && (
        <div
          className="fixed top-0 left-0 right-0 z-40 text-center py-2 text-sm"
          style={{ background: "rgba(225, 6, 0, 0.8)" }}
        >
          Showing cached data &middot; Last updated: {timeAgo(lastUpdated)}
        </div>
      )}

      {/* Header */}
      <div className="flex items-center justify-between px-8 pt-6 pb-4">
        <div>
          <h1 className="text-5xl font-black tracking-tight" style={{ color: "#E10600" }}>
            RACING POINT
          </h1>
          <p className="text-lg text-neutral-500 mt-1">May the Fastest Win.</p>
        </div>
        <div className="flex items-center gap-4">
          {/* Panel indicator dots */}
          <div className="flex gap-2">
            {Array.from({ length: PANEL_COUNT }).map((_, i) => (
              <div
                key={i}
                className="w-3 h-3 rounded-full transition-all duration-300"
                style={{
                  background: i === activePanel ? "#E10600" : "#333333",
                  transform: i === activePanel ? "scale(1.3)" : "scale(1)",
                }}
              />
            ))}
          </div>
        </div>
      </div>

      {/* Panel Content — fade transition */}
      <div
        className="px-8 pb-8 transition-opacity duration-500"
        style={{
          opacity: visible ? 1 : 0,
          height: "calc(100vh - 120px)",
        }}
      >
        {/* Panel 0: All-Time Records */}
        {activePanel === 0 && <AllTimeRecordsPanel records={records} />}

        {/* Panel 1: Top Drivers */}
        {activePanel === 1 && <TopDriversPanel drivers={topDrivers} />}

        {/* Panel 2: Time Trial */}
        {activePanel === 2 && (
          <TimeTrialPanel timeTrial={timeTrial} entries={timeTrialEntries} />
        )}

        {/* Panel 3: Per-Track Records */}
        {activePanel === 3 && (
          <PerTrackPanel trackName={currentTrackName} entries={currentTrackEntries} />
        )}
      </div>

      {/* Subtle footer */}
      <div className="fixed bottom-0 left-0 right-0 px-8 py-3 flex justify-between text-xs text-neutral-600">
        <span>racingpoint.cloud</span>
        <span>{new Date().toLocaleTimeString("en-IN", { timeZone: "Asia/Kolkata", hour: "2-digit", minute: "2-digit" })} IST</span>
      </div>

      <style jsx>{`
        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }
      `}</style>
    </div>
  );
}

// ─── Sub-Panels ─────────────────────────────────────────────────────────

function AllTimeRecordsPanel({ records }: { records: PublicTrackRecord[] }) {
  const displayRecords = records.slice(0, 10);
  return (
    <div className="h-full flex flex-col">
      <h2 className="text-4xl font-bold text-white mb-6">ALL-TIME RECORDS</h2>
      {displayRecords.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <p className="text-2xl text-neutral-500">No records yet</p>
        </div>
      ) : (
        <div className="flex-1 overflow-hidden">
          {/* Header row */}
          <div
            className="grid gap-4 pb-3 mb-3 text-sm uppercase tracking-wider text-neutral-500"
            style={{
              gridTemplateColumns: "60px 1fr 1fr 1fr 180px",
              borderBottom: "1px solid #333333",
            }}
          >
            <span>#</span>
            <span>Track</span>
            <span>Car</span>
            <span>Driver</span>
            <span className="text-right">Record</span>
          </div>
          {displayRecords.map((r, i) => (
            <div
              key={`${r.track}-${r.car}-${i}`}
              className="grid gap-4 py-3"
              style={{
                gridTemplateColumns: "60px 1fr 1fr 1fr 180px",
                borderBottom: "1px solid #222222",
                background: i < 3 ? "rgba(225, 6, 0, 0.05)" : "transparent",
              }}
            >
              <span
                className="text-3xl font-black"
                style={{ color: medalColor(i + 1) }}
              >
                {i + 1}
              </span>
              <span className="text-2xl text-white truncate self-center">{r.track}</span>
              <span className="text-lg text-neutral-400 truncate self-center">{r.car}</span>
              <span className="text-lg text-neutral-300 truncate self-center">{r.driver}</span>
              <span
                className="text-2xl font-mono font-bold text-right self-center"
                style={{ color: "#4ade80" }}
              >
                {r.best_lap_display}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function TopDriversPanel({
  drivers,
}: {
  drivers: (PublicTopDriver & { composite_rating?: number; rating_class?: string })[];
}) {
  const displayDrivers = drivers.slice(0, 10);
  return (
    <div className="h-full flex flex-col">
      <h2 className="text-4xl font-bold text-white mb-6">TOP DRIVERS</h2>
      {displayDrivers.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <p className="text-2xl text-neutral-500">No drivers yet</p>
        </div>
      ) : (
        <div className="flex-1 overflow-hidden">
          <div
            className="grid gap-4 pb-3 mb-3 text-sm uppercase tracking-wider text-neutral-500"
            style={{
              gridTemplateColumns: "60px 1fr 140px 140px 180px",
              borderBottom: "1px solid #333333",
            }}
          >
            <span>#</span>
            <span>Driver</span>
            <span className="text-center">Rating</span>
            <span className="text-right">Laps</span>
            <span className="text-right">Fastest</span>
          </div>
          {displayDrivers.map((d, i) => (
            <div
              key={d.name}
              className="grid gap-4 py-3"
              style={{
                gridTemplateColumns: "60px 1fr 140px 140px 180px",
                borderBottom: "1px solid #222222",
                background: i < 3 ? "rgba(225, 6, 0, 0.05)" : "transparent",
              }}
            >
              <span
                className="text-3xl font-black"
                style={{ color: medalColor(d.position) }}
              >
                {d.position}
              </span>
              <span className="text-2xl text-white truncate self-center">{d.name}</span>
              <span className="self-center text-center">
                {d.rating_class ? (
                  <span
                    className={`px-3 py-1 rounded-full text-sm font-bold uppercase ${ratingBadge(d.rating_class)}`}
                  >
                    {d.rating_class}
                  </span>
                ) : (
                  <span className="text-neutral-600 text-sm">---</span>
                )}
              </span>
              <span className="text-xl text-neutral-400 text-right self-center">
                {d.total_laps}
              </span>
              <span
                className="text-2xl font-mono font-bold text-right self-center"
                style={{ color: "#4ade80" }}
              >
                {d.fastest_lap_ms ? formatLapTime(d.fastest_lap_ms) : "---"}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function TimeTrialPanel({
  timeTrial,
  entries,
}: {
  timeTrial: TimeTrial | null;
  entries: TimeTrialEntry[];
}) {
  if (!timeTrial) {
    return (
      <div className="h-full flex flex-col items-center justify-center">
        <h2 className="text-4xl font-bold text-white mb-4">TIME TRIAL</h2>
        <p className="text-2xl text-neutral-500">No active time trial this week</p>
        <p className="text-lg text-neutral-600 mt-2">Check back soon for the next challenge!</p>
      </div>
    );
  }

  const displayEntries = entries.slice(0, 10);

  return (
    <div className="h-full flex flex-col">
      <div className="mb-6">
        <h2 className="text-4xl font-bold text-white mb-2">TIME TRIAL</h2>
        <div className="flex items-center gap-6 text-lg">
          <span className="text-neutral-400">
            Track: <span className="text-white font-semibold">{timeTrial.track}</span>
          </span>
          <span className="text-neutral-400">
            Car: <span className="text-white font-semibold">{timeTrial.car}</span>
          </span>
          <span className="text-neutral-600 text-sm">
            {timeTrial.week_start} &mdash; {timeTrial.week_end}
          </span>
        </div>
      </div>
      {displayEntries.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <p className="text-2xl text-neutral-500">No entries yet — be the first!</p>
        </div>
      ) : (
        <div className="flex-1 overflow-hidden">
          <div
            className="grid gap-4 pb-3 mb-3 text-sm uppercase tracking-wider text-neutral-500"
            style={{
              gridTemplateColumns: "60px 1fr 140px 180px",
              borderBottom: "1px solid #333333",
            }}
          >
            <span>#</span>
            <span>Driver</span>
            <span className="text-right">Attempts</span>
            <span className="text-right">Best Lap</span>
          </div>
          {displayEntries.map((e) => (
            <div
              key={`${e.driver}-${e.position}`}
              className="grid gap-4 py-3"
              style={{
                gridTemplateColumns: "60px 1fr 140px 180px",
                borderBottom: "1px solid #222222",
                background: e.position <= 3 ? "rgba(225, 6, 0, 0.05)" : "transparent",
              }}
            >
              <span
                className="text-3xl font-black"
                style={{ color: medalColor(e.position) }}
              >
                {e.position}
              </span>
              <span className="text-2xl text-white truncate self-center">{e.driver}</span>
              <span className="text-xl text-neutral-400 text-right self-center">
                {e.attempts}
              </span>
              <span
                className="text-2xl font-mono font-bold text-right self-center"
                style={{ color: "#4ade80" }}
              >
                {e.best_lap_display}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function PerTrackPanel({
  trackName,
  entries,
}: {
  trackName: string;
  entries: PublicTrackLeaderboardEntry[];
}) {
  if (!trackName) {
    return (
      <div className="h-full flex items-center justify-center">
        <p className="text-2xl text-neutral-500">No track data available</p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <h2 className="text-4xl font-bold text-white mb-2">TRACK RECORDS</h2>
      <p className="text-xl text-neutral-400 mb-6">{trackName}</p>
      {entries.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <p className="text-2xl text-neutral-500">No laps recorded on this track yet</p>
        </div>
      ) : (
        <div className="flex-1 overflow-hidden">
          <div
            className="grid gap-4 pb-3 mb-3 text-sm uppercase tracking-wider text-neutral-500"
            style={{
              gridTemplateColumns: "60px 1fr 1fr 180px",
              borderBottom: "1px solid #333333",
            }}
          >
            <span>#</span>
            <span>Driver</span>
            <span>Car</span>
            <span className="text-right">Best Lap</span>
          </div>
          {entries.map((e) => (
            <div
              key={`${e.driver}-${e.car}-${e.position}`}
              className="grid gap-4 py-4"
              style={{
                gridTemplateColumns: "60px 1fr 1fr 180px",
                borderBottom: "1px solid #222222",
                background: e.position <= 3 ? "rgba(225, 6, 0, 0.05)" : "transparent",
              }}
            >
              <span
                className="text-4xl font-black"
                style={{ color: medalColor(e.position) }}
              >
                {e.position}
              </span>
              <span className="text-2xl text-white truncate self-center">{e.driver}</span>
              <span className="text-lg text-neutral-400 truncate self-center">{e.car}</span>
              <span
                className="text-3xl font-mono font-bold text-right self-center"
                style={{ color: "#4ade80" }}
              >
                {e.best_lap_display}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
