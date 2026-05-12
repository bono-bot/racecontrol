"use client";

import { useState, type FormEvent } from "react";
import styles from "../app/page.module.css";

/*
 * Q-CUST-4 + Q-CUST-7 WhatsApp opt-in form with inline DPDP consent.
 *
 * Source: UI-SPEC v0.2 §9 Q-CUST-4 (a)(b)(c stub) + Q-CUST-7 (a)(b)(c) — CAPTAIN-RATIFIED 2026-05-12.
 *
 * Behaviour:
 *   - Phone input (E.164-compatible; +91 default for IN venue)
 *   - DPDP consent checkbox, unchecked by default, submit disabled until checked
 *   - On submit: POST { phone, consent_text, consent_ts, source: "v2-landing" } to /api/v2/marketing/whatsapp-optin
 *   - 4 visible states: idle · submitting · success · error
 *
 * Wording is the draft per Q-CUST-7 (legal-AMPLIFIER queued for finalization, non-blocking).
 */

const CONSENT_TEXT =
  "I agree to receive WhatsApp messages from RacingPoint about my sessions, offers, and events.";

type SubmitState = "idle" | "submitting" | "success" | "error";

export function WhatsAppOptInForm() {
  const [phone, setPhone] = useState("");
  const [consent, setConsent] = useState(false);
  const [state, setState] = useState<SubmitState>("idle");
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // E.164-compatible: optional `+`, 10-15 digits. Catches sub-10-digit strings
  // that would fail at WhatsApp Business API level (AMPLIFIER NIT-2 close).
  const PHONE_REGEX = /^\+?\d{10,15}$/;
  const canSubmit =
    consent && PHONE_REGEX.test(phone.trim()) && state !== "submitting";

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSubmit) return;
    setState("submitting");
    setErrorMsg(null);
    try {
      const res = await fetch("/v2/api/v2/marketing/whatsapp-optin", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          phone: phone.trim(),
          consent_text: CONSENT_TEXT,
          consent_ts: new Date().toISOString(),
          source: "v2-landing",
        }),
      });
      if (!res.ok) {
        const body = (await res.json().catch(() => null)) as
          | { error?: string }
          | null;
        setErrorMsg(body?.error ?? "submission_failed");
        setState("error");
        return;
      }
      setState("success");
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : "network_error");
      setState("error");
    }
  }

  if (state === "success") {
    return (
      <div className={styles.optInSuccess} role="status">
        <strong>Thanks — you are on the list.</strong>
        <p>
          We will message your WhatsApp shortly to confirm. Reply STOP anytime
          to opt out.
        </p>
      </div>
    );
  }

  return (
    <form className={styles.optInForm} onSubmit={handleSubmit} noValidate>
      <label className={styles.optInField}>
        <span>WhatsApp number</span>
        <input
          type="tel"
          name="phone"
          inputMode="tel"
          autoComplete="tel"
          placeholder="+91 98xxxxxxxx"
          value={phone}
          onChange={(e) => setPhone(e.target.value)}
          required
          aria-describedby="optin-consent"
        />
      </label>

      <label className={styles.optInConsent} id="optin-consent">
        <input
          type="checkbox"
          name="consent"
          checked={consent}
          onChange={(e) => setConsent(e.target.checked)}
          required
        />
        <span>{CONSENT_TEXT}</span>
      </label>

      <button
        type="submit"
        className={styles.optInSubmit}
        disabled={!canSubmit}
        aria-disabled={!canSubmit}
      >
        {state === "submitting" ? "Sending…" : "Get racing updates"}
      </button>

      {state === "error" && errorMsg && (
        <p className={styles.optInError} role="alert">
          {errorMsg === "submission_failed"
            ? "Something went wrong. Please try again or message us on WhatsApp directly."
            : "We could not reach our server. Please try again in a moment."}
        </p>
      )}

      <p className={styles.optInFinePrint}>
        See our{" "}
        <a href="/v2/privacy">Privacy Policy</a> for how we handle your
        number and how to opt out.
      </p>
    </form>
  );
}
