import styles from "../app/page.module.css";

const VENUE_WHATSAPP_PHONE = "917981264279";
const VENUE_WHATSAPP_LINK = `https://wa.me/${VENUE_WHATSAPP_PHONE}`;
const VENUE_EMAIL = "info@racingpoint.in";

export function SiteFooter() {
  return (
    <footer className={styles.footer}>
      <div className={styles.footerInner}>
        <p>© {new Date().getFullYear()} RacingPoint · Hyderabad</p>
        <p className={styles.footerLinks}>
          <a href="/v2/privacy">Privacy Policy</a>
          {" · "}
          <a href={VENUE_WHATSAPP_LINK}>WhatsApp</a>
          {" · "}
          <a href={`mailto:${VENUE_EMAIL}`}>{VENUE_EMAIL}</a>
        </p>
        <p className={styles.muted}>
          DPDP-compliant. We only send what you ask for. Opt out via Privacy
          Policy or message us on WhatsApp.
        </p>
      </div>
    </footer>
  );
}
