"use client";

import { useEffect, useState } from "react";
import { publicApi } from "@/lib/api";

function formatLapTime(ms: number): string {
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis.toString().padStart(3, "0")}`;
}

interface TrackRecord {
  track: string;
  car: string;
  driver: string;
  best_lap_ms: number;
  best_lap_display: string;
  achieved_at: string;
}

interface TrackInfo {
  name: string;
  total_laps: number;
}

interface TopDriver {
  position: number;
  name: string;
  total_laps: number;
  fastest_lap_ms: number | null;
}

interface TimeTrial {
  id: string;
  track: string;
  car: string;
  week_start: string;
  week_end: string;
}

interface TrackLeaderboardEntry {
  position: number;
  driver: string;
  car: string;
  best_lap_ms: number;
  best_lap_display: string;
  achieved_at: string;
}

export default function PublicLeaderboardPage() {
  const [records, setRecords] = useState<TrackRecord[]>([]);
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [topDrivers, setTopDrivers] = useState<TopDriver[]>([]);
  const [timeTrial, setTimeTrial] = useState<TimeTrial | null>(null);
  const [selectedTrack, setSelectedTrack] = useState<string | null>(null);
  const [trackLeaderboard, setTrackLeaderboard] = useState<TrackLeaderboardEntry[]>([]);
  const [trackStats, setTrackStats] = useState<{ total_laps: number; unique_drivers: number; unique_cars: number } | null>(null);
  const [tab, setTab] = useState<"records" | "drivers" | "tracks">("records");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    publicApi.leaderboard().then((data: { records?: TrackRecord[]; tracks?: TrackInfo[]; top_drivers?: TopDriver[]; time_trial?: TimeTrial | null }) => {
      setRecords(data.records || []);
      setTracks(data.tracks || []);
      setTopDrivers(data.top_drivers || []);
      setTimeTrial(data.time_trial || null);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  const loadTrackLeaderboard = (track: string) => {
    setSelectedTrack(track);
    publicApi.trackLeaderboard(track).then((data: { leaderboard?: TrackLeaderboardEntry[]; stats?: { total_laps: number; unique_drivers: number; unique_cars: number } }) => {
      setTrackLeaderboard(data.leaderboard || []);
      setTrackStats(data.stats || null);
    });
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-rp-dark">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-rp-dark">
      {/* Header */}
      <div className="bg-gradient-to-b from-rp-red/20 to-transparent pt-12 pb-8 px-4">
        <div className="max-w-2xl mx-auto text-center">
          <h1 className="text-3xl font-bold text-white tracking-tight">Leaderboard</h1>
          <p className="text-rp-grey text-sm mt-1">May the Fastest Win.</p>
        </div>
      </div>

      <div className="max-w-2xl mx-auto px-4 pb-8">
        {/* Time Trial Banner */}
        {timeTrial && (
          <div
            className="bg-rp-card border border-rp-red/30 rounded-xl p-4 mb-6 cursor-pointer hover:border-rp-red/60 transition-colors"
            onClick={() => loadTrackLeaderboard(timeTrial.track)}
          >
            <div className="flex items-center gap-2 mb-1">
              <span className="text-rp-red text-xs font-semibold uppercase tracking-wider">Weekly Time Trial</span>
              <span className="bg-rp-red/20 text-rp-red text-[10px] px-1.5 py-0.5 rounded-full font-medium">LIVE</span>
            </div>
            <p className="text-white font-bold">{timeTrial.track}</p>
            <p className="text-rp-grey text-xs">{timeTrial.car}</p>
          </div>
        )}

        {/* Tab switcher */}
        <div className="flex gap-1 bg-rp-card rounded-lg p-1 mb-6">
          {(["records", "drivers", "tracks"] as const).map((t) => (
            <button
              key={t}
              onClick={() => { setTab(t); setSelectedTrack(null); }}
              className={`flex-1 text-sm py-2 rounded-md transition-colors capitalize ${
                tab === t ? "bg-rp-red text-white font-medium" : "text-rp-grey hover:text-white"
              }`}
            >
              {t}
            </button>
          ))}
        </div>

        {/* Track Leaderboard Overlay */}
        {selectedTrack && (
          <div className="mb-6">
            <button
              onClick={() => setSelectedTrack(null)}
              className="text-rp-red text-sm mb-3 flex items-center gap-1"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-4 h-4">
                <path d="M19 12H5M12 19l-7-7 7-7" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              Back
            </button>

            <h2 className="text-xl font-bold text-white mb-1">{selectedTrack}</h2>
            {trackStats && (
              <div className="flex gap-4 text-xs text-rp-grey mb-4">
                <span>{trackStats.total_laps} laps</span>
                <span>{trackStats.unique_drivers} drivers</span>
                <span>{trackStats.unique_cars} cars</span>
              </div>
            )}

            <div className="bg-rp-card border border-rp-border rounded-xl overflow-hidden">
              <div className="grid grid-cols-[40px_1fr_1fr_90px] gap-1 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                <span>#</span>
                <span>Driver</span>
                <span>Car</span>
                <span className="text-right">Best Lap</span>
              </div>
              {trackLeaderboard.map((entry) => (
                <div
                  key={`${entry.driver}-${entry.car}`}
                  className={`grid grid-cols-[40px_1fr_1fr_90px] gap-1 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 ${
                    entry.position <= 3 ? "bg-rp-red/5" : ""
                  }`}
                >
                  <span className={`text-sm font-bold ${entry.position <= 3 ? "text-rp-red" : "text-neutral-500"}`}>
                    {entry.position}
                  </span>
                  <span className="text-sm text-white truncate">{entry.driver}</span>
                  <span className="text-xs text-rp-grey truncate">{entry.car}</span>
                  <span className="text-sm font-mono text-white text-right">{entry.best_lap_display}</span>
                </div>
              ))}
              {trackLeaderboard.length === 0 && (
                <p className="text-rp-grey text-sm text-center py-6">No laps recorded yet</p>
              )}
            </div>
          </div>
        )}

        {/* Records Tab */}
        {!selectedTrack && tab === "records" && (
          <div className="bg-rp-card border border-rp-border rounded-xl overflow-hidden">
            <div className="px-4 py-3 border-b border-rp-border">
              <h2 className="text-sm font-medium text-rp-grey">Track Records</h2>
            </div>
            {records.map((r, i) => (
              <div
                key={`${r.track}-${r.car}`}
                className="px-4 py-3 border-b border-rp-border/50 last:border-b-0 cursor-pointer hover:bg-white/5"
                onClick={() => loadTrackLeaderboard(r.track)}
              >
                <div className="flex justify-between items-start">
                  <div className="flex-1 min-w-0">
                    <p className="text-white font-medium text-sm truncate">{r.track}</p>
                    <p className="text-rp-grey text-xs truncate">{r.car}</p>
                  </div>
                  <div className="text-right ml-3">
                    <p className="text-white font-mono text-sm font-bold">{r.best_lap_display}</p>
                    <p className="text-rp-grey text-xs">{r.driver}</p>
                  </div>
                </div>
              </div>
            ))}
            {records.length === 0 && (
              <p className="text-rp-grey text-sm text-center py-8">No records yet. Be the first!</p>
            )}
          </div>
        )}

        {/* Drivers Tab */}
        {!selectedTrack && tab === "drivers" && (
          <div className="bg-rp-card border border-rp-border rounded-xl overflow-hidden">
            <div className="px-4 py-3 border-b border-rp-border">
              <h2 className="text-sm font-medium text-rp-grey">Top Drivers</h2>
            </div>
            {topDrivers.map((d) => (
              <div
                key={d.name}
                className={`flex items-center gap-3 px-4 py-3 border-b border-rp-border/50 last:border-b-0 ${
                  d.position <= 3 ? "bg-rp-red/5" : ""
                }`}
              >
                <span className={`w-8 text-center font-bold text-lg ${
                  d.position === 1 ? "text-yellow-400" :
                  d.position === 2 ? "text-neutral-300" :
                  d.position === 3 ? "text-amber-600" :
                  "text-neutral-500"
                }`}>
                  {d.position}
                </span>
                <div className="flex-1 min-w-0">
                  <p className="text-white font-medium text-sm truncate">{d.name}</p>
                  <p className="text-rp-grey text-xs">{d.total_laps} laps</p>
                </div>
                {d.fastest_lap_ms && (
                  <span className="text-xs font-mono text-rp-grey">
                    {formatLapTime(d.fastest_lap_ms)}
                  </span>
                )}
              </div>
            ))}
            {topDrivers.length === 0 && (
              <p className="text-rp-grey text-sm text-center py-8">No drivers yet</p>
            )}
          </div>
        )}

        {/* Tracks Tab */}
        {!selectedTrack && tab === "tracks" && (
          <div className="space-y-2">
            {tracks.map((t) => (
              <div
                key={t.name}
                className="bg-rp-card border border-rp-border rounded-xl px-4 py-3 cursor-pointer hover:border-rp-red/30 transition-colors"
                onClick={() => loadTrackLeaderboard(t.name)}
              >
                <div className="flex justify-between items-center">
                  <span className="text-white font-medium text-sm">{t.name}</span>
                  <span className="text-rp-grey text-xs">{t.total_laps} laps</span>
                </div>
              </div>
            ))}
            {tracks.length === 0 && (
              <p className="text-rp-grey text-sm text-center py-8">No tracks with lap data yet</p>
            )}
          </div>
        )}

        {/* Footer */}
        <div className="text-center mt-8">
          <p className="text-rp-grey text-xs">RacingPoint</p>
        </div>
      </div>
    </div>
  );
}
