import type { Metadata } from "next";
import styles from "./page.module.css";

export const metadata: Metadata = {
  title: "RacingPoint — Real cars. Real circuits. Real you.",
  description:
    "Eight pro-grade racing simulators · cafe · venue in Hyderabad. Book your first sim time.",
};

// Venue-specific values. Phone number + address line 1 default to graceful
// fallbacks so the page is customer-ready even before Captain fills them
// (wa.me/ with no number lands on WhatsApp's main page; address falls back
// to city-level + WhatsApp-for-directions framing). Captain fills with real
// venue values + rebuilds; rebuild is non-destructive (no schema change).
const VENUE_WHATSAPP_LINK = "https://wa.me/"; // append phone number when known
const PWA_BOOK_LINK = "https://app.racingpoint.cloud/";
const VENUE_ADDRESS_LINE_1 = ""; // street-line 1 (Captain to fill); rendered conditionally
const VENUE_ADDRESS_LINE_2 = "Hyderabad, Telangana, India";
const VENUE_HOURS = "Open daily · 12:00 – 24:00 IST";

export default function Home() {
  return (
    <>
      <a href="#main" className={styles.skipLink}>
        Skip to content
      </a>

      <Header />
      <main id="main">
        <Hero />
        <TrustBand />
        <Experiences />
        <CreditsExplainer />
        <Cafe />
        <ContactWhatsApp />
        <LocationHours />
      </main>
      <Footer />
    </>
  );
}

function Header() {
  return (
    <header className={styles.header}>
      <div className={styles.headerInner}>
        <span className={styles.brand} aria-label="RacingPoint">
          <span className={styles.brandMark}>RP</span>
          <span className={styles.brandWord}>RacingPoint</span>
        </span>
        <nav aria-label="Primary">
          <a href="#experiences" className={styles.navLink}>
            Experiences
          </a>
          <a href="#credits" className={styles.navLink}>
            Credits
          </a>
          <a href="#location" className={styles.navLink}>
            Visit
          </a>
          <a href={PWA_BOOK_LINK} className={styles.navCta}>
            Book
          </a>
        </nav>
      </div>
    </header>
  );
}

function Hero() {
  return (
    <section className={styles.hero} aria-labelledby="hero-heading">
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
          <a href={PWA_BOOK_LINK} className={styles.ctaPrimary}>
            Book your first sim time
          </a>
          <a href="#experiences" className={styles.ctaSecondary}>
            See what you can race
          </a>
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

const EXPERIENCES = [
  {
    title: "Solo Sim Session",
    description:
      "Pick your car, pick your circuit, drive. Coaching telemetry on every lap.",
    priceLine: "30 min · 700 credits",
    cta: "Book solo",
    href: PWA_BOOK_LINK,
  },
  {
    title: "Multi-player Race",
    description:
      "Two to eight drivers, same grid, same race. Sprints, leagues, league championships.",
    priceLine: "60 min · 900 credits",
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
        Prices shown in <strong>credits</strong>. 1 credit = ₹1. Top-ups
        carry an 18% GST charge at purchase.
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
          GST is applied at the top-up moment, not at every session — what
          you see on the wallet is what you spend.
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

function ContactWhatsApp() {
  return (
    <section
      className={styles.whatsapp}
      aria-labelledby="whatsapp-heading"
    >
      <div className={styles.whatsappInner}>
        <h2 id="whatsapp-heading">Questions? Reach us on WhatsApp.</h2>
        <p>
          The fastest way to ask about bookings, events, or group rates.
          We reply during venue hours.
        </p>
        <a href={VENUE_WHATSAPP_LINK} className={styles.ctaPrimary}>
          Message us on WhatsApp
        </a>
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
            {VENUE_ADDRESS_LINE_1 ? (
              <>
                {VENUE_ADDRESS_LINE_1}
                <br />
              </>
            ) : null}
            {VENUE_ADDRESS_LINE_2}
          </p>
          {!VENUE_ADDRESS_LINE_1 ? (
            <p className={styles.muted}>
              Ask on WhatsApp for the full venue address and pin.
            </p>
          ) : null}
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
            <a href={PWA_BOOK_LINK}>Customer app</a>
          </p>
        </div>
      </div>
    </section>
  );
}

function Footer() {
  return (
    <footer className={styles.footer}>
      <div className={styles.footerInner}>
        <p>© {new Date().getFullYear()} RacingPoint · Hyderabad</p>
        <p className={styles.muted}>
          By contacting us you agree to our messaging policy. We only send
          you what you ask for.
        </p>
      </div>
    </footer>
  );
}
