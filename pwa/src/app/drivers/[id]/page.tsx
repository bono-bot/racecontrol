"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import Link from "next/link";
import { publicApi } from "@/lib/api";

function formatLapTime(ms: number | null | undefined): string {
  if (!ms || ms <= 0) return "-";
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis
    .toString()
    .padStart(3, "0")}`;
}

function formatTotalTime(ms: number): string {
  const hours = Math.floor(ms / 3600000);
  const minutes = Math.floor((ms % 3600000) / 60000);
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr);
    return d.toLocaleDateString("en-IN", {
      day: "numeric",
      month: "short",
      year: "numeric",
    });
  } catch {
    return dateStr;
  }
}

function formatSector(ms: number | null | undefined): string {
  if (!ms || ms <= 0) return "-";
  const secs = Math.floor(ms / 1000);
  const millis = ms % 1000;
  return `${secs}.${millis.toString().padStart(3, "0")}`;
}

interface DriverData {
  display_name: string;
  total_laps: number;
  total_time_ms: number;
  avatar_url: string | null;
  member_since: string | null;
  class_badge: string | null;
}

interface PersonalBest {
  track: string;
  car: string;
  best_lap_ms: number;
  achieved_at: string;
}

interface LapHistoryEntry {
  track: string;
  car: string;
  lap_time_ms: number;
  sector1_ms: number | null;
  sector2_ms: number | null;
  sector3_ms: number | null;
  valid: boolean;
  created_at: string;
}

export default function DriverProfilePage() {
  const params = useParams();
  const id = params.id as string;
  const [driver, setDriver] = useState<DriverData | null>(null);
  const [personalBests, setPersonalBests] = useState<PersonalBest[]>([]);
  const [lapHistory, setLapHistory] = useState<LapHistoryEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [notFound, setNotFound] = useState(false);

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    publicApi
      .driverProfile(id)
      .then(
        (data: {
          driver?: DriverData;
          personal_bests?: PersonalBest[];
          lap_history?: LapHistoryEntry[];
          error?: string;
        }) => {
          if (data.error || !data.driver) {
            setNotFound(true);
          } else {
            setDriver(data.driver);
            // Sort personal bests by most recent first
            const pbs = (data.personal_bests || []).sort(
              (a, b) =>
                new Date(b.achieved_at).getTime() -
                new Date(a.achieved_at).getTime()
            );
            setPersonalBests(pbs);
            setLapHistory(data.lap_history || []);
          }
          setLoading(false);
        }
      )
      .catch(() => {
        setNotFound(true);
        setLoading(false);
      });
  }, [id]);

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen bg-rp-dark">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (notFound || !driver) {
    return (
      <div className="min-h-screen bg-rp-dark flex items-center justify-center">
        <div className="text-center px-4">
          <p className="text-white text-lg font-medium mb-2">
            Driver not found
          </p>
          <p className="text-rp-grey text-sm mb-4">
            This driver profile does not exist.
          </p>
          <Link
            href="/drivers"
            className="text-rp-red text-sm hover:underline"
          >
            Back to search
          </Link>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-rp-dark">
      {/* Header */}
      <div className="bg-gradient-to-b from-rp-red/20 to-transparent pt-12 pb-8 px-4">
        <div className="max-w-2xl mx-auto">
          <Link
            href="/drivers"
            className="text-rp-red text-sm mb-4 inline-flex items-center gap-1"
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
            Drivers
          </Link>

          <div className="flex items-center gap-4 mt-2">
            {driver.avatar_url ? (
              <img
                src={driver.avatar_url}
                alt={driver.display_name}
                className="w-16 h-16 rounded-full object-cover"
              />
            ) : (
              <div className="w-16 h-16 rounded-full bg-rp-red/20 flex items-center justify-center text-rp-red font-bold text-xl">
                {driver.display_name
                  .split(" ")
                  .map((w) => w[0])
                  .filter(Boolean)
                  .slice(0, 2)
                  .join("")
                  .toUpperCase()}
              </div>
            )}
            <div>
              <div className="flex items-center gap-2">
                <h1 className="text-2xl font-bold text-white">
                  {driver.display_name}
                </h1>
                {driver.class_badge && (
                  <span className="bg-rp-red/20 text-rp-red text-xs font-bold px-2 py-0.5 rounded-full">
                    {driver.class_badge}
                  </span>
                )}
              </div>
              {driver.member_since && (
                <p className="text-rp-grey text-xs mt-0.5">
                  Member since {formatDate(driver.member_since)}
                </p>
              )}
            </div>
          </div>
        </div>
      </div>

      <div className="max-w-2xl mx-auto px-4 pb-8">
        {/* Stats cards */}
        <div className="grid grid-cols-3 gap-3 mb-6">
          <div className="bg-rp-card border border-rp-border rounded-xl p-3 text-center">
            <p className="text-2xl font-bold text-white">
              {driver.total_laps}
            </p>
            <p className="text-xs text-rp-grey mt-0.5">Total Laps</p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-3 text-center">
            <p className="text-2xl font-bold text-white">
              {formatTotalTime(driver.total_time_ms)}
            </p>
            <p className="text-xs text-rp-grey mt-0.5">Total Time</p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-3 text-center">
            <p className="text-2xl font-bold text-white">
              {personalBests.length}
            </p>
            <p className="text-xs text-rp-grey mt-0.5">Personal Bests</p>
          </div>
        </div>

        {/* Personal Bests */}
        <div className="mb-6">
          <h2 className="text-sm font-medium text-rp-grey uppercase tracking-wider mb-3">
            Personal Bests
          </h2>

          {personalBests.length === 0 ? (
            <div className="bg-rp-card border border-rp-border rounded-xl p-4 text-center">
              <p className="text-rp-grey text-sm">No personal bests yet</p>
            </div>
          ) : (
            <>
              {/* Desktop table */}
              <div className="hidden sm:block bg-rp-card border border-rp-border rounded-xl overflow-hidden">
                <div className="grid grid-cols-[1fr_1fr_90px_90px] gap-1 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                  <span>Track</span>
                  <span>Car</span>
                  <span className="text-right">Best Lap</span>
                  <span className="text-right">Date</span>
                </div>
                {personalBests.map((pb, i) => (
                  <div
                    key={`${pb.track}-${pb.car}-${i}`}
                    className="grid grid-cols-[1fr_1fr_90px_90px] gap-1 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0"
                  >
                    <span className="text-sm text-white truncate">
                      {pb.track}
                    </span>
                    <span className="text-xs text-rp-grey truncate self-center">
                      {pb.car}
                    </span>
                    <span
                      className="text-sm font-mono text-white text-right"
                      style={{ fontSize: "14px" }}
                    >
                      {formatLapTime(pb.best_lap_ms)}
                    </span>
                    <span className="text-xs text-rp-grey text-right self-center">
                      {formatDate(pb.achieved_at)}
                    </span>
                  </div>
                ))}
              </div>

              {/* Mobile cards */}
              <div className="sm:hidden space-y-2">
                {personalBests.map((pb, i) => (
                  <div
                    key={`m-${pb.track}-${pb.car}-${i}`}
                    className="bg-rp-card border border-rp-border rounded-xl p-3"
                  >
                    <div className="flex justify-between items-start">
                      <div className="flex-1 min-w-0">
                        <p
                          className="text-sm text-white truncate"
                          style={{ fontSize: "14px" }}
                        >
                          {pb.track}
                        </p>
                        <p className="text-xs text-rp-grey truncate">
                          {pb.car}
                        </p>
                      </div>
                      <div className="text-right ml-3">
                        <p
                          className="font-mono text-white font-medium"
                          style={{ fontSize: "14px" }}
                        >
                          {formatLapTime(pb.best_lap_ms)}
                        </p>
                        <p className="text-xs text-rp-grey">
                          {formatDate(pb.achieved_at)}
                        </p>
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>

        {/* Lap History */}
        <div className="mb-6">
          <h2 className="text-sm font-medium text-rp-grey uppercase tracking-wider mb-3">
            Lap History
            {lapHistory.length > 0 && (
              <span className="text-rp-grey/60 ml-1">
                (last {lapHistory.length})
              </span>
            )}
          </h2>

          {lapHistory.length === 0 ? (
            <div className="bg-rp-card border border-rp-border rounded-xl p-4 text-center">
              <p className="text-rp-grey text-sm">No lap history yet</p>
            </div>
          ) : (
            <>
              {/* Desktop table */}
              <div className="hidden sm:block bg-rp-card border border-rp-border rounded-xl overflow-hidden">
                <div className="grid grid-cols-[1fr_1fr_80px_60px_60px_60px_50px_80px] gap-1 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                  <span>Track</span>
                  <span>Car</span>
                  <span className="text-right">Lap</span>
                  <span className="text-right">S1</span>
                  <span className="text-right">S2</span>
                  <span className="text-right">S3</span>
                  <span className="text-center">Valid</span>
                  <span className="text-right">Date</span>
                </div>
                {lapHistory.map((lap, i) => (
                  <div
                    key={i}
                    className={`grid grid-cols-[1fr_1fr_80px_60px_60px_60px_50px_80px] gap-1 px-4 py-2 border-b border-rp-border/50 last:border-b-0 ${
                      !lap.valid ? "opacity-60" : ""
                    }`}
                  >
                    <span className="text-sm text-white truncate">
                      {lap.track}
                    </span>
                    <span className="text-xs text-rp-grey truncate self-center">
                      {lap.car}
                    </span>
                    <span
                      className="text-sm font-mono text-white text-right"
                      style={{ fontSize: "14px" }}
                    >
                      {formatLapTime(lap.lap_time_ms)}
                    </span>
                    <span className="text-xs font-mono text-rp-grey text-right self-center">
                      {formatSector(lap.sector1_ms)}
                    </span>
                    <span className="text-xs font-mono text-rp-grey text-right self-center">
                      {formatSector(lap.sector2_ms)}
                    </span>
                    <span className="text-xs font-mono text-rp-grey text-right self-center">
                      {formatSector(lap.sector3_ms)}
                    </span>
                    <span className="text-center self-center">
                      {!lap.valid && (
                        <span className="text-[10px] text-neutral-500 font-medium">
                          Invalid
                        </span>
                      )}
                    </span>
                    <span className="text-xs text-rp-grey text-right self-center">
                      {formatDate(lap.created_at)}
                    </span>
                  </div>
                ))}
              </div>

              {/* Mobile cards */}
              <div className="sm:hidden space-y-2">
                {lapHistory.map((lap, i) => (
                  <div
                    key={`m-${i}`}
                    className={`bg-rp-card border border-rp-border rounded-xl p-3 ${
                      !lap.valid ? "opacity-70" : ""
                    }`}
                  >
                    <div className="flex justify-between items-start mb-1">
                      <div className="flex-1 min-w-0">
                        <p
                          className="text-sm text-white truncate"
                          style={{ fontSize: "14px" }}
                        >
                          {lap.track}
                        </p>
                        <p className="text-xs text-rp-grey truncate">
                          {lap.car}
                        </p>
                      </div>
                      <div className="text-right ml-3">
                        <p
                          className="font-mono text-white font-medium"
                          style={{ fontSize: "14px" }}
                        >
                          {formatLapTime(lap.lap_time_ms)}
                        </p>
                        {!lap.valid && (
                          <span className="text-[10px] text-neutral-500 font-medium">
                            Invalid
                          </span>
                        )}
                      </div>
                    </div>
                    <div className="flex gap-3 text-xs font-mono text-rp-grey mt-1 pl-0">
                      <span>S1: {formatSector(lap.sector1_ms)}</span>
                      <span>S2: {formatSector(lap.sector2_ms)}</span>
                      <span>S3: {formatSector(lap.sector3_ms)}</span>
                    </div>
                    <p className="text-xs text-rp-grey/60 mt-1">
                      {formatDate(lap.created_at)}
                    </p>
                  </div>
                ))}
              </div>
            </>
          )}
        </div>

        {/* Footer */}
        <div className="text-center mt-8">
          <p className="text-rp-grey text-xs">RacingPoint</p>
        </div>
      </div>
    </div>
  );
}
