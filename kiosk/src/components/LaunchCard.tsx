"use client";

import { useState } from "react";
import type { LaunchStatusCard, LaunchNoteEvent, LaunchState } from "@/lib/types";

interface LaunchCardProps {
  card: LaunchStatusCard;
  notes: LaunchNoteEvent[];
  onAddNote: (body: string) => void;
  onApproveFix: () => void;
  onDismiss: () => void;
}

const STATE_ORDER: LaunchState[] = [
  "launch_started",
  "ai_analysis_requested",
  "issue_being_fixed",
  "issue_fixed",
];

const STATE_LABELS: Record<LaunchState, string> = {
  launch_started: "Launch started",
  ai_analysis_requested: "Analyzing",
  issue_being_fixed: "Fixing",
  issue_fixed: "Playable",
  needs_manual_intervention: "Needs help",
};

function stateColor(s: LaunchState): string {
  switch (s) {
    case "launch_started":             return "text-rp-grey";
    case "ai_analysis_requested":      return "text-yellow-400";
    case "issue_being_fixed":          return "text-blue-400";
    case "issue_fixed":                return "text-green-400";
    case "needs_manual_intervention":  return "text-red-500";
  }
}

function stateDotColor(s: LaunchState): string {
  switch (s) {
    case "launch_started":             return "bg-zinc-400";
    case "ai_analysis_requested":      return "bg-yellow-400";
    case "issue_being_fixed":          return "bg-blue-400";
    case "issue_fixed":                return "bg-green-400";
    case "needs_manual_intervention":  return "bg-red-500";
  }
}

export default function LaunchCard({ card, notes, onAddNote, onApproveFix, onDismiss }: LaunchCardProps) {
  const [noteBody, setNoteBody] = useState("");
  const isTerminal = card.state === "issue_fixed" || card.state === "needs_manual_intervention";
  const showApprove =
    card.state === "issue_being_fixed" && card.ai_tier !== null && card.ai_tier >= 2;

  const currentStateIndex = STATE_ORDER.indexOf(card.state);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const trimmed = noteBody.trim();
    if (trimmed.length === 0) return;
    onAddNote(trimmed);
    setNoteBody("");
  };

  return (
    <div
      className="rp-card rp-border rounded p-4 mb-3"
      data-testid="launch-card"
      data-launch-id={card.launch_id}
      data-state={card.state}
    >
      {/* Pod badge + state label */}
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2">
          <span className="font-mono text-sm text-white">
            Pod {card.pod_id.replace("pod_", "")}
          </span>
          <span className="text-xs text-rp-grey">{card.sim_type.toUpperCase()}</span>
          <span className="text-xs text-rp-grey font-mono">{card.origin}</span>
        </div>
        <span
          className={`text-xs font-semibold ${stateColor(card.state)}`}
          data-testid="state-label"
        >
          {STATE_LABELS[card.state]}
        </span>
      </div>

      {/* 4-dot state timeline (does not include needs_manual_intervention) */}
      <div className="flex items-center gap-1 mb-2" data-testid="state-timeline">
        {STATE_ORDER.map((s, i) => (
          <div
            key={s}
            className={`h-2 w-2 rounded-full ${
              card.state === "needs_manual_intervention"
                ? "bg-red-500 opacity-40"
                : currentStateIndex >= i
                ? stateDotColor(card.state)
                : "bg-zinc-600 opacity-30"
            }`}
            data-testid={`timeline-dot-${s}`}
          />
        ))}
        {card.state === "needs_manual_intervention" && (
          <span className="text-red-500 text-xs ml-1">!</span>
        )}
      </div>

      {/* Detail text (D-15 — already sanitized server-side; render as-is) */}
      {card.detail && (
        <p className="text-sm text-rp-grey mb-2" data-testid="launch-detail">
          {card.detail}
        </p>
      )}

      {/* Fix action hint */}
      {card.fix_action && (
        <p className="text-xs text-blue-300 mb-2" data-testid="fix-action">
          Fix: {card.fix_action}
        </p>
      )}

      {/* Inline notes thread */}
      {notes.length > 0 && (
        <div className="mt-2 border-t border-rp-border pt-2" data-testid="notes-thread">
          {notes.map((n) => (
            <div key={n.note_id} className="text-xs mb-1">
              <span className="font-semibold text-zinc-300">{n.staff_name}:</span>{" "}
              <span className="text-zinc-400">{n.body}</span>
            </div>
          ))}
        </div>
      )}

      {/* Note composer */}
      <form onSubmit={handleSubmit} className="mt-2 flex gap-1" data-testid="note-composer">
        <input
          type="text"
          value={noteBody}
          onChange={(e) => setNoteBody(e.target.value)}
          placeholder="Add a note..."
          className="flex-1 text-xs bg-rp-black border border-rp-border rounded px-2 py-1 text-white placeholder-zinc-600 focus:outline-none focus:border-zinc-500"
          maxLength={2000}
          data-testid="note-input"
        />
        <button
          type="submit"
          className="text-xs bg-rp-border px-2 py-1 rounded text-zinc-300 hover:text-white"
          data-testid="note-submit"
        >
          Post
        </button>
      </form>

      {/* Action buttons */}
      <div className="mt-2 flex gap-2">
        {showApprove && (
          <button
            onClick={onApproveFix}
            className="text-xs bg-blue-600 hover:bg-blue-700 px-3 py-1 rounded text-white"
            data-testid="approve-fix-button"
          >
            Apply Tier {card.ai_tier} fix
          </button>
        )}
        {isTerminal && (
          <button
            onClick={onDismiss}
            className="text-xs bg-rp-border hover:bg-zinc-600 px-3 py-1 rounded text-zinc-300"
            data-testid="dismiss-button"
          >
            Dismiss
          </button>
        )}
      </div>
    </div>
  );
}
