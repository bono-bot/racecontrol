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

  useEffect(() => {
    api.venue().then(setVenue).catch(() => {});
    api.health().then(setHealth).catch(() => {});
  }, []);

  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-zinc-100">Settings</h1>
        <p className="text-sm text-zinc-500">System configuration and status</p>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Server Status */}
        <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-5">
          <h2 className="text-sm font-medium text-zinc-400 mb-4">Server Status</h2>
          <div className="space-y-3 text-sm">
            <div className="flex justify-between">
              <span className="text-zinc-500">Status</span>
              <span className={health?.status === "ok" ? "text-emerald-400" : "text-red-400"}>
                {health?.status || "Unknown"}
              </span>
            </div>
            <div className="flex justify-between">
              <span className="text-zinc-500">Version</span>
              <span className="text-zinc-300 font-mono">{health?.version || "—"}</span>
            </div>
          </div>
        </div>

        {/* Venue Info */}
        <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-5">
          <h2 className="text-sm font-medium text-zinc-400 mb-4">Venue</h2>
          <div className="space-y-3 text-sm">
            <div className="flex justify-between">
              <span className="text-zinc-500">Name</span>
              <span className="text-zinc-300">{venue?.name || "—"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-zinc-500">Location</span>
              <span className="text-zinc-300">{venue?.location || "—"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-zinc-500">Timezone</span>
              <span className="text-zinc-300 font-mono">{venue?.timezone || "—"}</span>
            </div>
            <div className="flex justify-between">
              <span className="text-zinc-500">Pod Capacity</span>
              <span className="text-zinc-300">{venue?.pods || "—"}</span>
            </div>
          </div>
        </div>
      </div>
    </DashboardLayout>
  );
}
