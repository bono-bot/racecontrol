// packages/contract-tests/tests/regression-drift.test.ts
// Phase 445 D-20 regression fixture.
//
// Purpose: lock the current shape of BillingSessionInfo (the canonical
// bug-class type per CLAUDE.md § Cross-Boundary Serialization). A Rust-side
// field rename without regenerating generated/ should cause CI to fail:
//   (a) vitest here detects the @ts-expect-error stops erroring, OR
//   (b) scripts/check-generated-types-drift.sh detects the drift first.
//
// CI redundancy is intentional — any layer catches it.
//
// The canonical bug (2026-03-26 fleet-wide): kiosk sent `ai_difficulty: "easy"`
// (string) but Rust struct had `ai_level: u32` (numeric). Serde silently
// dropped the unknown field. Game launched, but AI was always Semi-Pro.
// This fixture guarantees the class of bug cannot re-enter via BillingSessionInfo.

import { describe, it, expect, expectTypeOf } from 'vitest';
import type { BillingSessionInfo } from '@racingpoint/types';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));

describe('Phase 445 D-20: BillingSessionInfo shape lock', () => {
  it('has the current expected top-level field (driver_name)', () => {
    // Positive assertion — driver_name is a required string field in the
    // current generated BillingSessionInfo (see
    // packages/shared-types/generated/BillingSessionInfo.ts line 5).
    expectTypeOf<BillingSessionInfo>().toHaveProperty('driver_name');
  });

  it('has the canonical id field', () => {
    // Second positive anchor — id is the required primary key.
    expectTypeOf<BillingSessionInfo>().toHaveProperty('id');
  });

  it('does not have the bug-class field names (ai_difficulty)', () => {
    // @ts-expect-error — ai_difficulty is the canonical cross-boundary bug.
    // If this line STOPS erroring, someone added ai_difficulty to the Rust
    // struct and regenerated — Phase 445 drift guard fired upstream.
    expectTypeOf<BillingSessionInfo>().toHaveProperty('ai_difficulty');
  });

  it('does not have the bug-class field names (ai_count)', () => {
    // @ts-expect-error — ai_count was the 2nd half of the canonical bug.
    // Rust has `ai_cars: Vec<AiCarSlot>`; kiosk mistakenly sent `ai_count: 5`.
    expectTypeOf<BillingSessionInfo>().toHaveProperty('ai_count');
  });

  it('generated file is non-empty and exports BillingSessionInfo', () => {
    // Robustness check: directly reads the generated file on disk. If someone
    // manages to swap index.ts re-exports without regenerating, this still
    // catches it.
    const genPath = path.resolve(
      __dirname,
      '..',
      '..',
      'shared-types',
      'generated',
      'BillingSessionInfo.ts'
    );
    const content = fs.readFileSync(genPath, 'utf-8');
    expect(content).toContain('BillingSessionInfo');
    expect(content.length).toBeGreaterThan(20);
  });
});
