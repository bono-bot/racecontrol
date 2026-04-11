"use client";

import { useEffect, useRef } from "react";

interface InventoryStatusBannerProps {
  id?: string;
  lastCheckIst: string;
  onRetry: () => void;
  retrying: boolean;
}

/**
 * Hard-block banner shown when pod inventory fetch fails.
 * Per UI-SPEC Surface A (Phase 361-02).
 * - role="alert" aria-live="assertive" so screen readers announce immediately
 * - Retry button auto-focuses on mount
 * - Start Session is disabled while this banner is visible (no override)
 */
export function InventoryStatusBanner({
  id = "inventory-status-banner",
  lastCheckIst,
  onRetry,
  retrying,
}: InventoryStatusBannerProps) {
  const retryRef = useRef<HTMLButtonElement>(null);

  // Auto-focus Retry button on mount per accessibility spec
  useEffect(() => {
    retryRef.current?.focus();
  }, []);

  return (
    <div
      id={id}
      role="alert"
      aria-live="assertive"
      className="bg-rp-card border-l-4 border-rp-red rounded p-4 flex flex-col md:flex-row items-start gap-4"
    >
      {/* Warning triangle icon */}
      <svg
        className="w-5 h-5 text-rp-red shrink-0 mt-0.5"
        viewBox="0 0 20 20"
        fill="currentColor"
        aria-hidden="true"
      >
        <path
          fillRule="evenodd"
          d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z"
          clipRule="evenodd"
        />
      </svg>

      {/* Text content */}
      <div className="flex-1 min-w-0">
        <p className="text-lg font-semibold text-white">Pod inventory unreachable</p>
        <p className="text-sm text-rp-grey mt-1">
          We can&apos;t confirm which experiences are installed on this pod. Start Session is disabled
          until we reconnect.
        </p>
        <p className="text-xs text-rp-grey mt-1">Last check: {lastCheckIst}</p>
        <p className="text-xs text-rp-grey">Auto-refreshes every 30 seconds</p>
      </div>

      {/* Retry button */}
      <div className="shrink-0">
        <button
          ref={retryRef}
          onClick={onRetry}
          disabled={retrying}
          className={`bg-rp-red hover:bg-rp-red/90 text-white px-4 py-2 rounded text-sm font-semibold transition-colors focus:outline-none focus:ring-2 focus:ring-rp-red focus:ring-offset-2 focus:ring-offset-rp-card ${
            retrying ? "opacity-50 cursor-wait" : ""
          }`}
          aria-label="Pod inventory unreachable — retry to continue"
        >
          {retrying ? "Retrying..." : "Retry"}
        </button>
      </div>
    </div>
  );
}
