"use client";

import { useEffect, useState } from "react";
import { useRouter, useParams } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { SessionDetailSession, LapRecord, ShareReport, MultiplayerResultInfo, SessionEvent } from "@/lib/api";
import BottomNav from "@/components/BottomNav";
import { ConfettiOnMount } from "@/components/Confetti";

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

function formatCredits(paise: number | null | undefined): string {
  if (paise === null || paise === undefined) return "\u2014";
  return `${(paise / 100).toFixed(0)} credits`;
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

function formatGameName(game: string | null): string {
  if (!game) return "\u2014";
  const names: Record<string, string> = {
    assetto_corsa: "Assetto Corsa",
    iracing: "iRacing",
    f1_25: "F1 25",
    le_mans_ultimate: "LMU",
    forza: "Forza",
  };
  return names[game] || game;
}

// ─── Event timeline helpers ──────────────────────────────────────────────────

const eventLabels: Record<string, string> = {
  created: "Session Created",
  started: "Session Started",
  paused_manual: "Paused",
  paused_disconnect: "Paused (Disconnected)",
  resumed: "Resumed",
  resumed_disconnect: "Reconnected",
  resumed_manual: "Resumed",
  ended: "Session Completed",
  ended_early: "Session Ended Early",
  cancelled: "Session Cancelled",
  time_expired: "Time Expired",
  extended: "Time Extended",
  pause_timeout_ended: "Pause Timeout (Ended)",
};

function eventIcon(eventType: string): string {
  if (eventType.startsWith("paused")) return "\u23F8";
  if (eventType.startsWith("resumed")) return "\u25B6";
  if (eventType === "started") return "\uD83D\uDFE2";
  if (eventType === "ended" || eventType === "ended_early" || eventType === "time_expired") return "\uD83C\uDFC1";
  if (eventType === "extended") return "\u23F1";
  if (eventType === "cancelled") return "\u274C";
  return "\u2022";
}

// ─── Peak-end components ──────────────────────────────────────────────────────

function PeakMomentHeroCard({
  bestLapMs,
  peakLapNumber,
  isNewPb,
  improvementMs,
}: {
  bestLapMs: number;
  peakLapNumber: number | null;
  isNewPb: boolean;
  improvementMs: number | null;
}) {
  return (
    <div
      className={`bg-rp-card border ${
        isNewPb ? "border-yellow-500/30" : "border-rp-border"
      } rounded-xl p-5 mb-4`}
    >
      <div className="text-center mb-2">
        <span className="text-xs text-rp-grey uppercase tracking-wider">
          Peak Moment
        </span>
      </div>
      <div className="text-center mb-4">
        <p className="text-4xl font-bold text-white font-mono">
          {formatLapTime(bestLapMs)}
        </p>
        <p className="text-xs text-rp-grey mt-1">
          Best Lap
          {peakLapNumber != null ? ` \u00B7 Lap ${peakLapNumber}` : ""}
        </p>
      </div>
      {isNewPb && (
        <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-3 text-center">
          <p className="text-yellow-400 font-bold text-xl">
            NEW PERSONAL BEST!
          </p>
        </div>
      )}
      {isNewPb && improvementMs != null && improvementMs > 0 && (
        <p className="text-xs text-rp-grey text-center mt-2">
          <span className="text-emerald-400 font-mono">
            -{formatLapTime(improvementMs)}
          </span>{" "}
          faster than your previous best
        </p>
      )}
    </div>
  );
}

function PercentileRankingBanner({
  percentileRank,
  track,
  car,
}: {
  percentileRank: number;
  track: string | null;
  car: string | null;
}) {
  return (
    <div className="bg-rp-red/10 border border-rp-red/20 rounded-xl p-4 mb-4 text-center">
      <p className="text-rp-red font-bold text-xl">
        Faster than {percentileRank}% of drivers
      </p>
      {(track || car) && (
        <p className="text-xs text-rp-grey mt-1">
          {[track, car].filter(Boolean).join(" \u00B7 ")}
        </p>
      )}
    </div>
  );
}

// ─── Page component ──────────────────────────────────────────────────────────

export default function SessionDetailPage() {
  const router = useRouter();
  const params = useParams();
  const sessionId = params.id as string;

  const [session, setSession] = useState<SessionDetailSession | null>(null);
  const [laps, setLaps] = useState<LapRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [shareReport, setShareReport] = useState<ShareReport | null>(null);
  const [showShare, setShowShare] = useState(false);
  const [shareLoading, setShareLoading] = useState(false);
  const [mpResults, setMpResults] = useState<MultiplayerResultInfo[]>([]);
  const [events, setEvents] = useState<SessionEvent[]>([]);
  const [myDriverId, setMyDriverId] = useState<string | null>(null);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }

    async function loadSession() {
      try {
        const [detailRes, profileRes] = await Promise.all([
          api.sessionDetail(sessionId),
          api.profile(),
        ]);
        if (detailRes.error) {
          setError(detailRes.error);
        } else {
          setSession(detailRes.session);
          setLaps(detailRes.laps || []);
          setEvents(detailRes.events ?? []);

          // Fetch multiplayer results if this is a group session
          if (detailRes.session?.group_session_id) {
            try {
              const mpRes = await api.multiplayerResults(
                detailRes.session.group_session_id
              );
              if (mpRes.results) setMpResults(mpRes.results);
            } catch {
              // Silently fail — results may not be available yet
            }
          }
        }
        if (profileRes.driver) {
          setMyDriverId(profileRes.driver.id);
        }
      } catch {
        setError("Failed to load session");
      }
      setLoading(false);
    }
    loadSession();
  }, [router, sessionId]);

  const handleShare = async () => {
    setShareLoading(true);
    try {
      const res = await api.sessionShare(sessionId);
      if (res.share_report) {
        setShareReport(res.share_report);
        setShowShare(true);

        // Try native share
        if (navigator.share) {
          const r = res.share_report;
          const text = [
            `${r.driver_name} at RacingPoint`,
            r.track ? `Track: ${r.track}` : null,
            r.car ? `Car: ${r.car}` : null,
            r.best_lap_display ? `Best Lap: ${r.best_lap_display}` : null,
            r.total_laps ? `Laps: ${r.total_laps}` : null,
            r.percentile_text || null,
            r.is_new_pb ? "NEW PERSONAL BEST!" : null,
            "",
            "May the Fastest Win.",
          ].filter(Boolean).join("\n");

          try {
            await navigator.share({ title: "My RacingPoint Session", text });
          } catch {
            // User cancelled share — that's fine
          }
        }
      }
    } catch {
      // Silently fail
    }
    setShareLoading(false);
  };

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
          <BackButton onClick={() => router.push("/sessions")} />
          <p className="text-rp-grey">{error || "Session not found"}</p>
        </div>
        <BottomNav />
      </div>
    );
  }

  // ─── Derived data ────────────────────────────────────────────────────────

  const validLaps = laps.filter((l) => l.valid);
  const maxLapMs =
    validLaps.length > 0
      ? Math.max(...validLaps.map((l) => l.lap_time_ms))
      : 0;

  const netCharged =
    (session.wallet_debit_paise || 0) - (session.refund_paise || 0);

  // ─── Render ──────────────────────────────────────────────────────────────

  const isCompletedWithLaps =
    session.status === "completed" && laps.length > 0;

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        {/* ── 1. Back button ────────────────────────────────────────────── */}
        <BackButton onClick={() => router.push("/sessions")} />

        {/* ── 2. PeakMomentHeroCard ─────────────────────────────────────── */}
        {isCompletedWithLaps && session.best_lap_ms != null && (
          <PeakMomentHeroCard
            bestLapMs={session.best_lap_ms}
            peakLapNumber={session.peak_lap_number}
            isNewPb={session.is_new_pb}
            improvementMs={session.improvement_ms}
          />
        )}

        {/* ── Confetti on first PB view ─────────────────────────────────── */}
        <ConfettiOnMount
          enabled={session.is_new_pb && session.status === "completed"}
          sessionId={session.id}
        />

        {/* ── 3. PercentileRankingBanner ────────────────────────────────── */}
        {session.percentile_rank != null && (
          <PercentileRankingBanner
            percentileRank={session.percentile_rank}
            track={session.track}
            car={session.car}
          />
        )}

        {/* ── 4. Session Summary Header (condensed) ────────────────────── */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-5 mb-4">
          <div className="flex items-center justify-between mb-3">
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

          {/* Experience & game */}
          {(session.experience_name || session.sim_type) && (
            <div className="mb-3">
              {session.experience_name && (
                <p className="text-white font-semibold text-sm">
                  {session.experience_name}
                </p>
              )}
              <p className="text-xs text-rp-grey">
                {formatGameName(session.sim_type)}
                {session.track ? ` \u00B7 ${session.track}` : ""}
                {session.car ? ` \u00B7 ${session.car}` : ""}
              </p>
            </div>
          )}

          {/* Time usage (no usage bar) */}
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
        </div>

        {/* ── 5. Stats Grid ─────────────────────────────────────────────── */}
        <div className="grid grid-cols-2 gap-3 mb-4">
          <StatTile label="Total Laps" value={session.total_laps.toString()} />
          <div className="bg-rp-red/10 border border-rp-red/20 rounded-xl p-4">
            <p className="text-xs text-rp-grey mb-1">Best Lap</p>
            <p className="text-lg font-bold text-white font-mono">
              {session.best_lap_ms
                ? formatLapTime(session.best_lap_ms)
                : "\u2014"}
            </p>
          </div>
          <StatTile
            label="Avg Lap"
            value={
              session.average_lap_ms
                ? formatLapTime(session.average_lap_ms)
                : "\u2014"
            }
          />
          <StatTile
            label="Game"
            value={formatGameName(session.sim_type)}
            small
          />
          <StatTile label="Top Speed" value="N/A" />
        </div>

        {/* ── 6. Share Button ───────────────────────────────────────────── */}
        {session.status === "completed" && laps.length > 0 && (
          <button
            onClick={handleShare}
            disabled={shareLoading}
            className="w-full bg-rp-red hover:bg-rp-red/90 text-white font-semibold py-3 rounded-xl mb-4 flex items-center justify-center gap-2 transition-colors disabled:opacity-50"
          >
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth={2}
              className="w-5 h-5"
            >
              <path
                d="M4 12v8a2 2 0 002 2h12a2 2 0 002-2v-8M16 6l-4-4-4 4M12 2v13"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
            {shareLoading ? "Loading..." : "Share Session Report"}
          </button>
        )}

        {/* ── 7. Share Card Modal ───────────────────────────────────────── */}
        {showShare && shareReport && (
          <div className="bg-rp-card border border-rp-red/30 rounded-xl p-5 mb-4">
            <div className="flex justify-between items-start mb-4">
              <h3 className="text-white font-bold text-lg">Session Report</h3>
              <button
                onClick={() => setShowShare(false)}
                className="text-rp-grey hover:text-white"
              >
                <svg
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth={2}
                  className="w-5 h-5"
                >
                  <path
                    d="M18 6L6 18M6 6l12 12"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
              </button>
            </div>

            <div className="space-y-3">
              {shareReport.percentile_text && (
                <div className="bg-rp-red/10 border border-rp-red/20 rounded-lg p-3 text-center">
                  <p className="text-rp-red font-bold text-lg">
                    {shareReport.percentile_text}
                  </p>
                </div>
              )}

              {shareReport.is_new_pb && (
                <div className="bg-yellow-500/10 border border-yellow-500/30 rounded-lg p-3 text-center">
                  <p className="text-yellow-400 font-bold">
                    NEW PERSONAL BEST!
                  </p>
                </div>
              )}

              <div className="grid grid-cols-2 gap-3">
                <div className="bg-[#1A1A1A] rounded-lg p-3">
                  <p className="text-rp-grey text-xs">Best Lap</p>
                  <p className="text-white font-mono font-bold">
                    {shareReport.best_lap_display || "\u2014"}
                  </p>
                </div>
                <div className="bg-[#1A1A1A] rounded-lg p-3">
                  <p className="text-rp-grey text-xs">Total Laps</p>
                  <p className="text-white font-bold">{shareReport.total_laps}</p>
                </div>
                {shareReport.improvement_ms &&
                  shareReport.improvement_ms > 0 && (
                    <div className="bg-[#1A1A1A] rounded-lg p-3">
                      <p className="text-rp-grey text-xs">Improved By</p>
                      <p className="text-emerald-400 font-mono font-bold">
                        -{formatLapTime(shareReport.improvement_ms)}
                      </p>
                    </div>
                  )}
                {shareReport.consistency && (
                  <div className="bg-[#1A1A1A] rounded-lg p-3">
                    <p className="text-rp-grey text-xs">Consistency</p>
                    <p className="text-white font-bold">
                      {shareReport.consistency.rating}
                    </p>
                  </div>
                )}
              </div>

              {shareReport.track_record && (
                <div className="bg-[#1A1A1A] rounded-lg p-3">
                  <p className="text-rp-grey text-xs mb-1">Track Record</p>
                  <p className="text-white text-sm">
                    {formatLapTime(shareReport.track_record.time_ms)} by{" "}
                    {shareReport.track_record.holder}
                    {shareReport.track_record.gap_ms != null &&
                      shareReport.track_record.gap_ms > 0 && (
                        <span className="text-rp-grey text-xs ml-2">
                          (+{formatLapTime(shareReport.track_record.gap_ms)}{" "}
                          gap)
                        </span>
                      )}
                  </p>
                </div>
              )}
            </div>
          </div>
        )}

        {/* ── 8. Lap Times Chart ────────────────────────────────────────── */}
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
                const isBest = lap.lap_time_ms === session.best_lap_ms;
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
            {session.best_lap_ms && (
              <p className="text-[10px] text-rp-grey mt-2 text-center">
                Best: {formatLapTime(session.best_lap_ms)} (highlighted)
              </p>
            )}
          </div>
        )}

        {/* ── 9. Lap-by-Lap Table ───────────────────────────────────────── */}
        {laps.length > 0 && (
          <div className="bg-rp-card border border-rp-border rounded-xl overflow-hidden mb-4">
            <div className="px-4 py-3 border-b border-rp-border">
              <h2 className="text-sm font-medium text-rp-grey">Lap Details</h2>
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
              const isBest = lap.valid && lap.lap_time_ms === session.best_lap_ms;
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
                      <XIcon />
                    ) : isBest ? (
                      <CheckIcon accent />
                    ) : (
                      <CheckIcon />
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
            <p className="text-rp-grey text-sm">No lap data yet</p>
            <p className="text-rp-grey text-xs mt-1">
              Lap times will appear once you complete your session.
            </p>
          </div>
        )}

        {/* ── 10. Multiplayer Race Results ───────────────────────────────── */}
        {mpResults.length > 0 && (
          <div className="bg-rp-card border border-rp-border rounded-xl overflow-hidden mb-4">
            <div className="px-4 py-3 border-b border-rp-border flex items-center gap-2">
              <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth={2}
                className="w-4 h-4 text-yellow-400"
              >
                <path
                  d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </svg>
              <h2 className="text-sm font-medium text-white">Race Results</h2>
              <span className="text-rp-grey text-xs ml-auto">Multiplayer</span>
            </div>
            <div>
              {mpResults.map((result) => {
                const isMe = result.driver_id === myDriverId;
                const posColor =
                  result.position === 1
                    ? "text-yellow-400"
                    : result.position === 2
                    ? "text-neutral-300"
                    : result.position === 3
                    ? "text-amber-600"
                    : "text-neutral-500";
                return (
                  <div
                    key={result.driver_id}
                    className={`px-4 py-3 border-b border-rp-border/50 last:border-b-0 flex items-center gap-3 ${
                      isMe ? "bg-rp-red/5" : ""
                    }`}
                  >
                    {/* Position */}
                    <span
                      className={`text-lg font-bold w-8 text-center ${posColor}`}
                    >
                      #{result.position}
                    </span>

                    {/* Driver info */}
                    <div className="flex-1 min-w-0">
                      <p className="text-white font-medium text-sm truncate">
                        {result.driver_name}
                        {isMe && (
                          <span className="ml-1 text-xs text-rp-red">
                            (You)
                          </span>
                        )}
                        {result.position === 1 && (
                          <span className="ml-1 text-yellow-400 text-xs">
                            Winner
                          </span>
                        )}
                      </p>
                      <p className="text-rp-grey text-xs">
                        {result.laps_completed} lap
                        {result.laps_completed !== 1 ? "s" : ""}
                        {result.dnf ? (
                          <span className="ml-2 text-red-400 font-medium">
                            DNF
                          </span>
                        ) : result.total_time_ms ? (
                          <span className="ml-2 text-neutral-400">
                            Total: {formatLapTime(result.total_time_ms)}
                          </span>
                        ) : null}
                      </p>
                    </div>

                    {/* Best lap */}
                    <div className="text-right">
                      {result.best_lap_ms ? (
                        <p className="text-neutral-300 font-mono text-sm">
                          {formatLapTime(result.best_lap_ms)}
                        </p>
                      ) : (
                        <p className="text-neutral-500 text-xs">No time</p>
                      )}
                      <p className="text-rp-grey text-[10px]">Best lap</p>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* ── 11. Receipt / Billing Info ──────────────────────────────────── */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-4">
          <h2 className="text-sm font-medium text-rp-grey mb-3">Receipt</h2>
          <div className="space-y-2">
            <ReceiptRow
              label="Plan"
              value={session.pricing_tier_name || "\u2014"}
            />
            {session.discount_paise && session.discount_paise > 0 ? (
              <>
                <ReceiptRow
                  label="Original"
                  value={formatCredits(session.original_price_paise || session.price_paise)}
                />
                <ReceiptRow
                  label={`Discount${session.discount_reason ? ` (${session.discount_reason})` : ''}`}
                  value={`-${formatCredits(session.discount_paise)}`}
                  accent="green"
                />
              </>
            ) : null}
            <ReceiptRow
              label="Charged"
              value={formatCredits(session.wallet_debit_paise)}
            />
            {session.refund_paise && session.refund_paise > 0 ? (
              <ReceiptRow
                label="Refund"
                value={`+${formatCredits(session.refund_paise)}`}
                accent="green"
              />
            ) : null}
            <div className="border-t border-rp-border pt-2 mt-2">
              <ReceiptRow
                label="Net Cost"
                value={formatCredits(netCharged > 0 ? netCharged : session.price_paise)}
                bold
              />
            </div>
            {session.ended_at && (
              <ReceiptRow
                label="Ended"
                value={formatDate(session.ended_at)}
              />
            )}
          </div>
        </div>

        {/* ── 12. Session Timeline ──────────────────────────────────────── */}
        {events.length > 0 && (
          <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-4">
            <h2 className="text-sm font-medium text-rp-grey mb-3">Session Timeline</h2>
            <div className="relative">
              {/* Vertical line */}
              <div className="absolute left-3 top-2 bottom-2 w-px bg-rp-border" />
              <div className="space-y-3">
                {events.map((event) => (
                  <div key={event.id} className="flex items-start gap-3 relative">
                    <div className="w-6 h-6 flex items-center justify-center text-xs z-10 bg-rp-card">
                      {eventIcon(event.event_type)}
                    </div>
                    <div className="flex-1 min-w-0">
                      <p className="text-white text-sm">
                        {eventLabels[event.event_type] || event.event_type.replace(/_/g, " ")}
                      </p>
                      <p className="text-rp-grey text-xs">
                        {formatDate(event.created_at)}
                        {event.driving_seconds_at_event > 0 && (
                          <span className="ml-2">
                            ({formatDuration(event.driving_seconds_at_event)} driving)
                          </span>
                        )}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        )}

        {/* Session ID footer */}
        <p className="text-center text-rp-grey text-xs">
          Session {session.id.slice(0, 8)}
        </p>
      </div>
      <BottomNav />
    </div>
  );
}

// ─── Sub-components ──────────────────────────────────────────────────────────

function BackButton({ onClick }: { onClick: () => void }) {
  return (
    <button
      onClick={onClick}
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
  );
}

function ReceiptRow({
  label,
  value,
  bold = false,
  accent,
}: {
  label: string;
  value: string;
  bold?: boolean;
  accent?: string;
}) {
  return (
    <div className="flex justify-between items-center">
      <span className={`text-sm ${bold ? "text-neutral-200 font-medium" : "text-rp-grey"}`}>
        {label}
      </span>
      <span
        className={`text-sm ${
          bold
            ? "font-bold text-white"
            : accent === "green"
            ? "text-emerald-400"
            : "text-neutral-200"
        }`}
      >
        {value}
      </span>
    </div>
  );
}

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
        className={`font-bold ${accent ? "text-rp-red" : "text-white"} ${
          small ? "text-sm truncate" : "text-lg font-mono"
        }`}
      >
        {value}
      </p>
    </div>
  );
}

function CheckIcon({ accent }: { accent?: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={accent ? 2.5 : 2}
      className={`w-3.5 h-3.5 ${accent ? "text-rp-red" : "text-emerald-400"}`}
    >
      <path d="M5 13l4 4L19 7" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function XIcon() {
  return (
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
  );
}
