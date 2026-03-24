import { describe, test, expect } from 'vitest';
import type { GameCatalogEntry, SimType } from '@racingpoint/types';
import catalogFixture from './fixtures/games-catalog.json';

const VALID_SIM_TYPES: SimType[] = [
  "assetto_corsa", "assetto_corsa_evo", "assetto_corsa_rally",
  "iracing", "le_mans_ultimate", "f1_25", "forza", "forza_horizon_5",
];

function assertGameCatalogEntry(data: unknown): asserts data is GameCatalogEntry {
  const d = data as Record<string, unknown>;
  expect(typeof d.id, 'id must be string').toBe('string');
  expect(VALID_SIM_TYPES).toContain(d.id);
  expect(typeof d.name, 'name must be string').toBe('string');
  expect((d.name as string).length, 'name must not be empty').toBeGreaterThan(0);
  expect(typeof d.abbr, 'abbr must be string').toBe('string');
  expect((d.abbr as string).length, 'abbr must be 2-3 chars').toBeGreaterThanOrEqual(2);
  expect(typeof d.installed_pod_count, 'installed_pod_count must be number').toBe('number');
  expect(d.installed_pod_count as number, 'installed_pod_count must be >= 0').toBeGreaterThanOrEqual(0);
}

describe('GET /api/v1/games/catalog — GameCatalogEntry contract', () => {
  test('fixture has games array', () => {
    expect(Array.isArray(catalogFixture.games)).toBe(true);
    expect(catalogFixture.games.length).toBeGreaterThan(0);
  });

  test('catalog contains all 8 SimType variants', () => {
    const ids = catalogFixture.games.map((g: { id: string }) => g.id);
    for (const sim of VALID_SIM_TYPES) {
      expect(ids, `missing SimType: ${sim}`).toContain(sim);
    }
  });

  test('each game matches GameCatalogEntry contract', () => {
    catalogFixture.games.forEach((game: unknown, i: number) => {
      try {
        assertGameCatalogEntry(game);
      } catch (e) {
        throw new Error(`Game at index ${i} failed contract: ${String(e)}`);
      }
    });
  });

  test('no duplicate game IDs', () => {
    const ids = catalogFixture.games.map((g: { id: string }) => g.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});
