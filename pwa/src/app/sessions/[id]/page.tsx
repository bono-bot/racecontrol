"use client";

import { useEffect, useState } from "react";
import { useRouter, useParams } from "next/navigation";
import { api, isLoggedIn } from "@/lib/api";
import type { BillingSession } from "@/lib/api";
import BottomNav from "@/components/BottomNav";

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}m ${s}s`;
}

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleDateString("en-IN", {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatPrice(paise: number | null): string {
  if (!paise) return "—";
  return `₹${(paise / 100).toFixed(0)}`;
}

export default function SessionDetailPage() {
  const router = useRouter();
  const params = useParams();
  const sessionId = params.id as string;

  const [session, setSession] = useState<BillingSession | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    if (!isLoggedIn()) {
      router.replace("/login");
      return;
    }

    api.sessions().then((res) => {
      if (res.sessions) {
        const found = res.sessions.find((s) => s.id === sessionId);
        if (found) setSession(found);
      }
      setLoading(false);
    });
  }, [router, sessionId]);

  if (loading) {
    return (
      <div className="flex items-center justify-center min-h-screen">
        <div className="w-8 h-8 border-2 border-rp-orange border-t-transparent rounded-full animate-spin" />
      </div>
    );
  }

  if (!session) {
    return (
      <div className="min-h-screen pb-20">
        <div className="px-4 pt-12 max-w-lg mx-auto">
          <button onClick={() => router.back()} className="text-rp-orange text-sm mb-4">
            &larr; Back
          </button>
          <p className="text-zinc-500">Session not found</p>
        </div>
        <BottomNav />
      </div>
    );
  }

  const usagePercent = Math.min(
    100,
    (session.driving_seconds / session.allocated_seconds) * 100
  );

  return (
    <div className="min-h-screen pb-20">
      <div className="px-4 pt-12 pb-4 max-w-lg mx-auto">
        <button onClick={() => router.back()} className="text-rp-orange text-sm mb-4">
          &larr; Back
        </button>

        {/* Receipt header */}
        <div className="bg-rp-card border border-rp-border rounded-xl p-6 mb-4">
          <div className="text-center mb-6">
            <h1 className="text-xl font-bold text-rp-orange mb-1">
              RacingPoint
            </h1>
            <p className="text-xs text-zinc-500">Session Receipt</p>
          </div>

          <div className="space-y-4">
            <Row label="Pod" value={session.pod_id.replace("pod_", "#")} />
            <Row label="Status" value={session.status.replace("_", " ")} />
            <Row label="Started" value={formatDate(session.started_at)} />
            <Row label="Ended" value={formatDate(session.ended_at)} />

            <div className="border-t border-zinc-800 my-2" />

            <Row
              label="Allocated Time"
              value={formatDuration(session.allocated_seconds)}
            />
            <Row
              label="Drive Time"
              value={formatDuration(session.driving_seconds)}
              highlight
            />

            {session.custom_price_paise && (
              <>
                <div className="border-t border-zinc-800 my-2" />
                <Row
                  label="Amount"
                  value={formatPrice(session.custom_price_paise)}
                  highlight
                />
              </>
            )}
          </div>

          {/* Usage bar */}
          <div className="mt-6">
            <div className="flex justify-between text-xs text-zinc-500 mb-1">
              <span>Usage</span>
              <span>{usagePercent.toFixed(0)}%</span>
            </div>
            <div className="h-2 bg-zinc-800 rounded-full overflow-hidden">
              <div
                className="h-full bg-rp-orange rounded-full transition-all"
                style={{ width: `${usagePercent}%` }}
              />
            </div>
          </div>
        </div>

        <p className="text-center text-zinc-600 text-xs">
          ID: {session.id.slice(0, 8)}...
        </p>
      </div>
      <BottomNav />
    </div>
  );
}

function Row({
  label,
  value,
  highlight = false,
}: {
  label: string;
  value: string;
  highlight?: boolean;
}) {
  return (
    <div className="flex justify-between items-center">
      <span className="text-sm text-zinc-500">{label}</span>
      <span
        className={`text-sm font-medium ${
          highlight ? "text-zinc-100" : "text-zinc-300"
        }`}
      >
        {value}
      </span>
    </div>
  );
}
