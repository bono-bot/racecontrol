import { describe, test, expect } from "vitest";
import {
  formatLapTime,
  formatTimer,
  formatSessionTimer,
  formatUptime,
  gameLabel,
  formatTimeIST,
} from "../format";

describe("formatLapTime", () => {
  test("returns placeholder for zero", () => {
    expect(formatLapTime(0)).toBe("--:--.---");
  });

  test("returns placeholder for negative", () => {
    expect(formatLapTime(-100)).toBe("--:--.---");
  });

  test("formats sub-minute time", () => {
    expect(formatLapTime(45_123)).toBe("0:45.123");
  });

  test("formats multi-minute time", () => {
    expect(formatLapTime(93_456)).toBe("1:33.456");
  });

  test("formats exact minute", () => {
    expect(formatLapTime(60_000)).toBe("1:00.000");
  });

  test("pads seconds below 10", () => {
    const result = formatLapTime(65_500);
    expect(result).toBe("1:05.500");
  });
});

describe("formatTimer", () => {
  test("formats zero", () => {
    expect(formatTimer(0)).toBe("0:00");
  });

  test("formats seconds only", () => {
    expect(formatTimer(45)).toBe("0:45");
  });

  test("formats minutes and seconds", () => {
    expect(formatTimer(125)).toBe("2:05");
  });

  test("pads single-digit seconds", () => {
    expect(formatTimer(61)).toBe("1:01");
  });
});

describe("formatSessionTimer", () => {
  test("formats zero", () => {
    expect(formatSessionTimer(0)).toBe("00:00:00");
  });

  test("formats hours minutes seconds", () => {
    expect(formatSessionTimer(3661)).toBe("01:01:01");
  });

  test("formats sub-hour", () => {
    expect(formatSessionTimer(905)).toBe("00:15:05");
  });
});

describe("formatUptime", () => {
  test("returns placeholder for null", () => {
    expect(formatUptime(null)).toBe("--");
  });

  test("returns placeholder for undefined", () => {
    expect(formatUptime(undefined)).toBe("--");
  });

  test("formats hours and minutes", () => {
    expect(formatUptime(7380)).toBe("2h 3m");
  });

  test("formats zero", () => {
    expect(formatUptime(0)).toBe("0h 0m");
  });
});

describe("gameLabel", () => {
  test("maps known sim types", () => {
    expect(gameLabel("assetto_corsa")).toBe("AC");
    expect(gameLabel("f1_25")).toBe("F1");
    expect(gameLabel("iracing")).toBe("iR");
    expect(gameLabel("le_mans_ultimate")).toBe("LMU");
    expect(gameLabel("forza")).toBe("FRZ");
    expect(gameLabel("forza_horizon_5")).toBe("FH5");
    expect(gameLabel("assetto_corsa_evo")).toBe("ACE");
    expect(gameLabel("assetto_corsa_rally")).toBe("ACR");
  });

  test("maps aliases", () => {
    expect(gameLabel("ac")).toBe("AC");
    expect(gameLabel("f1")).toBe("F1");
    expect(gameLabel("lmu")).toBe("LMU");
  });

  test("falls back to uppercase prefix for unknown", () => {
    expect(gameLabel("some_new_game")).toBe("SOM");
  });

  test("handles empty string", () => {
    expect(gameLabel("")).toBe("");
  });
});

describe("formatTimeIST", () => {
  test("formats valid ISO timestamp", () => {
    const result = formatTimeIST("2026-03-24T04:30:00.000Z");
    // UTC 04:30 = IST 10:00
    expect(result).toBe("10:00:00");
  });

  test("returns placeholder for invalid timestamp", () => {
    expect(formatTimeIST("not-a-date")).toBe("--:--:--");
  });
});
