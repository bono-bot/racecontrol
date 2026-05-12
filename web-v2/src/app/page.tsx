import type { Metadata } from "next";
import { cookies } from "next/headers";
import styles from "./page.module.css";
import { WhatsAppOptInForm } from "../components/WhatsAppOptInForm";
import { SiteHeader } from "../components/SiteHeader";
import { SiteFooter } from "../components/SiteFooter";

/*
 * V2 Customer Entry Page — racingpoint.cloud apex landing.
 *
 * Source: racecontrol/.planning/specs/v2/V2-CUSTOMER-ENTRY/UI-SPEC-v0.2.md
 *   - §1–§8 inherited from UI-SPEC v0.1 (Goal · Segments · Page structure · Visual · Responsive · A11y · Perf · Composes-with)
 *   - §9 Q-CUST-1..7 dispositions: 2 AUTONOMOUS-LOCKED + 5 CAPTAIN-RATIFIED 2026-05-12 ~11:05 IST verbatim "all bono recommendations" (V2-MASTER-STATE §S-204 close-anchor)
 *
 * Returning-customer cookie (`rp_returning`) set by middleware.ts on first visit; read here to swap Hero CTA.
 *
 * Note on V2 nomenclature: "V2" / "V2.0" is internal-only per Captain 2026-05-11 ~09:28 IST.
 * Customer-facing copy never says "V2" — this is just the canonical RacingPoint landing.
 */

const SITE_TITLE = "RacingPoint — Real cars. Real circuits. Real you.";
const SITE_DESCRIPTION =
  "Eight pro-grade racing simulators · cafe · venue in Hyderabad. Book your first sim time.";
const SITE_URL = "https://v2.racingpoint.cloud/v2";

export const metadata: Metadata = {
  title: SITE_TITLE,
  description: SITE_DESCRIPTION,
  openGraph: {
    title: SITE_TITLE,
    description: SITE_DESCRIPTION,
    url: SITE_URL,
    siteName: "RacingPoint",
    locale: "en_IN",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: SITE_TITLE,
    description: SITE_DESCRIPTION,
  },
};

// Venue-specific values sourced from canonically-deployed apex (racingpoint-website pm2 service).
const VENUE_WHATSAPP_PHONE = "917981264279";
const VENUE_WHATSAPP_LINK = `https://wa.me/${VENUE_WHATSAPP_PHONE}`;
const PWA_BOOK_LINK = "https://app.racingpoint.cloud/book";
const PWA_DASHBOARD_LINK = "https://app.racingpoint.cloud";
const VENUE_ADDRESS_LINE_1 = "Bandlaguda, Hyderabad";
const VENUE_ADDRESS_LINE_2 = "Telangana, India";
const VENUE_MAPS_PIN = "https://share.google/nufGoHR5BectU5NFh";
const VENUE_EMAIL = "info@racingpoint.in";
const VENUE_INSTAGRAM = "https://www.instagram.com/racingpoint.in/";
const VENUE_HOURS = "Open daily · 12:00 – 24:00 IST";

// Q-CUST-2 server-side cookie read. Async because Next.js 16 cookies() returns a Promise.
async function isReturningCustomer(): Promise<boolean> {
  const cookieStore = await cookies();
  return cookieStore.get("rp_returning")?.value === "1";
}

export default async function Home() {
  const returning = await isReturningCustomer();
  return (
    <>
      <a href="#main" className={styles.skipLink}>
        Skip to content
      </a>

      <SiteHeader returning={returning} />
      <main id="main">
        <Hero returning={returning} />
        <TrustBand />
        <Experiences />
        <CreditsExplainer />
        <Cafe />
        <WhatsAppOptIn />
        <LocationHours />
      </main>
      <SiteFooter />
    </>
  );
}

