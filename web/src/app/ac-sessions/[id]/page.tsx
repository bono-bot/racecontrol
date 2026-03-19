"use client";

import { useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type { AcSessionLeaderboardData, AcSessionLeaderboardEntry } from "@/lib/api";

function formatLapTime(ms: number): string {
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  if (minutes > 0) {
    return `${minutes}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
  }
  return `${seconds}.${String(millis).padStart(3, "0")}`;
}

function formatGap(ms: number): string {
  if (ms <= 0) return "+0.000";
  if (ms < 1000) return `+0.${String(ms).padStart(3, "0")}`;
  const secs = Math.floor(ms / 1000);
  const millis = ms % 1000;
  return `+${secs}.${String(millis).padStart(3, "0")}`;
}

function toUtcDate(ts: string): Date {
  return new Date(/[Z+]/.test(ts) ? ts : ts + "Z");
}

export default function AcSessionLeaderboardPage() {
  const params = useParams();
  const router = useRouter();
  const sessionId = params.id as string;

  const [data, setData] = useState<AcSessionLeaderboardData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.acSessionLeaderboard(sessionId)
      .then((res) => {
        if ((res as unknown as { error?: string }).error) {
          setError((res as unknown as { error: string }).error);
        } else {
          setData(res);
        }
      })
      .catch(() => setError("Failed to load leaderboard"))
      .finally(() => setLoading(false));
  }, [sessionId]);

  return (
    <DashboardLayout>
      {/* Header */}
      <div className="mb-6">
        <button
          onClick={() => router.push("/ac-sessions")}
          className="text-rp-red text-sm mb-3 hover:underline"
        >
          &larr; Back to AC Sessions
        </button>
        <h1 className="text-2xl font-bold text-white">Session Leaderboard</h1>
        {data && (
          <div className="flex flex-wrap items-center gap-4 mt-2">
            {data.track && (
              <span className="text-sm text-neutral-300 bg-rp-card border border-rp-border rounded-lg px-3 py-1">
                {data.track}
              </span>
            )}
            <span className={`text-xs font-medium px-2 py-0.5 rounded ${
              data.status === "running" ? "bg-emerald-500/20 text-emerald-400" :
              data.status === "stopped" ? "bg-neutral-500/20 text-neutral-400" :
              "bg-rp-red/20 text-rp-red"
            }`}>
              {data.status.toUpperCase()}
            </span>
            <span className="text-xs text-rp-grey">
              {data.pod_ids.length} pod{data.pod_ids.length !== 1 ? "s" : ""}
            </span>
            <span className="text-xs text-rp-grey">
              {data.total_laps} total lap{data.total_laps !== 1 ? "s" : ""}
            </span>
            {data.started_at && (
              <span className="text-xs text-rp-grey">
                {toUtcDate(data.started_at).toLocaleString("en-IN", {
                  timeZone: "Asia/Kolkata",
                  day: "numeric",
                  month: "short",
                  year: "numeric",
                  hour: "2-digit",
                  minute: "2-digit",
                })}
              </span>
            )}
          </div>
        )}
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading leaderboard...</div>
      ) : error ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-rp-red mb-2">Error</p>
          <p className="text-rp-grey text-sm">{error}</p>
        </div>
      ) : !data || data.leaderboard.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No laps recorded</p>
          <p className="text-rp-grey text-sm">
            No valid laps were completed during this session.
          </p>
        </div>
      ) : (
        <div className="overflow-x-auto">
          <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden min-w-[700px]">
            {/* Table header */}
            <div className="grid grid-cols-[50px_1fr_1fr_100px_100px_100px_100px_80px_80px] gap-2 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
              <span>#</span>
              <span>Driver</span>
              <span>Car</span>
              <span className="text-right">Best Lap</span>
              <span className="text-right">Gap</span>
              <span className="text-right">S1</span>
              <span className="text-right">S2</span>
              <span className="text-right">S3</span>
              <span className="text-right">Laps</span>
            </div>

            {/* Rows */}
            {data.leaderboard.map((entry: AcSessionLeaderboardEntry) => (
              <div
                key={entry.driver_id}
                className={`grid grid-cols-[50px_1fr_1fr_100px_100px_100px_100px_80px_80px] gap-2 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 ${
                  entry.position <= 3 ? "bg-rp-red/5" : ""
                }`}
              >
                {/* Position */}
                <span className={`font-bold ${
                  entry.position === 1 ? "text-yellow-400" :
                  entry.position === 2 ? "text-neutral-300" :
                  entry.position === 3 ? "text-amber-600" :
                  "text-neutral-500"
                }`}>
                  {entry.position}
                </span>

                {/* Driver */}
                <span className="text-sm text-white truncate">{entry.driver}</span>

                {/* Car */}
                <span className="text-xs text-rp-grey truncate self-center">{entry.car}</span>

                {/* Best Lap */}
                <span className="text-sm font-mono text-emerald-400 text-right font-bold">
                  {formatLapTime(entry.best_lap_ms)}
                </span>

                {/* Gap */}
                <span className="text-xs font-mono text-neutral-400 text-right self-center">
                  {entry.gap_ms != null ? formatGap(entry.gap_ms) : "—"}
                </span>

                {/* Sectors */}
                <span className="text-xs font-mono text-neutral-400 text-right self-center">
                  {entry.sector1_ms != null ? formatLapTime(entry.sector1_ms) : "—"}
                </span>
                <span className="text-xs font-mono text-neutral-400 text-right self-center">
                  {entry.sector2_ms != null ? formatLapTime(entry.sector2_ms) : "—"}
                </span>
                <span className="text-xs font-mono text-neutral-400 text-right self-center">
                  {entry.sector3_ms != null ? formatLapTime(entry.sector3_ms) : "—"}
                </span>

                {/* Lap count */}
                <span className="text-xs text-rp-grey text-right self-center">
                  {entry.lap_count}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </DashboardLayout>
  );
}
