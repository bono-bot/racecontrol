"use client";

import { useEffect } from "react";

/**
 * Detects stale chunk 404s after server rebuild and auto-reloads.
 * When the Next.js app is rebuilt, all _next/static/chunks/ URLs change.
 * If Edge kiosk has old page loaded, client navigation fetches old chunks → 404.
 * This component catches those errors and forces a hard reload.
 */
export function ChunkErrorRecovery() {
  useEffect(() => {
    let reloadAttempted = false;

    const handleError = (event: ErrorEvent) => {
      const msg = event.message || "";
      // Next.js chunk load failures surface as "Loading chunk ... failed"
      // or "ChunkLoadError" in various bundlers
      if (
        !reloadAttempted &&
        (msg.includes("Loading chunk") ||
          msg.includes("ChunkLoadError") ||
          msg.includes("Failed to fetch dynamically imported module"))
      ) {
        reloadAttempted = true;
        console.warn("[RaceControl] Stale chunk detected, reloading...");
        window.location.reload();
      }
    };

    const handleUnhandledRejection = (event: PromiseRejectionEvent) => {
      const reason = String(event.reason || "");
      if (
        !reloadAttempted &&
        (reason.includes("Loading chunk") ||
          reason.includes("ChunkLoadError") ||
          reason.includes("Failed to fetch dynamically imported module"))
      ) {
        reloadAttempted = true;
        console.warn("[RaceControl] Stale chunk detected (promise), reloading...");
        window.location.reload();
      }
    };

    window.addEventListener("error", handleError);
    window.addEventListener("unhandledrejection", handleUnhandledRejection);
    return () => {
      window.removeEventListener("error", handleError);
      window.removeEventListener("unhandledrejection", handleUnhandledRejection);
    };
  }, []);

  return null;
}
