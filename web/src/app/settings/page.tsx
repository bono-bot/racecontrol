"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { AlertTriangle } from "lucide-react";
import { api, type BackupStatus } from "@/lib/api";

export default function SettingsPage() {
  const [venue, setVenue] = useState<{
    name: string;
    location: string;
    timezone: string;
    pods: number;
  } | null>(null);
  const [health, setHealth] = useState<{
    status: string;
    version: string;
  } | null>(null);
  const [posLocked, setPosLocked] = useState<boolean | null>(null);
  const [posToggling, setPosToggling] = useState(false);
  const [backup, setBackup] = useState<BackupStatus | null>(null);

  useEffect(() => {
    api.venue().then(setVenue).catch(() => {});
    api.health().then(setHealth).catch(() => {});
    api.getPosLockdown().then((r) => setPosLocked(r.locked)).catch(() => {});
    api.backupStatus().then(setBackup).catch(() => {});
  }, []);

  const togglePosLockdown = async () => {
    if (posLocked === null) return;
    setPosToggling(true);
    try {
      const res = await api.setPosLockdown(!posLocked);
      if (res.ok) setPosLocked(res.locked);
    } catch {
      // ignore
    }
    setPosToggling(false);
  };

  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white">Settings</h1>
        <p className="text-sm text-rp-grey">System configuration and status</p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Server Status */}
        <div className="bg-rp-card border border-rp-border rounded-lg p-5">
          <h2 className="text-sm font-medium text-neutral-400 mb-4">Server Status</h2>
          <div className="space-y-3 text-sm">
            <div className="flex justify-between">
              <span className="text-rp-grey">Status</span>
              <span className={health?.status === "ok" ? "text-emerald-400" : "text-red-400"}>
                {health?.status || "Unknown"}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Version</span>
              <span className="text-neutral-300 font-mono">{health?.version || "---"}</span>
            </div>
          </div>
        </div>

        {/* Venue Info */}
        <div className="bg-rp-card border border-rp-border rounded-lg p-5">
          <h2 className="text-sm font-medium text-neutral-400 mb-4">Venue</h2>
          <div className="space-y-3 text-sm">
            <div className="flex justify-between">
              <span className="text-rp-grey">Name</span>
              <span className="text-neutral-300">{venue?.name || "---"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Location</span>
              <span className="text-neutral-300">{venue?.location || "---"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Timezone</span>
              <span className="text-neutral-300 font-mono">{venue?.timezone || "---"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Pod Capacity</span>
              <span className="text-neutral-300">{venue?.pods || "---"}</span>
            </div>
          </div>
        </div>

        {/* POS Lockdown */}
        <div className="bg-rp-card border border-rp-border rounded-lg p-5">
          <h2 className="text-sm font-medium text-neutral-400 mb-4">POS Terminal</h2>
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <p className="text-sm text-neutral-300">Kiosk Lockdown</p>
                <p className="text-xs text-rp-grey mt-0.5">
                  {posLocked
                    ? "POS is locked to billing dashboard only"
                    : "POS has full desktop access"}
                </p>
              </div>
              <button
                onClick={togglePosLockdown}
                disabled={posLocked === null || posToggling}
                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                  posLocked
                    ? "bg-rp-red"
                    : "bg-neutral-600"
                } ${posToggling ? "opacity-50" : ""}`}
              >
                <span
                  className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                    posLocked ? "translate-x-6" : "translate-x-1"
                  }`}
                />
              </button>
            </div>
            <div className="flex justify-between text-sm">
              <span className="text-rp-grey">Status</span>
              <span className={posLocked ? "text-rp-red" : "text-emerald-400"}>
                {posLocked === null ? "Loading..." : posLocked ? "Locked" : "Unlocked"}
              </span>
            </div>
            {posLocked && (
              <div className="flex items-center gap-2 text-xs text-amber-400 bg-amber-500/10 border border-amber-500/20 rounded px-3 py-2">
                <AlertTriangle className="w-4 h-4 shrink-0" />
                <span>POS terminal is restricted to billing only</span>
              </div>
            )}
          </div>
        </div>

        {/* Backup Status */}
        <div className="bg-rp-card border border-rp-border rounded-lg p-5">
          <h2 className="text-sm font-medium text-neutral-400 mb-4">Backup Status</h2>
          <div className="space-y-3 text-sm">
            <div className="flex justify-between">
              <span className="text-rp-grey">Last Backup</span>
              <span className="text-neutral-300">{backup?.last_backup_at ?? "Never"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Size</span>
              <span className="text-neutral-300 font-mono">
                {backup?.last_backup_size_bytes != null
                  ? `${(backup.last_backup_size_bytes / 1024 / 1024).toFixed(1)} MB`
                  : "---"}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Local Backups</span>
              <span className="text-neutral-300">{backup?.backup_count_local ?? "---"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Remote (Bono VPS)</span>
              <span className={backup?.remote_reachable ? "text-emerald-400" : "text-red-400"}>
                {backup ? (backup.remote_reachable ? "Reachable" : "Unreachable") : "---"}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Last Transfer</span>
              <span className="text-neutral-300">{backup?.last_remote_transfer_at ?? "Never"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-rp-grey">Checksum Match</span>
              <span className={
                backup?.last_checksum_match === true ? "text-emerald-400" :
                backup?.last_checksum_match === false ? "text-red-400" :
                "text-neutral-500"
              }>
                {backup?.last_checksum_match === true ? "OK" :
                 backup?.last_checksum_match === false ? "MISMATCH" : "---"}
              </span>
            </div>
            {backup?.staleness_hours != null && backup.staleness_hours > 2 && (
              <div className="flex items-center gap-2 text-amber-400 mt-2">
                <AlertTriangle className="w-4 h-4" />
                <span>Backup stale ({backup.staleness_hours.toFixed(1)}h)</span>
              </div>
            )}
          </div>
        </div>

        {/* Brand Theme Preview */}
        <div className="bg-rp-card border border-rp-border rounded-lg p-5 lg:col-span-2">
          <h2 className="text-sm font-medium text-neutral-400 mb-4">Brand Theme</h2>
          <div className="flex flex-wrap gap-3">
            {[
              { name: "Racing Red", token: "--rp-red", hex: "#E10600", cls: "bg-rp-red" },
              { name: "Asphalt Black", token: "--rp-black", hex: "#1A1A1A", cls: "bg-[#1A1A1A]" },
              { name: "Card Surface", token: "--rp-card", hex: "#222222", cls: "bg-rp-card" },
              { name: "Border", token: "--rp-border", hex: "#333333", cls: "bg-rp-border" },
              { name: "Gunmetal Grey", token: "--rp-grey", hex: "#5A5A5A", cls: "bg-rp-grey" },
              { name: "Emerald (active)", token: "emerald-400", hex: "#34D399", cls: "bg-emerald-400" },
              { name: "Amber (warning)", token: "amber-400", hex: "#FBBF24", cls: "bg-amber-400" },
            ].map((color) => (
              <div key={color.token} className="flex items-center gap-2">
                <div className={`w-8 h-8 rounded border border-rp-border ${color.cls}`} />
                <div>
                  <p className="text-xs text-neutral-300 font-medium">{color.name}</p>
                  <p className="text-[10px] font-mono text-rp-grey">{color.hex}</p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Danger Zone */}
      <div className="mt-6 border border-red-500/20 bg-red-500/5 rounded-lg p-5">
        <h2 className="text-sm font-medium text-red-400 mb-2">Danger Zone</h2>
        <p className="text-xs text-rp-grey">Destructive venue operations. Contact support before using.</p>
      </div>
    </DashboardLayout>
  );
}
