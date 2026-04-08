"use client";

/* eslint-disable @next/next/no-img-element */

// ─── Mock data to show the full design with active sessions ─────────────

function formatLapTime(ms: number): string {
  if (ms <= 0) return "--:--.---";
  const totalSec = ms / 1000;
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toFixed(3).padStart(6, "0")}`;
}

function formatTimer(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function padPod(n: number): string {
  return String(n).padStart(2, "0");
}

const MOCK_PODS = [
  { number: 1, status: "in_session", driver: "Rahul Sharma", game: "assetto_corsa", car: "Ferrari 488 GT3", track: "Monza", speed: 247, rpm: 8200, maxRpm: 9000, gear: 5, lap: 5, remaining: 1423, bestLap: 102387, lastLap: 103012,
    laps: [
      { time: 102387, valid: true },
      { time: 103012, valid: true },
      { time: 105800, valid: false },
      { time: 103500, valid: true },
      { time: 104200, valid: true },
    ]},
  { number: 2, status: "in_session", driver: "Arjun Patel", game: "f1_25", car: "McLaren MCL39", track: "Silverstone", speed: 312, rpm: 11400, maxRpm: 12000, gear: 7, lap: 12, remaining: 234, bestLap: 78432, lastLap: 79100,
    laps: [
      { time: 78432, valid: true },
      { time: 79100, valid: true },
      { time: 82300, valid: false },
      { time: 79800, valid: true },
    ]},
  { number: 3, status: "idle" },
  { number: 4, status: "in_session", driver: "Vikram Singh", game: "iracing", car: "Porsche 911 RSR", track: "Spa-Francorchamps", speed: 189, rpm: 6800, maxRpm: 8500, gear: 4, lap: 8, remaining: 890, bestLap: 95200, lastLap: 96800,
    laps: [
      { time: 95200, valid: true },
      { time: 96800, valid: true },
      { time: 97100, valid: true },
    ]},
  { number: 5, status: "idle" },
  { number: 6, status: "in_session", driver: "Priya Nair", game: "le_mans_ultimate", car: "Toyota GR010", track: "Le Mans", speed: 276, rpm: 7500, maxRpm: 8000, gear: 6, lap: 3, remaining: 1680, bestLap: 134500, lastLap: 136200,
    laps: [
      { time: 134500, valid: true },
      { time: 136200, valid: true },
      { time: 140100, valid: false },
    ]},
  { number: 7, status: "idle" },
  { number: 8, status: "offline" },
];

// ─── RPM Arc Gauge (large SVG for pod cards) ───────────────────────────
function RpmGauge({ rpm, maxRpm, gear, speed }: { rpm: number; maxRpm: number; gear: number; speed: number }) {
  const size = 170;
  const cx = size / 2;
  const cy = size / 2 + 6;
  const r = 68;
  const startAngle = -225;
  const endAngle = 45;
  const totalAngle = endAngle - startAngle;

  const pct = Math.min(rpm / maxRpm, 1);
  const activeAngle = pct * totalAngle;

  const toRad = (deg: number) => (deg * Math.PI) / 180;
  const arcStart = startAngle;
  const arcEnd = startAngle + activeAngle;

  const x1 = cx + r * Math.cos(toRad(arcStart));
  const y1 = cy + r * Math.sin(toRad(arcStart));
  const x2 = cx + r * Math.cos(toRad(arcEnd));
  const y2 = cy + r * Math.sin(toRad(arcEnd));

  const bgX2 = cx + r * Math.cos(toRad(endAngle));
  const bgY2 = cy + r * Math.sin(toRad(endAngle));

  const largeArc = activeAngle > 180 ? 1 : 0;
  const bgLargeArc = totalAngle > 180 ? 1 : 0;

  const rpmColor = pct > 0.9 ? "#E10600" : pct > 0.7 ? "#F59E0B" : "#22C55E";
  const gearLabel = gear === -1 ? "R" : gear === 0 ? "N" : String(gear);
  const rpmDisplay = Math.round(rpm).toLocaleString();

  return (
    <svg width={size} height={size - 16} viewBox={`0 0 ${size} ${size - 16}`}>
      {/* Background arc */}
      <path
        d={`M ${x1} ${y1} A ${r} ${r} 0 ${bgLargeArc} 1 ${bgX2} ${bgY2}`}
        fill="none" stroke="#2A2A2A" strokeWidth={8} strokeLinecap="round"
      />
      {/* Active RPM arc */}
      {pct > 0.01 && (
        <path
          d={`M ${x1} ${y1} A ${r} ${r} 0 ${largeArc} 1 ${x2} ${y2}`}
          fill="none" stroke={rpmColor} strokeWidth={8} strokeLinecap="round"
          style={{ filter: `drop-shadow(0 0 8px ${rpmColor}90)` }}
        />
      )}
      {/* Gear number (large center) */}
      <text x={cx} y={cy - 12} textAnchor="middle" dominantBaseline="central"
        className="font-display" fill="white" fontSize="42" fontWeight="bold">
        {gearLabel}
      </text>
      {/* Speed */}
      <text x={cx} y={cy + 20} textAnchor="middle" dominantBaseline="central"
        className="font-mono" fill="white" fontSize="20" fontWeight="700">
        {Math.round(speed)}
      </text>
      <text x={cx} y={cy + 35} textAnchor="middle" dominantBaseline="central"
        className="font-mono" fill="#555" fontSize="9">
        km/h
      </text>
      {/* RPM value at bottom of arc */}
      <text x={cx} y={cy + 52} textAnchor="middle" dominantBaseline="central"
        className="font-mono" fill={rpmColor} fontSize="10" fontWeight="600">
        {rpmDisplay} RPM
      </text>
    </svg>
  );
}

// ─── Lap History (valid vs invalid) ─────────────────────────────────────
function LapHistory({ laps, bestLapMs }: { laps: { time: number; valid: boolean }[]; bestLapMs: number }) {
  return (
    <div className="flex flex-col gap-0.5 max-h-[60px] overflow-hidden">
      {laps.slice(0, 4).map((lap, i) => {
        const isBest = lap.valid && lap.time === bestLapMs;
        return (
          <div key={i} className="flex items-center justify-between text-xs font-mono">
            <span className="text-[#555] w-6">L{laps.length - i}</span>
            <span className={
              !lap.valid
                ? "text-red-500/70 line-through"
                : isBest
                  ? "text-purple-400 font-semibold"
                  : "text-emerald-400"
            }>
              {formatLapTime(lap.time)}
            </span>
            {!lap.valid && <span className="text-red-500/50 text-[9px] ml-1">INV</span>}
            {isBest && <span className="text-purple-400 text-[9px] ml-1">PB</span>}
            {lap.valid && !isBest && <span className="text-transparent text-[9px] ml-1">--</span>}
          </div>
        );
      })}
    </div>
  );
}

const GAME_INFO: Record<string, { name: string; abbr: string; logo: string; color: string }> = {
  assetto_corsa: { name: "Assetto Corsa", abbr: "AC", logo: "/game-logos/assetto-corsa.svg", color: "#E10600" },
  f1_25: { name: "F1 25", abbr: "F1", logo: "/game-logos/f1-25.svg", color: "#E10600" },
  iracing: { name: "iRacing", abbr: "iR", logo: "/game-logos/iracing.svg", color: "#0056B3" },
  le_mans_ultimate: { name: "Le Mans Ultimate", abbr: "LMU", logo: "/game-logos/le-mans-ultimate.svg", color: "#003366" },
};

const MOCK_LAPS = [
  { podNum: 1, track: "Monza", time: "1:42.387", driver: "Rahul Sharma" },
  { podNum: 4, track: "Spa", time: "1:35.200", driver: "Vikram Singh" },
  { podNum: 2, track: "Silverstone", time: "1:18.432", driver: "Arjun Patel" },
  { podNum: 6, track: "Le Mans", time: "2:14.500", driver: "Priya Nair" },
  { podNum: 1, track: "Monza", time: "1:43.012", driver: "Rahul Sharma" },
  { podNum: 2, track: "Silverstone", time: "1:19.100", driver: "Arjun Patel" },
];

export default function DemoPage() {
  return (
    <div className="h-screen flex flex-col bg-[#0A0A0A] overflow-hidden">
      {/* ── Header ── */}
      <header className="flex items-center justify-between px-6 py-3 bg-[#141414] border-b border-rp-red">
        <h1 className="text-xl tracking-widest uppercase text-white font-display">
          RACING <span className="text-rp-red">POINT</span>
        </h1>

        <div className="flex items-center gap-3">
          <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-emerald-500/15 text-emerald-500 text-xs font-semibold font-mono tracking-wide">
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
            3 AVAILABLE
          </span>
          <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-rp-red/15 text-rp-red text-xs font-semibold font-mono tracking-wide">
            <span className="w-1.5 h-1.5 rounded-full bg-rp-red" />
            4 RACING
          </span>
          <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-zinc-700/40 text-zinc-500 text-xs font-semibold font-mono tracking-wide">
            <span className="w-1.5 h-1.5 rounded-full bg-zinc-500" />
            1 OFFLINE
          </span>
        </div>

        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <span className="w-2 h-2 rounded-full bg-emerald-500 pulse-dot" />
            <span className="text-xs text-[#666]">LIVE</span>
          </div>
          <span className="text-lg text-white font-mono tabular-nums tracking-wider">14:32:18</span>
        </div>
      </header>

      {/* ── Pod Grid 4x2 ── */}
      <main className="flex-1 p-4 overflow-hidden">
        <div className="grid grid-cols-4 grid-rows-2 gap-3 h-full">
          {MOCK_PODS.map((pod) => {
            if (pod.status === "offline") {
              return (
                <div key={pod.number} className="rounded-lg bg-[#0F0F0F] border border-[#2A2A2A] flex flex-col items-center justify-center opacity-30">
                  <span className="text-3xl font-bold text-[#333] font-display">{padPod(pod.number)}</span>
                  <span className="text-xs text-[#333] mt-1 font-mono uppercase tracking-wider">Offline</span>
                </div>
              );
            }

            if (pod.status === "idle") {
              return (
                <div key={pod.number} className="rounded-lg bg-[#141414] border-l-[3px] border-l-emerald-500 border border-[#2A2A2A] flex flex-col items-center justify-center gap-2 glow-available">
                  <span className="text-4xl font-bold text-white font-display">{padPod(pod.number)}</span>
                  <span className="flex items-center gap-1.5 px-3 py-1 rounded-full bg-emerald-500/15 text-emerald-500 text-xs font-semibold font-mono uppercase tracking-wider">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                    Available
                  </span>
                  <span className="text-2xl font-bold text-white/60 font-display breathe">READY</span>
                </div>
              );
            }

            // Active session
            const game = GAME_INFO[pod.game!];
            const timerCritical = (pod.remaining ?? 0) < 300;

            return (
              <div key={pod.number} className="rounded-lg bg-[#141414] border-l-[3px] border-l-rp-red border border-[#2A2A2A] flex flex-col overflow-hidden glow-active">
                {/* Header bar */}
                <div className="px-3 pt-2.5 pb-2 border-b border-[#2A2A2A]">
                  {/* Row 1: Pod number + Game badge */}
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="text-sm font-bold text-rp-red font-display tracking-wider">POD {padPod(pod.number)}</span>
                    {game && (
                      <span className="flex items-center gap-1.5 px-2 py-0.5 rounded bg-[#1E1E1E] border border-[#333]">
                        <span className="w-2.5 h-2.5 rounded-sm flex-shrink-0" style={{ backgroundColor: game.color }} />
                        <span className="text-[11px] text-[#aaa] font-semibold tracking-wide">{game.abbr}</span>
                      </span>
                    )}
                  </div>
                  {/* Row 2: Driver name — large */}
                  <p className="text-base font-semibold text-white truncate leading-tight">{pod.driver}</p>
                  {/* Row 3: Car + Track */}
                  <p className="text-[10px] text-[#555] font-mono truncate mt-0.5">{pod.car}  ·  {pod.track}</p>
                </div>

                {/* Telemetry: Gauge centered + data below */}
                <div className="flex-1 flex flex-col overflow-hidden">
                  {/* Session timer + Lap count — compact top row */}
                  <div className="flex items-center justify-between px-3 py-1">
                    <span className={`text-base font-mono tabular-nums font-bold ${timerCritical ? "text-rp-red animate-pulse" : "text-white"}`}>
                      {formatTimer(pod.remaining!)}
                    </span>
                    <span className="text-sm font-mono text-[#888]">LAP {pod.lap}</span>
                  </div>

                  {/* RPM Gauge — centered */}
                  <div className="flex-1 flex items-center justify-center -mt-1 -mb-2">
                    <RpmGauge rpm={pod.rpm!} maxRpm={pod.maxRpm!} gear={pod.gear!} speed={pod.speed!} />
                  </div>

                  {/* Lap history — bottom */}
                  <div className="px-3 pb-2">
                    <LapHistory laps={pod.laps ?? []} bestLapMs={pod.bestLap!} />
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </main>

      {/* ── Bottom Ticker ── */}
      <div className="h-10 bg-[#111] border-t border-[#2A2A2A] flex items-center overflow-hidden relative">
        <div className="ticker-scroll flex items-center gap-0 whitespace-nowrap">
          {[0, 1].map((copy) => (
            <div key={copy} className="flex items-center gap-0">
              {MOCK_LAPS.map((item, i) => (
                <span key={`${copy}-${i}`} className="flex items-center gap-3 px-6 text-xs font-mono">
                  <span className="text-white font-semibold">POD {padPod(item.podNum)}</span>
                  <span className="text-rp-red">|</span>
                  <span className="text-[#888]">{item.track}</span>
                  <span className="text-rp-red">|</span>
                  <span className="text-white font-semibold tabular-nums">{item.time}</span>
                  <span className="text-rp-red">|</span>
                  <span className="text-[#888]">{item.driver}</span>
                </span>
              ))}
            </div>
          ))}
        </div>
      </div>

      {/* ── Footer ── */}
      <footer className="flex items-center justify-center py-2 bg-[#0A0A0A]">
        <span className="px-6 py-1.5 text-xs font-medium border border-[#2A2A2A] rounded-lg text-[#444]">
          DEMO MODE — Mock Data
        </span>
      </footer>
    </div>
  );
}
