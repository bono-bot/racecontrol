"use client";

import { useState, useEffect } from "react";
import DashboardLayout from "@/components/DashboardLayout";
import { api, type AiSuggestion } from "@/lib/api";

export default function AiInsightsPage() {
  const [suggestions, setSuggestions] = useState<AiSuggestion[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<"all" | "active" | "dismissed">("all");

  useEffect(() => {
    loadSuggestions();
  }, []);

  async function loadSuggestions() {
    setLoading(true);
    try {
      const data = await api.aiSuggestions({ limit: 100 });
      setSuggestions(data.suggestions || []);
    } catch {
      // API unavailable
    }
    setLoading(false);
  }

  async function handleDismiss(id: string) {
    await api.dismissAiSuggestion(id);
    setSuggestions((prev) =>
      prev.map((s) => (s.id === id ? { ...s, dismissed: true } : s))
    );
  }

  const filtered = suggestions.filter((s) => {
    if (filter === "active") return !s.dismissed;
    if (filter === "dismissed") return s.dismissed;
    return true;
  });

  return (
    <DashboardLayout>
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold">AI Insights</h1>
          <p className="text-sm text-rp-grey">Crash analysis & pattern alerts</p>
        </div>
        <div className="flex gap-2">
          {(["all", "active", "dismissed"] as const).map((f) => (
            <button
              key={f}
              onClick={() => setFilter(f)}
              className={`px-3 py-1.5 rounded-lg text-xs font-medium transition-colors ${
                filter === f
                  ? "bg-violet-600/20 text-violet-300 border border-violet-500/30"
                  : "bg-rp-card text-rp-grey border border-rp-border hover:text-white"
              }`}
            >
              {f.charAt(0).toUpperCase() + f.slice(1)}
            </button>
          ))}
          <button
            onClick={loadSuggestions}
            className="px-3 py-1.5 rounded-lg text-xs font-medium bg-rp-card text-rp-grey border border-rp-border hover:text-white transition-colors"
          >
            Refresh
          </button>
        </div>
      </div>

      {loading ? (
        <div className="text-center text-rp-grey py-16">
          <p className="animate-pulse">Loading AI insights...</p>
        </div>
      ) : filtered.length === 0 ? (
        <div className="text-center text-rp-grey py-16">
          <p className="text-sm">No AI insights yet.</p>
          <p className="text-xs mt-1">Suggestions will appear here when crashes are analyzed.</p>
        </div>
      ) : (
        <div className="space-y-3">
          {filtered.map((s) => (
            <div
              key={s.id}
              className={`rounded-lg border px-4 py-3 ${
                s.dismissed
                  ? "bg-rp-card/50 border-rp-border/50 opacity-60"
                  : "bg-violet-500/10 border-violet-500/30"
              }`}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <span className="text-xs font-semibold text-violet-300">
                      Pod {s.pod_id}
                    </span>
                    <span className="text-xs text-rp-grey">{s.sim_type}</span>
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-rp-card text-rp-grey border border-rp-border">
                      {s.source}
                    </span>
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-rp-card text-rp-grey border border-rp-border">
                      {s.model}
                    </span>
                  </div>
                  {s.error_context && (
                    <p className="text-xs text-rp-grey mb-1 truncate">{s.error_context}</p>
                  )}
                  <p className="text-sm text-neutral-200 whitespace-pre-wrap">{s.suggestion}</p>
                  <p className="text-[10px] text-rp-grey mt-1">{s.created_at}</p>
                </div>
                {!s.dismissed && (
                  <button
                    onClick={() => handleDismiss(s.id)}
                    className="text-rp-grey hover:text-white text-lg shrink-0"
                  >
                    &times;
                  </button>
                )}
              </div>
            </div>
          ))}
        </div>
      )}
    </DashboardLayout>
  );
}
