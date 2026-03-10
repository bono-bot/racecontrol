"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, publicApi, isLoggedIn } from "@/lib/api";
import BottomNav from "@/components/BottomNav";
import dynamic from "next/dynamic";

const TelemetryChart = dynamic(() => import("@/components/TelemetryChart"), {
  ssr: false,
  loading: () => (
    <div className="flex justify-center py-12">
      <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
    </div>
  ),
});

function formatLapTime(ms: number): string {
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis
    .toString()
    .padStart(3, "0")}`;
}

interface LeaderboardEntry {
  position: number;
  driver: string;
  car: string;
  best_lap_ms: number;
  is_personal_best?: boolean;
  is_track_record?: boolean;
  lap_id?: string | null;
}

interface TrackInfo {
  name: string;
  total_laps: number;
}

export default function LeaderboardPage() {
  const router = useRouter();
  const [tracks, setTracks] = useState<TrackInfo[]>([]);
  const [selectedTrack, setSelectedTrack] = useState<string | null>(null);
  const [entries, setEntries] = useState<LeaderboardEntry[]>([]);
  const [loadingTracks, setLoadingTracks] = useState(true);
  const [loadingEntries, setLoadingEntries] = useState(false);
  const [telemetryLapId, setTelemetryLapId] = useState<string | null>(null);

  // Load available tracks from public leaderboard
  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    publicApi.leaderboard().then((data: { tracks?: TrackInfo[] }) => {
      const t = data.tracks || [];
      setTracks(t);
      if (t.length > 0) setSelectedTrack(t[0].name);
      setLoadingTracks(false);
    }).catch(() => setLoadingTracks(false));
  }, [router]);

  // Load entries for selected track
  useEffect(() => {
    if (!selectedTrack) return;
    setLoadingEntries(true);
    setTelemetryLapId(null);
    api.leaderboard(selectedTrack).then((res) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const data = res as any;
      if (Array.isArray(data?.records)) {
        setEntries(data.records);
      } else {
        setEntries([]);
      }
      setLoadingEntries(false);
    }).catch(() => setLoadingEntries(false));
  }, [selectedTrack]);

  const loading = loadingTracks || loadingEntries;

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-white mb-4">Leaderboard</h1>

        {/* Track selector */}
        {tracks.length > 0 && (
          <div className="flex gap-2 overflow-x-auto pb-2 mb-6 -mx-4 px-4 no-scrollbar">
            {tracks.map((track) => (
              <button
                key={track.name}
                onClick={() => setSelectedTrack(track.name)}
                className={`px-4 py-2 rounded-full text-sm font-medium whitespace-nowrap transition-colors ${
                  selectedTrack === track.name
                    ? "bg-rp-red text-white"
                    : "bg-rp-card border border-rp-border text-neutral-400"
                }`}
              >
                {track.name}
              </button>
            ))}
          </div>
        )}

        {loading ? (
          <div className="flex justify-center py-12">
            <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
          </div>
        ) : tracks.length === 0 ? (
          <div className="text-center py-12">
            <p className="text-rp-grey">No tracks with lap data yet</p>
          </div>
        ) : entries.length === 0 ? (
          <div className="text-center py-12">
            <p className="text-rp-grey">No times recorded for this track</p>
          </div>
        ) : (
          <div className="space-y-2">
            {entries.map((entry, i) => (
              <button
                key={i}
                onClick={() => {
                  if (entry.lap_id) {
                    setTelemetryLapId(
                      telemetryLapId === entry.lap_id ? null : entry.lap_id
                    );
                  }
                }}
                className={`w-full text-left bg-rp-card border rounded-xl p-3 flex items-center gap-3 transition-colors ${
                  entry.is_track_record
                    ? "border-rp-red/50"
                    : telemetryLapId === entry.lap_id
                    ? "border-rp-red/30"
                    : "border-rp-border"
                } ${entry.lap_id ? "cursor-pointer active:bg-rp-border/20" : "cursor-default"}`}
              >
                <div
                  className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold flex-shrink-0 ${
                    i === 0
                      ? "bg-yellow-500/20 text-yellow-400"
                      : i === 1
                      ? "bg-neutral-400/20 text-neutral-300"
                      : i === 2
                      ? "bg-rp-red/20 text-rp-red"
                      : "bg-rp-card text-rp-grey"
                  }`}
                >
                  {entry.position}
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-neutral-200 truncate">
                    {entry.driver}
                  </p>
                  <p className="text-xs text-rp-grey truncate">{entry.car}</p>
                </div>
                <div className="text-right flex items-center gap-2">
                  <div>
                    <p className="text-sm font-mono font-medium text-white">
                      {formatLapTime(entry.best_lap_ms)}
                    </p>
                    {entry.is_track_record && (
                      <span className="text-[10px] text-rp-red font-medium">
                        RECORD
                      </span>
                    )}
                  </div>
                  {entry.lap_id && (
                    <svg
                      className={`w-4 h-4 text-rp-grey transition-transform ${
                        telemetryLapId === entry.lap_id ? "rotate-90" : ""
                      }`}
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth={2}
                    >
                      <path
                        d="M9 18l6-6-6-6"
                        strokeLinecap="round"
                        strokeLinejoin="round"
                      />
                    </svg>
                  )}
                </div>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Telemetry overlay */}
      {telemetryLapId && (
        <TelemetryChart
          lapId={telemetryLapId}
          onClose={() => setTelemetryLapId(null)}
        />
      )}

      <BottomNav />
    </div>
  );
}
