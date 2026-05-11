"use client";

/**
 * PhoneLookupInput — POS .130 phone-entry component for CIRS lookup.
 *
 * PACT-20260506-001 Phase 1 wire-up Session 4.
 * Implements §AMEND-1.B NF-james-B Indian-mobile-prefix WARN gate
 * (digit[0] ∈ {6,7,8,9} → submit-eligible; otherwise WARN-but-still-submittable).
 *
 * Substrate stays permissive for E.164 / cross-tz international customers.
 * Gate is staff-typed-input-discipline at UI layer; Rust substrate
 * `cirs::canonicalize_phone` does not enforce the prefix rule.
 */

import { useState } from "react";
import { isIndianMobilePrefix } from "@/lib/types/cirs-lookup";
import styles from "./PhoneLookupInput.module.css";

interface Props {
  onSubmit: (phone: string) => void;
  disabled?: boolean;
}

export default function PhoneLookupInput({ onSubmit, disabled = false }: Props) {
  const [phone, setPhone] = useState("");
  const [warnAcknowledged, setWarnAcknowledged] = useState(false);

  const digitsOnly = phone.replace(/\D/g, "");
  const showWarn = digitsOnly.length > 0 && !isIndianMobilePrefix(digitsOnly);
  const minDigitsMet = digitsOnly.length >= 10;
  const submittable = minDigitsMet && (!showWarn || warnAcknowledged) && !disabled;

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const next = e.target.value.replace(/\D/g, "").slice(0, 15);
    setPhone(next);
    if (isIndianMobilePrefix(next) || next.length === 0) {
      setWarnAcknowledged(false);
    }
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!submittable) return;
    onSubmit(digitsOnly);
  };

  return (
    <form className={styles.wrapper} onSubmit={handleSubmit}>
      <label htmlFor="phone-lookup-input" className={styles.label}>
        Phone number
      </label>
      <div className={styles.inputRow}>
        <input
          id="phone-lookup-input"
          className={`${styles.input} ${showWarn ? styles.warn : ""}`}
          type="tel"
          inputMode="numeric"
          autoComplete="off"
          autoFocus
          value={phone}
          onChange={handleChange}
          placeholder="10-digit Indian mobile"
          aria-describedby={showWarn ? "phone-warn" : undefined}
          disabled={disabled}
        />
        <button type="submit" className={styles.submit} disabled={!submittable}>
          {showWarn && !warnAcknowledged ? "Confirm & lookup" : "Lookup"}
        </button>
      </div>
      {showWarn && (
        <p id="phone-warn" className={styles.warnBanner}>
          This doesn&apos;t look like an Indian mobile number (Indian mobiles
          start with 6, 7, 8, or 9).
          {!warnAcknowledged && (
            <>
              {" "}
              <button
                type="button"
                onClick={() => setWarnAcknowledged(true)}
                style={{
                  background: "none",
                  border: "none",
                  color: "var(--rp-warning)",
                  textDecoration: "underline",
                  cursor: "pointer",
                  font: "inherit",
                  padding: 0,
                }}
              >
                Continue anyway
              </button>
            </>
          )}
        </p>
      )}
    </form>
  );
}
