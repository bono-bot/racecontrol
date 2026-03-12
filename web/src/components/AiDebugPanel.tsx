"use client";

import { useState } from "react";
import type { AiDebugSuggestion } from "@/lib/api";

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

interface AiDebugPanelProps {
  suggestions: AiDebugSuggestion[];
  pods: { id: string; number: number; name: string }[];
}

export default function AiDebugPanel({ suggestions, pods }: AiDebugPanelProps) {
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());

  const visible = suggestions.filter(
    (s) => !dismissed.has(s.pod_id + s.created_at)
  );

  if (visible.length === 0) return null;

  return (
    <div className="mb-4 space-y-2">
      {visible.map((s) => {
        const pod = pods.find((p) => p.id === s.pod_id);
        const podLabel = pod
          ? `Pod ${String(pod.number).padStart(2, "0")}`
          : s.pod_id;
        const key = s.pod_id + s.created_at;

        return (
          <div
            key={key}
            className="bg-violet-500/10 border border-violet-500/30 rounded-lg px-4 py-3"
          >
            <div className="flex items-start justify-between gap-3">
              <div className="flex items-start gap-3 flex-1 min-w-0">
                <span className="text-violet-400 text-lg mt-0.5 shrink-0">
                  &#129302;
                </span>
                <div className="min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <span className="text-sm font-semibold text-violet-200">
                      AI Debug — {podLabel}
                    </span>
                    <span className="text-xs text-rp-grey">
                      {simLabels[s.sim_type] || s.sim_type}
                    </span>
                    <span className="text-xs text-rp-grey">
                      via {s.model}
                    </span>
                  </div>
                  <p className="text-xs text-rp-grey mb-1">
                    Error: {s.error_context}
                  </p>
                  <p className="text-sm text-violet-100 whitespace-pre-wrap">
                    {s.suggestion}
                  </p>
                </div>
              </div>
              <button
                onClick={() =>
                  setDismissed((prev) => new Set(prev).add(key))
                }
                className="text-rp-grey hover:text-neutral-400 transition-colors text-sm shrink-0"
              >
                &times;
              </button>
            </div>
          </div>
        );
      })}
    </div>
  );
}
