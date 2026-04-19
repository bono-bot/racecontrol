"use client";

import { useEffect, useRef } from "react";
import { useToast } from "@/components/Toast";

const POLL_INTERVAL_MS = 60_000;
const DRIFT_THRESHOLD = 3;
const IDLE_THRESHOLD_MS = 30_000;

interface HealthResponse {
  git_commit?: string;
  build_id?: string;
  status?: string;
}

function isDisplayRoute(pathname: string): boolean {
  return (
    pathname.startsWith("/billing") ||
    pathname.startsWith("/kiosk") ||
    pathname.startsWith("/presenter") ||
    pathname.startsWith("/leaderboard-display") ||
    pathname.startsWith("/spectator")
  );
}

export function useBuildIdRefresh() {
  const { toast } = useToast();
  const initialCommitRef = useRef<string | null>(null);
  const driftCountRef = useRef(0);
  const toastShownRef = useRef(false);
  const lastActivityRef = useRef<number>(Date.now());

  useEffect(() => {
    if (typeof window === "undefined") return;

    const bumpActivity = () => {
      lastActivityRef.current = Date.now();
    };
    window.addEventListener("mousemove", bumpActivity, { passive: true });
    window.addEventListener("keydown", bumpActivity, { passive: true });
    window.addEventListener("click", bumpActivity, { passive: true });
    window.addEventListener("touchstart", bumpActivity, { passive: true });

    const tick = async () => {
      try {
        const r = await fetch("/api/health", { cache: "no-store" });
        if (!r.ok) return;
        const j = (await r.json()) as HealthResponse;
        const commit = j.git_commit;
        if (!commit || commit === "unknown") return;

        if (initialCommitRef.current === null) {
          initialCommitRef.current = commit;
          return;
        }

        if (commit !== initialCommitRef.current) {
          driftCountRef.current += 1;
          if (driftCountRef.current < DRIFT_THRESHOLD) return;

          const idleFor = Date.now() - lastActivityRef.current;
          const hidden = document.visibilityState === "hidden";
          const autoRoute = isDisplayRoute(window.location.pathname);

          if (hidden || idleFor >= IDLE_THRESHOLD_MS || autoRoute) {
            window.location.reload();
            return;
          }

          if (!toastShownRef.current) {
            toast({
              type: "warning",
              message:
                "New version deployed — this page will reload when idle.",
            });
            toastShownRef.current = true;
          }
        } else {
          driftCountRef.current = 0;
        }
      } catch {
        // Network errors don't count as drift — ignore.
      }
    };

    const id = window.setInterval(tick, POLL_INTERVAL_MS);
    void tick();

    return () => {
      window.clearInterval(id);
      window.removeEventListener("mousemove", bumpActivity);
      window.removeEventListener("keydown", bumpActivity);
      window.removeEventListener("click", bumpActivity);
      window.removeEventListener("touchstart", bumpActivity);
    };
  }, [toast]);
}
