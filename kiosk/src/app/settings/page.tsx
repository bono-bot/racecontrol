"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { KioskExperience, KioskSettings } from "@/lib/types";
import { DeployPanel } from "@/components/DeployPanel";
import { useKioskSocket } from "@/hooks/useKioskSocket";

export default function SettingsPage() {
  const [experiences, setExperiences] = useState<KioskExperience[]>([]);
  const [settings, setSettings] = useState<KioskSettings | null>(null);
  const [loading, setLoading] = useState(true);
  const [allowlist, setAllowlist] = useState<{id: string; process_name: string; added_by: string; notes: string | null; created_at: string}[]>([]);
  const [hardcodedCount, setHardcodedCount] = useState<number>(70);
  const [newProcessName, setNewProcessName] = useState("");
  const { deployStates, sendDeployRolling } = useKioskSocket();

  useEffect(() => {
    loadData();
  }, []);

  async function loadData() {
    try {
      const [expRes, setRes, allowlistRes] = await Promise.all([
        api.listExperiences(),
        api.getSettings(),
        api.listKioskAllowlist(),
      ]);
      setExperiences(expRes.experiences || []);
      setSettings(setRes.settings || null);
      setAllowlist(allowlistRes.allowlist || []);
      setHardcodedCount(allowlistRes.hardcoded_count ?? 70);
    } catch (e) {
      console.error("Failed to load settings:", e);
    } finally {
      setLoading(false);
    }
  }

  async function handleSettingChange(key: string, value: string) {
    await api.updateSettings({ [key]: value });
    setSettings((prev) => (prev ? { ...prev, [key]: value } : prev));
  }

  async function handleDeleteExperience(id: string) {
    await api.deleteExperience(id);
    setExperiences((prev) => prev.filter((e) => e.id !== id));
  }

  async function handleAddAllowlistEntry() {
    const name = newProcessName.trim();
    if (!name) return;
    try {
      const res = await api.addKioskAllowlistEntry(name);
      if (res.status === "already_in_baseline" || res.status === "already_exists") {
        console.warn(res.message || `'${name}' is already in the allowlist`);
      } else {
        const allowlistRes = await api.listKioskAllowlist();
        setAllowlist(allowlistRes.allowlist || []);
        setHardcodedCount(allowlistRes.hardcoded_count ?? 70);
      }
    } catch (e) {
      console.error("Failed to add allowlist entry:", e);
    }
    setNewProcessName("");
  }

  async function handleDeleteAllowlistEntry(processName: string) {
    try {
      await api.deleteKioskAllowlistEntry(processName);
      setAllowlist((prev) => prev.filter((e) => e.process_name !== processName));
    } catch (e) {
      console.error("Failed to delete allowlist entry:", e);
    }
  }

  if (loading) {
    return (
      <div className="h-screen flex items-center justify-center">
        <div className="w-8 h-8 border-2 border-rp-red border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  return (
    <div className="h-screen overflow-auto bg-rp-black text-white">
      {/* Header */}
      <header className="flex items-center justify-between px-6 py-4 border-b border-rp-border bg-rp-card">
        <div>
          <h1 className="text-xl font-bold">Kiosk Settings</h1>
          <p className="text-xs text-rp-grey">Configure experiences, venue, and display preferences</p>
        </div>
        <a href="/" className="px-4 py-2 text-sm border border-rp-border text-rp-grey hover:text-white rounded transition-colors">
          Back to Terminal
        </a>
      </header>

      <div className="max-w-5xl mx-auto p-6 space-y-8">
        {/* Venue Settings */}
        <section>
          <h2 className="text-lg font-semibold mb-4 border-b border-rp-border pb-2">Venue</h2>
          {settings && (
            <div className="grid grid-cols-2 gap-4">
              {[
                { key: "venue_name", label: "Venue Name" },
                { key: "tagline", label: "Tagline" },
                { key: "business_hours_start", label: "Business Hours Start" },
                { key: "business_hours_end", label: "Business Hours End" },
              ].map(({ key, label }) => (
                <div key={key} className="space-y-1">
                  <label className="text-xs text-rp-grey uppercase tracking-wider">{label}</label>
                  <input
                    type="text"
                    value={settings[key] || ""}
                    onChange={(e) => handleSettingChange(key, e.target.value)}
                    className="w-full px-3 py-2 bg-rp-card border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
                  />
                </div>
              ))}
            </div>
          )}
        </section>

        {/* Display Settings */}
        <section>
          <h2 className="text-lg font-semibold mb-4 border-b border-rp-border pb-2">Spectator Display</h2>
          {settings && (
            <div className="grid grid-cols-2 gap-4">
              {[
                { key: "spectator_auto_rotate", label: "Auto-Rotate Pods" },
                { key: "spectator_show_leaderboard", label: "Show Leaderboard" },
              ].map(({ key, label }) => (
                <div key={key} className="flex items-center justify-between bg-rp-card border border-rp-border rounded px-4 py-3">
                  <span className="text-sm">{label}</span>
                  <button
                    onClick={() =>
                      handleSettingChange(key, settings[key] === "true" ? "false" : "true")
                    }
                    className={`w-12 h-6 rounded-full relative transition-colors ${
                      settings[key] === "true" ? "bg-rp-red" : "bg-zinc-700"
                    }`}
                  >
                    <span
                      className={`absolute top-0.5 w-5 h-5 bg-white rounded-full transition-transform ${
                        settings[key] === "true" ? "translate-x-6" : "translate-x-0.5"
                      }`}
                    />
                  </button>
                </div>
              ))}
            </div>
          )}
        </section>

        {/* Pod Display */}
        <section>
          <h2 className="text-lg font-semibold mb-4 border-b border-rp-border pb-2">Pod Display</h2>
          <p className="text-xs text-rp-grey mb-4">Lock screen appearance on customer pods</p>
          {settings && (
            <div className="space-y-1">
              <label className="text-xs text-rp-grey uppercase tracking-wider">Lock Screen Wallpaper URL</label>
              <input
                type="url"
                value={settings?.lock_screen_wallpaper_url ?? ""}
                onChange={(e) => handleSettingChange("lock_screen_wallpaper_url", e.target.value)}
                placeholder="https://example.com/wallpaper.jpg or leave blank for default"
                className="w-full px-3 py-2 bg-rp-card border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
              />
              <p className="text-xs text-rp-grey mt-1">
                Enter an image URL accessible from the pod network. Visible on pods within 10 seconds. Leave blank for the default Racing Point gradient.
              </p>
            </div>
          )}
        </section>

        {/* Experiences */}
        <section>
          <div className="flex items-center justify-between mb-4 border-b border-rp-border pb-2">
            <h2 className="text-lg font-semibold">Experiences</h2>
            <span className="text-xs text-rp-grey">{experiences.length} configured</span>
          </div>
          <div className="space-y-2">
            {experiences.map((exp) => (
              <div
                key={exp.id}
                className="flex items-center justify-between bg-rp-card border border-rp-border rounded px-4 py-3"
              >
                <div className="flex-1">
                  <p className="text-sm font-semibold text-white">{exp.name}</p>
                  <p className="text-xs text-rp-grey">
                    {exp.game} &middot; {exp.track} &middot; {exp.car} &middot; {exp.duration_minutes}min
                    {exp.car_class && ` &middot; Class ${exp.car_class}`}
                  </p>
                </div>
                <button
                  onClick={() => handleDeleteExperience(exp.id)}
                  className="px-3 py-1 text-xs border border-rp-border text-rp-grey hover:text-rp-red hover:border-rp-red/50 rounded transition-colors"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        </section>

        {/* Kiosk Allowlist */}
        <section>
          <div className="flex items-center justify-between mb-4 border-b border-rp-border pb-2">
            <h2 className="text-lg font-semibold">Kiosk Allowlist</h2>
            <span className="text-xs text-rp-grey">{allowlist.length} staff-added &middot; {hardcodedCount} baseline</span>
          </div>
          <p className="text-xs text-rp-grey mb-4">
            Add process names that the kiosk lock screen should allow. The ~{hardcodedCount} baseline entries (game clients, system processes) are built in to rc-agent and cannot be removed here.
          </p>
          {/* Add entry row */}
          <div className="flex gap-2 mb-4">
            <input
              type="text"
              value={newProcessName}
              onChange={(e) => setNewProcessName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleAddAllowlistEntry()}
              placeholder="e.g. myapp.exe"
              className="flex-1 px-3 py-2 bg-rp-card border border-rp-border rounded text-sm text-white focus:outline-none focus:border-rp-red"
            />
            <button
              onClick={handleAddAllowlistEntry}
              className="bg-rp-red hover:bg-red-700 text-white text-sm px-4 py-2 rounded transition-colors"
            >
              Add
            </button>
          </div>
          {/* Staff-added entries */}
          <div className="space-y-2">
            {allowlist.length === 0 && (
              <p className="text-xs text-rp-grey py-2">No staff-added entries. Use the field above to add a process name.</p>
            )}
            {allowlist.map((entry) => (
              <div
                key={entry.id}
                className="flex items-center justify-between bg-rp-card border border-rp-border rounded px-4 py-3"
              >
                <div className="flex-1">
                  <p className="text-sm font-semibold text-white">{entry.process_name}</p>
                  <p className="text-xs text-rp-grey">
                    Added by {entry.added_by}
                    {entry.notes && ` · ${entry.notes}`}
                    {" · "}{new Date(entry.created_at).toLocaleDateString()}
                  </p>
                </div>
                <button
                  onClick={() => handleDeleteAllowlistEntry(entry.process_name)}
                  className="px-3 py-1 text-xs border border-rp-border text-rp-grey hover:text-rp-red hover:border-rp-red/50 rounded transition-colors"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>
        </section>

        {/* Deploy */}
        <section>
          <div className="border-b border-rp-border pb-2 mb-4">
            <h2 className="text-lg font-semibold">Agent Deploy</h2>
            <p className="text-xs text-rp-grey mt-0.5">
              Roll out a new rc-agent binary to all pods without disrupting active sessions.
            </p>
          </div>
          <div className="bg-rp-card border border-rp-border rounded p-4">
            <DeployPanel
              deployStates={deployStates}
              onDeploy={sendDeployRolling}
            />
          </div>
        </section>
      </div>
    </div>
  );
}
