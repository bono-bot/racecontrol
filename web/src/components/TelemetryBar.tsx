import type { TelemetryFrame } from "@/lib/api";

export default function TelemetryBar({ data }: { data: TelemetryFrame | null }) {
  if (!data) {
    return (
      <div className="bg-rp-card border border-rp-border rounded-lg p-4 text-center text-rp-grey text-sm">
        No live telemetry — waiting for pod connection
      </div>
    );
  }

  return (
    <div className="bg-rp-card border border-rp-border rounded-lg p-4">
      <div className="flex items-center justify-between mb-3">
        <div>
          <span className="text-rp-red font-bold">{data.driver_name}</span>
          <span className="text-rp-grey text-sm ml-2">{data.car} @ {data.track}</span>
        </div>
        <span className="text-rp-grey text-xs">Lap {data.lap_number}</span>
      </div>
      <div className="grid grid-cols-5 gap-4">
        {/* Speed */}
        <div className="text-center">
          <div className="text-2xl font-mono font-bold text-neutral-200">
            {Math.round(data.speed_kmh)}
          </div>
          <div className="text-xs text-rp-grey">km/h</div>
        </div>
        {/* Throttle */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs text-rp-grey">Throttle</span>
            <span className="text-xs text-emerald-400">{Math.round(data.throttle * 100)}%</span>
          </div>
          <div className="h-2 bg-rp-card rounded-full overflow-hidden">
            <div
              className="h-full bg-emerald-500 rounded-full transition-all duration-100"
              style={{ width: `${data.throttle * 100}%` }}
            />
          </div>
        </div>
        {/* Brake */}
        <div>
          <div className="flex items-center justify-between mb-1">
            <span className="text-xs text-rp-grey">Brake</span>
            <span className="text-xs text-red-400">{Math.round(data.brake * 100)}%</span>
          </div>
          <div className="h-2 bg-rp-card rounded-full overflow-hidden">
            <div
              className="h-full bg-red-500 rounded-full transition-all duration-100"
              style={{ width: `${data.brake * 100}%` }}
            />
          </div>
        </div>
        {/* Gear */}
        <div className="text-center">
          <div className="text-2xl font-mono font-bold text-neutral-200">
            {data.gear === 0 ? "N" : data.gear === -1 ? "R" : data.gear}
          </div>
          <div className="text-xs text-rp-grey">Gear</div>
        </div>
        {/* RPM */}
        <div className="text-center">
          <div className="text-2xl font-mono font-bold text-neutral-200">
            {(data.rpm / 1000).toFixed(1)}k
          </div>
          <div className="text-xs text-rp-grey">RPM</div>
        </div>
      </div>
    </div>
  );
}