function Hero({ returning }: { returning: boolean }) {
  return (
    <section className={styles.hero} aria-labelledby="hero-heading">
      {/*
       * Q-CUST-1 hero photography — brand-assets/photos/ directory not present at
       * authoring time 2026-05-12; CAPTAIN-RATIFIED fallback (b) per §S-204:
       *   Asphalt-Black gradient-overlay placeholder + CSS speed-streaks texture,
       *   no broken-image state, no blank box. Replace with real photography at
       *   next venue reopen + photography session.
       */}
      <div className={styles.heroBackdrop} aria-hidden="true" />
      <div className={styles.heroContent}>
        <p className={styles.eyebrow}>RacingPoint · Hyderabad</p>
        <h1 id="hero-heading" className={styles.heroHeading}>
          Real cars.
          <br />
          Real circuits.
          <br />
          <span className={styles.heroAccent}>Real you.</span>
        </h1>
        <p className={styles.heroLede}>
          Eight pro-grade racing simulators, a cafe, and a venue built for
          drivers — from first-timers to league regulars.
        </p>
        <div className={styles.heroCtaRow}>
          {returning ? (
            <>
              <a href={PWA_DASHBOARD_LINK} className={styles.ctaPrimary}>
                Continue to your dashboard
              </a>
              <a href="#experiences" className={styles.ctaSecondary}>
                See what is on this week
              </a>
            </>
          ) : (
            <>
              <a href={PWA_BOOK_LINK} className={styles.ctaPrimary}>
                Book your first sim time
              </a>
              <a href="#experiences" className={styles.ctaSecondary}>
                See what you can race
              </a>
            </>
          )}
        </div>
      </div>
    </section>
  );
}

function TrustBand() {
  return (
    <section className={styles.trust} aria-label="Venue overview">
      <ul className={styles.trustList}>
        <li>
          <strong>8</strong>
          <span>Racing simulators</span>
        </li>
        <li>
          <strong>Cafe</strong>
          <span>On-site, always separate</span>
        </li>
        <li>
          <strong>Hyderabad</strong>
          <span>Open noon to midnight, daily</span>
        </li>
      </ul>
    </section>
  );
}

/*
 * Q-CUST-3 EXPERIENCES pricing — CAPTAIN-RATIFIED 2026-05-12 (a)+(b)+(c):
 *   - Static hardcoded values (a): ₹700 / 30 min · ₹900 / 60 min — no API fetch, no latency
 *   - GST-INCLUSIVE framing (b): displayed price IS the final price; 18% GST already inside
 *   - No pricing-page navigation alternative (c): inline disclosure in Experiences strip
 * v1 path gates on Wave 2 Dynamic Pricing (server-side fetch with v0 fallback on failure).
 */
const EXPERIENCES = [
  {
    title: "Solo Sim Session",
    description:
      "Pick your car, pick your circuit, drive. Coaching telemetry on every lap.",
    priceLine: "₹700 / 30 min · GST inclusive",
    cta: "Book solo",
    href: PWA_BOOK_LINK,
  },
  {
    title: "Multi-player Race",
    description:
      "Two to eight drivers, same grid, same race. Sprints, leagues, league championships.",
    priceLine: "₹900 / 60 min · GST inclusive",
    cta: "Book group",
    href: PWA_BOOK_LINK,
  },
  {
    title: "Group & Corporate",
    description:
      "Team building, birthdays, corporate events. We organise the format around your group.",
    priceLine: "Custom · talk to us",
    cta: "Plan an event",
    href: VENUE_WHATSAPP_LINK,
  },
];

function Experiences() {
  return (
    <section
      id="experiences"
      className={styles.experiences}
      aria-labelledby="experiences-heading"
    >
      <div className={styles.sectionHeader}>
        <h2 id="experiences-heading">What you can race</h2>
        <p>
          Three formats. One venue. Each one set up so anyone can step in
          today.
        </p>
      </div>
      <div className={styles.cardGrid}>
        {EXPERIENCES.map((exp) => (
          <article key={exp.title} className={styles.card}>
            <h3>{exp.title}</h3>
            <p className={styles.cardDesc}>{exp.description}</p>
            <p className={styles.cardPrice}>{exp.priceLine}</p>
            <a href={exp.href} className={styles.cardCta}>
              {exp.cta} →
            </a>
          </article>
        ))}
      </div>
      <p className={styles.priceNote}>
        Prices shown above are <strong>final</strong> — 18% GST is already
        included. 1 credit = ₹1 in your wallet; the wallet redeems for sim
        time only. Cafe is always separate.
      </p>
    </section>
  );
}

