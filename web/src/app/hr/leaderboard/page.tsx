"use client";

import { useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";

interface StaffEntry {
  id: string;
  name: string;
  tasks_completed: number;
  avg_time_minutes: number;
  quality_score: number;
}

const MOCK_STAFF: StaffEntry[] = [
  { id: "s-1", name: "Arjun Mehta", tasks_completed: 47, avg_time_minutes: 12, quality_score: 4.8 },
  { id: "s-2", name: "Priya Sharma", tasks_completed: 41, avg_time_minutes: 15, quality_score: 4.6 },
  { id: "s-3", name: "Rahul Verma", tasks_completed: 38, avg_time_minutes: 18, quality_score: 4.5 },
  { id: "s-4", name: "Sneha Patel", tasks_completed: 29, avg_time_minutes: 20, quality_score: 4.3 },
  { id: "s-5", name: "Vikram Singh", tasks_completed: 24, avg_time_minutes: 22, quality_score: 4.1 },
];

const RANK_BADGES: Record<number, { label: string; color: string }> = {
  1: { label: "Gold", color: "bg-yellow-500/20 text-yellow-400 border-yellow-500/40" },
  2: { label: "Silver", color: "bg-neutral-400/20 text-neutral-300 border-neutral-400/40" },
  3: { label: "Bronze", color: "bg-orange-700/20 text-orange-400 border-orange-700/40" },
};

export default function StaffLeaderboardPage() {
  const [staff] = useState<StaffEntry[]>(MOCK_STAFF);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Staff Leaderboard</h1>
          <p className="text-sm text-rp-grey">Maintenance performance rankings</p>
        </div>
        <a
          href="/hr"
          className="px-3 py-1.5 text-xs font-medium bg-rp-card border border-rp-border rounded-lg text-neutral-300 hover:text-white hover:border-neutral-500 transition-colors"
        >
          Back to HR
        </a>
      </div>

      <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-rp-border text-left">
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider w-16">Rank</th>
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Name</th>
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider text-right">Tasks Completed</th>
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider text-right">Avg Time</th>
                <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider text-right">Quality Score</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-rp-border">
              {staff.map((member, idx) => {
                const rank = idx + 1;
                const badge = RANK_BADGES[rank];
                return (
                  <tr key={member.id} className="hover:bg-white/5 transition-colors">
                    <td className="px-4 py-3">
                      {badge ? (
                        <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-bold border ${badge.color}`}>
                          {badge.label}
                        </span>
                      ) : (
                        <span className="text-sm text-rp-grey font-mono pl-2">#{rank}</span>
                      )}
                    </td>
                    <td className="px-4 py-3 text-sm font-medium text-white">{member.name}</td>
                    <td className="px-4 py-3 text-sm text-neutral-300 text-right">{member.tasks_completed}</td>
                    <td className="px-4 py-3 text-sm text-neutral-300 text-right">{member.avg_time_minutes}m</td>
                    <td className="px-4 py-3 text-right">
                      <span className="text-sm font-medium text-yellow-400">{member.quality_score.toFixed(1)}</span>
                      <span className="text-xs text-rp-grey ml-1">/ 5</span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      <p className="text-xs text-neutral-500 mt-4">
        Showing mock data. Staff performance API not yet connected.
      </p>
    </DashboardLayout>
  );
}
