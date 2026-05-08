/**
 * NotFoundCTA — rendered when CIRS lookup returns status: "not_found".
 *
 * PACT-20260506-001 Phase 1 wire-up Session 4.
 *
 * Provides two paths from the lookup-not-found state:
 *   - Register new customer (route customer to PWA self-register flow;
 *     staff-assisted in V2.0 — actual deeplink target finalizes in Session 5)
 *   - Walk-In Guest fallback (toggles back to Walk-In dropdown view)
 */

import styles from "./NotFoundCTA.module.css";

interface Props {
  searchedPhone: string;
  onUseWalkIn: () => void;
  onRegisterNew: () => void;
}

export default function NotFoundCTA({
  searchedPhone,
  onUseWalkIn,
  onRegisterNew,
}: Props) {
  return (
    <section className={styles.wrapper} role="status" aria-live="polite">
      <h2 className={styles.heading}>No customer found</h2>
      <p className={styles.body}>
        We didn&apos;t find a registered customer for{" "}
        <span className={styles.code}>{searchedPhone}</span>. Either register
        them now (PWA), or use a Walk-In Guest slot for this session.
      </p>
      <div className={styles.actions}>
        <button
          type="button"
          className={styles.btnPrimary}
          onClick={onRegisterNew}
        >
          Register new customer
        </button>
        <button
          type="button"
          className={styles.btnSecondary}
          onClick={onUseWalkIn}
        >
          Use Walk-In Guest
        </button>
      </div>
    </section>
  );
}
