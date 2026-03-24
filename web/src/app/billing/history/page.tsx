"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api, fetchApi } from "@/lib/api";
import { getToken } from "@/lib/auth";
import type { DailyReport, BillingSessionRecord } from "@/lib/api";

type RefundMethod = "wallet" | "cash" | "upi";

interface RefundModalProps {
  session: BillingSessionRecord;
  onClose: () => void;
  onSuccess: () => void;
}

function RefundModal({ session, onClose, onSuccess }: RefundModalProps) {
  const [amount, setAmount] = useState<number>(Math.floor(session.price_paise / 100));
  const [method, setMethod] = useState<RefundMethod>("wallet");
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const maxCredits = Math.floor(session.price_paise / 100);

  async function handleSubmit() {
    if (!reason.trim()) { setError("Reason is required"); return; }
    if (amount <= 0 || amount > maxCredits) { setError(`Amount must be 1-${maxCredits} credits`); return; }

    setSubmitting(true);
    setError(null);
    try {
      const token = getToken();
      const res = await fetch(
        `${process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080"}/api/v1/billing/${session.id}/refund`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            ...(token ? { Authorization: `Bearer ${token}` } : {}),
          },
          body: JSON.stringify({
            amount_paise: amount * 100,
            method,
            reason: reason.trim(),
          }),
        }
      );
      const data = await res.json();
      if (data.error) {
        setError(data.error);
      } else {
        onSuccess();
      }
    } catch {
      setError("Failed to process refund");
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div className="bg-[#222] border border-[#333] rounded-xl p-6 w-full max-w-md" onClick={(e) => e.stopPropagation()}>
        <h2 className="text-lg font-bold text-white mb-4">Refund Session</h2>
        <div className="text-sm text-neutral-400 mb-4">
          <div>{session.driver_name} — {session.pricing_tier_name}</div>
          <div>Charged: {maxCredits} credits</div>
        </div>

        <label className="block text-xs text-neutral-400 mb-1">Amount (credits)</label>
        <input
          type="number"
          min={1}
          max={maxCredits}
          value={amount}
          onChange={(e) => setAmount(parseInt(e.target.value) || 0)}
          className="w-full bg-[#1A1A1A] border border-[#333] rounded-lg px-3 py-2 text-sm text-white mb-3"
        />

        <label className="block text-xs text-neutral-400 mb-1">Method</label>
        <div className="grid grid-cols-3 gap-2 mb-3">
          {(["wallet", "cash", "upi"] as const).map((m) => (
            <button
              key={m}
              onClick={() => setMethod(m)}
              className={`px-3 py-2 rounded-lg text-sm border transition-colors ${
                method === m
                  ? "border-[#E10600] bg-[#E10600]/10 text-white"
                  : "border-[#333] text-neutral-400 hover:border-neutral-500"
              }`}
            >
              {m.toUpperCase()}
            </button>
          ))}
        </div>

        <label className="block text-xs text-neutral-400 mb-1">Reason (required)</label>
        <textarea
          value={reason}
          onChange={(e) => setReason(e.target.value)}
          placeholder="Why is this refund being issued?"
          rows={2}
          className="w-full bg-[#1A1A1A] border border-[#333] rounded-lg px-3 py-2 text-sm text-white mb-3 resize-none"
        />

        {error && <div className="text-red-400 text-sm mb-3">{error}</div>}

        <div className="flex gap-3">
          <button onClick={onClose} className="flex-1 px-4 py-2 rounded-lg border border-[#333] text-neutral-400 text-sm hover:bg-[#333]/50">
            Cancel
          </button>
          <button
            onClick={handleSubmit}
            disabled={submitting}
            className="flex-1 px-4 py-2 rounded-lg bg-[#E10600] text-white text-sm font-medium hover:bg-[#E10600]/80 disabled:opacity-50"
          >
            {submitting ? "Processing..." : `Refund ${amount} cr`}
          </button>
        </div>
      </div>
    </div>
  );
}

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
  const [refundSession, setRefundSession] = useState<BillingSessionRecord | null>(null);

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
                  <th className="text-center px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">
                    Actions
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
                    <td className="px-4 py-3 text-center">
                      {(s.status === "completed" || s.status === "ended_early") && (
                        <button
                          onClick={() => setRefundSession(s)}
                          className="px-2 py-1 rounded text-xs bg-amber-900/40 text-amber-400 border border-amber-800 hover:bg-amber-900/60 transition-colors"
                        >
                          Refund
                        </button>
                      )}
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
      {refundSession && (
        <RefundModal
          session={refundSession}
          onClose={() => setRefundSession(null)}
          onSuccess={() => {
            setRefundSession(null);
            // Reload report
            api.dailyBillingReport(date).then(setReport).catch(() => {});
          }}
        />
      )}
    </DashboardLayout>
  );
}
