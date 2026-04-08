"use client";

import { useMemo } from "react";
import type { TelemetryFrame, Lap, BillingSession } from "@/lib/types";

// ─── Helpers ───────────────────────────────────────────────────────────────

function fmtTime(ms: number): string {
  if (ms <= 0) return "--:--.---";
  const m = Math.floor(ms / 60000);
  const s = ((ms % 60000) / 1000).toFixed(3);
  return `${m}:${parseFloat(s) < 10 ? "0" : ""}${s}`;
}

function fmtSector(ms?: number): string {
  if (!ms || ms <= 0) return "--.---";
  return (ms / 1000).toFixed(3);
}

function fmtDelta(cur: number, best: number): { text: string; cls: string } {
  if (best <= 0 || cur <= 0) return { text: "", cls: "" };
  const d = (cur - best) / 1000;
  if (Math.abs(d) < 0.001) return { text: "+0.000", cls: "text-white/50" };
  return d < 0 ? { text: d.toFixed(3), cls: "text-emerald-400" } : { text: `+${d.toFixed(3)}`, cls: "text-amber-400" };
}

// ─── Types ─────────────────────────────────────────────────────────────────

interface RacingHUDProps {
  telemetry: TelemetryFrame;
  billing: BillingSession;
  recentLaps?: Lap[];
}

// ─── Vertical Pedal Bar ──────────────────────────────────────────────────

function PedalBar({ value, color, label }: { value: number; color: string; label: string }) {
  const pct = Math.min(100, Math.max(0, value * 100));
  return (
    <div className="flex flex-col items-center gap-1">
      <div className="relative w-[10px] h-[60px] bg-zinc-800/60 rounded-full overflow-hidden">
        <div
          className={`absolute bottom-0 left-0 right-0 rounded-full transition-[height] duration-75 ${color}`}
          style={{ height: `${pct}%` }}
        />
      </div>
      <span className="text-[10px] font-mono text-zinc-600 uppercase">{label}</span>
    </div>
  );
}

// ─── RPM Dots ─────────────────────────────────────────────────────────────

function RpmDots({ rpm }: { rpm: number }) {
  const max = 18000;
  const lit = Math.round((Math.min(rpm, max) / max) * 10);
  return (
    <div className="flex gap-1 items-center">
      {Array.from({ length: 10 }).map((_, i) => {
        const on = i < lit;
        const c = i < 3 ? (on ? "bg-emerald-500" : "bg-emerald-950/30")
          : i < 7 ? (on ? "bg-amber-400" : "bg-amber-950/30")
          : (on ? "bg-rp-red" : "bg-red-950/30");
        return <div key={i} className={`w-3 h-3 rounded-full transition-colors duration-75 ${c}`} />;
      })}
    </div>
  );
}

// ─── Main HUD — Top Bar ──────────────────────────────────────────────────