function CreditsExplainer() {
  return (
    <section
      id="credits"
      className={styles.credits}
      aria-labelledby="credits-heading"
    >
      <div className={styles.sectionHeader}>
        <h2 id="credits-heading">How credits work</h2>
        <p>One wallet. One purpose. No surprises.</p>
      </div>
      <ol className={styles.creditsList}>
        <li>
          <strong>You top up credits</strong> at the counter or on the app.
          GST (18%) is built into the price you see — what you pay for the
          top-up is what you spend.
        </li>
        <li>
          <strong>Credits redeem for sim time and console gaming</strong>.
          That is it. The wallet is a single-purpose voucher, locked to
          racing.
        </li>
        <li>
          <strong>Cafe orders are always separate</strong>. Coffee, food,
          and drinks are paid for at the counter. Your race wallet stays
          racing-only.
        </li>
        <li>
          <strong>Your balance does not expire</strong>. Top up today, drive
          next month.
        </li>
      </ol>
    </section>
  );
}

function Cafe() {
  return (
    <section className={styles.cafe} aria-labelledby="cafe-heading">
      <div className={styles.cafeInner}>
        <div className={styles.cafePhoto} aria-hidden="true" />
        <div className={styles.cafeText}>
          <h2 id="cafe-heading">The cafe</h2>
          <p>
            Coffee, light bites, and a space to debrief between races. Open
            to anyone — drivers, spectators, friends, family. Pay at the
            counter; the cafe is always its own thing.
          </p>
        </div>
      </div>
    </section>
  );
}

/*
 * Q-CUST-4 + Q-CUST-7 — WhatsApp opt-in with inline DPDP consent.
 *   - Q-CUST-4 (a): POST to /api/v2/marketing/whatsapp-optin (Next.js route → api-gateway proxy in future)
 *   - Q-CUST-4 (b): payload schema { phone, consent_text, consent_ts, source: "v2-landing" } per §S-158 audit-log
 *   - Q-CUST-4 (c stub): revocation page stub at /privacy (footer link); full implementation tracked as launch-gate
 *   - Q-CUST-7 (a): inline-with-WhatsApp-opt-in checkbox (unchecked default; submit gated until checked)
 *   - Q-CUST-7 (b): standalone Privacy Policy footer link (covers browse-without-opt-in visitors)
 *   - Q-CUST-7 (c): at-arrival modal REJECTED for v0
 * Legal-AMPLIFIER queued for wording finalization (non-blocking on ship; entry
 * `legal-amplifier-queue-qcust4-qcust7-wording-20260512-1110-IST`).
 */
function WhatsAppOptIn() {
  return (
    <section
      className={styles.whatsapp}
      aria-labelledby="whatsapp-heading"
    >
      <div className={styles.whatsappInner}>
        <h2 id="whatsapp-heading">Stay in the loop on WhatsApp</h2>
        <p>
          Get session reminders, new track unlocks, and league invites
          straight on WhatsApp. We only message about racing — never spam.
        </p>
        <WhatsAppOptInForm />
        <p className={styles.whatsappFallback}>
          Prefer a one-off question?{" "}
          <a href={VENUE_WHATSAPP_LINK}>Message us on WhatsApp</a>.
        </p>
      </div>
    </section>
  );
}

function LocationHours() {
  return (
    <section
      id="location"
      className={styles.location}
      aria-labelledby="location-heading"
    >
      <div className={styles.sectionHeader}>
        <h2 id="location-heading">Find us</h2>
      </div>
      <div className={styles.locationGrid}>
        <div className={styles.locationBlock}>
          <h3>Address</h3>
          <p>
            {VENUE_ADDRESS_LINE_1}
            <br />
            {VENUE_ADDRESS_LINE_2}
          </p>
          <p>
            <a
              href={VENUE_MAPS_PIN}
              target="_blank"
              rel="noopener noreferrer"
            >
              Open in Maps →
            </a>
          </p>
        </div>
        <div className={styles.locationBlock}>
          <h3>Hours</h3>
          <p>{VENUE_HOURS}</p>
          <p className={styles.muted}>
            Late-night iRacing sessions run past midnight when there is a
            live event — check WhatsApp for the schedule.
          </p>
        </div>
        <div className={styles.locationBlock}>
          <h3>Get in touch</h3>
          <p>
            <a href={VENUE_WHATSAPP_LINK}>WhatsApp</a>
            <br />
            <a href={`mailto:${VENUE_EMAIL}`}>{VENUE_EMAIL}</a>
            <br />
            <a
              href={VENUE_INSTAGRAM}
              target="_blank"
              rel="noopener noreferrer"
            >
              Instagram
            </a>
            <br />
            <a href={PWA_BOOK_LINK}>Customer app</a>
          </p>
        </div>
      </div>
    </section>
  );
}

