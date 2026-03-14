"use client";

import { useEffect, useState } from "react";
import { publicApi } from "@/lib/api";

function formatLapTime(ms: number | null | undefined): string {
  if (!ms || ms <= 0) return "-";
  const mins = Math.floor(ms / 60000);
  const secs = Math.floor((ms % 60000) / 1000);
  const millis = ms % 1000;
  return `${mins}:${secs.toString().padStart(2, "0")}.${millis.toString().padStart(3, "0")}`;
}

const SIM_TYPES = [
  { value: "assetto_corsa", label: "Assetto Corsa" },
  { value: "f1_25", label: "F1 25" },
] as const;

interface CircuitRecord {
  track: string;
  car: string;
  sim_type: string;
  best_lap_ms: number;
  best_lap_display: string;
  driver: string;
  achieved_at: string;
}

interface VehicleRecord {
  track: string;
  sim_type: string;
  best_lap_ms: number;
  best_lap_display: string;
  driver: string;
  achieved_at: string;
}

export default function RecordsPage() {
  const [records, setRecords] = useState<CircuitRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [simType, setSimType] = useState("assetto_corsa");
  const [selectedCar, setSelectedCar] = useState<string | null>(null);
  const [vehicleRecords, setVehicleRecords] = useState<VehicleRecord[]>([]);
  const [loadingVehicle, setLoadingVehicle] = useState(false);

  // Fetch circuit records on mount and when sim_type changes
  useEffect(() => {
    setLoading(true);
    setSelectedCar(null);
    publicApi.circuitRecords({ sim_type: simType }).then((data: { records?: CircuitRecord[] }) => {
      setRecords(data.records || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, [simType]);

  // Fetch vehicle records when a car is selected
  useEffect(() => {
    if (!selectedCar) return;
    setLoadingVehicle(true);
    publicApi.vehicleRecords(selectedCar, { sim_type: simType }).then((data: { records?: VehicleRecord[] }) => {
      setVehicleRecords(data.records || []);
      setLoadingVehicle(false);
    }).catch(() => setLoadingVehicle(false));
  }, [selectedCar, simType]);

  // Group circuit records by track
  const recordsByTrack = records.reduce<Record<string, CircuitRecord[]>>((acc, r) => {
    if (!acc[r.track]) acc[r.track] = [];
    acc[r.track].push(r);
    return acc;
  }, {});

  // Unique cars from records for the car filter
  const uniqueCars = Array.from(new Set(records.map(r => r.car))).sort();

  return (
    <div className="min-h-screen bg-rp-dark">
      {/* Header */}
      <div className="bg-gradient-to-b from-rp-red/20 to-transparent pt-12 pb-8 px-4">
        <div className="max-w-2xl mx-auto text-center">
          <h1 className="text-3xl font-bold text-white tracking-tight">Records</h1>
          <p className="text-rp-grey text-sm mt-1">Circuit and vehicle records</p>
        </div>
      </div>

      <div className="max-w-2xl mx-auto px-4 pb-8">
        {/* Controls */}
        <div className="flex flex-wrap items-center gap-3 mb-6">
          <select
            value={simType}
            onChange={(e) => setSimType(e.target.value)}
            className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-white focus:border-rp-red focus:outline-none"
          >
            {SIM_TYPES.map((st) => (
              <option key={st.value} value={st.value}>
                {st.label}
              </option>
            ))}
          </select>

          <select
            value={selectedCar || ""}
            onChange={(e) => setSelectedCar(e.target.value || null)}
            className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-white focus:border-rp-red focus:outline-none"
          >
            <option value="">All Cars</option>
            {uniqueCars.map((car) => (
              <option key={car} value={car}>
                {car}
              </option>
            ))}
          </select>
        </div>

        {loading ? (
          <div className="flex justify-center py-12">
            <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
          </div>
        ) : selectedCar ? (
          /* Vehicle Records View */
          <div>
            <button
              onClick={() => setSelectedCar(null)}
              className="text-rp-red text-sm mb-3 flex items-center gap-1"
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="w-4 h-4">
                <path d="M19 12H5M12 19l-7-7 7-7" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
              All Records
            </button>

            <h2 className="text-xl font-bold text-white mb-4">{selectedCar}</h2>

            {loadingVehicle ? (
              <div className="flex justify-center py-12">
                <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
              </div>
            ) : vehicleRecords.length === 0 ? (
              <p className="text-rp-grey text-sm text-center py-8">No records for this car yet</p>
            ) : (
              <>
                {/* Desktop table */}
                <div className="hidden sm:block bg-rp-card border border-rp-border rounded-xl overflow-hidden">
                  <div className="grid grid-cols-[1fr_90px_1fr] gap-1 px-4 py-2 text-[10px] text-rp-grey uppercase tracking-wider border-b border-rp-border">
                    <span>Track</span>
                    <span className="text-right">Best Lap</span>
                    <span className="text-right">Driver</span>
                  </div>
                  {vehicleRecords.map((r) => (
                    <div
                      key={`${r.track}-${r.sim_type}`}
                      className="grid grid-cols-[1fr_90px_1fr] gap-1 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0"
                    >
                      <span className="text-sm text-white truncate">{r.track}</span>
                      <span className="text-sm font-mono text-white text-right" style={{ fontSize: "14px" }}>{r.best_lap_display || formatLapTime(r.best_lap_ms)}</span>
                      <span className="text-xs text-rp-grey truncate text-right self-center">{r.driver}</span>
                    </div>
                  ))}
                </div>

                {/* Mobile cards */}
                <div className="sm:hidden space-y-2">
                  {vehicleRecords.map((r) => (
                    <div
                      key={`m-${r.track}-${r.sim_type}`}
                      className="bg-rp-card border border-rp-border rounded-xl p-3"
                    >
                      <div className="flex justify-between items-start">
                        <div className="flex-1 min-w-0">
                          <p className="text-sm text-white truncate" style={{ fontSize: "14px" }}>{r.track}</p>
                          <p className="text-xs text-rp-grey">{r.driver}</p>
                        </div>
                        <span className="font-mono text-white font-medium ml-3" style={{ fontSize: "14px" }}>
                          {r.best_lap_display || formatLapTime(r.best_lap_ms)}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </>
            )}
          </div>
        ) : records.length === 0 ? (
          <p className="text-rp-grey text-sm text-center py-8">No records yet. Be the first!</p>
        ) : (
          /* Circuit Records grouped by track */
          <div className="space-y-4">
            {Object.entries(recordsByTrack).map(([track, trackRecords]) => (
              <div key={track} className="bg-rp-card border border-rp-border rounded-xl overflow-hidden">
                <div className="px-4 py-3 border-b border-rp-border">
                  <h2 className="text-sm font-medium text-white">{track}</h2>
                </div>

                {/* Desktop table */}
                <div className="hidden sm:block">
                  {trackRecords.map((r) => (
                    <div
                      key={`${r.track}-${r.car}`}
                      className="grid grid-cols-[1fr_90px_1fr] gap-1 px-4 py-2.5 border-b border-rp-border/50 last:border-b-0 cursor-pointer hover:bg-white/5"
                      onClick={() => setSelectedCar(r.car)}
                    >
                      <span className="text-sm text-white truncate">{r.car}</span>
                      <span className="text-sm font-mono text-white text-right" style={{ fontSize: "14px" }}>{r.best_lap_display || formatLapTime(r.best_lap_ms)}</span>
                      <span className="text-xs text-rp-grey truncate text-right self-center">{r.driver}</span>
                    </div>
                  ))}
                </div>

                {/* Mobile cards */}
                <div className="sm:hidden">
                  {trackRecords.map((r) => (
                    <div
                      key={`m-${r.track}-${r.car}`}
                      className="px-4 py-3 border-b border-rp-border/50 last:border-b-0 cursor-pointer active:bg-white/5"
                      onClick={() => setSelectedCar(r.car)}
                    >
                      <div className="flex justify-between items-start">
                        <div className="flex-1 min-w-0">
                          <p className="text-sm text-white truncate" style={{ fontSize: "14px" }}>{r.car}</p>
                          <p className="text-xs text-rp-grey">{r.driver}</p>
                        </div>
                        <span className="font-mono text-white font-medium ml-3" style={{ fontSize: "14px" }}>
                          {r.best_lap_display || formatLapTime(r.best_lap_ms)}
                        </span>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        )}

        {/* Footer */}
        <div className="text-center mt-8">
          <p className="text-rp-grey text-xs">RacingPoint</p>
        </div>
      </div>
    </div>
  );
}
