"use client";

import { useState, useEffect } from "react";

const API_BASE =
  process.env.NEXT_PUBLIC_API_URL ||
  (typeof window !== "undefined"
    ? `${window.location.protocol}//${window.location.host}`
    : "http://localhost:8080");

interface PodStatus {
  pod_number: number;
  ws_connected: boolean;
  http_reachable: boolean;
}

export default function ScarcityBanner() {
  const [available, setAvailable] = useState<number | null>(null);
  const [total, setTotal] = useState(8);

  useEffect(() => {
    let active = true;
    const load = () => {
      fetch(`${API_BASE}/api/v1/fleet/health`)
        .then((r) => r.json())
        .then((d) => {
          if (!active || !Array.isArray(d?.pods)) return;
          const pods: PodStatus[] = d.pods;
          setTotal(pods.length);
          setAvailable(pods.filter((p) => p.ws_connected && p.http_reachable).length);
        })
        .catch(() => {});
    };
    load();
    const id = setInterval(load, 10000);
    return () => { active = false; clearInterval(id); };
  }, []);

  if (available === null) return null;

  const colorClass =
    available >= 5
      ? "text-green-400"
      : available >= 2
        ? "text-yellow-400"
        : "text-[#E10600]";

  const bgClass =
    available >= 5
      ? "bg-green-400/10 border-green-400/30"
      : available >= 2
        ? "bg-yellow-400/10 border-yellow-400/30"
        : "bg-[#E10600]/10 border-[#E10600]/30";

  return (
    <div
      data-testid="scarcity-banner"
      className={`rounded-lg border px-3 py-2 text-center text-sm ${bgClass}`}
    >
      {available === 0 ? (
        <span className={colorClass}>
          All pods in use — next slot likely in ~30min
        </span>
      ) : (
        <span className={colorClass}>
          {available} of {total} pods available now
        </span>
      )}
    </div>
  );
}
