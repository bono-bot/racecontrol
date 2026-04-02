"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { PodFleetStatus, BillingSession } from "@/lib/types";

function formatUptime(secs: number | null | undefined): string {
  if (secs == null) return "--";
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  return `${hours}h ${minutes}m`;
}

function statusBorder(ws: boolean, http: boolean, maintenance: boolean): string {
  if (maintenance) return "border-l-[#E10600]";
  if (ws && http) return "border-l-green-500";
  if (ws && !http) return "border-l-yellow-500";
  if (!ws && http) return "border-l-orange-500";
  return "border-l-red-500/50";
}

function statusLabel(ws: boolean, http: boolean, maintenance: boolean): string {
  if (maintenance) return "Maintenance";
  if (ws && http) return "Healthy";
  if (ws && !http) return "WS Only";
  if (!ws && http) return "HTTP Only";
  return "Offline";
}

function statusLabelColor(ws: boolean, http: boolean, maintenance: boolean): string {
  if (maintenance) return "text-[#E10600]";
  if (ws && http) return "text-green-500";
  if (ws && !http) return "text-yellow-500";
  if (!ws && http) return "text-orange-500";
  return "text-red-500/50";
}

interface StatusDotProps {
  active: boolean;
}

function StatusDot({ active }: StatusDotProps) {
  return (
    <span
      className={`w-2 h-2 rounded-full inline-block mr-1.5 ${active ? "bg-green-500" : "bg-red-500/50"}`}
    />
  );
}

function CountdownRing({ remaining, allocated }: { remaining: number; allocated: number }) {
  const R = 26;
  const CIRC = 2 * Math.PI * R;
  const pct = allocated > 0 ? Math.max(0, remaining / allocated) : 0;
  const dashOffset = CIRC * (1 - pct);
  const isLow = remaining < 300;
  return (
    <div className="flex flex-col items-center gap-1">
      <svg width="64" height="64" className="-rotate-90">
        <circle cx="32" cy="32" r={R} fill="none" stroke="#333" strokeWidth="5" />
        <circle
          cx="32" cy="32" r={R} fill="none"
          stroke={isLow ? "#E10600" : "#22c55e"}
          strokeWidth="5"
          strokeDasharray={`${CIRC} ${CIRC}`}
          strokeDashoffset={dashOffset}
          strokeLinecap="round"
          className={isLow ? "animate-pulse" : ""}
        />
      </svg>
      <span className={`text-xs font-mono ${isLow ? "text-[#E10600] animate-pulse" : "text-white"}`}>
        {Math.floor(remaining / 60)}:{String(Math.floor(remaining % 60)).padStart(2, "0")}
      </span>
    </div>
  );
}

interface ServerAlert {
  app: string;
  status: string;
  message: string;
  severity: string;
  timestamp: string;
}

