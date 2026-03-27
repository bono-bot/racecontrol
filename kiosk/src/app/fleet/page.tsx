"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { PodFleetStatus } from "@/lib/types";

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

export default function FleetPage() {
  const [pods, setPods] = useState<PodFleetStatus[]>([]);
  const [lastUpdate, setLastUpdate] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selectedMaintenancePod, setSelectedMaintenancePod] = useState<number | null>(null);
  const [pinVerified, setPinVerified] = useState(false);
  const [pinInput, setPinInput] = useState("");
  const [pinError, setPinError] = useState("");
  const [clearing, setClearing] = useState(false);

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

  function formatTimestamp(ts: string): string {
    try {
      const d = new Date(ts);
      return d.toLocaleTimeString("en-IN", { timeZone: "Asia/Kolkata", hour12: false });
    } catch {
      return ts;
    }
  }

  return (
    <div className="bg-[var(--color-rp-black)] min-h-screen text-white">
      <div className="px-4 pt-6 pb-2">
        <h1 className="text-xl font-bold">Fleet Health</h1>
        <p className="text-xs text-gray-500 mt-1">
          {lastUpdate ? `Last updated: ${formatTimestamp(lastUpdate)}` : "Connecting..."}
        </p>
        {error && (
          <p className="text-yellow-500 text-xs mt-1">{error}</p>
        )}
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 px-4 pb-4">
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

              {pod.in_maintenance && (
                <button
                  onClick={() => {
                    setSelectedMaintenancePod(pod.pod_number);
                    setPinVerified(false);
                    setPinInput("");
                  }}
                  className="mt-1.5 px-2 py-0.5 rounded text-xs font-bold text-white"
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
                  <p className="text-sm text-gray-400 mb-3">Enter staff PIN to view details</p>
                  <input
                    type="password"
                    inputMode="numeric"
                    maxLength={4}
                    value={pinInput}
                    onChange={e => { setPinInput(e.target.value.replace(/\D/g, "")); setPinError(""); }}
                    onKeyDown={async e => {
                      if (e.key === "Enter" && pinInput.length === 4) {
                        try {
                          const res = await api.validateStaffPin(pinInput);
                          if (res.error) { setPinError(res.error); return; }
                          if (res.token) sessionStorage.setItem("kiosk_staff_token", res.token);
                          setPinVerified(true);
                        } catch { setPinError("Network error"); }
                      }
                    }}
                    className="w-full bg-[#1A1A1A] border border-[#333333] text-white text-center text-2xl tracking-widest rounded px-4 py-2 mb-3"
                    placeholder="----"
                    autoFocus
                  />
                  {pinError && <p className="text-red-400 text-xs mb-2">{pinError}</p>}
                  <button
                    onClick={async () => {
                      if (pinInput.length !== 4) return;
                      try {
                        const res = await api.validateStaffPin(pinInput);
                        if (res.error) { setPinError(res.error); return; }
                        if (res.token) sessionStorage.setItem("kiosk_staff_token", res.token);
                        setPinVerified(true);
                      } catch { setPinError("Network error"); }
                    }}
                    disabled={pinInput.length < 4}
                    className="w-full py-2 rounded text-sm font-bold text-white disabled:opacity-40"
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
                    className="w-full py-2 rounded text-sm font-bold text-white disabled:opacity-60"
                    style={{ backgroundColor: "#E10600" }}
                  >
                    {clearing ? "Clearing..." : "Clear Maintenance"}
                  </button>
                </div>
              )}

              <button
                onClick={() => setSelectedMaintenancePod(null)}
                className="w-full py-2 mt-2 rounded text-sm text-gray-400 hover:text-white"
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
