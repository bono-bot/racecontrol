"use client";

/**
 * ManagerPill — render-condition UI primitive for privileged staff actions.
 *
 * PACT-20260506-001 §AMEND-1.E + Phase 1 wire-up Session 5.
 *
 * Render contract (Q5-A enforcement):
 *   if (PRIVILEGED_ACTIONS.includes(action)) → render ManagerPill +
 *     parent must invoke PIN re-entry flow before submitting the action.
 *
 * This component is INTENTIONALLY render-only (kaizen-discipline: smallest
 * invariant for observed requirement). The PIN modal + verification API
 * call live in a parent flow (sequenced post-idle-timeout middleware).
 *
 * Composes-with:
 *   - lib/types/privileged-action (Rust enum mirror)
 *   - Captain customer-satisfaction-first / minimal-compromise: pill is
 *     unobtrusive when staff is already PIN-fresh; only prompts when needed.
 */

import {
  type PrivilegedAction,
  ACTION_LABEL,
  CATEGORY_LABEL,
  categoryOf,
} from "@/lib/types/privileged-action";
import styles from "./ManagerPill.module.css";

interface Props {
  action: PrivilegedAction;
  /**
   * `true` when staff has a fresh PIN cookie (verified within idle-timeout
   * window). Pill renders as a passive badge in this state — no click action.
   */
  pinFresh?: boolean;
  /**
   * Invoked when staff clicks the pill while `pinFresh` is `false`. Parent
   * is expected to open the PIN modal and re-call the privileged handler
   * after verification succeeds.
   */
  onPinRequired?: (action: PrivilegedAction) => void;
  /**
   * Disabled state — visually muted; click is a no-op. Used while the parent
   * flow is mid-submit or while a refresh is pending.
   */
  disabled?: boolean;
}

export default function ManagerPill({
  action,
  pinFresh = false,
  onPinRequired,
  disabled = false,
}: Props) {
  const category = categoryOf(action);
  const label = ACTION_LABEL[action];
  const categoryLabel = CATEGORY_LABEL[category];

  const interactive = !pinFresh && !disabled && Boolean(onPinRequired);

  const handleClick = () => {
    if (!interactive) return;
    onPinRequired?.(action);
  };

  const stateClass = pinFresh
    ? styles.fresh
    : disabled
      ? styles.disabled
      : styles.needsPin;

  return (
    <button
      type="button"
      className={`${styles.pill} ${stateClass}`}
      onClick={handleClick}
      disabled={!interactive}
      aria-label={
        pinFresh
          ? `${label} (manager PIN verified)`
          : `${label} — manager PIN required`
      }
      data-action={action}
      data-category={category}
    >
      <span className={styles.category}>{categoryLabel}</span>
      <span className={styles.label}>{label}</span>
      <span className={styles.indicator} aria-hidden="true">
        {pinFresh ? "✓" : "PIN"}
      </span>
    </button>
  );
}
