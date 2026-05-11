"use client";

/**
 * WalkInGuestDropdown — Guest 1 / Guest 2 fallback for no-PWA / no-phone walk-in.
 *
 * PACT-20260506-001 Phase 1 wire-up Session 4.
 * DoD §1.2 path B (no-phone customer).
 *
 * Absorbs gap #7 (UI evaluation 2026-05-08 ~05:15 IST): slot occupancy state.
 *   Each slot renders FREE / OCCUPIED clearly so staff don't double-book a
 *   Walk-In Guest already in-session on another pod.
 *
 *   Wave 0 reality: occupancy data source not yet wired (active sessions API
 *   returns Walk-In Guest 1/2 mapping in Session 5+ scope). Component takes
 *   `slotsOccupied` as a prop with a sensible default so the visual surface
 *   ships in Wave 0; data wiring lights up later without a structural change.
 *
 * Selection emits `walk_in_guest_id: 1 | 2` matching the Rust LookupInput
 * variant for record_lookup audit row tagging.
 */

import styles from "./WalkInGuestDropdown.module.css";

interface SlotOccupancy {
  guest1: boolean;
  guest2: boolean;
}

interface Props {
  onSelect: (guestId: 1 | 2) => void;
  slotsOccupied?: SlotOccupancy;
  disabled?: boolean;
}

export default function WalkInGuestDropdown({
  onSelect,
  slotsOccupied = { guest1: false, guest2: false },
  disabled = false,
}: Props) {
  const slot = (id: 1 | 2, occupied: boolean) => (
    <button
      key={id}
      type="button"
      className={styles.slot}
      onClick={() => onSelect(id)}
      disabled={disabled || occupied}
      aria-label={
        occupied
          ? `Walk-In Guest ${id} (currently in session — unavailable)`
          : `Select Walk-In Guest ${id}`
      }
    >
      <span className={styles.slotName}>Walk-In Guest {id}</span>
      <span
        className={`${styles.slotStatus} ${
          occupied ? styles.statusOccupied : styles.statusFree
        }`}
      >
        <span className={styles.statusDot} aria-hidden="true" />
        {occupied ? "In session" : "Free"}
      </span>
    </button>
  );

  return (
    <div className={styles.wrapper}>
      <p className={styles.label}>Walk-In Guest fallback</p>
      <p className={styles.subLabel}>
        For customers without a PWA-registered phone. Discount-ineligible by
        default.
      </p>
      <div className={styles.slots}>
        {slot(1, slotsOccupied.guest1)}
        {slot(2, slotsOccupied.guest2)}
      </div>
      {(slotsOccupied.guest1 && slotsOccupied.guest2) && (
        <p className={styles.note}>
          Both Walk-In slots are in active sessions. Wait for one to end, or
          register the customer via PWA + phone capture.
        </p>
      )}
    </div>
  );
}
