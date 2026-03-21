"use client";

import { useEffect } from "react";
import confetti from "canvas-confetti";

/**
 * Fire RacingPoint-branded confetti burst.
 * Colors: Racing Red (#E10600), Gold (#FFD700), White (#FFFFFF).
 * Three-burst pattern: center spread, then left and right side bursts at 250ms delay.
 */
export function fireConfetti() {
  confetti({
    particleCount: 100,
    spread: 70,
    origin: { y: 0.6 },
    colors: ["#E10600", "#FFD700", "#FFFFFF"],
  });
  setTimeout(() => {
    confetti({
      particleCount: 50,
      angle: 60,
      spread: 55,
      origin: { x: 0 },
      colors: ["#E10600", "#FFD700"],
    });
    confetti({
      particleCount: 50,
      angle: 120,
      spread: 55,
      origin: { x: 1 },
      colors: ["#E10600", "#FFD700"],
    });
  }, 250);
}

/**
 * Component that fires confetti on mount when enabled.
 * Renders nothing (returns null).
 * Uses sessionStorage gate to prevent re-trigger on page revisits.
 */
export function ConfettiOnMount({
  enabled,
  sessionId,
}: {
  enabled: boolean;
  sessionId: string;
}) {
  useEffect(() => {
    if (!enabled) return;
    const key = `confetti_shown_${sessionId}`;
    if (typeof window !== "undefined" && sessionStorage.getItem(key)) return;
    const timer = setTimeout(() => {
      fireConfetti();
      if (typeof window !== "undefined") {
        sessionStorage.setItem(key, "1");
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [enabled, sessionId]);
  return null;
}
