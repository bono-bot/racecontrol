/**
 * LookupErrorBanner — rendered when CIRS lookup returns status: "error"
 * (server error, network failure, etc).
 *
 * PACT-20260506-001 Phase 1 wire-up Session 4.
 *
 * Surfaces the error code + message non-blockingly, with retry + walk-in
 * fallback options so staff can keep the customer interaction moving even
 * if the lookup substrate is degraded.
 */

import styles from "./LookupErrorBanner.module.css";

interface Props {
  code: string;
  message: string;
  onRetry: () => void;
  onUseWalkIn: () => void;
}

export default function LookupErrorBanner({
  code,
  message,
  onRetry,
  onUseWalkIn,
}: Props) {
  return (
    <section className={styles.wrapper} role="alert">
      <h2 className={styles.heading}>
        <span className={styles.icon} aria-hidden="true">
          !
        </span>
        Lookup failed
      </h2>
      <p className={styles.body}>{message}</p>
      <p className={styles.code}>code: {code}</p>
      <div className={styles.actions}>
        <button type="button" className={styles.btn} onClick={onRetry}>
          Retry
        </button>
        <button type="button" className={styles.btn} onClick={onUseWalkIn}>
          Use Walk-In Guest
        </button>
      </div>
    </section>
  );
}
