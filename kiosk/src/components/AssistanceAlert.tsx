"use client";

interface AssistanceRequest {
  pod_id: string;
  driver_name: string;
  game: string;
  reason: string;
  timestamp: number;
}

interface AssistanceAlertProps {
  requests: AssistanceRequest[];
  onAcknowledge: (podId: string) => void;
}

export type { AssistanceRequest };

export function AssistanceAlert({ requests, onAcknowledge }: AssistanceAlertProps) {
  if (requests.length === 0) return null;

  return (
    <div className="bg-red-900/80 border-b border-red-500/50 px-4 py-2">
      <div className="max-w-screen-2xl mx-auto flex items-center gap-4 overflow-x-auto">
        <div className="flex items-center gap-2 shrink-0">
          <div className="w-3 h-3 bg-red-500 rounded-full animate-pulse" />
          <span className="text-red-200 font-semibold text-sm uppercase tracking-wider">
            Assistance Needed
          </span>
        </div>
        {requests.map((req) => (
          <div
            key={`${req.pod_id}-${req.timestamp}`}
            className="flex items-center gap-3 bg-red-950/60 border border-red-500/30 rounded-lg px-3 py-1.5 shrink-0"
          >
            <div>
              <span className="text-white font-semibold text-sm">
                Pod {req.pod_id.replace("pod_", "#")}
              </span>
              <span className="text-red-300 text-xs ml-2">
                {req.driver_name} — {formatGameName(req.game)}
              </span>
            </div>
            <button
              onClick={() => onAcknowledge(req.pod_id)}
              className="px-3 py-1 bg-red-600 hover:bg-red-500 text-white text-xs font-semibold rounded transition-colors"
            >
              Acknowledge
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}

function formatGameName(game: string): string {
  const names: Record<string, string> = {
    f1_25: "F1 25",
    f1: "F1 25",
    assetto_corsa: "Assetto Corsa",
    iracing: "iRacing",
    le_mans_ultimate: "LMU",
    forza: "Forza",
  };
  return names[game] || game;
}
