"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api } from "@/lib/api";
import type { DailyReport, BillingSessionRecord } from "@/lib/api";

const formatCredits = (paise: number) => `${Math.floor(paise / 100)} cr`;

function formatMMSS(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
}

function formatTime(iso?: string): string {
  if (!iso) return "--:--";
  return new Date(iso).toLocaleTimeString("en-IN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: true,
  });
}

function todayISO(): string {
  const d = new Date();
  return d.toISOString().split("T")[0];
}

const statusColors: Record<string, string> = {
  completed: "bg-emerald-900/50 text-emerald-400",
  ended_early: "bg-amber-900/50 text-amber-400",
  cancelled: "bg-red-900/50 text-red-400",
  active: "bg-blue-900/50 text-blue-400",
  paused_manual: "bg-rp-card text-neutral-400",
};

export default function BillingHistoryPage() {
  const [date, setDate] = useState(todayISO());
  const [report, setReport] = useState<DailyReport | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    api
      .dailyBillingReport(date)
      .then((res) => {
        setReport(res);
        setLoading(false);
      })
      .catch(() => {
        setReport(null);
        setLoading(false);
      });
  }, [date]);

  const sessions: BillingSessionRecord[] = report?.sessions || [];

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Billing History</h1>
          <p className="text-sm text-rp-grey">Daily session records</p>
        </div>
        <input
          type="date"
          value={date}
          onChange={(e) => setDate(e.target.value)}
          className="bg-rp-card border border-rp-border rounded-lg px-3 py-2 text-sm text-neutral-200 focus:outline-none focus:border-rp-red transition-colors"
        />
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">
          Loading report...
        </div>
      ) : sessions.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No sessions for this date</p>
          <p className="text-rp-grey text-sm">
            Select a different date or start a billing session.
          </p>
        </div>
      ) : (
        <div className="space-y-4">
          {/* Sessions Table */}
          <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border">
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Time
                  </th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Driver
                  </th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Pod
                  </th>
                  <th className="text-left px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Tier
                  </th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Allocated
                  </th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Drove
                  </th>
                  <th className="text-right px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Price
                  </th>
                  <th className="text-center px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Status
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-rp-border/50">
                {sessions.map((s) => (
                  <tr
                    key={s.id}
                    className="hover:bg-rp-card/30 transition-colors"
                  >
                    <td className="px-4 py-3 text-neutral-300 font-mono text-xs">
                      {formatTime(s.started_at)}
                    </td>
                    <td className="px-4 py-3 text-neutral-200">
                      {s.driver_name}
                    </td>
                    <td className="px-4 py-3 text-neutral-400 font-mono text-xs">
                      {s.pod_id.slice(0, 8)}
                    </td>
                    <td className="px-4 py-3 text-neutral-400">
                      {s.pricing_tier_name}
                    </td>
                    <td className="px-4 py-3 text-neutral-400 text-right font-mono">
                      {formatMMSS(s.allocated_seconds)}
                    </td>
                    <td className="px-4 py-3 text-neutral-300 text-right font-mono">
                      {formatMMSS(s.driving_seconds)}
                    </td>
                    <td className="px-4 py-3 text-rp-red text-right font-mono">
                      {formatCredits(s.price_paise)}
                    </td>
                    <td className="px-4 py-3 text-center">
                      <span
                        className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${
                          statusColors[s.status] || "bg-rp-card text-neutral-400"
                        }`}
                      >
                        {s.status.replace(/_/g, " ")}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Summary */}
          {report && (
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
              <div className="bg-rp-card border border-rp-border rounded-lg p-4">
                <div className="text-xs text-rp-grey mb-1">
                  Total Sessions
                </div>
                <div className="text-2xl font-bold text-white font-mono">
                  {report.total_sessions}
                </div>
              </div>
              <div className="bg-rp-card border border-rp-border rounded-lg p-4">
                <div className="text-xs text-rp-grey mb-1">Total Credits</div>
                <div className="text-2xl font-bold text-rp-red font-mono">
                  {formatCredits(report.total_revenue_paise)}
                </div>
              </div>
              <div className="bg-rp-card border border-rp-border rounded-lg p-4">
                <div className="text-xs text-rp-grey mb-1">
                  Total Driving Time
                </div>
                <div className="text-2xl font-bold text-white font-mono">
                  {formatMMSS(report.total_driving_seconds)}
                </div>
              </div>
            </div>
          )}
        </div>
      )}
    </DashboardLayout>
  );
}
