"use client";

import { useBuildIdRefresh } from "@/hooks/useBuildIdRefresh";

export function BuildIdRefresh() {
  useBuildIdRefresh();
  return null;
}
