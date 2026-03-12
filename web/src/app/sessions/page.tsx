"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import StatusBadge from "@/components/StatusBadge";
import type { Session } from "@/lib/api";
import { api } from "@/lib/api";

const simLabels: Record<string, string> = {
  assetto_corsa: "Assetto Corsa",
  assetto_corsa_evo: "AC EVO",
  assetto_corsa_rally: "AC Rally",
  f1_25: "F1 25",
  iracing: "iRacing",
  le_mans_ultimate: "Le Mans Ultimate",
  forza: "Forza Motorsport",
  forza_horizon_5: "Forza Horizon 5",
};

export default function SessionsPage() {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.listSessions().then((res) => {
      setSessions(res.sessions || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Sessions</h1>
          <p className="text-sm text-rp-grey">Practice, race, and qualifying sessions</p>
        </div>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading sessions...</div>
      ) : sessions.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No sessions yet</p>
          <p className="text-rp-grey text-sm">
            Sessions are created when you start a practice, race, or qualifying run.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {sessions.map((session) => (
            <div
              key={session.id}
              className="flex items-center justify-between bg-rp-card border border-rp-border rounded-lg px-4 py-3"
            >
              <div className="flex items-center gap-4">
                <div>
                  <span className="text-neutral-200 font-medium capitalize">
                    {session.type}
                  </span>
                  <span className="text-rp-grey text-sm ml-2">
                    {session.track}
                  </span>
                </div>
                <span className="text-xs text-rp-grey">
                  {simLabels[session.sim_type] || session.sim_type}
                </span>
                {session.car_class && (
                  <span className="text-xs text-rp-grey">{session.car_class}</span>
                )}
              </div>
              <div className="flex items-center gap-3">
                {session.started_at && (
                  <span className="text-xs text-rp-grey">
                    {new Date(session.started_at).toLocaleString()}
                  </span>
                )}
                <StatusBadge status={session.status} />
              </div>
            </div>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
