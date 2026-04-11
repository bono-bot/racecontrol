'use client';

import { useState } from 'react';

interface VerifyResult {
  pod_id: string;
  pass: boolean;
  detail: string;
  last_mismatch_check: string | null;
  elapsed_ms: number;
}

const POD_IDS = ['1', '2', '3', '4', '5', '6', '7', '8'];

function StatusDot({ pass, verifying }: { pass: boolean | null; verifying: boolean }) {
  if (verifying) return <span className="inline-block w-3 h-3 rounded-full bg-yellow-400 animate-pulse" />;
  if (pass === null) return <span className="inline-block w-3 h-3 rounded-full bg-[#5A5A5A]" />;
  return <span className="inline-block w-3 h-3 rounded-full" style={{ background: pass ? '#22c55e' : '#E10600' }} />;
}

export default function PodVerifyPage() {
  const [results, setResults] = useState<Record<string, VerifyResult | null>>({});
  const [verifying, setVerifying] = useState<Record<string, boolean>>({});
  const [verifyingAll, setVerifyingAll] = useState(false);

  async function verifyPod(podId: string) {
    setVerifying(prev => ({ ...prev, [podId]: true }));
    try {
      const res = await fetch(`/api/rc/admin/pods/${podId}/verify`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
      });
      const data: VerifyResult = await res.json();
      setResults(prev => ({ ...prev, [podId]: data }));
    } catch {
      setResults(prev => ({ ...prev, [podId]: { pod_id: podId, pass: false, detail: 'Network error', last_mismatch_check: null, elapsed_ms: 0 } }));
    } finally {
      setVerifying(prev => ({ ...prev, [podId]: false }));
    }
  }

  async function verifyAll() {
    setVerifyingAll(true);
    for (const podId of POD_IDS) {
      await verifyPod(podId);
    }
    setVerifyingAll(false);
  }

  const anyVerifying = verifyingAll || POD_IDS.some(id => verifying[id]);

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-white" style={{ fontFamily: 'Montserrat, sans-serif' }}>
          On-Demand Pod Verification
        </h1>
        <button
          onClick={verifyAll}
          disabled={anyVerifying}
          className="px-4 py-2 bg-[#E10600] text-white rounded text-sm font-medium disabled:opacity-50 disabled:cursor-not-allowed hover:bg-red-600 transition-colors"
          style={{ fontFamily: 'Montserrat, sans-serif' }}
        >
          {anyVerifying ? 'Verifying...' : 'Verify All'}
        </button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {POD_IDS.map(podId => {
          const result = results[podId] ?? null;
          const isVerifying = !!verifying[podId];
          const borderColor = result === null ? '#333' : result.pass ? '#22c55e' : '#E10600';

          return (
            <div
              key={podId}
              className="bg-[#222] rounded-lg p-4 border"
              style={{ borderColor }}
            >
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  <StatusDot pass={result?.pass ?? null} verifying={isVerifying} />
                  <span className="text-white font-bold text-lg" style={{ fontFamily: 'Montserrat, sans-serif' }}>
                    Pod {podId}
                  </span>
                </div>
                {result && (
                  <span className="text-xs text-gray-500">{result.elapsed_ms}ms</span>
                )}
              </div>

              {result && (
                <div className="mb-3 space-y-1">
                  <p className={`text-sm ${result.pass ? 'text-green-400' : 'text-red-400'}`}>
                    {result.pass ? 'PASS' : 'FAIL'}
                  </p>
                  <p className="text-xs text-gray-400 leading-relaxed">{result.detail}</p>
                  {result.last_mismatch_check && (
                    <p className="text-xs text-gray-500">
                      Last check: {new Date(result.last_mismatch_check).toLocaleString('en-IN', { hour12: true })}
                    </p>
                  )}
                </div>
              )}

              <button
                onClick={() => verifyPod(podId)}
                disabled={isVerifying || verifyingAll}
                className="w-full px-3 py-1.5 bg-[#333] text-white rounded text-sm disabled:opacity-50 disabled:cursor-not-allowed hover:bg-[#444] transition-colors"
              >
                {isVerifying ? 'Verifying...' : 'Verify'}
              </button>
            </div>
          );
        })}
      </div>

      <p className="text-xs text-gray-600">
        Verification checks WS connection status and config-mismatch verifier activity. Results returned within 15 seconds.
        Phase B (GLD-G-02) -- deferred synthetic mismatch test from Phase 362.
      </p>
    </div>
  );
}
