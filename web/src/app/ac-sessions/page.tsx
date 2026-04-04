"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import DashboardLayout from "@/components/DashboardLayout";
import StatusBadge from "@/components/StatusBadge";
import { api } from "@/lib/api";
import type { AcSessionRecord } from "@/lib/api";

export default function AcSessionsPage() {
  const router = useRouter();
  const [sessions, setSessions] = useState<AcSessionRecord[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.listAcSessions({ limit: 50 }).then((res) => {
      setSessions(res.sessions || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  return (
    <DashboardLayout>
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white">AC Session Results</h1>
        <p className="text-sm text-rp-grey">
          Assetto Corsa LAN sessions — click to view leaderboard
        </p>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading sessions...</div>
      ) : sessions.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No AC sessions yet</p>
          <p className="text-rp-grey text-sm">
            Start an AC LAN session from the AC LAN Race page.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {sessions.map((session) => (
            <button
              key={session.id}
              onClick={() => router.push(`/ac-sessions/${session.id}`)}
              className="w-full flex items-center justify-between bg-rp-card border border-rp-border rounded-lg px-4 py-3 hover:border-rp-red/30 transition-colors text-left"
            >
              <div className="flex items-center gap-4">
                <div>
                  <span className="text-neutral-200 font-medium">
                    AC LAN Session
                  </span>
                  {session.pod_ids && (
                    <span className="text-rp-grey text-xs ml-2">
                      {(() => {
                        try {
                          const pods = JSON.parse(session.pod_ids) as string[];
                          return pods.map(p => p.replace("pod_", "Pod ")).join(", ");
                        } catch {
                          return session.pod_ids;
                        }
                      })()}
                    </span>
                  )}
                </div>
              </div>
              <div className="flex items-center gap-3">
                {session.started_at && (
                  <span className="text-xs text-rp-grey">
                    {new Date(/[Z+]/.test(session.started_at) ? session.started_at : session.started_at + "Z").toLocaleString("en-IN", {
                      timeZone: "Asia/Kolkata",
                      day: "numeric",
                      month: "short",
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </span>
                )}
                <StatusBadge status={session.status} />
                <svg className="w-4 h-4 text-rp-grey" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2}>
                  <path d="M9 18l6-6-6-6" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              </div>
            </button>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
