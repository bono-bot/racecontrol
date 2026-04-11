'use client';

import { useEffect, useState } from 'react';
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell } from 'recharts';

interface SuspectSession {
  session_id: string;
  driver_name: string | null;
  driver_id: string | null;
  pod_id: string | null;
  suspect_reasons: string[];
  telemetry_coverage_pct: number | null;
  lap_count_actual: number | null;
  lap_count_expected: number | null;
  lap_count_flag: string | null;
  started_at: string | null;
  ended_at: string | null;
}

interface LapHeatmapEntry {
  lap_id: string;
  lap_number: number;
  lap_time_ms: number;
  valid: boolean;
  suspect: boolean;
  sample_count: number;
}

function fmtClock(iso: string | null) {
  if (!iso) return '-';
  const d = new Date(iso);
  return d.toLocaleString('en-IN', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', hour12: true });
}

function lapCellColor(sampleCount: number): string {
  if (sampleCount === 0) return '#5A5A5A';
  if (sampleCount < 50) return '#E10600';
  if (sampleCount < 150) return '#f59e0b';
  return '#22c55e';
}

export default function SuspectSessionsPage() {
  const [sessions, setSessions] = useState<SuspectSession[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [heatmapData, setHeatmapData] = useState<LapHeatmapEntry[]>([]);
  const [heatmapLoading, setHeatmapLoading] = useState(false);

  useEffect(() => {
    fetch('/api/rc/admin/suspect-sessions?page=0&limit=50')
      .then(r => r.json())
      .then(data => {
        setSessions(data.sessions || []);
        setTotal(data.total || 0);
        setLoading(false);
      })
      .catch(e => {
        setError(String(e));
        setLoading(false);
      });
  }, []);

  function handleSelectSession(sessionId: string) {
    if (selectedId === sessionId) {
      setSelectedId(null);
      setHeatmapData([]);
      return;
    }
    setSelectedId(sessionId);
    setHeatmapData([]);
    setHeatmapLoading(true);
    fetch(`/api/rc/admin/sessions/${sessionId}/telemetry-heatmap`)
      .then(r => r.json())
      .then(data => {
        setHeatmapData(data.laps || []);
        setHeatmapLoading(false);
      })
      .catch(() => setHeatmapLoading(false));
  }

  if (loading) return (
    <div className="p-6 text-gray-400">Loading suspect sessions...</div>
  );

  if (error) return (
    <div className="p-6 text-red-400">Error: {error}</div>
  );

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-white" style={{ fontFamily: 'Montserrat, sans-serif' }}>
          Suspect Sessions
        </h1>
        <span className="text-gray-400 text-sm">{total} total flagged</span>
      </div>

      {sessions.length === 0 ? (
        <div className="bg-[#222] rounded-lg p-8 text-center text-gray-400">
          No suspect sessions found. Phase 363 must be deployed and sessions must have completed.
        </div>
      ) : (
        <div className="space-y-2">
          {sessions.map(s => (
            <div key={s.session_id}>
              <div
                className="bg-[#222] rounded-lg p-4 cursor-pointer hover:bg-[#2a2a2a] transition-colors border border-[#333]"
                style={{ borderColor: selectedId === s.session_id ? '#E10600' : '#333' }}
                onClick={() => handleSelectSession(s.session_id)}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-4">
                    <span className="text-white font-medium">{s.driver_name || s.driver_id || 'Unknown'}</span>
                    <span className="text-gray-400 text-sm">Pod {s.pod_id}</span>
                    <span className="bg-red-900/40 text-red-400 text-xs px-2 py-0.5 rounded border border-red-800">
                      SUSPECT
                    </span>
                  </div>
                  <div className="flex items-center gap-6 text-sm text-gray-400">
                    <span>Coverage: {s.telemetry_coverage_pct != null ? `${s.telemetry_coverage_pct.toFixed(0)}%` : '-'}</span>
                    <span>Laps: {s.lap_count_actual ?? '-'}/{s.lap_count_expected ?? '-'}</span>
                    <span>{fmtClock(s.ended_at)}</span>
                  </div>
                </div>
                {s.suspect_reasons.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-1">
                    {s.suspect_reasons.map((r, i) => (
                      <span key={i} className="bg-yellow-900/30 text-yellow-400 text-xs px-2 py-0.5 rounded border border-yellow-800/50">
                        {r}
                      </span>
                    ))}
                  </div>
                )}
              </div>

              {selectedId === s.session_id && (
                <div className="bg-[#1a1a1a] border border-[#333] rounded-b-lg p-4 -mt-1">
                  <h3 className="text-sm text-gray-400 mb-3">Per-Lap Telemetry Coverage</h3>
                  {heatmapLoading ? (
                    <div className="text-gray-500 text-sm">Loading heatmap...</div>
                  ) : heatmapData.length === 0 ? (
                    <div className="text-gray-500 text-sm">No lap data available.</div>
                  ) : (
                    <ResponsiveContainer width="100%" height={120}>
                      <BarChart data={heatmapData} margin={{ top: 4, right: 8, left: 0, bottom: 4 }}>
                        <XAxis dataKey="lap_number" tick={{ fill: '#9ca3af', fontSize: 11 }} label={{ value: 'Lap', position: 'insideBottom', offset: -2, fill: '#9ca3af', fontSize: 11 }} />
                        <YAxis tick={{ fill: '#9ca3af', fontSize: 11 }} label={{ value: 'Samples', angle: -90, position: 'insideLeft', fill: '#9ca3af', fontSize: 11 }} />
                        <Tooltip
                          formatter={(v: number) => [`${v} samples`, 'Coverage']}
                          contentStyle={{ background: '#222', border: '1px solid #333', color: '#fff' }}
                        />
                        <Bar dataKey="sample_count" radius={[2, 2, 0, 0]}>
                          {heatmapData.map((entry, index) => (
                            <Cell key={index} fill={lapCellColor(entry.sample_count)} />
                          ))}
                        </Bar>
                      </BarChart>
                    </ResponsiveContainer>
                  )}
                  <div className="mt-2 flex gap-4 text-xs text-gray-500">
                    <span><span className="inline-block w-3 h-3 rounded mr-1" style={{ background: '#22c55e' }} />Good (&gt;150 samples)</span>
                    <span><span className="inline-block w-3 h-3 rounded mr-1" style={{ background: '#f59e0b' }} />Partial (50-150)</span>
                    <span><span className="inline-block w-3 h-3 rounded mr-1" style={{ background: '#E10600' }} />Poor (&lt;50)</span>
                    <span><span className="inline-block w-3 h-3 rounded mr-1" style={{ background: '#5A5A5A' }} />No data</span>
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
