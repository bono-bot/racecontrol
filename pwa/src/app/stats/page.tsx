"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { CustomerStats, LapRecord } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

function formatLapTime(ms: number): string {
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis
    .toString()
    .padStart(3, "0")}`;
}

function formatHours(seconds: number): string {
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

export default function StatsPage() {
  const router = useRouter();
  const [stats, setStats] = useState<CustomerStats | null>(null);
  const [recentLaps, setRecentLaps] = useState<LapRecord[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }

    Promise.all([api.stats(), api.laps()]).then(([sRes, lRes]) => {
      if (sRes.stats) setStats(sRes.stats);
      if (lRes.laps) setRecentLaps(lRes.laps.slice(0, 20));
      setLoading(false);
    });
  }, [router]);

  if (loading) {
    return (
      <div className="min-h-screen pb-20 flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-rp-orange border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-zinc-100 mb-6">Stats</h1>

        {stats && (
          <>
            {/* Stats grid */}
            <div className="grid grid-cols-2 gap-3 mb-8">
              <StatCard
                label="Total Sessions"
                value={stats.total_sessions.toString()}
                icon="🏁"
              />
              <StatCard
                label="Total Laps"
                value={stats.total_laps.toString()}
                icon="🔄"
              />
              <StatCard
                label="Drive Time"
                value={formatHours(stats.total_driving_seconds)}
                icon="⏱"
              />
              <StatCard
                label="Personal Bests"
                value={stats.personal_bests.toString()}
                icon="⚡"
              />
            </div>

            {stats.favourite_car && (
              <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-8">
                <p className="text-xs text-zinc-500 mb-1">Most Driven Car</p>
                <p className="text-lg font-semibold text-zinc-100">
                  {stats.favourite_car}
                </p>
              </div>
            )}
          </>
        )}

        {/* Recent laps */}
        <h2 className="text-sm font-medium text-zinc-500 mb-3">Recent Laps</h2>
        {recentLaps.length === 0 ? (
          <p className="text-zinc-600 text-sm">No laps recorded yet</p>
        ) : (
          <div className="space-y-2">
            {recentLaps.map((lap) => (
              <div
                key={lap.id}
                className="bg-rp-card border border-rp-border rounded-xl p-3 flex items-center justify-between"
              >
                <div>
                  <p className="text-sm font-medium text-zinc-200">
                    {lap.track}
                  </p>
                  <p className="text-xs text-zinc-500">{lap.car}</p>
                </div>
                <div className="text-right">
                  <p
                    className={`text-sm font-mono font-medium ${
                      lap.valid ? "text-zinc-100" : "text-red-400 line-through"
                    }`}
                  >
                    {formatLapTime(lap.lap_time_ms)}
                  </p>
                  <p className="text-[10px] text-zinc-600">
                    {new Date(lap.created_at).toLocaleDateString("en-IN", {
                      day: "numeric",
                      month: "short",
                    })}
                  </p>
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

function StatCard({
  label,
  value,
  icon,
}: {
  label: string;
  value: string;
  icon: string;
}) {
  return (
    <div className="bg-rp-card border border-rp-border rounded-xl p-4">
      <div className="flex items-center gap-2 mb-2">
        <span className="text-lg">{icon}</span>
        <span className="text-xs text-zinc-500">{label}</span>
      </div>
      <p className="text-2xl font-bold text-zinc-100">{value}</p>
    </div>
  );
}
