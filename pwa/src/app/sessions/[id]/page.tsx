"use client";

import { useEffect, useState } from "react";
import { useRouter, useParams } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { BillingSession, LapRecord } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

// ─── Formatters ──────────────────────────────────────────────────────────────

function formatLapTime(ms: number): string {
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis
    .toString()
    .padStart(3, "0")}`;
}

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}m ${s}s`;
}

function formatDate(iso: string | null): string {
  if (!iso) return "\u2014";
  const d = new Date(iso);
  return d.toLocaleDateString("en-IN", {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatPrice(paise: number | null): string {
  if (!paise) return "\u2014";
  return `\u20B9${(paise / 100).toFixed(0)}`;
}

// ─── Status helpers ──────────────────────────────────────────────────────────

function statusBadgeClasses(status: string): string {
  switch (status) {
    case "active":
      return "bg-emerald-500/20 text-emerald-400 border-emerald-500/30";
    case "completed":
      return "bg-neutral-500/20 text-neutral-300 border-neutral-500/30";
    case "ended_early":
      return "bg-rp-red/20 text-rp-red border-rp-red/30";
    case "cancelled":
      return "bg-red-500/20 text-red-400 border-red-500/30";
    case "paused_manual":
      return "bg-yellow-500/20 text-yellow-400 border-yellow-500/30";
    default:
      return "bg-rp-grey/20 text-rp-grey border-rp-grey/30";
  }
}

function statusLabel(status: string): string {
  switch (status) {
    case "active":
      return "Active";
    case "completed":
      return "Completed";
    case "ended_early":
      return "Ended Early";
    case "paused_manual":
      return "Paused";
    case "cancelled":
      return "Cancelled";
    case "pending":
      return "Pending";
    default:
      return status;
  }
}

// ─── Page component ──────────────────────────────────────────────────────────

export default function SessionDetailPage() {
  const router = useRouter();
  const params = useParams();
  const sessionId = params.id as string;

  const [session, setSession] = useState<BillingSession | null>(null);
  const [laps, setLaps] = useState<LapRecord[]>([]);
  const [track, setTrack] = useState<string | null>(null);
  const [car, setCar] = useState<string | null>(null);
  const [totalLaps, setTotalLaps] = useState(0);
  const [bestLapMs, setBestLapMs] = useState<number | null>(null);
  const [avgLapMs, setAvgLapMs] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }

    api
      .sessionDetail(sessionId)
      .then((res) => {
        if (res.error) {
          setError(res.error);
        } else {
          setSession(res.session);
          setLaps(res.laps || []);
          setTrack(res.track ?? null);
          setCar(res.car ?? null);
          setTotalLaps(res.total_laps ?? 0);
          setBestLapMs(res.best_lap_ms ?? null);
          setAvgLapMs(res.avg_lap_ms ?? null);
        }
        setLoading(false);
      })
      .catch(() => {
        setError("Failed to load session");
        setLoading(false);
      });
  }, [router, sessionId]);

  // ─── Loading state ───────────────────────────────────────────────────────

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  // ─── Error / not found ───────────────────────────────────────────────────

  if (error || !session) {
    return (
      <div className="min-h-screen pb-20">
        <div className="px-4 pt-12 max-w-lg mx-auto">
          <button
            onClick={() => router.push("/sessions")}
            className="text-rp-red text-sm mb-4 flex items-center gap-1"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              className="w-4 h-4"
            >
              <path
                d="M19 12H5M12 19l-7-7 7-7"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            Back to Sessions
          </button>
          <p className="text-rp-grey">{error || "Session not found"}</p>
        </div>
        <BottomNav />
      </div>
    );
  }

  // ─── Derived data ────────────────────────────────────────────────────────

  const usagePercent = Math.min(
    100,
    session.allocated_seconds > 0
      ? (session.driving_seconds / session.allocated_seconds) * 100
      : 0
  );

  const validLaps = laps.filter((l) => l.valid);
  const maxLapMs = validLaps.length > 0
    ? Math.max(...validLaps.map((l) => l.lap_time_ms))
    : 0;

  // ─── Render ──────────────────────────────────────────────────────────────

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        {/* Back button */}
        <button
          onClick={() => router.push("/sessions")}
          className="text-rp-red text-sm mb-4 flex items-center gap-1"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth={2}
            className="w-4 h-4"
          >
            <path
              d="M19 12H5M12 19l-7-7 7-7"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          Back to Sessions
        </button>

        {/* ── 1. Session Summary Header ─────────────────────────────────── */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-5 mb-4">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-3">
              <span className="text-lg font-bold text-white">
                Pod {session.pod_id.replace("pod_", "#")}
              </span>
              <span
                className={`text-[10px] font-semibold uppercase tracking-wider px-2 py-0.5 rounded-full border ${statusBadgeClasses(
                  session.status
                )}`}
              >
                {statusLabel(session.status)}
              </span>
            </div>
          </div>

          <div className="flex items-end justify-between">
            <div>
              <p className="text-2xl font-bold text-white">
                {formatDuration(session.driving_seconds)}
              </p>
              <p className="text-xs text-rp-grey mt-0.5">
                of {formatDuration(session.allocated_seconds)} allocated
              </p>
            </div>
            <p className="text-xs text-rp-grey text-right">
              {formatDate(session.started_at)}
            </p>
          </div>

          {/* Usage bar */}
          <div className="mt-4">
            <div className="flex justify-between text-xs text-rp-grey mb-1">
              <span>Usage</span>
              <span>{usagePercent.toFixed(0)}%</span>
            </div>
            <div className="h-2 bg-[#1A1A1A] rounded-full overflow-hidden">
              <div
                className="h-full bg-rp-red rounded-full transition-all"
                style={{ width: `${usagePercent}%` }}
              />
            </div>
          </div>

          {session.custom_price_paise && (
            <div className="mt-4 pt-3 border-t border-rp-border flex justify-between items-center">
              <span className="text-sm text-rp-grey">Amount Charged</span>
              <span className="text-sm font-bold text-white">
                {formatPrice(session.custom_price_paise)}
              </span>
            </div>
          )}
        </div>

        {/* ── 2. Session Stats ──────────────────────────────────────────── */}
        <div className="grid grid-cols-2 gap-3 mb-4">
          <StatTile label="Total Laps" value={totalLaps.toString()} />
          <StatTile
            label="Best Lap"
            value={bestLapMs ? formatLapTime(bestLapMs) : "\u2014"}
            accent
          />
          <StatTile
            label="Avg Lap"
            value={avgLapMs ? formatLapTime(avgLapMs) : "\u2014"}
          />
          <StatTile label="Track" value={track || "\u2014"} small />
        </div>

        {car && (
          <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-4">
            <p className="text-xs text-rp-grey mb-1">Car</p>
            <p className="text-sm font-semibold text-white">{car}</p>
          </div>
        )}

        {/* ── 3. Telemetry Chart (CSS bar chart) ────────────────────────── */}
        {validLaps.length > 0 && (
          <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-4">
            <h2 className="text-sm font-medium text-rp-grey mb-3">
              Lap Times
            </h2>
            <div className="flex items-end gap-1" style={{ height: 120 }}>
              {laps.map((lap, i) => {
                if (!lap.valid) {
                  return (
                    <div
                      key={lap.id}
                      className="flex-1 flex flex-col items-center justify-end"
                      style={{ height: "100%" }}
                    >
                      <div
                        className="w-full rounded-t bg-rp-grey/30"
                        style={{
                          height: `${
                            maxLapMs > 0
                              ? (lap.lap_time_ms / maxLapMs) * 100
                              : 0
                          }%`,
                          minHeight: 4,
                        }}
                      />
                      <span className="text-[8px] text-rp-grey mt-1">
                        {i + 1}
                      </span>
                    </div>
                  );
                }
                const isBest = lap.lap_time_ms === bestLapMs;
                return (
                  <div
                    key={lap.id}
                    className="flex-1 flex flex-col items-center justify-end"
                    style={{ height: "100%" }}
                  >
                    <div
                      className={`w-full rounded-t transition-all ${
                        isBest ? "bg-rp-red" : "bg-rp-red/50"
                      }`}
                      style={{
                        height: `${
                          maxLapMs > 0
                            ? (lap.lap_time_ms / maxLapMs) * 100
                            : 0
                        }%`,
                        minHeight: 4,
                      }}
                    />
                    <span
                      className={`text-[8px] mt-1 ${
                        isBest ? "text-rp-red font-bold" : "text-rp-grey"
                      }`}
                    >
                      {i + 1}
                    </span>
                  </div>
                );
              })}
            </div>
            {bestLapMs && (
              <p className="text-[10px] text-rp-grey mt-2 text-center">
                Best: {formatLapTime(bestLapMs)} (highlighted)
              </p>
            )}
          </div>
        )}

        {/* ── 4. Lap-by-Lap Table ───────────────────────────────────────── */}
        {laps.length > 0 && (
          <div className="bg-rp-card border border-rp-border rounded-xl overflow-hidden mb-4">
            <div className="px-4 py-3 border-b border-rp-border">
              <h2 className="text-sm font-medium text-rp-grey">
                Lap Details
              </h2>
            </div>

            {/* Table header */}
            <div className="grid grid-cols-[40px_1fr_1fr_1fr_1fr_28px] gap-1 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
              <span>Lap</span>
              <span>Time</span>
              <span>S1</span>
              <span>S2</span>
              <span>S3</span>
              <span></span>
            </div>

            {/* Lap rows */}
            {laps.map((lap, i) => {
              const isBest = lap.valid && lap.lap_time_ms === bestLapMs;
              const isInvalid = !lap.valid;

              return (
                <div
                  key={lap.id}
                  className={`grid grid-cols-[40px_1fr_1fr_1fr_1fr_28px] gap-1 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 ${
                    isBest ? "bg-rp-red/10" : ""
                  } ${isInvalid ? "opacity-40" : ""}`}
                >
                  <span
                    className={`text-xs font-medium ${
                      isBest ? "text-rp-red" : "text-neutral-400"
                    }`}
                  >
                    {i + 1}
                  </span>
                  <span
                    className={`text-xs font-mono font-medium ${
                      isBest
                        ? "text-rp-red"
                        : isInvalid
                        ? "text-neutral-500 line-through"
                        : "text-white"
                    }`}
                  >
                    {formatLapTime(lap.lap_time_ms)}
                  </span>
                  <span className="text-xs font-mono text-neutral-400">
                    {lap.sector1_ms ? formatLapTime(lap.sector1_ms) : "\u2014"}
                  </span>
                  <span className="text-xs font-mono text-neutral-400">
                    {lap.sector2_ms ? formatLapTime(lap.sector2_ms) : "\u2014"}
                  </span>
                  <span className="text-xs font-mono text-neutral-400">
                    {lap.sector3_ms ? formatLapTime(lap.sector3_ms) : "\u2014"}
                  </span>
                  <span className="flex items-center justify-center">
                    {isInvalid ? (
                      <svg
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth={2}
                        className="w-3.5 h-3.5 text-red-400"
                      >
                        <path
                          d="M18 6L6 18M6 6l12 12"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        />
                      </svg>
                    ) : isBest ? (
                      <svg
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth={2.5}
                        className="w-3.5 h-3.5 text-rp-red"
                      >
                        <path
                          d="M5 13l4 4L19 7"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        />
                      </svg>
                    ) : (
                      <svg
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth={2}
                        className="w-3.5 h-3.5 text-emerald-400"
                      >
                        <path
                          d="M5 13l4 4L19 7"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        />
                      </svg>
                    )}
                  </span>
                </div>
              );
            })}
          </div>
        )}

        {/* No laps placeholder */}
        {laps.length === 0 && (
          <div className="bg-rp-card border border-rp-border rounded-xl p-6 mb-4 text-center">
            <p className="text-rp-grey text-sm">No lap data for this session</p>
            <p className="text-rp-grey text-xs mt-1">
              Lap times are recorded during sim racing gameplay
            </p>
          </div>
        )}

        {/* Session ID footer */}
        <p className="text-center text-rp-grey text-xs">
          Session ID: {session.id.slice(0, 8)}...
        </p>
      </div>
      <BottomNav />
    </div>
  );
}

// ─── Sub-components ──────────────────────────────────────────────────────────

function StatTile({
  label,
  value,
  accent = false,
  small = false,
}: {
  label: string;
  value: string;
  accent?: boolean;
  small?: boolean;
}) {
  return (
    <div className="bg-rp-card border border-rp-border rounded-xl p-4">
      <p className="text-xs text-rp-grey mb-1">{label}</p>
      <p
        className={`font-bold ${
          accent ? "text-rp-red" : "text-white"
        } ${small ? "text-sm truncate" : "text-lg font-mono"}`}
      >
        {value}
      </p>
    </div>
  );
}