export default function FleetPage() {
  const [pods, setPods] = useState<PodFleetStatus[]>([]);
  const [lastUpdate, setLastUpdate] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [billingSessions, setBillingSessions] = useState<Map<string, BillingSession>>(new Map());
  const [selectedMaintenancePod, setSelectedMaintenancePod] = useState<number | null>(null);
  const [pinVerified, setPinVerified] = useState(false);
  const [pin, setPin] = useState("");
  const [pinError, setPinError] = useState("");
  const [clearing, setClearing] = useState(false);
  const [serverAlerts, setServerAlerts] = useState<ServerAlert[]>([]);

  useEffect(() => {
    let intervalId: ReturnType<typeof setInterval>;

    async function fetchFleet() {
      try {
        const data = await api.fleetHealth();
        setPods(data.pods);
        setLastUpdate(data.timestamp);
        setError(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to fetch fleet status");
      }
    }

    fetchFleet();
    intervalId = setInterval(fetchFleet, 5000);

    return () => clearInterval(intervalId);
  }, []);

  // Poll server app health alerts (30s interval — less frequent than fleet health)
  useEffect(() => {
    let intervalId: ReturnType<typeof setInterval>;

    async function fetchServerAlerts() {
      try {
        const res = await fetch(
          `${process.env.NEXT_PUBLIC_API_URL || ""}/api/v1/app-health`
        );
        if (res.ok) {
          const data = await res.json();
          setServerAlerts(data.server_alerts || []);
        }
      } catch {
        // Server alerts are supplementary — don't break fleet view
      }
    }

    fetchServerAlerts();
    intervalId = setInterval(fetchServerAlerts, 30000);

    return () => clearInterval(intervalId);
  }, []);

  // Fetch active billing sessions for countdown rings
  useEffect(() => {
    let intervalId: ReturnType<typeof setInterval>;

    async function fetchSessions() {
      try {
        const data = await api.activeBillingSessions();
        const map = new Map<string, BillingSession>();
        for (const s of data.sessions || []) {
          if (s.pod_id) map.set(s.pod_id, s);
        }
        setBillingSessions(map);
      } catch {
        // Billing sessions are optional — don't break fleet view
      }
    }

    fetchSessions();
    intervalId = setInterval(fetchSessions, 5000);

    return () => clearInterval(intervalId);
  }, []);

  function formatTimestamp(ts: string): string {
    try {
      const d = new Date(ts);
      return d.toLocaleTimeString("en-IN", { timeZone: "Asia/Kolkata", hour12: false });
    } catch {
      return ts;
    }
  }

  return (
    <div className="bg-[var(--color-rp-black)] h-screen overflow-hidden text-white flex flex-col">
      <div className="px-4 pt-6 pb-2">
        <h1 className="text-xl font-bold">Fleet Health</h1>
        <p className="text-xs text-gray-500 mt-1">
          {lastUpdate ? `Last updated: ${formatTimestamp(lastUpdate)}` : "Connecting..."}
        </p>
        {error && (
          <p className="text-yellow-500 text-xs mt-1">{error}</p>
        )}
      </div>

      {/* Server App Health Alert Banner */}
      {serverAlerts.length > 0 && (
        <div className="mx-4 mb-2 px-3 py-2 bg-red-900/60 border border-red-500/40 rounded-lg flex items-center gap-2">
          <span className="w-2 h-2 rounded-full bg-red-500 animate-pulse flex-shrink-0" />
          <div className="text-xs text-red-200 flex-1">
            <span className="font-semibold text-red-400">Server Alert: </span>
            {serverAlerts.map((a, i) => (
              <span key={a.app}>
                {a.app} {a.status}
                {a.message !== "unhealthy" && ` (${a.message})`}
                {i < serverAlerts.length - 1 && " · "}
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 px-4 pb-4 flex-1 overflow-y-auto content-start">
        {pods.map((pod) => {
          const ws = pod.ws_connected;
          const http = pod.http_reachable;
          const offline = !ws && !http;
          const maintenance = pod.in_maintenance;

          return (
            <div
              key={pod.pod_number}
              className={`bg-[var(--color-rp-card)] rounded-lg p-3 border-l-4 ${statusBorder(ws, http, maintenance)} ${offline && !maintenance ? "opacity-50" : ""}`}
            >
              <div className="flex items-baseline justify-between mb-0.5">
                <span className="text-sm font-bold text-white">Pod {pod.pod_number}</span>
                <span className="text-xs text-gray-400">{pod.version ? `v${pod.version}` : "v--"}</span>
              </div>

              <div className={`text-xs font-medium mb-2 ${statusLabelColor(ws, http, maintenance)}`}>
                {statusLabel(ws, http, maintenance)}
              </div>

              <div className="space-y-1">
                <div className="flex items-center text-xs">
                  <StatusDot active={ws} />
                  <span className="text-gray-500 mr-1">WS</span>
                  <span className={ws ? "text-white" : "text-gray-600"}>
                    {ws ? "Connected" : "Disconnected"}
                  </span>
                </div>
                <div className="flex items-center text-xs">
                  <StatusDot active={http} />
                  <span className="text-gray-500 mr-1">HTTP</span>
                  <span className={http ? "text-white" : "text-gray-600"}>
                    {http ? "Reachable" : "Blocked"}
                  </span>
                </div>
              </div>

              <div className="mt-2 text-xs text-gray-500">
                Uptime: {formatUptime(pod.uptime_secs)}
              </div>

              {(pod.violation_count_24h ?? 0) > 0 && (
                <div
                  className="mt-1.5 inline-block px-2 py-0.5 rounded text-xs font-bold text-white"
                  style={{ backgroundColor: '#E10600' }}
                  title={pod.last_violation_at ? `Last: ${pod.last_violation_at}` : 'Process violations detected'}
                >
                  {pod.violation_count_24h} {pod.violation_count_24h === 1 ? 'violation' : 'violations'}
                </div>
              )}

              {pod.crash_recovery === true && (
                <div className="mt-1 text-xs text-red-500">Crash recovered</div>
              )}

              {pod.pod_id && billingSessions.has(pod.pod_id) && ws && http && (() => {
                const session = billingSessions.get(pod.pod_id!)!;
                return (
                  <div className="mt-2 flex items-center gap-2">
                    <CountdownRing remaining={session.remaining_seconds} allocated={session.allocated_seconds} />
                    <div className="text-xs">
                      <p className="text-white font-medium">{session.driver_name}</p>
                      <p className="text-gray-500">{session.pricing_tier_name}</p>
                    </div>
                  </div>
                );
              })()}

              {pod.in_maintenance && (
                <button
                  onClick={() => {
                    setSelectedMaintenancePod(pod.pod_number);
                    setPinVerified(false);
                    setPin("");
                  }}
                  className="mt-1.5 px-3 py-2 rounded text-xs font-bold text-white min-h-[44px] active:scale-[0.97] transition-transform"
                  style={{ backgroundColor: "#E10600" }}
                >
                  Maintenance
                </button>
              )}
            </div>
          );
        })}

        {pods.length === 0 && !error && (
          <div className="col-span-2 sm:col-span-4 text-center text-gray-500 text-sm py-8">
            Loading pod data...
          </div>
        )}
      </div>

      {selectedMaintenancePod !== null && (() => {
        const pod = pods.find(p => p.pod_number === selectedMaintenancePod);
        if (!pod) return null;
        return (
          <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50" onClick={() => setSelectedMaintenancePod(null)}>
            <div className="bg-[#222222] rounded-lg p-6 max-w-sm w-full mx-4 border border-[#333333]" onClick={e => e.stopPropagation()}>
              <h2 className="text-lg font-bold text-white mb-2">Pod {pod.pod_number} — Maintenance</h2>

              {!pinVerified ? (
                <div>
                  <p className="text-sm text-gray-400 mb-3">Enter 6-digit staff PIN</p>

                  {/* PIN dot display */}
                  <div className="flex gap-2 justify-center mb-4">
                    {[0, 1, 2, 3, 4, 5].map((i) => (
                      <div key={i} className={`w-10 h-12 rounded border-2 flex items-center justify-center
                        ${i < pin.length ? "border-[#E10600] bg-[#E10600]/10" : "border-[#333333] bg-[#1A1A1A]"}`}>
                        {pin[i] ? <span className="text-white text-xl">*</span> : null}
                      </div>
                    ))}
                  </div>

                  {pinError && <p className="text-red-400 text-xs mb-2">{pinError}</p>}

                  {/* On-screen numpad */}
                  <div className="grid grid-cols-3 gap-2 mb-3">
                    {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map(d => (
                      <button key={d} onClick={() => { if (pin.length < 6) { setPin(p => p + d); setPinError(""); } }}
                        className="h-14 rounded-lg bg-[#1A1A1A] border border-[#333333] text-xl font-bold text-white active:scale-[0.97] transition-transform min-h-[44px]">
                        {d}
                      </button>
                    ))}
                    <button onClick={() => { setPin(""); setPinError(""); }}
                      className="h-14 rounded-lg bg-[#1A1A1A] border border-[#333333] text-xs text-gray-500 active:scale-[0.97] transition-transform min-h-[44px]">
                      Clear
                    </button>
                    <button onClick={() => { if (pin.length < 6) { setPin(p => p + "0"); setPinError(""); } }}
                      className="h-14 rounded-lg bg-[#1A1A1A] border border-[#333333] text-xl font-bold text-white active:scale-[0.97] transition-transform min-h-[44px]">
                      0
                    </button>
                    <button onClick={() => setPin(p => p.slice(0, -1))}
                      className="h-14 rounded-lg bg-[#1A1A1A] border border-[#333333] text-gray-500 active:scale-[0.97] transition-transform min-h-[44px] flex items-center justify-center">
                      &#x232B;
                    </button>
                  </div>

                  {/* Verify button */}
                  <button
                    onClick={async () => {
                      if (pin.length !== 6) return;
                      try {
                        const res = await api.validateStaffPin(pin);
                        if (res.error) { setPinError(res.error); return; }
                        if (res.token) sessionStorage.setItem("kiosk_staff_token", res.token);
                        setPinVerified(true);
                      } catch { setPinError("Network error"); }
                    }}
                    disabled={pin.length < 6}
                    className="w-full py-3 rounded text-sm font-bold text-white disabled:opacity-40 min-h-[44px]"
                    style={{ backgroundColor: "#E10600" }}
                  >
                    Verify
                  </button>
                </div>
              ) : (
                <div>
                  <p className="text-sm text-gray-400 mb-2">Failed checks:</p>
                  <ul className="list-disc list-inside text-sm text-white mb-4 space-y-1">
                    {(pod.maintenance_failures ?? []).map((f, i) => (
                      <li key={i}>{f}</li>
                    ))}
                    {(!pod.maintenance_failures || pod.maintenance_failures.length === 0) && (
                      <li className="text-gray-500">No details available</li>
                    )}
                  </ul>
                  <button
                    onClick={async () => {
                      setClearing(true);
                      try {
                        await api.clearMaintenance(pod.pod_id!);
                        setSelectedMaintenancePod(null);
                      } catch (err) {
                        console.error("Failed to clear maintenance:", err);
                      } finally {
                        setClearing(false);
                      }
                    }}
                    disabled={clearing}
                    className="w-full py-2 rounded text-sm font-bold text-white disabled:opacity-60 min-h-[44px] active:scale-[0.97] transition-transform"
                    style={{ backgroundColor: "#E10600" }}
                  >
                    {clearing ? "Clearing..." : "Clear Maintenance"}
                  </button>
                </div>
              )}

              <button
                onClick={() => setSelectedMaintenancePod(null)}
                className="w-full py-2 mt-2 rounded text-sm text-gray-400 hover:text-white min-h-[44px]"
              >
                Close
              </button>
            </div>
          </div>
        );
      })()}
    </div>
  );
}
