import { describe, test, expect } from "vitest";
import { GAME_DISPLAY, mergeApiCatalog } from "../gameDisplayInfo";

describe("GAME_DISPLAY", () => {
  test("has all 8 SimType entries", () => {
    const expected = [
      "assetto_corsa", "assetto_corsa_evo", "assetto_corsa_rally",
      "iracing", "f1_25", "le_mans_ultimate", "forza", "forza_horizon_5",
    ];
    for (const id of expected) {
      expect(GAME_DISPLAY[id], `missing: ${id}`).toBeDefined();
    }
  });

  test("every entry has name, logo, abbr", () => {
    for (const [id, entry] of Object.entries(GAME_DISPLAY)) {
      expect(entry.name.length, `${id}.name empty`).toBeGreaterThan(0);
      expect(entry.logo.length, `${id}.logo empty`).toBeGreaterThan(0);
      expect(entry.abbr.length, `${id}.abbr empty`).toBeGreaterThanOrEqual(2);
    }
  });

  test("no duplicate abbreviations", () => {
    const abbrs = Object.values(GAME_DISPLAY).map((e) => e.abbr);
    expect(new Set(abbrs).size).toBe(abbrs.length);
  });
});

describe("mergeApiCatalog", () => {
  test("preserves fallback when API returns same games", () => {
    const apiGames = [
      { id: "assetto_corsa", name: "Assetto Corsa", abbr: "AC" },
    ];
    const merged = mergeApiCatalog(apiGames);
    expect(merged.assetto_corsa.name).toBe("Assetto Corsa");
    expect(merged.assetto_corsa.logo).toBe("/game-logos/assetto-corsa.png");
  });

  test("updates name from API (API is authoritative)", () => {
    const apiGames = [
      { id: "assetto_corsa", name: "Assetto Corsa Ultimate", abbr: "ACU" },
    ];
    const merged = mergeApiCatalog(apiGames);
    expect(merged.assetto_corsa.name).toBe("Assetto Corsa Ultimate");
    expect(merged.assetto_corsa.abbr).toBe("ACU");
  });

  test("adds new game from API not in fallback", () => {
    const apiGames = [
      { id: "gran_turismo", name: "Gran Turismo 8", abbr: "GT8" },
    ];
    const merged = mergeApiCatalog(apiGames);
    expect(merged.gran_turismo).toBeDefined();
    expect(merged.gran_turismo.name).toBe("Gran Turismo 8");
    expect(merged.gran_turismo.abbr).toBe("GT8");
    expect(merged.gran_turismo.logo).toBe("/game-logos/gran-turismo.png");
  });

  test("does not mutate original GAME_DISPLAY", () => {
    const before = { ...GAME_DISPLAY };
    mergeApiCatalog([{ id: "new_game", name: "New", abbr: "NW" }]);
    expect(GAME_DISPLAY).toEqual(before);
  });

  test("handles empty API response", () => {
    const merged = mergeApiCatalog([]);
    expect(Object.keys(merged).length).toBe(Object.keys(GAME_DISPLAY).length);
  });
});
