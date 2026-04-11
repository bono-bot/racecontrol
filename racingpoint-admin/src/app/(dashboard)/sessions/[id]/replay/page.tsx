'use client';

import { useEffect, useRef, useState } from 'react';
import { useParams } from 'next/navigation';

interface ReplayEvent {
  type: 'lap_start' | 'lap_end' | 'telemetry';
  lap: number;
  offset_ms?: number;
  lap_time_ms?: number;
  speed?: number;
  throttle?: number;
  brake?: number;
  steering?: number;
  gear?: number;
  rpm?: number;
  valid?: boolean;
}

interface ReplayData {
  session_id: string;
  lap_count: number;
  events: ReplayEvent[];
  truncated: boolean;
  total_events: number;
}

const SPEEDS = [1, 2, 5, 10];

function GaugeBar({ label, value, max, color }: { label: string; value: number | undefined; max: number; color: string }) {
  const pct = Math.min(100, ((value ?? 0) / max) * 100);
  return (
    <div className="space-y-1">
      <div className="flex justify-between text-xs text-gray-400">
        <span>{label}</span>
        <span>{value !== undefined ? value.toFixed(1) : '-'}</span>
      </div>
      <div className="h-2 rounded bg-[#333] overflow-hidden">
        <div className="h-full rounded transition-all duration-75" style={{ width: `${pct}%`, background: color }} />
      </div>
    </div>
  );
}

export default function SessionReplayPage() {
  const params = useParams();
  const sessionId = params.id as string;

  const [data, setData] = useState<ReplayData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    if (!sessionId) return;
    fetch(`/api/rc/admin/sessions/${sessionId}/replay`)
      .then(r => r.json())
      .then((d: ReplayData) => {
        setData(d);
        setLoading(false);
      })
      .catch(e => {
        setError(String(e));
        setLoading(false);
      });
  }, [sessionId]);

  useEffect(() => {
    if (intervalRef.current) clearInterval(intervalRef.current);
    if (playing && data) {
      intervalRef.current = setInterval(() => {
        setCurrentIndex(prev => {
          const next = prev + speed;
          if (next >= data.events.length) {
            setPlaying(false);
            return data.events.length - 1;
          }
          return next;
        });
      }, 16); // ~60fps tick
    }
    return () => { if (intervalRef.current) clearInterval(intervalRef.current); };
  }, [playing, speed, data]);

  if (loading) return <div className="p-6 text-gray-400">Loading replay data...</div>;
  if (error) return <div className="p-6 text-red-400">Error: {error}</div>;
  if (!data || data.events.length === 0) return (
    <div className="p-6 text-gray-400">
      No telemetry data available for session {sessionId}.
    </div>
  );

  const current = data.events[currentIndex];
  const progress = (currentIndex / (data.events.length - 1)) * 100;

  const lapMarkers = data.events.reduce<{index: number; lap: number}[]>((acc, e, i) => {
    if (e.type === 'lap_start') acc.push({ index: i, lap: e.lap });
    return acc;
  }, []);

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white" style={{ fontFamily: 'Montserrat, sans-serif' }}>
            Session Replay
          </h1>
          <p className="text-sm text-gray-500 mt-0.5">
            {sessionId} -- {data.lap_count} laps -- {data.total_events} events
            {data.truncated && <span className="text-yellow-400 ml-2">(truncated at 10,000 events)</span>}
          </p>
        </div>
        <span className="text-sm text-gray-400">
          Lap {current?.lap ?? '-'} / {data.lap_count}
        </span>
      </div>

      {/* Telemetry gauges */}
      <div className="bg-[#222] rounded-lg p-4 border border-[#333]">
        <div className="grid grid-cols-2 md:grid-cols-3 gap-4">
          <div className="text-center">
            <div className="text-3xl font-bold text-white" style={{ fontFamily: 'Montserrat, sans-serif' }}>
              {current?.speed !== undefined ? `${current.speed.toFixed(0)}` : '-'}
            </div>
            <div className="text-xs text-gray-500">km/h</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold text-white" style={{ fontFamily: 'Montserrat, sans-serif' }}>
              {current?.gear !== undefined ? current.gear : '-'}
            </div>
            <div className="text-xs text-gray-500">GEAR</div>
          </div>
          <div className="text-center">
            <div className="text-3xl font-bold text-white" style={{ fontFamily: 'Montserrat, sans-serif' }}>
              {current?.rpm !== undefined ? `${(current.rpm / 1000).toFixed(1)}k` : '-'}
            </div>
            <div className="text-xs text-gray-500">RPM</div>
          </div>
        </div>
        <div className="mt-4 space-y-2">
          <GaugeBar label="Throttle" value={current?.throttle} max={1} color="#22c55e" />
          <GaugeBar label="Brake" value={current?.brake} max={1} color="#E10600" />
          <GaugeBar label="Steering" value={current?.steering !== undefined ? Math.abs(current.steering) : undefined} max={1} color="#3b82f6" />
        </div>
      </div>

      {/* Scrubber */}
      <div className="bg-[#1a1a1a] rounded-lg p-4 border border-[#333] space-y-3">
        <div className="relative">
          <input
            type="range"
            min={0}
            max={data.events.length - 1}
            value={currentIndex}
            onChange={e => {
              setPlaying(false);
              setCurrentIndex(Number(e.target.value));
            }}
            className="w-full h-2 rounded appearance-none cursor-pointer"
            style={{ background: `linear-gradient(to right, #E10600 ${progress}%, #333 ${progress}%)` }}
          />
          {lapMarkers.map(({ index, lap }) => (
            <div
              key={lap}
              className="absolute top-0 w-0.5 h-2 bg-white/40"
              style={{ left: `${(index / (data.events.length - 1)) * 100}%` }}
              title={`Lap ${lap}`}
            />
          ))}
        </div>

        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <button
              onClick={() => setPlaying(p => !p)}
              className="px-4 py-1.5 rounded text-sm font-medium text-white"
              style={{ background: playing ? '#5A5A5A' : '#E10600', fontFamily: 'Montserrat, sans-serif' }}
            >
              {playing ? 'Pause' : 'Play'}
            </button>
            <button
              onClick={() => { setPlaying(false); setCurrentIndex(0); }}
              className="px-3 py-1.5 bg-[#333] rounded text-sm text-gray-300 hover:bg-[#444]"
            >
              Reset
            </button>
          </div>
          <div className="flex items-center gap-1">
            {SPEEDS.map(s => (
              <button
                key={s}
                onClick={() => setSpeed(s)}
                className="px-2 py-1 rounded text-xs"
                style={{
                  background: speed === s ? '#E10600' : '#333',
                  color: speed === s ? '#fff' : '#9ca3af',
                  fontFamily: 'Montserrat, sans-serif'
                }}
              >
                {s}x
              </button>
            ))}
          </div>
        </div>
      </div>

      <p className="text-xs text-gray-600">
        Event {currentIndex + 1} of {data.events.length}. Session: {sessionId}
      </p>
    </div>
  );
}
