"use client";

import { useEffect, useState, useRef } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { TelemetryFrame } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

const ERS_MODES = ["None", "Medium", "Hotlap", "Overtake"] as const;

function formatLapTime(ms: number | undefined): string {
  if (!ms || ms === 0) return "--:--.---";
  const minutes = Math.floor(ms / 60000);
  const seconds = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${minutes}:${String(seconds).padStart(2, "0")}.${String(millis).padStart(3, "0")}`;
}

function formatSector(ms: number | undefined): string {
  if (!ms || ms === 0) return "--.---";
  const seconds = Math.floor(ms / 1000);
  const millis = ms % 1000;
  return `${seconds}.${String(millis).padStart(3, "0")}`;
}

export default function TelemetryPage() {
  const router = useRouter();
  const [frame, setFrame] = useState<TelemetryFrame | null>(null);
  const [loading, setLoading] = useState(true);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }

    const poll = async () => {
      try {
        const res = await api.telemetry();
        if (res.frame) setFrame(res.frame);
      } catch {
        // Silently retry on next poll
      }
      setLoading(false);
    };

    poll();
    intervalRef.current = setInterval(poll, 500);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [router]);

  if (loading) {
    return (
      <div className="min-h-screen pb-20 flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  const t = frame;
  const isF1 = t?.drs_active !== undefined;

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-white mb-1">Live Telemetry</h1>
        {t ? (
          <p className="text-sm text-rp-grey mb-6">
            <span className="text-rp-red font-semibold">{t.driver_name || "Unknown"}</span>
            {" "}&mdash; {t.car} @ {t.track}
          </p>
        ) : (
          <p className="text-sm text-rp-grey mb-6">No active session</p>
        )}

        {!t ? (
          <div className="bg-rp-card border border-rp-border rounded-xl p-8 text-center text-rp-grey text-sm">
            No telemetry data available. Start a session to see live data here.
          </div>
        ) : (
          <div className="space-y-3">
            {/* Speed + Gear */}
            <div className="grid grid-cols-2 gap-3">
              <div className="bg-rp-card border border-rp-border rounded-xl p-5 text-center">
                <div className="text-4xl font-mono font-bold text-white">
                  {Math.round(t.speed_kmh)}
                </div>
                <div className="text-[10px] text-rp-grey mt-1 uppercase">km/h</div>
              </div>
              <div className="bg-rp-card border border-rp-border rounded-xl p-5 text-center">
                <div className="text-4xl font-mono font-bold text-white">
                  {t.gear === 0 ? "N" : t.gear === -1 ? "R" : t.gear}
                </div>
                <div className="text-[10px] text-rp-grey mt-1 uppercase">Gear</div>
              </div>
            </div>

            {/* RPM Bar */}
            <div className="bg-rp-card border border-rp-border rounded-xl p-4">
              <div className="flex items-baseline justify-between mb-2">
                <span className="text-xs text-rp-grey uppercase">RPM</span>
                <span className="text-lg font-mono font-bold text-white">
                  {(t.rpm / 1000).toFixed(1)}k
                </span>
              </div>
              <RpmBar rpm={t.rpm} maxRpm={isF1 ? 15000 : 9000} />
            </div>

            {/* DRS + ERS (F1 only) */}
            {isF1 && (
              <div className="grid grid-cols-2 gap-3">
                {/* DRS */}
                <div
                  className={`rounded-xl p-4 text-center border ${
                    t.drs_active
                      ? "bg-emerald-500/20 border-emerald-500/50"
                      : t.drs_available
                        ? "bg-yellow-500/15 border-yellow-500/40"
                        : "bg-rp-card border-rp-border"
                  }`}
                >
                  <div
                    className={`text-lg font-bold ${
                      t.drs_active
                        ? "text-emerald-400"
                        : t.drs_available
                          ? "text-yellow-400"
                          : "text-rp-grey"
                    }`}
                  >
                    DRS
                  </div>
                  <div
                    className={`text-[10px] uppercase ${
                      t.drs_active
                        ? "text-emerald-400"
                        : t.drs_available
                          ? "text-yellow-400"
                          : "text-rp-grey"
                    }`}
                  >
                    {t.drs_active ? "Active" : t.drs_available ? "Available" : "Off"}
                  </div>
                </div>

                {/* ERS */}
                <div className="bg-rp-card border border-rp-border rounded-xl p-4">
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-[10px] text-rp-grey uppercase">ERS</span>
                    <span
                      className={`text-xs font-bold ${
                        t.ers_deploy_mode === 3
                          ? "text-rp-red"
                          : t.ers_deploy_mode === 2
                            ? "text-purple-400"
                            : t.ers_deploy_mode === 1
                              ? "text-blue-400"
                              : "text-rp-grey"
                      }`}
                    >
                      {ERS_MODES[Math.min(t.ers_deploy_mode ?? 0, 3)]}
                    </span>
                  </div>
                  <div className="h-2.5 bg-neutral-800 rounded-full overflow-hidden">
                    <div
                      className={`h-full rounded-full transition-all duration-150 ${
                        t.ers_deploy_mode === 3
                          ? "bg-rp-red"
                          : t.ers_deploy_mode === 2
                            ? "bg-purple-500"
                            : t.ers_deploy_mode === 1
                              ? "bg-blue-500"
                              : "bg-neutral-600"
                      }`}
                      style={{ width: `${Math.round(t.ers_store_percent ?? 0)}%` }}
                    />
                  </div>
                </div>
              </div>
            )}

            {/* Sector Times */}
            <div className="grid grid-cols-3 gap-2">
              {[
                { label: "S1", ms: t.sector1_ms, idx: 0 },
                { label: "S2", ms: t.sector2_ms, idx: 1 },
                { label: "S3", ms: t.sector3_ms, idx: 2 },
              ].map((s) => {
                const isActive = t.sector === s.idx;
                return (
                  <div
                    key={s.label}
                    className={`rounded-xl p-3 text-center border ${
                      isActive
                        ? "bg-rp-red/10 border-rp-red/50"
                        : "bg-rp-card border-rp-border"
                    }`}
                  >
                    <div
                      className={`text-[10px] uppercase ${
                        isActive ? "text-rp-red" : "text-rp-grey"
                      }`}
                    >
                      {s.label}
                    </div>
                    <div
                      className={`text-sm font-mono font-bold ${
                        isActive ? "text-rp-red" : s.ms ? "text-white" : "text-rp-grey"
                      }`}
                    >
                      {formatSector(s.ms)}
                    </div>
                  </div>
                );
              })}
            </div>

            {/* Lap Timer + Best Lap */}
            <div className="grid grid-cols-2 gap-3">
              <div className="bg-rp-card border border-rp-border rounded-xl p-4">
                <div className="flex items-center justify-between mb-1">
                  <span className="text-[10px] text-rp-grey uppercase">Current</span>
                  <span className="text-[10px] text-rp-grey">L{t.lap_number}</span>
                </div>
                <div
                  className={`text-xl font-mono font-bold ${
                    t.current_lap_invalid ? "text-red-400 line-through" : "text-white"
                  }`}
                >
                  {formatLapTime(t.lap_time_ms)}
                </div>
              </div>
              <div className="bg-rp-card border border-rp-border rounded-xl p-4">
                <div className="flex items-center justify-between mb-1">
                  <span className="text-[10px] text-rp-grey uppercase">Best</span>
                  {t.best_lap_ms && (
                    <span className="text-[8px] font-bold bg-purple-500/20 text-purple-400 px-1.5 py-0.5 rounded">
                      PB
                    </span>
                  )}
                </div>
                <div className="text-xl font-mono font-bold text-purple-400">
                  {formatLapTime(t.best_lap_ms)}
                </div>
              </div>
            </div>

            {/* Throttle + Brake */}
            <div className="grid grid-cols-2 gap-3">
              <div className="bg-rp-card border border-rp-border rounded-xl p-3">
                <div className="flex items-center justify-between mb-1.5">
                  <span className="text-[10px] text-rp-grey uppercase">Throttle</span>
                  <span className="text-xs font-mono text-emerald-400">
                    {Math.round(t.throttle * 100)}%
                  </span>
                </div>
                <div className="h-2.5 bg-neutral-800 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-emerald-500 rounded-full transition-all duration-75"
                    style={{ width: `${t.throttle * 100}%` }}
                  />
                </div>
              </div>
              <div className="bg-rp-card border border-rp-border rounded-xl p-3">
                <div className="flex items-center justify-between mb-1.5">
                  <span className="text-[10px] text-rp-grey uppercase">Brake</span>
                  <span className="text-xs font-mono text-red-400">
                    {Math.round(t.brake * 100)}%
                  </span>
                </div>
                <div className="h-2.5 bg-neutral-800 rounded-full overflow-hidden">
                  <div
                    className="h-full bg-red-500 rounded-full transition-all duration-75"
                    style={{ width: `${t.brake * 100}%` }}
                  />
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
      <BottomNav />
    </div>
  );
}

function RpmBar({ rpm, maxRpm }: { rpm: number; maxRpm: number }) {
  const percent = Math.min((rpm / maxRpm) * 100, 100);
  const redZone = 85;

  return (
    <div className="h-3 bg-neutral-800 rounded-full overflow-hidden relative">
      <div
        className="absolute top-0 bottom-0 bg-red-900/30 rounded-r-full"
        style={{ left: `${redZone}%`, right: 0 }}
      />
      <div
        className={`h-full rounded-full transition-all duration-75 ${
          percent >= redZone ? "bg-red-500" : "bg-blue-500"
        }`}
        style={{ width: `${percent}%` }}
      />
    </div>
  );
}
