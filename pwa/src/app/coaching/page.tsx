"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { CompareLapsResult, LapRecord } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

function formatLapTime(ms: number): string {
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis.toString().padStart(3, "0")}`;
}

function deltaColor(ms: number | null): string {
  if (ms === null) return "text-rp-grey";
  if (ms <= 0) return "text-emerald-400";
  if (ms < 500) return "text-yellow-400";
  return "text-red-400";
}

function deltaText(ms: number | null): string {
  if (ms === null) return "\u2014";
  if (ms <= 0) return `-${formatLapTime(Math.abs(ms))}`;
  return `+${formatLapTime(ms)}`;
}

export default function CoachingPage() {
  const router = useRouter();
  const [laps, setLaps] = useState<LapRecord[]>([]);
  const [comparison, setComparison] = useState<CompareLapsResult | null>(null);
  const [loading, setLoading] = useState(true);
  const [comparing, setComparing] = useState(false);
  const [selectedTrack, setSelectedTrack] = useState("");
  const [selectedCar, setSelectedCar] = useState("");

  // Unique track/car combos from user's laps
  const trackCars = laps.reduce<{ track: string; car: string }[]>((acc, l) => {
    if (!acc.find((tc) => tc.track === l.track && tc.car === l.car)) {
      acc.push({ track: l.track, car: l.car });
    }
    return acc;
  }, []);

  useEffect(() => {
    if (!isLoggedIn()) { router.replace("/login"); return; }
    api.laps().then((res) => {
      setLaps(res.laps || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, [router]);

  const handleCompare = async () => {
    if (!selectedTrack || !selectedCar) return;
    setComparing(true);
    try {
      const res = await api.compareLaps(selectedTrack, selectedCar);
      if (!res.error) setComparison(res);
      else alert(res.error);
    } catch { /* ignore */ }
    setComparing(false);
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <h1 className="text-2xl font-bold text-white mb-1">Coaching</h1>
        <p className="text-rp-grey text-sm mb-6">Compare your laps to the track record and find where to improve.</p>

        {/* Track/Car Selector */}
        {trackCars.length === 0 ? (
          <div className="bg-rp-card border border-rp-border rounded-xl p-8 text-center">
            <p className="text-rp-grey">No lap data yet</p>
            <p className="text-rp-grey text-xs mt-1">Complete a session to start coaching analysis</p>
          </div>
        ) : (
          <>
            <div className="bg-rp-card border border-rp-border rounded-xl p-4 mb-4">
              <p className="text-sm text-rp-grey mb-2">Select Track & Car</p>
              <div className="space-y-2">
                {trackCars.map((tc) => (
                  <button
                    key={`${tc.track}-${tc.car}`}
                    onClick={() => { setSelectedTrack(tc.track); setSelectedCar(tc.car); setComparison(null); }}
                    className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors ${
                      selectedTrack === tc.track && selectedCar === tc.car
                        ? "bg-rp-red/20 border border-rp-red/40 text-white"
                        : "bg-[#1A1A1A] text-rp-grey hover:text-white"
                    }`}
                  >
                    <span className="font-medium">{tc.track}</span>
                    <span className="text-xs ml-2 opacity-70">{tc.car}</span>
                  </button>
                ))}
              </div>

              {selectedTrack && (
                <button
                  onClick={handleCompare}
                  disabled={comparing}
                  className="w-full mt-3 bg-rp-red hover:bg-rp-red/90 text-white font-semibold py-2.5 rounded-lg transition-colors disabled:opacity-50"
                >
                  {comparing ? "Analyzing..." : "Compare vs Track Record"}
                </button>
              )}
            </div>

            {/* Comparison Results */}
            {comparison && (
              <div className="space-y-4">
                {/* Gap Summary */}
                <div className="bg-rp-card border border-rp-border rounded-xl p-4">
                  <h2 className="text-sm font-medium text-rp-grey mb-3">Gap to Reference</h2>
                  <div className="flex items-center justify-between mb-4">
                    <div>
                      <p className="text-xs text-rp-grey">Your Best</p>
                      <p className="text-white font-mono font-bold text-lg">{formatLapTime(comparison.my_best.time_ms)}</p>
                    </div>
                    {comparison.reference && (
                      <>
                        <div className="text-center">
                          <p className="text-xs text-rp-grey">Delta</p>
                          <p className={`font-mono font-bold text-lg ${deltaColor(comparison.sector_analysis?.total_delta_ms ?? null)}`}>
                            {comparison.sector_analysis ? deltaText(comparison.sector_analysis.total_delta_ms) : "\u2014"}
                          </p>
                        </div>
                        <div className="text-right">
                          <p className="text-xs text-rp-grey">{comparison.reference.driver}</p>
                          <p className="text-white font-mono font-bold text-lg">{formatLapTime(comparison.reference.time_ms)}</p>
                        </div>
                      </>
                    )}
                  </div>
                </div>

                {/* Sector Analysis */}
                {comparison.sector_analysis && (
                  <div className="bg-rp-card border border-rp-border rounded-xl p-4">
                    <h2 className="text-sm font-medium text-rp-grey mb-3">Sector Breakdown</h2>
                    <div className="grid grid-cols-3 gap-3">
                      {(["s1", "s2", "s3"] as const).map((s, i) => {
                        const key = `${s}_delta_ms` as keyof typeof comparison.sector_analysis;
                        const delta = comparison.sector_analysis?.[key] as number | null;
                        const myMs = [comparison.my_best.s1_ms, comparison.my_best.s2_ms, comparison.my_best.s3_ms][i];
                        return (
                          <div key={s} className="bg-[#1A1A1A] rounded-lg p-3 text-center">
                            <p className="text-rp-grey text-xs uppercase mb-1">S{i + 1}</p>
                            <p className="text-white font-mono text-sm">{myMs ? formatLapTime(myMs) : "\u2014"}</p>
                            <p className={`font-mono text-xs mt-1 ${deltaColor(delta)}`}>{deltaText(delta)}</p>
                          </div>
                        );
                      })}
                    </div>
                    {comparison.sector_analysis.weakest_sector && (
                      <div className="mt-3 bg-red-500/10 border border-red-500/20 rounded-lg p-3">
                        <p className="text-red-400 text-xs font-medium">Weakest: {comparison.sector_analysis.weakest_sector}</p>
                      </div>
                    )}
                  </div>
                )}

                {/* Trend */}
                {comparison.recent_trend.length > 0 && (
                  <div className="bg-rp-card border border-rp-border rounded-xl p-4">
                    <div className="flex items-center justify-between mb-3">
                      <h2 className="text-sm font-medium text-rp-grey">Recent Trend</h2>
                      {comparison.improving !== null && (
                        <span className={`text-xs font-medium ${comparison.improving ? "text-emerald-400" : "text-yellow-400"}`}>
                          {comparison.improving ? "Improving" : "Plateauing"}
                        </span>
                      )}
                    </div>
                    <div className="flex items-end gap-1" style={{ height: 80 }}>
                      {(() => {
                        const max = Math.max(...comparison.recent_trend);
                        const min = Math.min(...comparison.recent_trend);
                        const range = max - min || 1;
                        return comparison.recent_trend.map((t, i) => {
                          const height = 20 + ((max - t) / range) * 60;
                          const isBest = t === min;
                          return (
                            <div key={i} className="flex-1 flex flex-col items-center justify-end" style={{ height: "100%" }}>
                              <div
                                className={`w-full rounded-t ${isBest ? "bg-rp-red" : "bg-rp-red/40"}`}
                                style={{ height: `${height}%`, minHeight: 4 }}
                              />
                            </div>
                          );
                        });
                      })()}
                    </div>
                  </div>
                )}

                {/* Tip */}
                {comparison.tip && (
                  <div className="bg-rp-card border border-rp-red/20 rounded-xl p-4">
                    <p className="text-rp-red text-xs font-semibold uppercase mb-1">Coach Tip</p>
                    <p className="text-white text-sm">{comparison.tip}</p>
                  </div>
                )}
              </div>
            )}
          </>
        )}
      </div>
      <BottomNav />
    </div>
  );
}
