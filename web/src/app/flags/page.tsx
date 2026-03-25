"use client";

import { useEffect, useState, useCallback } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type { FeatureFlagRow, UpdateFlagRequest } from "@/lib/api";
import { useWebSocket } from "@/hooks/useWebSocket";

// Parse overrides JSON string into Record<string, boolean>
function parseOverrides(raw: string): Record<string, boolean> {
  try {
    const parsed = JSON.parse(raw);
    if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
      return parsed as Record<string, boolean>;
    }
    return {};
  } catch {
    return {};
  }
}

// Format timestamp to readable IST
function formatIST(ts: string | null): string {
  if (!ts) return "--";
  try {
    return new Date(ts).toLocaleString("en-IN", { timeZone: "Asia/Kolkata" });
  } catch {
    return ts;
  }
}

interface ScopeEditorProps {
  flag: FeatureFlagRow;
  onSave: (name: string, overrides: Record<string, boolean>) => void;
  onCancel: () => void;
}

function ScopeEditor({ flag, onSave, onCancel }: ScopeEditorProps) {
  const currentOverrides = parseOverrides(flag.overrides);
  const hasOverrides = Object.keys(currentOverrides).length > 0;
  const [mode, setMode] = useState<"fleet" | "per-pod">(hasOverrides ? "per-pod" : "fleet");
  const [podStates, setPodStates] = useState<Record<string, boolean>>(() => {
    const states: Record<string, boolean> = {};
    for (let i = 1; i <= 8; i++) {
      const key = `pod_${i}`;
      states[key] = currentOverrides[key] ?? flag.enabled;
    }
    return states;
  });

  const handleSave = () => {
    if (mode === "fleet") {
      onSave(flag.name, {});
    } else {
      // Only include pods that differ from the fleet-wide enabled state
      const overrides: Record<string, boolean> = {};
      for (const [key, val] of Object.entries(podStates)) {
        if (val !== flag.enabled) {
          overrides[key] = val;
        }
      }
      onSave(flag.name, overrides);
    }
  };

  return (
    <tr>
      <td colSpan={6} className="px-4 py-3 bg-neutral-900/50">
        <div className="space-y-3">
          <div className="flex items-center gap-4">
            <label className="text-xs text-neutral-400">Scope:</label>
            <select
              value={mode}
              onChange={(e) => setMode(e.target.value as "fleet" | "per-pod")}
              className="bg-neutral-800 border border-rp-border rounded px-2 py-1 text-xs text-neutral-300"
            >
              <option value="fleet">Fleet-wide</option>
              <option value="per-pod">Per-pod</option>
            </select>
          </div>
          {mode === "per-pod" && (
            <div className="grid grid-cols-4 gap-2">
              {Array.from({ length: 8 }, (_, i) => i + 1).map((n) => {
                const key = `pod_${n}`;
                return (
                  <label
                    key={key}
                    className="flex items-center gap-2 text-xs text-neutral-300 bg-neutral-800 rounded px-2 py-1.5"
                  >
                    <input
                      type="checkbox"
                      checked={podStates[key]}
                      onChange={(e) =>
                        setPodStates((prev) => ({ ...prev, [key]: e.target.checked }))
                      }
                      className="rounded border-rp-border"
                    />
                    Pod {n}
                  </label>
                );
              })}
            </div>
          )}
          <div className="flex items-center gap-2">
            <button
              onClick={handleSave}
              className="px-3 py-1 bg-emerald-600 hover:bg-emerald-500 text-white text-xs rounded transition-colors"
            >
              Save
            </button>
            <button
              onClick={onCancel}
              className="px-3 py-1 bg-neutral-700 hover:bg-neutral-600 text-neutral-300 text-xs rounded transition-colors"
            >
              Cancel
            </button>
          </div>
        </div>
      </td>
    </tr>
  );
}

interface FlagToggleProps {
  flag: FeatureFlagRow;
  onToggle: (name: string, enabled: boolean) => void;
}

function FlagToggle({ flag, onToggle }: FlagToggleProps) {
  const isKillSwitch = flag.name.startsWith("kill_");
  const [toggling, setToggling] = useState(false);

  const handleToggle = async () => {
    if (toggling) return;
    setToggling(true);
    onToggle(flag.name, !flag.enabled);
    setToggling(false);
  };

  return (
    <button
      role="switch"
      aria-checked={flag.enabled}
      onClick={handleToggle}
      disabled={toggling}
      className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
        flag.enabled
          ? isKillSwitch
            ? "bg-red-600"
            : "bg-emerald-600"
          : "bg-neutral-600"
      } ${toggling ? "opacity-50" : ""}`}
    >
      <span
        className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
          flag.enabled ? "translate-x-6" : "translate-x-1"
        }`}
      />
    </button>
  );
}

