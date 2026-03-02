"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

const TRACKS = [
  "monza",
  "spa",
  "nurburgring",
  "imola",
  "suzuka",
  "mugello",
  "barcelona",
  "silverstone",
];

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
  driver_name: string;
  car: string;
  best_lap_ms: number;
  is_personal_best: boolean;
  is_track_record: boolean;
}

export default function LeaderboardPage() {
  const router = useRouter();
  const [selectedTrack, setSelectedTrack] = useState(TRACKS[0]);
  const [entries, setEntries] = useState<LeaderboardEntry[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }
    setLoading(true);
    api.leaderboard(selectedTrack).then((res) => {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const lb = res.leaderboard as any;
      if (lb?.entries) {
        setEntries(lb.entries);
      } else {
        setEntries([]);
      }
      setLoading(false);
    });
  }, [router, selectedTrack]);

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-zinc-100 mb-4">Leaderboard</h1>

        {/* Track selector */}
        <div className="flex gap-2 overflow-x-auto pb-2 mb-6 -mx-4 px-4 no-scrollbar">
          {TRACKS.map((track) => (
            <button
              key={track}
              onClick={() => setSelectedTrack(track)}
              className={`px-4 py-2 rounded-full text-sm font-medium whitespace-nowrap transition-colors ${
                selectedTrack === track
                  ? "bg-rp-orange text-white"
                  : "bg-rp-card border border-rp-border text-zinc-400"
              }`}
            >
              {track.charAt(0).toUpperCase() + track.slice(1).replace("_", " ")}
            </button>
          ))}
        </div>

        {loading ? (
          <div className="flex justify-center py-12">
            <div className="w-8 h-8 border-2 border-rp-orange border-t-transparent rounded-full animate-spin" />
          </div>
        ) : entries.length === 0 ? (
          <div className="text-center py-12">
            <p className="text-zinc-500">No times recorded for this track</p>
          </div>
        ) : (
          <div className="space-y-2">
            {entries.map((entry, i) => (
              <div
                key={i}
                className={`bg-rp-card border rounded-xl p-3 flex items-center gap-3 ${
                  entry.is_track_record
                    ? "border-rp-orange/50"
                    : "border-rp-border"
                }`}
              >
                <div
                  className={`w-8 h-8 rounded-full flex items-center justify-center text-sm font-bold ${
                    i === 0
                      ? "bg-yellow-500/20 text-yellow-400"
                      : i === 1
                      ? "bg-zinc-400/20 text-zinc-300"
                      : i === 2
                      ? "bg-orange-700/20 text-orange-400"
                      : "bg-zinc-800 text-zinc-500"
                  }`}
                >
                  {entry.position}
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-medium text-zinc-200 truncate">
                    {entry.driver_name}
                  </p>
                  <p className="text-xs text-zinc-500 truncate">{entry.car}</p>
                </div>
                <div className="text-right">
                  <p className="text-sm font-mono font-medium text-zinc-100">
                    {formatLapTime(entry.best_lap_ms)}
                  </p>
                  {entry.is_track_record && (
                    <span className="text-[10px] text-rp-orange font-medium">
                      RECORD
                    </span>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
      <BottomNav />
    </div>
  );
}
