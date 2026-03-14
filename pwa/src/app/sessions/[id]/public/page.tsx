"use client";

import { useEffect, useState } from "react";
import { useParams } from "next/navigation";
import { publicApi } from "@/lib/api";
import type { PublicSessionSummary } from "@/lib/api";

function formatLapTime(ms: number): string {
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis.toString().padStart(3, "0")}`;
}

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}m ${s}s`;
}

function formatGameName(simType: string | null): string {
  if (!simType) return "\u2014";
  const names: Record<string, string> = {
    assettocorsa: "Assetto Corsa",
    assetto_corsa: "Assetto Corsa",
    f1: "F1",
    f1_25: "F1 25",
    forza: "Forza",
    iracing: "iRacing",
    lmu: "Le Mans Ultimate",
    le_mans_ultimate: "Le Mans Ultimate",
  };
  return names[simType] || simType;
}

export default function PublicSessionPage() {
  const params = useParams();
  const id = params.id as string;
  const [data, setData] = useState<PublicSessionSummary | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!id) return;
    publicApi.sessionSummary(id)
      .then((res) => {
        if (res.error) {
          setError(res.error);
        } else {
          setData(res);
        }
      })
      .catch(() => setError("Failed to load session"))
      .finally(() => setLoading(false));
  }, [id]);

  if (loading) {
    return (
      <div className="min-h-screen bg-rp-black flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="min-h-screen bg-rp-black flex items-center justify-center p-4">
        <div className="bg-rp-card border border-rp-border rounded-xl p-6 text-center max-w-sm">
          <p className="text-white text-lg font-bold mb-2">Session Not Found</p>
          <p className="text-rp-grey text-sm">{error || "This session does not exist."}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="min-h-screen bg-rp-black">
      <div className="max-w-md mx-auto px-4 py-8">
        {/* Header */}
        <div className="text-center mb-6">
          <h1 className="text-rp-red font-bold text-2xl tracking-wide">RACINGPOINT</h1>
          <p className="text-rp-grey text-sm mt-1">Session Report</p>
        </div>

        {/* Driver card */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-5 mb-4">
          <p className="text-white text-xl font-bold">{data.driver_first_name}</p>
          <p className="text-rp-grey text-sm mt-1">{data.pricing_tier} Session</p>
        </div>

        {/* Stats grid */}
        <div className="grid grid-cols-2 gap-3 mb-4">
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <p className="text-rp-grey text-xs mb-1">Duration</p>
            <p className="text-white font-bold">{formatDuration(data.duration_seconds)}</p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <p className="text-rp-grey text-xs mb-1">Total Laps</p>
            <p className="text-white font-bold">{data.total_laps}</p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <p className="text-rp-grey text-xs mb-1">Best Lap</p>
            <p className="text-rp-red font-mono font-bold">
              {data.best_lap_ms ? formatLapTime(data.best_lap_ms) : "\u2014"}
            </p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded-xl p-4">
            <p className="text-rp-grey text-xs mb-1">Game</p>
            <p className="text-white font-bold text-sm">{formatGameName(data.sim_type)}</p>
          </div>
        </div>

        {/* Track & Car */}
        {(data.track || data.car) && (
          <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-6">
            {data.track && (
              <div className="flex justify-between mb-1">
                <span className="text-rp-grey text-sm">Track</span>
                <span className="text-white text-sm">{data.track}</span>
              </div>
            )}
            {data.car && (
              <div className="flex justify-between">
                <span className="text-rp-grey text-sm">Car</span>
                <span className="text-white text-sm">{data.car}</span>
              </div>
            )}
          </div>
        )}

        {/* CTA */}
        <a
          href="https://racingpoint.in"
          target="_blank"
          rel="noopener noreferrer"
          className="block w-full bg-rp-red hover:bg-rp-red/90 text-white font-semibold py-3 rounded-xl text-center transition-colors"
        >
          Race at RacingPoint
        </a>

        <p className="text-center text-rp-grey text-xs mt-4">
          RacingPoint eSports &amp; Cafe
        </p>
      </div>
    </div>
  );
}
