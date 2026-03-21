"use client";

import { useState, useEffect, useCallback } from "react";
import DashboardLayout from "@/components/DashboardLayout";

const SENTRY_BASE = "http://192.168.31.27:8096";

interface CameraInfo {
  name: string;
  role: string;
  stream_url: string;
  status: string;
}

export default function CamerasPage() {
  const [cameras, setCameras] = useState<CameraInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchCameras = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const res = await fetch(`${SENTRY_BASE}/api/v1/cameras`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data: CameraInfo[] = await res.json();
      setCameras(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to fetch cameras");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchCameras();
  }, [fetchCameras]);

  function statusColor(status: string): string {
    switch (status.toLowerCase()) {
      case "connected":
        return "bg-green-500";
      case "reconnecting":
        return "bg-yellow-500";
      default:
        return "bg-red-500";
    }
  }

  function isOffline(status: string): boolean {
    const s = status.toLowerCase();
    return s === "disconnected" || s === "offline";
  }

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold">Live Cameras</h1>
          <p className="text-sm text-rp-grey">
            Security camera feeds from rc-sentry-ai
          </p>
        </div>
        <button
          onClick={fetchCameras}
          className="px-3 py-1.5 rounded-lg text-xs font-medium bg-rp-card text-rp-grey border border-rp-border hover:text-white transition-colors"
        >
          Refresh
        </button>
      </div>

      {loading ? (
        <div className="text-center text-rp-grey py-16">
          <p className="animate-pulse">Loading camera feeds...</p>
        </div>
      ) : error ? (
        <div className="text-center py-16">
          <p className="text-red-400 text-sm mb-3">{error}</p>
          <button
            onClick={fetchCameras}
            className="px-4 py-2 rounded-lg text-xs font-medium bg-rp-card text-rp-grey border border-rp-border hover:text-white transition-colors"
          >
            Retry
          </button>
        </div>
      ) : cameras.length === 0 ? (
        <div className="text-center text-rp-grey py-16">
          <p className="text-sm">No cameras configured.</p>
          <p className="text-xs mt-1">
            Add cameras in rc-sentry-ai to see live feeds here.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {cameras.map((camera) => (
            <div
              key={camera.name}
              className="bg-rp-card border border-rp-border rounded-lg overflow-hidden"
            >
              <div className="flex items-center justify-between px-4 py-2 border-b border-rp-border">
                <div className="flex items-center gap-2">
                  <span className="font-bold text-sm">{camera.name}</span>
                  <span className="text-[10px] px-1.5 py-0.5 rounded bg-rp-black text-rp-grey border border-rp-border">
                    {camera.role}
                  </span>
                </div>
                <div className="flex items-center gap-1.5">
                  <span
                    className={`w-2 h-2 rounded-full ${statusColor(camera.status)}`}
                  />
                  <span className="text-[10px] text-rp-grey">
                    {camera.status}
                  </span>
                </div>
              </div>

              {isOffline(camera.status) ? (
                <div className="w-full aspect-video bg-black flex items-center justify-center">
                  <span className="text-rp-grey text-sm">Camera Offline</span>
                </div>
              ) : (
                /* eslint-disable-next-line @next/next/no-img-element */
                <img
                  src={`${SENTRY_BASE}${camera.stream_url}`}
                  alt={camera.name}
                  className="w-full aspect-video object-cover bg-black"
                />
              )}
            </div>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
