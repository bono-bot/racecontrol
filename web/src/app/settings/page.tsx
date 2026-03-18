"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";

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

  useEffect(() => {
    api.venue().then(setVenue).catch(() => {});
    api.health().then(setHealth).catch(() => {});
    api.getPosLockdown().then((r) => setPosLocked(r.locked)).catch(() => {});
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
          </div>
        </div>
      </div>
    </DashboardLayout>
  );
}
