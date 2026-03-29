"use client";

import { useEffect, useState, useRef, useCallback } from "react";
import dynamic from "next/dynamic";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type {
  PublicTrackRecord,
  PublicTrackInfo,
  PublicTopDriver,
  PublicTrackLeaderboardEntry,
} from "@/lib/api";

const WS_BASE = process.env.NEXT_PUBLIC_WS_URL || "ws://localhost:8080/ws/dashboard";
const WS_TOKEN = process.env.NEXT_PUBLIC_WS_TOKEN || "";
const WS_URL = WS_TOKEN ? `${WS_BASE}?token=${WS_TOKEN}` : WS_BASE;

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

const TelemetryChart = dynamic(() => import("@/components/TelemetryChart"), {
  ssr: false,
  loading: () => (
    <div className="bg-rp-card border border-rp-border rounded-lg p-6 flex items-center justify-center gap-3">
      <div className="w-5 h-5 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      <span className="text-rp-grey text-sm">Loading chart...</span>
    </div>
  ),
});

function formatLapTime(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  if (minutes > 0) {
    return `${minutes}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
  }
  return `${seconds}.${String(millis).padStart(3, "0")}`;
}

// All games — must match SimType enum in rc-common/types.rs
const SIM_TYPES = [
  { value: "assetto_corsa", label: "Assetto Corsa" },
  { value: "assetto_corsa_evo", label: "AC EVO" },
  { value: "assetto_corsa_rally", label: "EA WRC" },
  { value: "iracing", label: "iRacing" },
  { value: "f1_25", label: "F1 25" },
  { value: "le_mans_ultimate", label: "Le Mans Ultimate" },
  { value: "forza", label: "Forza Motorsport" },
  { value: "forza_horizon_5", label: "Forza Horizon 5" },
] as const;

export default function LeaderboardsPage() {
  const [records, setRecords] = useState<PublicTrackRecord[]>([]);
  const [tracks, setTracks] = useState<PublicTrackInfo[]>([]);
  const [topDrivers, setTopDrivers] = useState<PublicTopDriver[]>([]);
  const [loading, setLoading] = useState(true);

  // Track drill-down state
  const [selectedTrack, setSelectedTrack] = useState<string | null>(null);
  const [trackEntries, setTrackEntries] = useState<PublicTrackLeaderboardEntry[]>([]);
  const [trackStats, setTrackStats] = useState<{ total_laps: number; unique_drivers: number; unique_cars: number } | null>(null);
  const [loadingTrack, setLoadingTrack] = useState(false);

  // Telemetry expansion
  const [expandedLapId, setExpandedLapId] = useState<string | null>(null);

  // Filters
  const [simType, setSimType] = useState("assetto_corsa");
  const [showInvalid, setShowInvalid] = useState(false);
  const [carFilter, setCarFilter] = useState("");
  const [availableCars, setAvailableCars] = useState<string[]>([]);

  // Tab
  const [tab, setTab] = useState<"records" | "drivers" | "tracks">("records");

  // Phase 254: Track which record was just broken for highlight animation
  const [highlightKey, setHighlightKey] = useState<string | null>(null);
  const highlightTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Phase 254: Reload functions for WS-triggered refresh
  const reloadOverview = useCallback(() => {
    api.publicLeaderboard().then((data) => {
      setRecords(data.records || []);
      setTracks(data.tracks || []);
      setTopDrivers(data.top_drivers || []);
    }).catch(() => { /* ignore */ });
  }, []);

  const selectedTrackRef = useRef(selectedTrack);
  selectedTrackRef.current = selectedTrack;
  const simTypeRef = useRef(simType);
  simTypeRef.current = simType;
  const showInvalidRef = useRef(showInvalid);
  showInvalidRef.current = showInvalid;

  const reloadTrack = useCallback(() => {
    const track = selectedTrackRef.current;
    if (!track) return;
    api.publicTrackLeaderboard(track, { sim_type: simTypeRef.current, show_invalid: showInvalidRef.current }).then((data) => {
      setTrackEntries(data.leaderboard || []);
      setTrackStats(data.stats || null);
      setAvailableCars(Array.from(new Set((data.leaderboard || []).map((e: PublicTrackLeaderboardEntry) => e.car))).sort());
    }).catch(() => { /* ignore */ });
  }, []);

  // Phase 254: WebSocket subscription for real-time leaderboard updates
  useEffect(() => {
    const socket = new WebSocket(WS_URL);
    socket.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as { event: string; data: unknown };
        if (msg.event === "record_broken") {
          const data = msg.data as RecordBrokenEvent;
          // Set highlight key for CSS animation
          const key = `${data.track}-${data.car}-${data.driver_name}`;
          setHighlightKey(key);
          if (highlightTimer.current) clearTimeout(highlightTimer.current);
          highlightTimer.current = setTimeout(() => setHighlightKey(null), 5000);

          // Refresh data
          reloadOverview();
          reloadTrack();
        }
      } catch {
        // ignore parse errors
      }
    };
    socket.onclose = () => {
      // Reconnection handled by browser or could add retry
    };
    return () => {
      socket.close();
      if (highlightTimer.current) clearTimeout(highlightTimer.current);
    };
  }, [reloadOverview, reloadTrack]);

  // Load overview
  useEffect(() => {
    api.publicLeaderboard().then((data) => {
      setRecords(data.records || []);
      setTracks(data.tracks || []);
      setTopDrivers(data.top_drivers || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  // Load track leaderboard
  const loadTrack = (track: string) => {
    setSelectedTrack(track);
    setCarFilter("");
    setLoadingTrack(true);
    api.publicTrackLeaderboard(track, { sim_type: simType, show_invalid: showInvalid }).then((data) => {
      setTrackEntries(data.leaderboard || []);
      setTrackStats(data.stats || null);
      setAvailableCars(Array.from(new Set((data.leaderboard || []).map((e) => e.car))).sort());
      setLoadingTrack(false);
    }).catch(() => setLoadingTrack(false));
  };

  // Re-fetch when sim_type or show_invalid changes
  useEffect(() => {
    if (!selectedTrack) return;
    setCarFilter("");
    setLoadingTrack(true);
    api.publicTrackLeaderboard(selectedTrack, { sim_type: simType, show_invalid: showInvalid }).then((data) => {
      setTrackEntries(data.leaderboard || []);
      setTrackStats(data.stats || null);
      setAvailableCars(Array.from(new Set((data.leaderboard || []).map((e) => e.car))).sort());
      setLoadingTrack(false);
    }).catch(() => setLoadingTrack(false));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [simType, showInvalid]);

  // Re-fetch when car filter changes
  useEffect(() => {
    if (!selectedTrack || !carFilter) return;
    setLoadingTrack(true);
    api.publicTrackLeaderboard(selectedTrack, { sim_type: simType, show_invalid: showInvalid, car: carFilter }).then((data) => {
      setTrackEntries(data.leaderboard || []);
      setTrackStats(data.stats || null);
      setLoadingTrack(false);
    }).catch(() => setLoadingTrack(false));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [carFilter]);

  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white">Leaderboards</h1>
        <p className="text-sm text-rp-grey">Track records, top drivers, and per-track rankings</p>
      </div>

      {/* Filters */}
      <div className="flex flex-wrap items-center gap-3 mb-6">
        <select
          value={simType}
          onChange={(e) => setSimType(e.target.value)}
          className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-white focus:border-rp-red focus:outline-none"
        >
          {SIM_TYPES.map((st) => (
            <option key={st.value} value={st.value}>{st.label}</option>
          ))}
        </select>

        <label className="flex items-center gap-2 text-sm text-neutral-400 cursor-pointer select-none">
          <input
            type="checkbox"
            checked={showInvalid}
            onChange={(e) => setShowInvalid(e.target.checked)}
            className="w-4 h-4 rounded border-rp-border bg-rp-card accent-rp-red"
          />
          Show Invalid
        </label>

        {selectedTrack && availableCars.length > 1 && (
          <select
            value={carFilter}
            onChange={(e) => setCarFilter(e.target.value)}
            className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-white focus:border-rp-red focus:outline-none"
          >
            <option value="">All Cars</option>
            {availableCars.map((c) => (
              <option key={c} value={c}>{c}</option>
            ))}
          </select>
        )}
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading leaderboards...</div>
      ) : selectedTrack ? (
        /* ── Track Drill-Down ── */
        <div>
          <button
            onClick={() => setSelectedTrack(null)}
            className="text-rp-red text-sm mb-4 hover:underline"
          >
            &larr; Back to overview
          </button>

          <div className="flex items-baseline gap-4 mb-4">
            <h2 className="text-xl font-bold text-white">{selectedTrack}</h2>
            {trackStats && (
              <div className="flex gap-4 text-xs text-rp-grey">
                <span>{trackStats.total_laps} laps</span>
                <span>{trackStats.unique_drivers} drivers</span>
                <span>{trackStats.unique_cars} cars</span>
              </div>
            )}
          </div>

          {loadingTrack ? (
            <div className="text-center py-8 text-rp-grey text-sm">Loading...</div>
          ) : (
            <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
              <div className="grid grid-cols-[50px_1fr_1fr_120px_140px_36px] gap-2 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                <span>#</span>
                <span>Driver</span>
                <span>Car</span>
                <span className="text-right">Best Lap</span>
                <span className="text-right">Date</span>
                <span />
              </div>
              {trackEntries.map((entry) => {
                const isExpanded = expandedLapId === entry.lap_id;
                return (
                  <div key={`${entry.driver}-${entry.car}-${entry.position}`}>
                    <div
                      className={`grid grid-cols-[50px_1fr_1fr_120px_140px_36px] gap-2 px-4 py-2.5 border-b border-rp-border/50 transition-colors duration-500 ${
                        entry.position <= 3 ? "bg-rp-red/5" : ""
                      } ${isExpanded ? "border-b-0" : ""} ${
                        highlightKey && highlightKey.includes(entry.driver) && highlightKey.includes(entry.car) ? "animate-record-flash" : ""
                      }`}
                    >
                      <span className={`font-bold ${
                        entry.position === 1 ? "text-yellow-400" :
                        entry.position === 2 ? "text-neutral-300" :
                        entry.position === 3 ? "text-amber-600" :
                        "text-neutral-500"
                      }`}>
                        {entry.position}
                      </span>
                      <span className="text-sm text-white truncate">{entry.driver}</span>
                      <span className="text-xs text-rp-grey truncate self-center">{entry.car}</span>
                      <span className="text-sm font-mono text-emerald-400 text-right font-bold">{entry.best_lap_display}</span>
                      <span className="text-xs text-rp-grey text-right self-center">{entry.achieved_at?.slice(0, 10) || ""}</span>
                      <span className="flex items-center justify-center">
                        {entry.lap_id && (
                          <button
                            onClick={() => setExpandedLapId(isExpanded ? null : (entry.lap_id ?? null))}
                            className={`w-7 h-7 flex items-center justify-center rounded-md transition-colors ${
                              isExpanded
                                ? "bg-rp-red/20 text-rp-red"
                                : "text-rp-grey hover:text-white hover:bg-white/10"
                            }`}
                            title="View telemetry"
                          >
                            <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                              <polyline points="1,12 4,4 7,9 10,2 13,8 15,6" />
                            </svg>
                          </button>
                        )}
                      </span>
                    </div>
                    {isExpanded && entry.lap_id && (
                      <div className="px-4 pb-3 border-b border-rp-border/50">
                        <TelemetryChart
                          lapId={entry.lap_id}
                          onClose={() => setExpandedLapId(null)}
                        />
                      </div>
                    )}
                  </div>
                );
              })}
              {trackEntries.length === 0 && (
                <p className="text-rp-grey text-sm text-center py-6">No laps recorded yet</p>
              )}
            </div>
          )}
        </div>
      ) : (
        /* ── Overview Tabs ── */
        <>
          <div className="flex gap-1 bg-rp-card rounded-lg p-1 mb-6 max-w-md">
            {(["records", "drivers", "tracks"] as const).map((t) => (
              <button
                key={t}
                onClick={() => setTab(t)}
                className={`flex-1 text-sm py-2 rounded-md transition-colors capitalize ${
                  tab === t ? "bg-rp-red text-white font-medium" : "text-rp-grey hover:text-white"
                }`}
              >
                {t}
              </button>
            ))}
          </div>

          {/* Records Tab */}
          {tab === "records" && (
            <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
              <div className="grid grid-cols-[1fr_1fr_1fr_120px] gap-2 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                <span>Track</span>
                <span>Car</span>
                <span>Driver</span>
                <span className="text-right">Record</span>
              </div>
              {records.map((r) => (
                <div
                  key={`${r.track}-${r.car}`}
                  className={`grid grid-cols-[1fr_1fr_1fr_120px] gap-2 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 cursor-pointer hover:bg-white/5 transition-colors duration-500 ${
                    highlightKey === `${r.track}-${r.car}-${r.driver}` ? "animate-record-flash" : ""
                  }`}
                  onClick={() => loadTrack(r.track)}
                >
                  <span className="text-sm text-white truncate">{r.track}</span>
                  <span className="text-xs text-rp-grey truncate self-center">{r.car}</span>
                  <span className="text-xs text-neutral-400 truncate self-center">{r.driver}</span>
                  <span className="text-sm font-mono text-emerald-400 text-right font-bold">{r.best_lap_display}</span>
                </div>
              ))}
              {records.length === 0 && (
                <p className="text-rp-grey text-sm text-center py-8">No records yet</p>
              )}
            </div>
          )}

          {/* Drivers Tab */}
          {tab === "drivers" && (
            <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
              <div className="grid grid-cols-[50px_1fr_100px_120px] gap-2 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                <span>#</span>
                <span>Driver</span>
                <span className="text-right">Laps</span>
                <span className="text-right">Fastest</span>
              </div>
              {topDrivers.map((d) => (
                <div
                  key={d.name}
                  className={`grid grid-cols-[50px_1fr_100px_120px] gap-2 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 ${
                    d.position <= 3 ? "bg-rp-red/5" : ""
                  }`}
                >
                  <span className={`font-bold ${
                    d.position === 1 ? "text-yellow-400" :
                    d.position === 2 ? "text-neutral-300" :
                    d.position === 3 ? "text-amber-600" :
                    "text-neutral-500"
                  }`}>
                    {d.position}
                  </span>
                  <span className="text-sm text-white truncate">{d.name}</span>
                  <span className="text-xs text-rp-grey text-right self-center">{d.total_laps}</span>
                  <span className="text-sm font-mono text-emerald-400 text-right">
                    {d.fastest_lap_ms ? formatLapTime(d.fastest_lap_ms) : "—"}
                  </span>
                </div>
              ))}
              {topDrivers.length === 0 && (
                <p className="text-rp-grey text-sm text-center py-8">No drivers yet</p>
              )}
            </div>
          )}

          {/* Tracks Tab */}
          {tab === "tracks" && (
            <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
              <div className="grid grid-cols-[1fr_100px] gap-2 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                <span>Track</span>
                <span className="text-right">Laps</span>
              </div>
              {tracks.map((t) => (
                <div
                  key={t.name}
                  className="grid grid-cols-[1fr_100px] gap-2 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 cursor-pointer hover:bg-white/5"
                  onClick={() => loadTrack(t.name)}
                >
                  <span className="text-sm text-white">{t.name}</span>
                  <span className="text-xs text-rp-grey text-right self-center">{t.total_laps}</span>
                </div>
              ))}
              {tracks.length === 0 && (
                <p className="text-rp-grey text-sm text-center py-8">No tracks yet</p>
              )}
            </div>
          )}
        </>
      )}
    </DashboardLayout>
  );
}
