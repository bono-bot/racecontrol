// flag-sdk.ts — V2 Feature Flag SDK (stub)
//
// HANDOFF.md §4 contract surface. Backend (Postgres + Redis cache + SSE
// for sub-500ms propagation) is V2.1+ work — this stub returns isOn:true
// by default and supports per-flag override via /emergency.json so kill-
// switch behavior is exercisable today.
//
// Usage:
//   const { isOn } = useFlag('pwa.lap_compare');
//   if (!isOn) return null;
//
// Key namespace: <context>.<area>.<thing>
//   e.g. 'pwa.lap_compare'  (route flag)
//        'cockpit.pod.kill' (action flag)
//        'pos.api.refund'   (endpoint flag)
//
// When backend lands: swap the loader without UI churn.
// Mirrored across kiosk/, web/, pwa/ src/lib/. Consolidate to
// packages/shared-flag-sdk/ in a follow-up refactor.

'use client';

import { useEffect, useState } from 'react';

type FlagOverrides = Record<string, boolean>;

let emergencyCache: FlagOverrides | null = null;
let emergencyLoadPromise: Promise<FlagOverrides> | null = null;

async function loadEmergencyOverrides(): Promise<FlagOverrides> {
  if (emergencyCache) return emergencyCache;
  if (emergencyLoadPromise) return emergencyLoadPromise;

  emergencyLoadPromise = (async () => {
    try {
      const res = await fetch('/emergency.json', { cache: 'no-store' });
      if (!res.ok) {
        emergencyCache = {};
        return emergencyCache;
      }
      const data = (await res.json()) as unknown;
      emergencyCache =
        data && typeof data === 'object' && !Array.isArray(data)
          ? (data as FlagOverrides)
          : {};
      return emergencyCache;
    } catch {
      emergencyCache = {};
      return emergencyCache;
    }
  })();

  return emergencyLoadPromise;
}

/**
 * React hook — returns the current state of a feature flag.
 * Default is isOn:true; /emergency.json can flip individual flags off.
 */
export function useFlag(key: string): { isOn: boolean } {
  const [isOn, setIsOn] = useState<boolean>(true);

  useEffect(() => {
    let cancelled = false;
    void loadEmergencyOverrides().then((overrides) => {
      if (cancelled) return;
      if (key in overrides) setIsOn(overrides[key] === true);
    });
    return () => {
      cancelled = true;
    };
  }, [key]);

  return { isOn };
}

/**
 * Synchronous read — returns the cached emergency state without triggering
 * a fetch. Use only in code paths where loadEmergencyOverrides() has already
 * resolved (e.g. after preloadFlags()). Otherwise returns the default isOn:true.
 */
export function getFlagSync(key: string): boolean {
  if (emergencyCache && key in emergencyCache) {
    return emergencyCache[key] === true;
  }
  return true;
}

/**
 * Force-load the emergency overrides. Call once at app boot if you need
 * getFlagSync() to be authoritative before the first render.
 */
export function preloadFlags(): Promise<void> {
  return loadEmergencyOverrides().then(() => undefined);
}
