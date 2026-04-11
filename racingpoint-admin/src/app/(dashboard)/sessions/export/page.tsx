'use client';

import { useState } from 'react';

interface EstimateResult {
  billing_rows: number;
  lap_rows: number;
  telemetry_rows: number;
  total_rows: number;
}

function todayStr() {
  return new Date().toISOString().slice(0, 10);
}

function thirtyDaysAgo() {
  const d = new Date();
  d.setDate(d.getDate() - 30);
  return d.toISOString().slice(0, 10);
}

export default function BatchExportPage() {
  const [from, setFrom] = useState(thirtyDaysAgo());
  const [to, setTo] = useState(todayStr());
  const [format, setFormat] = useState<'csv' | 'jsonl'>('csv');
  const [includeBilling, setIncludeBilling] = useState(true);
  const [includeLaps, setIncludeLaps] = useState(true);
  const [includeTelemetry, setIncludeTelemetry] = useState(false);
  const [estimate, setEstimate] = useState<EstimateResult | null>(null);
  const [estimating, setEstimating] = useState(false);
  const [estimateError, setEstimateError] = useState<string | null>(null);

  const daysDiff = Math.ceil((new Date(to).getTime() - new Date(from).getTime()) / (1000 * 60 * 60 * 24));
  const rangeExceeds = daysDiff > 30;

  const includeParam = [
    includeBilling ? 'billing' : null,
    includeLaps ? 'laps' : null,
    includeTelemetry ? 'telemetry' : null,
  ].filter(Boolean).join(',');

  function buildQueryString() {
    return `from=${from}&to=${to}&format=${format}&include=${includeParam}`;
  }

  async function handleEstimate() {
    if (rangeExceeds) return;
    setEstimating(true);
    setEstimateError(null);
    try {
      const res = await fetch(`/api/rc/admin/export/estimate?from=${from}&to=${to}&include=${includeParam}`);
      const data = await res.json();
      if (data.error) throw new Error(data.error);
      setEstimate(data);
    } catch (e) {
      setEstimateError(String(e));
    } finally {
      setEstimating(false);
    }
  }

  function handleExport() {
    if (rangeExceeds || !includeParam) return;
    window.open(`/api/rc/admin/export?${buildQueryString()}`);
  }

  return (
    <div className="p-6 space-y-6 max-w-2xl">
      <h1 className="text-2xl font-bold text-white" style={{ fontFamily: 'Montserrat, sans-serif' }}>
        Batch Export
      </h1>
      <p className="text-sm text-gray-400">
        Export session data as CSV or JSONL for offline analysis. Maximum 30-day range per export.
      </p>

      {/* Date range */}
      <div className="bg-[#222] rounded-lg p-4 border border-[#333] space-y-4">
        <h2 className="text-sm font-medium text-gray-300" style={{ fontFamily: 'Montserrat, sans-serif' }}>Date Range</h2>
        <div className="flex items-center gap-3">
          <div className="flex-1">
            <label className="text-xs text-gray-500 block mb-1">From</label>
            <input
              type="date"
              value={from}
              onChange={e => setFrom(e.target.value)}
              className="w-full bg-[#333] border border-[#444] rounded px-3 py-2 text-white text-sm"
            />
          </div>
          <div className="flex-1">
            <label className="text-xs text-gray-500 block mb-1">To</label>
            <input
              type="date"
              value={to}
              onChange={e => setTo(e.target.value)}
              className="w-full bg-[#333] border border-[#444] rounded px-3 py-2 text-white text-sm"
            />
          </div>
        </div>
        {rangeExceeds && (
          <p className="text-sm text-red-400">
            Date range is {daysDiff} days -- maximum is 30 days. Please narrow the range.
          </p>
        )}
        {!rangeExceeds && daysDiff > 0 && (
          <p className="text-xs text-gray-500">{daysDiff} day{daysDiff !== 1 ? 's' : ''} selected</p>
        )}
      </div>

      {/* Format + includes */}
      <div className="bg-[#222] rounded-lg p-4 border border-[#333] space-y-4">
        <h2 className="text-sm font-medium text-gray-300" style={{ fontFamily: 'Montserrat, sans-serif' }}>Format</h2>
        <div className="flex gap-4">
          {(['csv', 'jsonl'] as const).map(f => (
            <label key={f} className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="format"
                value={f}
                checked={format === f}
                onChange={() => setFormat(f)}
                className="accent-[#E10600]"
              />
              <span className="text-sm text-gray-300">{f.toUpperCase()}</span>
              {f === 'csv' && <span className="text-xs text-gray-500">(recommended for Excel)</span>}
              {f === 'jsonl' && <span className="text-xs text-gray-500">(for tooling)</span>}
            </label>
          ))}
        </div>

        <h2 className="text-sm font-medium text-gray-300" style={{ fontFamily: 'Montserrat, sans-serif' }}>Include</h2>
        <div className="space-y-2">
          {[
            { key: 'billing', label: 'Billing Sessions', desc: 'session_id, driver, pod, times, amount, suspect flag', checked: includeBilling, set: setIncludeBilling },
            { key: 'laps', label: 'Lap Data', desc: 'lap_id, billing_session_id, lap_time, sectors, valid', checked: includeLaps, set: setIncludeLaps },
            { key: 'telemetry', label: 'Telemetry Samples', desc: 'Per-sample speed, throttle, brake, steering, gear, rpm (large)', checked: includeTelemetry, set: setIncludeTelemetry },
          ].map(({ key, label, desc, checked, set }) => (
            <label key={key} className="flex items-start gap-3 cursor-pointer">
              <input
                type="checkbox"
                checked={checked}
                onChange={e => set(e.target.checked)}
                className="mt-0.5 accent-[#E10600]"
              />
              <div>
                <span className="text-sm text-gray-300">{label}</span>
                <p className="text-xs text-gray-500">{desc}</p>
              </div>
            </label>
          ))}
        </div>
      </div>

      {/* Estimate + export */}
      <div className="flex items-center gap-3">
        <button
          onClick={handleEstimate}
          disabled={rangeExceeds || estimating || !includeParam}
          className="px-4 py-2 bg-[#333] text-white rounded text-sm disabled:opacity-50 disabled:cursor-not-allowed hover:bg-[#444] transition-colors"
        >
          {estimating ? 'Estimating...' : 'Estimate Rows'}
        </button>
        <button
          onClick={handleExport}
          disabled={rangeExceeds || !includeParam}
          className="px-4 py-2 bg-[#E10600] text-white rounded text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed hover:bg-red-600 transition-colors"
          style={{ fontFamily: 'Montserrat, sans-serif' }}
        >
          Export
        </button>
      </div>

      {estimateError && (
        <p className="text-sm text-red-400">{estimateError}</p>
      )}

      {estimate && (
        <div className="bg-[#1a1a1a] rounded-lg p-4 border border-[#333] space-y-2">
          <h3 className="text-sm font-medium text-gray-300">Estimate</h3>
          {includeBilling && <p className="text-xs text-gray-400">Billing rows: <span className="text-white">{estimate.billing_rows.toLocaleString()}</span></p>}
          {includeLaps && <p className="text-xs text-gray-400">Lap rows: <span className="text-white">{estimate.lap_rows.toLocaleString()}</span></p>}
          {includeTelemetry && <p className="text-xs text-gray-400">Telemetry rows (est.): <span className="text-white">{estimate.telemetry_rows.toLocaleString()}</span></p>}
          <p className="text-sm font-medium text-gray-300">Total: <span className="text-white">{estimate.total_rows.toLocaleString()} rows</span></p>
          {estimate.total_rows > 100000 && (
            <p className="text-xs text-yellow-400">Large export -- may take 30+ seconds. Consider narrowing date range or excluding telemetry.</p>
          )}
        </div>
      )}
    </div>
  );
}
