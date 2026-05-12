import type { Metadata } from "next";

export const metadata: Metadata = {
  title: "Privacy Policy — RacingPoint",
  description:
    "How RacingPoint handles your data, your WhatsApp consent, and how to opt out.",
};

/*
 * Q-CUST-7 (c stub) — Privacy Policy stub.
 *
 * V0 stub acceptable per CAPTAIN-RATIFIED 2026-05-12: full implementation
 * tracked as launch-gate Verify-by. Legal-AMPLIFIER queued for wording
 * finalization (in-flight ledger entry
 * `legal-amplifier-queue-qcust4-qcust7-wording-20260512-1110-IST`).
 *
 * Until legal-AMPLIFIER returns, this page satisfies DPDP baseline disclosure
 * for browse-without-opt-in visitors and the inline opt-in form footer link.
 */

export default function PrivacyPolicyPage() {
  return (
    <>
      <a href="#main" className="rp-skip-link">
        Skip to content
      </a>
      <main id="main">
      <h1>Privacy Policy</h1>
      <p>
        <strong>Last updated:</strong> 12 May 2026 — initial v0 publication.
      </p>

      <h2>What we collect</h2>
      <p>
        When you opt in to RacingPoint WhatsApp updates on our landing page,
        we collect:
      </p>
      <ul>
        <li>Your WhatsApp phone number</li>
        <li>The exact consent text you ticked</li>
        <li>The date and time you opted in</li>
        <li>The page you opted in from</li>
      </ul>

      <h2>How we use it</h2>
      <p>
        We use your number only to message you about RacingPoint sessions,
        offers, and events. We do not sell your number, we do not share it
        with third parties for marketing, and we do not use it to contact
        you for anything outside that scope.
      </p>

      <h2>How to opt out</h2>
      <p>
        At any time, you can:
      </p>
      <ul>
        <li>
          Reply <strong>STOP</strong> to any RacingPoint WhatsApp message
          and we will remove you from the list.
        </li>
        <li>
          Email <a href="mailto:info@racingpoint.in">info@racingpoint.in</a>{" "}
          with the subject line <em>&quot;Opt out&quot;</em> and the phone
          number you opted in with.
        </li>
        <li>
          Message us directly on{" "}
          <a
            href="https://wa.me/917981264279"
            target="_blank"
            rel="noopener noreferrer"
          >
            WhatsApp
          </a>{" "}
          and ask to be removed.
        </li>
      </ul>
      <p>
        We will remove your number from our active list within one business
        day and confirm in writing.
      </p>

      <h2>DPDP compliance</h2>
      <p>
        We follow India&apos;s Digital Personal Data Protection Act (2023).
        That means: explicit opt-in (you tick the box, we never assume),
        clear scope (racing-only), easy revocation (any of the routes
        above), and we retain only what we need for as long as you are
        subscribed.
      </p>

      <h2>Returning visitor cookie</h2>
      <p>
        Our landing page sets a small first-party cookie called{" "}
        <code>rp_returning</code> on your first visit. It only stores the
        value <code>1</code> — nothing personal — and we use it to skip the
        first-time-visitor introduction when you come back. The cookie is
        HTTP-only and expires after 90 days.
      </p>

      <h2>Contact</h2>
      <p>
        Questions or concerns? Email{" "}
        <a href="mailto:info@racingpoint.in">info@racingpoint.in</a> or
        message us on{" "}
        <a
          href="https://wa.me/917981264279"
          target="_blank"
          rel="noopener noreferrer"
        >
          WhatsApp
        </a>
        .
      </p>

      <p style={{ marginTop: "3rem", fontSize: "0.875rem", color: "#a0a0a0" }}>
        <a href="/v2/">← Back to RacingPoint</a>
      </p>
      </main>
    </>
  );
}
