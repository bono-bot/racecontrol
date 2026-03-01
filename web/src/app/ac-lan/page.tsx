"use client";

import { useEffect, useState, useCallback } from "react";
import { useWebSocket } from "@/hooks/useWebSocket";
import {
  api,
  defaultAcConfig,
  type AcLanSessionConfig,
  type AcServerInfo,
  type AcPresetSummary,
  type AcTrack,
  type AcCar,
  type Pod,
} from "@/lib/api";

export default function AcLanPage() {
  const {
    connected,
    pods,
    acServerInfo,
    acPresets,
    acLoadedConfig,
    sendCommand,
  } = useWebSocket();

  const [config, setConfig] = useState<AcLanSessionConfig>(defaultAcConfig());
  const [selectedPods, setSelectedPods] = useState<Set<string>>(new Set());
  const [tracks, setTracks] = useState<AcTrack[]>([]);
  const [cars, setCars] = useState<AcCar[]>([]);
  const [presets, setPresets] = useState<AcPresetSummary[]>([]);
  const [presetName, setPresetName] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load tracks, cars, and presets on mount
  useEffect(() => {
    api.acTracks().then((r) => setTracks(r.tracks)).catch(() => {});
    api.acCars().then((r) => setCars(r.cars)).catch(() => {});
    api.listAcPresets().then((r) => setPresets(r.presets)).catch(() => {});
  }, []);

  // Sync WebSocket preset list
  useEffect(() => {
    if (acPresets.length > 0) setPresets(acPresets);
  }, [acPresets]);

  // Handle preset loaded via WebSocket
  useEffect(() => {
    if (acLoadedConfig) {
      setConfig(acLoadedConfig.config);
    }
  }, [acLoadedConfig]);

  const onlinePods = pods.filter(
    (p) => p.status !== "offline"
  );

  const updateConfig = useCallback(
    (partial: Partial<AcLanSessionConfig>) => {
      setConfig((prev) => ({ ...prev, ...partial }));
    },
    []
  );

  const togglePod = (podId: string) => {
    setSelectedPods((prev) => {
      const next = new Set(prev);
      if (next.has(podId)) next.delete(podId);
      else next.add(podId);
      return next;
    });
  };

  const selectAllPods = () => {
    setSelectedPods(new Set(onlinePods.map((p) => p.id)));
  };

  const deselectAllPods = () => {
    setSelectedPods(new Set());
  };

  const toggleSession = (type: "practice" | "qualifying" | "race") => {
    const exists = config.sessions.find((s) => s.session_type === type);
    if (exists) {
      updateConfig({ sessions: config.sessions.filter((s) => s.session_type !== type) });
    } else {
      const defaults: Record<string, { name: string; duration_minutes: number; laps: number; wait_time_secs: number }> = {
        practice: { name: "Practice", duration_minutes: 10, laps: 0, wait_time_secs: 30 },
        qualifying: { name: "Qualifying", duration_minutes: 10, laps: 0, wait_time_secs: 60 },
        race: { name: "Race", duration_minutes: 0, laps: 10, wait_time_secs: 60 },
      };
      const d = defaults[type];
      updateConfig({
        sessions: [...config.sessions, { ...d, session_type: type }],
      });
    }
  };

  const updateSession = (type: string, field: string, value: number) => {
    updateConfig({
      sessions: config.sessions.map((s) =>
        s.session_type === type ? { ...s, [field]: value } : s
      ),
    });
  };

  const toggleCar = (carId: string) => {
    const exists = config.cars.includes(carId);
    if (exists) {
      if (config.cars.length > 1) {
        updateConfig({ cars: config.cars.filter((c) => c !== carId) });
      }
    } else {
      updateConfig({ cars: [...config.cars, carId] });
    }
  };

  const handleStart = async () => {
    if (selectedPods.size === 0) {
      setError("Select at least one pod");
      return;
    }
    setStarting(true);
    setError(null);
    try {
      await api.startAcSession(config, Array.from(selectedPods));
    } catch (e) {
      setError(String(e));
    } finally {
      setStarting(false);
    }
  };

  const handleStop = async () => {
    if (!acServerInfo) return;
    try {
      await api.stopAcSession(acServerInfo.session_id);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSavePreset = async () => {
    if (!presetName.trim()) return;
    try {
      await api.saveAcPreset(presetName.trim(), config);
      setPresetName("");
      const r = await api.listAcPresets();
      setPresets(r.presets);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleLoadPreset = async (id: string) => {
    try {
      const r = await api.getAcPreset(id);
      setConfig(r.config);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDeletePreset = async (id: string) => {
    try {
      await api.deleteAcPreset(id);
      const r = await api.listAcPresets();
      setPresets(r.presets);
    } catch (e) {
      setError(String(e));
    }
  };

  const carsByClass = cars.reduce<Record<string, typeof cars>>((acc, car) => {
    if (!acc[car.class]) acc[car.class] = [];
    acc[car.class].push(car);
    return acc;
  }, {});

  // If there's an active session, show the live panel
  if (acServerInfo && acServerInfo.status !== "stopped") {
    return (
      <div className="p-6 space-y-6">
        <div className="flex items-center justify-between">
          <h1 className="text-2xl font-bold">AC LAN Race</h1>
          <StatusBadge status={acServerInfo.status} />
        </div>

        <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-6 space-y-4">
          <div className="grid grid-cols-2 gap-6">
            <div>
              <label className="text-xs text-zinc-500 uppercase">Track</label>
              <p className="text-lg font-medium">
                {tracks.find((t) => t.id === acServerInfo.config.track)?.name || acServerInfo.config.track}
                {acServerInfo.config.track_config && ` (${acServerInfo.config.track_config})`}
              </p>
            </div>
            <div>
              <label className="text-xs text-zinc-500 uppercase">Cars</label>
              <p className="text-lg font-medium">
                {acServerInfo.config.cars.map((c) => cars.find((car) => car.id === c)?.name || c).join(", ")}
              </p>
            </div>
            <div>
              <label className="text-xs text-zinc-500 uppercase">Sessions</label>
              <div className="flex gap-2 mt-1">
                {acServerInfo.config.sessions.map((s) => (
                  <span
                    key={s.session_type}
                    className="px-2 py-1 bg-zinc-800 rounded text-sm"
                  >
                    {s.name} {s.session_type === "race" && s.laps > 0 ? `(${s.laps} laps)` : `(${s.duration_minutes}min)`}
                  </span>
                ))}
              </div>
            </div>
            <div>
              <label className="text-xs text-zinc-500 uppercase">Pods</label>
              <p className="text-lg font-medium">
                {acServerInfo.connected_pods.length} connected
              </p>
            </div>
          </div>

          <div className="border-t border-zinc-800 pt-4">
            <label className="text-xs text-zinc-500 uppercase">Join URL</label>
            <div className="flex items-center gap-2 mt-1">
              <code className="flex-1 bg-zinc-800 px-3 py-2 rounded text-sm font-mono text-orange-400">
                {acServerInfo.join_url}
              </code>
              <button
                onClick={() => navigator.clipboard.writeText(acServerInfo.join_url)}
                className="px-3 py-2 bg-zinc-800 hover:bg-zinc-700 rounded text-sm transition-colors"
              >
                Copy
              </button>
            </div>
          </div>

          {acServerInfo.error_message && (
            <div className="bg-red-500/10 border border-red-500/30 rounded p-3 text-red-400 text-sm">
              {acServerInfo.error_message}
            </div>
          )}

          <button
            onClick={handleStop}
            className="px-6 py-2 bg-red-600 hover:bg-red-700 text-white rounded font-medium transition-colors"
          >
            Stop Session
          </button>
        </div>
      </div>
    );
  }

  // Setup form
  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">AC LAN Race</h1>
        <span className={`text-xs px-2 py-1 rounded ${connected ? "bg-green-500/20 text-green-400" : "bg-red-500/20 text-red-400"}`}>
          {connected ? "Connected" : "Disconnected"}
        </span>
      </div>

      {error && (
        <div className="bg-red-500/10 border border-red-500/30 rounded p-3 text-red-400 text-sm flex justify-between">
          <span>{error}</span>
          <button onClick={() => setError(null)} className="text-red-400 hover:text-red-300">&times;</button>
        </div>
      )}

      {/* Presets */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
        <div className="flex items-center gap-3">
          <select
            className="flex-1 bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm"
            defaultValue=""
            onChange={(e) => {
              if (e.target.value) handleLoadPreset(e.target.value);
            }}
          >
            <option value="">Load a preset...</option>
            {presets.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} ({p.track}, {p.cars.length} car{p.cars.length !== 1 ? "s" : ""})
              </option>
            ))}
          </select>
          <input
            type="text"
            placeholder="Preset name"
            value={presetName}
            onChange={(e) => setPresetName(e.target.value)}
            className="bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm w-48"
          />
          <button
            onClick={handleSavePreset}
            disabled={!presetName.trim()}
            className="px-4 py-2 bg-zinc-700 hover:bg-zinc-600 disabled:opacity-50 rounded text-sm transition-colors"
          >
            Save As
          </button>
        </div>
        {presets.length > 0 && (
          <div className="flex flex-wrap gap-2 mt-3">
            {presets.map((p) => (
              <div key={p.id} className="flex items-center gap-1 bg-zinc-800 rounded px-2 py-1 text-xs">
                <button onClick={() => handleLoadPreset(p.id)} className="hover:text-orange-400 transition-colors">
                  {p.name}
                </button>
                <button onClick={() => handleDeletePreset(p.id)} className="text-zinc-500 hover:text-red-400 ml-1">&times;</button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Track Selection */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
        <label className="text-sm font-medium text-zinc-300 mb-2 block">Track</label>
        <div className="grid grid-cols-2 gap-3">
          <select
            value={config.track}
            onChange={(e) => {
              const track = tracks.find((t) => t.id === e.target.value);
              updateConfig({
                track: e.target.value,
                track_config: track?.configs[0] || "",
              });
            }}
            className="bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm"
          >
            {tracks.map((t) => (
              <option key={t.id} value={t.id}>{t.name}</option>
            ))}
          </select>
          {(() => {
            const track = tracks.find((t) => t.id === config.track);
            if (!track || track.configs.length <= 1) return null;
            return (
              <select
                value={config.track_config}
                onChange={(e) => updateConfig({ track_config: e.target.value })}
                className="bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm"
              >
                {track.configs.map((c) => (
                  <option key={c} value={c}>{c || "Default"}</option>
                ))}
              </select>
            );
          })()}
        </div>
      </div>

      {/* Car Selection */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
        <label className="text-sm font-medium text-zinc-300 mb-2 block">
          Cars ({config.cars.length} selected)
        </label>
        <div className="space-y-3 max-h-64 overflow-y-auto">
          {Object.entries(carsByClass).map(([cls, classCars]) => (
            <div key={cls}>
              <p className="text-xs text-zinc-500 uppercase mb-1">{cls}</p>
              <div className="flex flex-wrap gap-2">
                {classCars.map((car) => {
                  const selected = config.cars.includes(car.id);
                  return (
                    <button
                      key={car.id}
                      onClick={() => toggleCar(car.id)}
                      className={`px-3 py-1.5 rounded text-xs transition-colors ${
                        selected
                          ? "bg-orange-500/20 text-orange-400 border border-orange-500/40"
                          : "bg-zinc-800 text-zinc-400 border border-zinc-700 hover:border-zinc-600"
                      }`}
                    >
                      {car.name}
                    </button>
                  );
                })}
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Session Types */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
        <label className="text-sm font-medium text-zinc-300 mb-3 block">Sessions</label>
        <div className="space-y-3">
          {(["practice", "qualifying", "race"] as const).map((type) => {
            const session = config.sessions.find((s) => s.session_type === type);
            return (
              <div key={type} className="flex items-center gap-4">
                <label className="flex items-center gap-2 w-28">
                  <input
                    type="checkbox"
                    checked={!!session}
                    onChange={() => toggleSession(type)}
                    className="rounded border-zinc-700"
                  />
                  <span className="text-sm capitalize">{type}</span>
                </label>
                {session && (
                  <div className="flex items-center gap-3">
                    {type === "race" ? (
                      <>
                        <label className="text-xs text-zinc-500">Laps:</label>
                        <input
                          type="number"
                          value={session.laps}
                          onChange={(e) => updateSession(type, "laps", parseInt(e.target.value) || 0)}
                          className="w-20 bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm"
                        />
                        <label className="text-xs text-zinc-500">or Minutes:</label>
                        <input
                          type="number"
                          value={session.duration_minutes}
                          onChange={(e) => updateSession(type, "duration_minutes", parseInt(e.target.value) || 0)}
                          className="w-20 bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm"
                        />
                      </>
                    ) : (
                      <>
                        <label className="text-xs text-zinc-500">Minutes:</label>
                        <input
                          type="number"
                          value={session.duration_minutes}
                          onChange={(e) => updateSession(type, "duration_minutes", parseInt(e.target.value) || 0)}
                          className="w-20 bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm"
                        />
                      </>
                    )}
                    <label className="text-xs text-zinc-500">Wait:</label>
                    <input
                      type="number"
                      value={session.wait_time_secs}
                      onChange={(e) => updateSession(type, "wait_time_secs", parseInt(e.target.value) || 0)}
                      className="w-16 bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm"
                    />
                    <span className="text-xs text-zinc-500">sec</span>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      </div>

      {/* Pod Selection */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
        <div className="flex items-center justify-between mb-3">
          <label className="text-sm font-medium text-zinc-300">
            Pods ({selectedPods.size} of {onlinePods.length} selected)
          </label>
          <div className="flex gap-2">
            <button
              onClick={selectAllPods}
              className="text-xs text-zinc-400 hover:text-orange-400 transition-colors"
            >
              Select All
            </button>
            <button
              onClick={deselectAllPods}
              className="text-xs text-zinc-400 hover:text-orange-400 transition-colors"
            >
              Deselect All
            </button>
          </div>
        </div>
        <div className="grid grid-cols-4 gap-2">
          {onlinePods.map((pod) => {
            const selected = selectedPods.has(pod.id);
            return (
              <button
                key={pod.id}
                onClick={() => togglePod(pod.id)}
                className={`p-3 rounded border text-left transition-colors ${
                  selected
                    ? "bg-orange-500/10 border-orange-500/40 text-orange-400"
                    : "bg-zinc-800 border-zinc-700 text-zinc-400 hover:border-zinc-600"
                }`}
              >
                <div className="font-medium text-sm">{pod.name || `Pod ${pod.number}`}</div>
                <div className="text-xs mt-1 opacity-70">
                  {pod.status === "offline" ? "Offline" : pod.current_game || "Idle"}
                </div>
              </button>
            );
          })}
          {onlinePods.length === 0 && (
            <p className="col-span-4 text-zinc-500 text-sm text-center py-4">
              No pods connected. Start rc-agent on gaming PCs to connect.
            </p>
          )}
        </div>
      </div>

      {/* Advanced Settings */}
      <div className="bg-zinc-900 border border-zinc-800 rounded-lg">
        <button
          onClick={() => setShowAdvanced(!showAdvanced)}
          className="w-full p-4 flex items-center justify-between text-sm font-medium text-zinc-300 hover:text-zinc-100 transition-colors"
        >
          <span>Advanced Settings</span>
          <span className="text-zinc-500">{showAdvanced ? "▲" : "▼"}</span>
        </button>
        {showAdvanced && (
          <div className="px-4 pb-4 space-y-4 border-t border-zinc-800 pt-4">
            {/* Session Name & Password */}
            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="text-xs text-zinc-500 mb-1 block">Session Name</label>
                <input
                  type="text"
                  value={config.name}
                  onChange={(e) => updateConfig({ name: e.target.value })}
                  className="w-full bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm"
                />
              </div>
              <div>
                <label className="text-xs text-zinc-500 mb-1 block">Password</label>
                <input
                  type="text"
                  value={config.password}
                  onChange={(e) => updateConfig({ password: e.target.value })}
                  className="w-full bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm"
                  placeholder="Leave empty for no password"
                />
              </div>
            </div>

            {/* Max Clients & Pickup Mode */}
            <div className="grid grid-cols-3 gap-4">
              <div>
                <label className="text-xs text-zinc-500 mb-1 block">Max Clients</label>
                <input
                  type="number"
                  value={config.max_clients}
                  onChange={(e) => updateConfig({ max_clients: parseInt(e.target.value) || 16 })}
                  className="w-full bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm"
                  min={1}
                  max={16}
                />
              </div>
              <div className="flex items-end">
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={config.pickup_mode}
                    onChange={(e) => updateConfig({ pickup_mode: e.target.checked })}
                    className="rounded border-zinc-700"
                  />
                  <span className="text-sm text-zinc-300">Pickup Mode</span>
                </label>
              </div>
              <div>
                <label className="text-xs text-zinc-500 mb-1 block">CSP Version</label>
                <input
                  type="number"
                  value={config.min_csp_version}
                  onChange={(e) => updateConfig({ min_csp_version: parseInt(e.target.value) || 0 })}
                  className="w-full bg-zinc-800 border border-zinc-700 rounded px-3 py-2 text-sm"
                  placeholder="0 = none"
                />
              </div>
            </div>

            {/* Assists */}
            <div>
              <label className="text-xs text-zinc-500 mb-2 block">Assists</label>
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className="text-xs text-zinc-400">ABS</label>
                  <select
                    value={config.abs_allowed}
                    onChange={(e) => updateConfig({ abs_allowed: parseInt(e.target.value) })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  >
                    <option value={0}>Off</option>
                    <option value={1}>Factory</option>
                    <option value={2}>On</option>
                  </select>
                </div>
                <div>
                  <label className="text-xs text-zinc-400">TC</label>
                  <select
                    value={config.tc_allowed}
                    onChange={(e) => updateConfig({ tc_allowed: parseInt(e.target.value) })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  >
                    <option value={0}>Off</option>
                    <option value={1}>Factory</option>
                    <option value={2}>On</option>
                  </select>
                </div>
                <div className="space-y-1 pt-1">
                  <label className="flex items-center gap-2 text-xs">
                    <input type="checkbox" checked={config.autoclutch_allowed} onChange={(e) => updateConfig({ autoclutch_allowed: e.target.checked })} className="rounded border-zinc-700" />
                    Autoclutch
                  </label>
                  <label className="flex items-center gap-2 text-xs">
                    <input type="checkbox" checked={config.stability_allowed} onChange={(e) => updateConfig({ stability_allowed: e.target.checked })} className="rounded border-zinc-700" />
                    Stability
                  </label>
                </div>
              </div>
            </div>

            {/* Rates */}
            <div>
              <label className="text-xs text-zinc-500 mb-2 block">Simulation Rates (%)</label>
              <div className="grid grid-cols-3 gap-4">
                <div>
                  <label className="text-xs text-zinc-400">Damage</label>
                  <input
                    type="number"
                    value={config.damage_multiplier}
                    onChange={(e) => updateConfig({ damage_multiplier: parseInt(e.target.value) || 100 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs text-zinc-400">Fuel</label>
                  <input
                    type="number"
                    value={config.fuel_rate}
                    onChange={(e) => updateConfig({ fuel_rate: parseInt(e.target.value) || 100 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs text-zinc-400">Tyre Wear</label>
                  <input
                    type="number"
                    value={config.tyre_wear_rate}
                    onChange={(e) => updateConfig({ tyre_wear_rate: parseInt(e.target.value) || 100 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  />
                </div>
              </div>
            </div>

            {/* Weather */}
            <div>
              <label className="text-xs text-zinc-500 mb-2 block">Weather</label>
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className="text-xs text-zinc-400">Preset</label>
                  <select
                    value={config.weather[0]?.graphics || "3_clear"}
                    onChange={(e) => {
                      const w = [...config.weather];
                      if (w[0]) w[0] = { ...w[0], graphics: e.target.value };
                      updateConfig({ weather: w });
                    }}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  >
                    <option value="3_clear">Clear</option>
                    <option value="7_heavy_clouds">Heavy Clouds</option>
                    <option value="4_mid_clear">Mid Clear</option>
                    <option value="2_light_fog">Light Fog</option>
                    <option value="5_light_clouds">Light Clouds</option>
                    <option value="6_mid_clouds">Mid Clouds</option>
                    <option value="1_heavy_fog">Heavy Fog</option>
                  </select>
                </div>
                <div>
                  <label className="text-xs text-zinc-400">Ambient Temp</label>
                  <input
                    type="number"
                    value={config.weather[0]?.base_temperature_ambient || 26}
                    onChange={(e) => {
                      const w = [...config.weather];
                      if (w[0]) w[0] = { ...w[0], base_temperature_ambient: parseInt(e.target.value) || 26 };
                      updateConfig({ weather: w });
                    }}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs text-zinc-400">Road Temp</label>
                  <input
                    type="number"
                    value={config.weather[0]?.base_temperature_road || 32}
                    onChange={(e) => {
                      const w = [...config.weather];
                      if (w[0]) w[0] = { ...w[0], base_temperature_road: parseInt(e.target.value) || 32 };
                      updateConfig({ weather: w });
                    }}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  />
                </div>
              </div>
            </div>

            {/* Ports */}
            <div>
              <label className="text-xs text-zinc-500 mb-2 block">Server Ports</label>
              <div className="grid grid-cols-3 gap-4">
                <div>
                  <label className="text-xs text-zinc-400">UDP</label>
                  <input
                    type="number"
                    value={config.udp_port}
                    onChange={(e) => updateConfig({ udp_port: parseInt(e.target.value) || 9600 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs text-zinc-400">TCP</label>
                  <input
                    type="number"
                    value={config.tcp_port}
                    onChange={(e) => updateConfig({ tcp_port: parseInt(e.target.value) || 9600 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  />
                </div>
                <div>
                  <label className="text-xs text-zinc-400">HTTP</label>
                  <input
                    type="number"
                    value={config.http_port}
                    onChange={(e) => updateConfig({ http_port: parseInt(e.target.value) || 8081 })}
                    className="w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm mt-1"
                  />
                </div>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Start Button */}
      <div className="flex items-center justify-between">
        <p className="text-sm text-zinc-500">
          {config.cars.length} car{config.cars.length !== 1 ? "s" : ""} &middot;{" "}
          {config.sessions.length} session{config.sessions.length !== 1 ? "s" : ""} &middot;{" "}
          {selectedPods.size} pod{selectedPods.size !== 1 ? "s" : ""}
        </p>
        <button
          onClick={handleStart}
          disabled={starting || selectedPods.size === 0 || config.cars.length === 0}
          className="px-8 py-3 bg-orange-600 hover:bg-orange-700 disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg font-bold text-lg transition-colors"
        >
          {starting ? "Starting..." : "Start Race"}
        </button>
      </div>
    </div>
  );
}

function StatusBadge({ status }: { status: string }) {
  const colors: Record<string, string> = {
    starting: "bg-amber-500/20 text-amber-400",
    running: "bg-green-500/20 text-green-400",
    stopping: "bg-amber-500/20 text-amber-400",
    stopped: "bg-zinc-500/20 text-zinc-400",
    error: "bg-red-500/20 text-red-400",
  };
  return (
    <span className={`px-3 py-1 rounded text-sm font-medium ${colors[status] || colors.stopped}`}>
      {status.charAt(0).toUpperCase() + status.slice(1)}
    </span>
  );
}
