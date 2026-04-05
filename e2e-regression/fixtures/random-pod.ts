// ═══════════════════════════════════════════════════════════════
// Random Pod Selection — pick random idle pods with game installed
// ═══════════════════════════════════════════════════════════════

import { RCApiClient, FleetPod, PodInventory } from './api-client';
import { POD_IPS, GameId } from './test-data';

// Cache: pod_number → installed games
let podInventoryCache: Map<number, string[]> | null = null;

export async function scanPodInventory(api: RCApiClient): Promise<Map<number, string[]>> {
  if (podInventoryCache) return podInventoryCache;

  const fleet = await api.fleetHealth();
  const inventory = new Map<number, string[]>();

  for (const pod of fleet) {
    if (!pod.ws_connected) continue;
    try {
      const inv = await api.podInventory(`pod_${pod.pod_number}`);
      inventory.set(pod.pod_number, inv.installed_games || []);
    } catch {
      // Pod unreachable — skip
      inventory.set(pod.pod_number, []);
    }
  }

  podInventoryCache = inventory;
  return inventory;
}

// Get a random idle pod that has the specified game installed
export async function getRandomIdlePod(
  api: RCApiClient,
  game: GameId,
): Promise<{ podNumber: number; podId: string; podIp: string } | null> {
  const fleet = await api.fleetHealth();
  const inventory = await scanPodInventory(api);

  // Filter: ws_connected, no active billing session, game installed, sim pods only (1-8)
  const candidates = fleet.filter(pod => {
    if (pod.pod_number < 1 || pod.pod_number > 8) return false; // Skip POS (pod 9)
    if (!pod.ws_connected) return false;
    if (pod.billing_session_id) return false;
    const games = inventory.get(pod.pod_number) || [];
    // If inventory is empty (couldn't scan), allow all games (optimistic)
    if (games.length === 0) return true;
    return games.includes(game);
  });

  if (candidates.length === 0) return null;

  // Shuffle and pick random
  const shuffled = candidates.sort(() => Math.random() - 0.5);
  const picked = shuffled[0];

  return {
    podNumber: picked.pod_number,
    podId: `pod_${picked.pod_number}`,
    podIp: POD_IPS[picked.pod_number] || '',
  };
}

// Get any random idle pod (for non-game-specific tests)
export async function getAnyIdlePod(
  api: RCApiClient,
): Promise<{ podNumber: number; podId: string; podIp: string } | null> {
  const fleet = await api.fleetHealth();

  const idle = fleet.filter(p => p.pod_number >= 1 && p.pod_number <= 8 && p.ws_connected && !p.billing_session_id);
  if (idle.length === 0) return null;

  const shuffled = idle.sort(() => Math.random() - 0.5);
  const picked = shuffled[0];

  return {
    podNumber: picked.pod_number,
    podId: `pod_${picked.pod_number}`,
    podIp: POD_IPS[picked.pod_number] || '',
  };
}

// Clear cache (call between test suites if needed)
export function clearPodInventoryCache(): void {
  podInventoryCache = null;
}
