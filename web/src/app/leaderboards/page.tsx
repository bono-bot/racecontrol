"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type {
  PublicTrackRecord,
  PublicTrackInfo,
  PublicTopDriver,
  PublicTrackLeaderboardEntry,
} from "@/lib/api";

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

  // Filters
  const [simType, setSimType] = useState("assetto_corsa");
  const [showInvalid, setShowInvalid] = useState(false);
  const [carFilter, setCarFilter] = useState("");
  const [availableCars, setAvailableCars] = useState<string[]>([]);

  // Tab
  const [tab, setTab] = useState<"records" | "drivers" | "tracks">("records");

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
              <div className="grid grid-cols-[50px_1fr_1fr_120px_140px] gap-2 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                <span>#</span>
                <span>Driver</span>
                <span>Car</span>
                <span className="text-right">Best Lap</span>
                <span className="text-right">Date</span>
              </div>
              {trackEntries.map((entry) => (
                <div
                  key={`${entry.driver}-${entry.car}-${entry.position}`}
                  className={`grid grid-cols-[50px_1fr_1fr_120px_140px] gap-2 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 ${
                    entry.position <= 3 ? "bg-rp-red/5" : ""
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
                </div>
              ))}
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
                  className="grid grid-cols-[1fr_1fr_1fr_120px] gap-2 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 cursor-pointer hover:bg-white/5"
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
