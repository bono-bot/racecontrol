"use client";

import type { TelemetryFrame } from "@/lib/types";

// ─── F1 2020 Speedometer ─────────────────────────────────────────────────────
// Layered PNG texture-swap gauge inspired by the F1 2020 steering wheel HUD.
// All images live in /f1hud/ (public folder).

const BASE = "/f1hud";

// Original AC app is 345×345 — we scale to fit the kiosk.
const NATIVE = 345;

function gearFile(gear: number): string {
  if (gear === -1) return `${BASE}/gears/gear_r.png`;
  if (gear === 0) return `${BASE}/gears/gear_n.png`;
  return `${BASE}/gears/gear_${Math.min(gear, 9)}.png`;
}

function speedFile(speedKmh: number): string {
  const v = Math.max(0, Math.min(399, Math.round(speedKmh)));
  return `${BASE}/speed_steps/speed_${String(v).padStart(3, "0")}.png`;
}

function throttleFile(throttle: number): string {
  const v = Math.max(0, Math.min(59, Math.round(throttle * 59)));
  return `${BASE}/throttle/throttle_${String(v).padStart(2, "0")}.png`;
}

function brakeFile(brake: number): string {
  return brake > 0 ? `${BASE}/brake/brake_on.png` : `${BASE}/brake/brake_off.png`;
}

function rpmDigitFile(digit: number): string {
  return `${BASE}/rpm_digits/rpm_digits_${digit}.png`;
}

interface F1SpeedometerProps {
  telemetry: TelemetryFrame;
  /** Rendered size in CSS px. Defaults to 345. */
  size?: number;
}

export function F1Speedometer({ telemetry, size = 345 }: F1SpeedometerProps) {
  const scale = size / NATIVE;

  // Pre-render RPM as 5-digit string
  const rpmStr = String(Math.max(0, Math.round(telemetry.rpm))).padStart(5, "0");

  return (
    <div
      className="relative select-none"
      style={{ width: size, height: size }}
    >
      {/* Layer 1: Background gauge */}
      <img
        src={`${BASE}/background/background.png`}
        alt=""
        className="absolute inset-0 w-full h-full"
        draggable={false}
      />

      {/* Layer 2: Brake arc (right side, red) */}
      <img
        src={brakeFile(telemetry.brake)}
        alt=""
        className="absolute inset-0 w-full h-full"
        draggable={false}
      />

      {/* Layer 3: Throttle arc (left side, green) */}
      <img
        src={throttleFile(telemetry.throttle)}
        alt=""
        className="absolute inset-0 w-full h-full"
        draggable={false}
      />

      {/* Layer 4: Speed arc (outer ring, blue) */}
      <img
        src={speedFile(telemetry.speed_kmh)}
        alt=""
        className="absolute inset-0 w-full h-full"
        draggable={false}
      />

      {/* Layer 5: DRS — hidden for AC (no DRS data) */}

      {/* Layer 6: Labels overlay (speed markings) */}
      <img
        src={`${BASE}/background/labels.png`}
        alt=""
        className="absolute inset-0 w-full h-full"
        draggable={false}
      />

      {/* Layer 7: Gear indicator */}
      <img
        src={gearFile(telemetry.gear)}
        alt=""
        className="absolute"
        style={{
          width: 23 * scale,
          height: 22 * scale,
          left: 188 * scale,
          top: 305 * scale,
        }}
        draggable={false}
      />

      {/* Layer 8: RPM digits (5 digits) */}
      {Array.from({ length: 5 }).map((_, i) => (
        <img
          key={i}
          src={rpmDigitFile(Number(rpmStr[i]))}
          alt=""
          className="absolute"
          style={{
            width: 25 * scale,
            height: 25 * scale,
            left: (110 + i * 25) * scale,
            top: 186 * scale,
          }}
          draggable={false}
        />
      ))}
    </div>
  );
}
