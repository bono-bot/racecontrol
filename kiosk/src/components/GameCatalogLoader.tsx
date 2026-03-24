"use client";

import { useEffect } from "react";
import { loadGameCatalog } from "@/lib/constants";

/**
 * Invisible component that fetches the game catalog from API on mount.
 * Placed in the root layout so the catalog is available before any page renders.
 * Falls back silently to hardcoded data if API is unreachable.
 */
export function GameCatalogLoader() {
  useEffect(() => {
    loadGameCatalog();
  }, []);
  return null;
}
