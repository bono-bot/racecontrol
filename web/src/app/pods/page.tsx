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
          <h1 className="text-2xl font-bold text-white">Pods</h1>
          <p className="text-sm text-rp-grey">Manage simulator stations</p>
        </div>
        <span className="text-xs text-rp-grey">{pods.length} pods registered</span>
      </div>

      {loading ? (
        <div className="text-center py-12 text-rp-grey text-sm">Loading pods...</div>
      ) : pods.length === 0 ? (
        <div className="bg-rp-card border border-rp-border rounded-lg p-8 text-center">
          <p className="text-neutral-400 mb-2">No pods registered</p>
          <p className="text-rp-grey text-sm">
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
