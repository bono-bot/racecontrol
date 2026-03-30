"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { fetchApi } from "@/lib/api";

interface MaintenanceTask {
  id: string;
  pod_id: number | null;
  component: string;
  title: string;
  description: string;
  priority: string;
  status: string;
  assigned_to: string | null;
  created_at: string;
  updated_at: string;
}

const STATUS_COLORS: Record<string, string> = {
  Open: "bg-blue-500/20 text-blue-400 border-blue-500/30",
  InProgress: "bg-yellow-500/20 text-yellow-400 border-yellow-500/30",
  Completed: "bg-green-500/20 text-green-400 border-green-500/30",
  Failed: "bg-red-500/20 text-red-400 border-red-500/30",
};

const PRIORITY_COLORS: Record<string, string> = {
  Critical: "text-red-400",
  High: "text-orange-400",
  Medium: "text-yellow-400",
  Low: "text-blue-400",
};

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString("en-IN", { timeZone: "Asia/Kolkata", hour12: false });
  } catch {
    return iso;
  }
}

export default function MaintenanceTasksPage() {
  const [tasks, setTasks] = useState<MaintenanceTask[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchApi<{ tasks: MaintenanceTask[] }>("/maintenance/tasks")
      .then((res) => {
        setTasks(res?.tasks ?? []);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, []);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Maintenance Tasks</h1>
          <p className="text-sm text-rp-grey">Scheduled and pending maintenance work</p>
        </div>
        <span className="text-xs text-rp-grey">{tasks.length} tasks</span>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading tasks...</div>
      ) : error ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-red-400 mb-2">Failed to load tasks</p>
          <p className="text-rp-grey text-sm">{error}</p>
        </div>
      ) : tasks.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No maintenance tasks</p>
          <p className="text-rp-grey text-sm">Tasks appear when maintenance events require manual intervention.</p>
        </div>
      ) : (
        <div className="bg-rp-card border border-rp-border rounded-lg overflow-hidden">
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-rp-border text-left">
                  <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Priority</th>
                  <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Pod</th>
                  <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Component</th>
                  <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Title</th>
                  <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Status</th>
                  <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Assigned</th>
                  <th className="px-4 py-3 text-xs font-medium text-rp-grey uppercase tracking-wider">Created</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-rp-border">
                {tasks.map((task) => (
                  <tr key={task.id} className="hover:bg-white/5 transition-colors">
                    <td className="px-4 py-3">
                      <span className={`text-xs font-medium ${PRIORITY_COLORS[task.priority] ?? "text-neutral-400"}`}>
                        {task.priority}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-xs font-mono text-rp-grey">
                      {task.pod_id != null ? `Pod ${task.pod_id}` : "—"}
                    </td>
                    <td className="px-4 py-3 text-xs text-neutral-400">{task.component}</td>
                    <td className="px-4 py-3 text-sm text-neutral-300">{task.title}</td>
                    <td className="px-4 py-3">
                      <span
                        className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium border ${STATUS_COLORS[task.status] ?? "bg-neutral-500/20 text-neutral-400 border-neutral-500/30"}`}
                      >
                        {task.status}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-xs text-neutral-400">
                      {task.assigned_to ?? "Unassigned"}
                    </td>
                    <td className="px-4 py-3 text-xs text-rp-grey">{formatTimestamp(task.created_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </DashboardLayout>
  );
}
