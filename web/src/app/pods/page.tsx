"use client";

import { useEffect, useState } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import PodCard from "@/components/PodCard";
import type { Pod } from "@/lib/api";
import { api } from "@/lib/api";

export default function PodsPage() {
  const [pods, setPods] = useState<Pod[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    api.listPods().then((res) => {
      setPods(res.pods || []);
      setLoading(false);
    }).catch(() => setLoading(false));
  }, []);

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold text-zinc-100">Pods</h1>
          <p className="text-sm text-zinc-500">Manage simulator stations</p>
        </div>
        <span className="text-xs text-zinc-500">{pods.length} pods registered</span>
      </div>

      {loading ? (
        <div className="text-center py-12 text-zinc-500 text-sm">Loading pods...</div>
      ) : pods.length === 0 ? (
        <div className="bg-zinc-900 border border-zinc-800 rounded-lg p-8 text-center">
          <p className="text-zinc-400 mb-2">No pods registered</p>
          <p className="text-zinc-500 text-sm">
            Pods appear automatically when rc-agent connects from a sim PC.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
          {pods.map((pod) => (
            <PodCard key={pod.id} pod={pod} />
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