export function RacingHUD({ telemetry, billing, recentLaps = [] }: RacingHUDProps) {
  const { bestLap, lastLap, bestSectors } = useMemo(() => {
    if (!recentLaps.length) return { bestLap: null, lastLap: null, bestSectors: { s1: 0, s2: 0, s3: 0 } };
    const valid = recentLaps.filter((l) => l.valid && l.lap_time_ms > 0);
    const sorted = [...valid].sort((a, b) => a.lap_time_ms - b.lap_time_ms);
    const best = sorted[0] || null;
    const last = valid.length ? valid.reduce((a, b) => ((a.lap_number ?? 0) > (b.lap_number ?? 0) ? a : b)) : null;
    const pick = (fn: (l: Lap) => number | undefined) => {
      const vals = valid.map(fn).filter((v): v is number => !!v && v > 0);
      return vals.length ? Math.min(...vals) : 0;
    };
    return { bestLap: best, lastLap: last, bestSectors: { s1: pick(l => l.sector1_ms), s2: pick(l => l.sector2_ms), s3: pick(l => l.sector3_ms) } };
  }, [recentLaps]);

  const remaining = billing.remaining_seconds ?? 0;
  const rm = Math.floor(remaining / 60);
  const rs = remaining % 60;
  const speed = Math.round(telemetry.speed_kmh);
  const gear = telemetry.gear === -1 ? "R" : telemetry.gear === 0 ? "N" : String(telemetry.gear);

  const curSector = lastLap?.sector1_ms && !lastLap?.sector2_ms ? 1
    : lastLap?.sector2_ms && !lastLap?.sector3_ms ? 2 : 3;

  return (
    <div className="w-full bg-zinc-950/85 backdrop-blur-sm border-b border-zinc-800/40">
      <div className="relative flex items-center px-6 h-[105px]">

        {/* ── LEFT: Lap + Current Time + Sectors (left-aligned) ──── */}
        <div className="flex items-center gap-5">
          {/* Lap number */}
          <div className="flex items-baseline gap-1.5">
            <span className="text-sm text-zinc-500 font-mono uppercase">Lap</span>
            <span className="text-3xl font-bold text-white tabular-nums" style={{ fontFamily: "var(--font-display, 'Orbitron'), sans-serif" }}>
              {telemetry.lap_number}
            </span>
          </div>

          <div className="w-px h-10 bg-zinc-800/50" />

          {/* Current lap time */}
          <span className="text-xl font-mono text-white tabular-nums font-semibold">{fmtTime(telemetry.lap_time_ms)}</span>

          <div className="w-px h-10 bg-zinc-800/50" />

          {/* Sectors inline */}
          <div className="flex gap-2">
            {[
              { l: "1", ms: lastLap?.sector1_ms, best: bestSectors.s1, active: curSector === 1 },
              { l: "2", ms: lastLap?.sector2_ms, best: bestSectors.s2, active: curSector === 2 },
              { l: "3", ms: lastLap?.sector3_ms, best: bestSectors.s3, active: curSector === 3 },
            ].map((s) => {
              const delta = s.ms && s.best ? fmtDelta(s.ms, s.best) : null;
              return (
                <div key={s.l} className={`flex items-center gap-2 px-3 py-1 rounded font-mono ${
                  s.active ? "bg-rp-red/10 text-rp-red" : "text-zinc-500"
                }`}>
                  <span className="text-sm uppercase font-semibold">S{s.l}</span>
                  <span className="text-xl text-white tabular-nums font-bold">{fmtSector(s.ms)}</span>
                  {delta?.text && <span className={`text-sm tabular-nums ${delta.cls}`}>{delta.text}</span>}
                </div>
              );
            })}
          </div>
        </div>

        {/* ── CENTER: Pedals | Speed | Gear | RPM (absolute center) ── */}
        <div className="absolute left-1/2 -translate-x-1/2 z-10 flex items-center gap-4 bg-zinc-950/85">
          {/* Throttle pedal */}
          <PedalBar value={telemetry.throttle} color="bg-emerald-500" label="T" />

          {/* Speed + Gear cluster */}
          <div className="flex items-baseline gap-2">
            <span
              className="text-[68px] font-black text-white leading-none tabular-nums"
              style={{ fontFamily: "var(--font-display, 'Orbitron'), sans-serif" }}
            >
              {speed}
            </span>
            <div className="flex flex-col items-center">
              <span
                className="text-3xl font-black text-white/80"
                style={{ fontFamily: "var(--font-display, 'Orbitron'), sans-serif" }}
              >
                {gear}
              </span>
              <span className="text-[11px] text-zinc-600 font-mono uppercase -mt-0.5">Gear</span>
            </div>
          </div>

          {/* Brake pedal */}
          <PedalBar value={telemetry.brake} color="bg-rp-red" label="B" />

          {/* RPM dots */}
          <div className="flex flex-col items-center gap-1 ml-2">
            <RpmDots rpm={telemetry.rpm} />
            <span className="text-sm font-mono text-zinc-500 tabular-nums">{telemetry.rpm.toLocaleString()}</span>
          </div>
        </div>

        {/* ── RIGHT: Last | Best | Timer (right-aligned) ────────── */}
        <div className="ml-auto flex items-center gap-5">
          {/* Last lap */}
          <div className="text-right">
            <p className="text-xs text-zinc-500 font-mono uppercase tracking-wider">Last</p>
            <p className="text-xl font-mono tabular-nums text-zinc-300 font-semibold">{lastLap ? fmtTime(lastLap.lap_time_ms) : "--:--.---"}</p>
            {lastLap && bestLap && (() => {
              const d = fmtDelta(lastLap.lap_time_ms, bestLap.lap_time_ms);
              return d.text ? <p className={`text-sm font-mono tabular-nums ${d.cls}`}>{d.text}</p> : null;
            })()}
          </div>

          <div className="w-px h-10 bg-zinc-800/50" />

          {/* Best lap */}
          <div className="text-right">
            <p className="text-xs text-rp-purple font-mono uppercase tracking-wider">Best</p>
            <p className="text-xl font-mono tabular-nums text-rp-purple font-semibold">{bestLap ? fmtTime(bestLap.lap_time_ms) : "--:--.---"}</p>
          </div>

          <div className="w-px h-10 bg-zinc-800/50" />

          {/* Session timer */}
          <div className="text-right">
            <p className="text-xs text-zinc-500 font-mono uppercase tracking-wider">Session</p>
            <p className={`text-2xl font-mono tabular-nums font-bold ${
              remaining < 30 ? "text-rp-red animate-pulse" : remaining < 120 ? "text-amber-400" : "text-zinc-300"
            }`}>
              {rm}:{String(rs).padStart(2, "0")}
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}
