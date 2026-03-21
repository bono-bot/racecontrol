"use client";

import { Toaster } from "sonner";

/**
 * RacingPoint-themed sonner Toaster.
 * Dark theme, top-center position, card colors matching rp-card/rp-border.
 */
export default function RpToaster() {
  return (
    <Toaster
      theme="dark"
      position="top-center"
      richColors
      toastOptions={{
        style: {
          background: "#222222",
          border: "1px solid #333333",
          color: "#FFFFFF",
        },
      }}
    />
  );
}
