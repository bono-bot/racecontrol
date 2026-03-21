"use client";

import { useState, useEffect } from "react";
import type { GameLaunchRequest } from "@/hooks/useKioskSocket";
import { GAME_DISPLAY } from "@/lib/gameDisplayInfo";

interface GameLaunchRequestBannerProps {
  requests: GameLaunchRequest[];
  onConfirm: (request: GameLaunchRequest) => void;
  onDismiss: (requestId: string) => void;
}

function RequestBanner({
  request,
  onConfirm,
  onDismiss,
}: {
  request: GameLaunchRequest;
  onConfirm: (request: GameLaunchRequest) => void;
  onDismiss: (requestId: string) => void;
}) {
  const [expired, setExpired] = useState(false);

  // Show "Request expired" text for 2s after the 60s timeout fires
  // The parent auto-removes after 60s, but we can show the expiry state briefly
  useEffect(() => {
    const expireTimer = setTimeout(() => {
      setExpired(true);
    }, 60 * 1000);
    return () => clearTimeout(expireTimer);
  }, []);

  const gameName = GAME_DISPLAY[request.sim_type]?.name ?? request.sim_type;

  return (
    <div
      className="flex items-center gap-4 px-4 w-full"
      style={{
        minHeight: "48px",
        backgroundColor: "var(--color-rp-surface, #2A2A2A)",
        borderLeft: "2px solid #ca8a04",
        paddingTop: "10px",
        paddingBottom: "10px",
      }}
    >
      {expired ? (
        <p className="flex-1 text-sm" style={{ color: "#ca8a04" }}>
          Request expired
        </p>
      ) : (
        <>
          <p className="flex-1 text-white" style={{ fontSize: "14px" }}>
            <span className="font-semibold">{request.driver_name}</span>{" "}
            wants to play{" "}
            <span className="font-semibold">{gameName}</span>
          </p>
          <button
            onClick={() => onConfirm(request)}
            className="px-4 py-1.5 rounded-md text-white flex-shrink-0 transition-colors"
            style={{
              fontSize: "12px",
              fontWeight: 600,
              backgroundColor: "var(--color-rp-red, #E10600)",
            }}
            onMouseEnter={(e) => {
              (e.currentTarget as HTMLButtonElement).style.backgroundColor =
                "var(--color-rp-red-hover, #FF1A1A)";
            }}
            onMouseLeave={(e) => {
              (e.currentTarget as HTMLButtonElement).style.backgroundColor =
                "var(--color-rp-red, #E10600)";
            }}
          >
            Confirm Launch
          </button>
          <button
            onClick={() => onDismiss(request.request_id)}
            className="text-white flex-shrink-0 hover:opacity-70 transition-opacity"
            style={{ fontSize: "14px" }}
          >
            Dismiss
          </button>
        </>
      )}
    </div>
  );
}

export function GameLaunchRequestBanner({
  requests,
  onConfirm,
  onDismiss,
}: GameLaunchRequestBannerProps) {
  if (requests.length === 0) return null;

  return (
    <div
      className="w-full flex flex-col gap-px"
      style={{ position: "relative", zIndex: 50 }}
    >
      {requests.map((req) => (
        <RequestBanner
          key={req.request_id}
          request={req}
          onConfirm={onConfirm}
          onDismiss={onDismiss}
        />
      ))}
    </div>
  );
}