export default function FeatureFlagsPage() {
  const [flags, setFlags] = useState<FeatureFlagRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [editingFlag, setEditingFlag] = useState<string | null>(null);
  const { featureFlags } = useWebSocket();

  const loadFlags = useCallback(() => {
    api
      .listFlags()
      .then((data) => {
        setFlags(data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  useEffect(() => {
    loadFlags();
  }, [loadFlags]);

  // Merge WebSocket updates into local state
  useEffect(() => {
    if (featureFlags.length > 0) {
      setFlags((prev) => {
        const merged = new Map<string, FeatureFlagRow>();
        prev.forEach((f) => merged.set(f.name, f));
        featureFlags.forEach((f) => merged.set(f.name, f));
        return Array.from(merged.values());
      });
    }
  }, [featureFlags]);

  const handleToggle = async (name: string, enabled: boolean) => {
    // Optimistic update
    setFlags((prev) =>
      prev.map((f) => (f.name === name ? { ...f, enabled } : f))
    );
    try {
      const updated = await api.updateFlag(name, { enabled });
      setFlags((prev) =>
        prev.map((f) => (f.name === name ? updated : f))
      );
    } catch {
      // Revert on failure
      setFlags((prev) =>
        prev.map((f) => (f.name === name ? { ...f, enabled: !enabled } : f))
      );
      alert("Failed to update flag: " + name);
    }
  };

  const handleScopeUpdate = async (name: string, overrides: Record<string, boolean>) => {
    try {
      const updated = await api.updateFlag(name, { overrides });
      setFlags((prev) =>
        prev.map((f) => (f.name === name ? updated : f))
      );
      setEditingFlag(null);
    } catch {
      alert("Failed to update scope for flag: " + name);
    }
  };

  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white">Feature Flags</h1>
        <p className="text-sm text-rp-grey">Toggle flags fleet-wide or per-pod</p>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading flags...</div>
      ) : flags.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No feature flags registered</p>
          <p className="text-rp-grey text-sm">
            Feature flags appear when the server registers them via the flag registry.
          </p>
        </div>
      ) : (
        <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-rp-border">
                <th className="text-left px-4 py-3 text-rp-grey font-medium">Name</th>
                <th className="text-left px-4 py-3 text-rp-grey font-medium">Enabled</th>
                <th className="text-left px-4 py-3 text-rp-grey font-medium">Default</th>
                <th className="text-left px-4 py-3 text-rp-grey font-medium">Scope / Overrides</th>
                <th className="text-left px-4 py-3 text-rp-grey font-medium">Version</th>
                <th className="text-left px-4 py-3 text-rp-grey font-medium">Updated</th>
              </tr>
            </thead>
            <tbody>
              {flags.map((flag) => {
                const isKillSwitch = flag.name.startsWith("kill_");
                const overrides = parseOverrides(flag.overrides);
                const hasOverrides = Object.keys(overrides).length > 0;

                return (
                  <>
                    <tr
                      key={flag.name}
                      className={`border-b border-rp-border/50 hover:bg-neutral-800/30 ${
                        isKillSwitch ? "border-l-2 border-l-red-500" : ""
                      }`}
                    >
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2">
                          {isKillSwitch && (
                            <span className="flex items-center justify-center w-5 h-5 rounded-full bg-red-900/50 text-red-400 text-xs font-bold">
                              !
                            </span>
                          )}
                          <code className="text-neutral-300 font-mono text-xs">
                            {flag.name}
                          </code>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <FlagToggle flag={flag} onToggle={handleToggle} />
                      </td>
                      <td className="px-4 py-3">
                        <span
                          className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                            flag.default_value
                              ? "bg-emerald-900/50 text-emerald-400"
                              : "bg-neutral-700 text-neutral-400"
                          }`}
                        >
                          {flag.default_value ? "ON" : "OFF"}
                        </span>
                      </td>
                      <td className="px-4 py-3">
                        <div className="flex items-center gap-2 flex-wrap">
                          {!hasOverrides ? (
                            <span className="inline-flex items-center px-2 py-0.5 rounded text-xs font-medium bg-blue-900/50 text-blue-400">
                              Fleet-wide
                            </span>
                          ) : (
                            Object.entries(overrides).map(([key, val]) => (
                              <span
                                key={key}
                                className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                                  val
                                    ? "bg-emerald-900/50 text-emerald-400"
                                    : "bg-neutral-700 text-neutral-400"
                                }`}
                              >
                                {key.replace("pod_", "Pod ")}: {val ? "ON" : "OFF"}
                              </span>
                            ))
                          )}
                          <button
                            onClick={() =>
                              setEditingFlag(editingFlag === flag.name ? null : flag.name)
                            }
                            className="text-xs text-rp-grey hover:text-white transition-colors underline"
                          >
                            Edit Scope
                          </button>
                        </div>
                      </td>
                      <td className="px-4 py-3">
                        <code className="text-neutral-400 font-mono text-xs">
                          {flag.version}
                        </code>
                      </td>
                      <td className="px-4 py-3 text-neutral-400 text-xs">
                        {formatIST(flag.updated_at)}
                      </td>
                    </tr>
                    {editingFlag === flag.name && (
                      <ScopeEditor
                        key={`edit-${flag.name}`}
                        flag={flag}
                        onSave={handleScopeUpdate}
                        onCancel={() => setEditingFlag(null)}
                      />
                    )}
                  </>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </DashboardLayout>
  );
}
