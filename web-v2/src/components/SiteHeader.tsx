import styles from "../app/page.module.css";

const PWA_BOOK_LINK = "https://app.racingpoint.cloud/book";
const PWA_DASHBOARD_LINK = "https://app.racingpoint.cloud";

export function SiteHeader({ returning }: { returning: boolean }) {
  return (
    <header className={styles.header}>
      <div className={styles.headerInner}>
        <a href="/v2/" className={styles.brand} aria-label="RacingPoint">
          <span className={styles.brandMark}>RP</span>
          <span className={styles.brandWord}>RacingPoint</span>
        </a>
        <nav aria-label="Primary">
          <a href="/v2/#experiences" className={styles.navLink}>
            Experiences
          </a>
          <a href="/v2/#credits" className={styles.navLink}>
            Credits
          </a>
          <a href="/v2/#location" className={styles.navLink}>
            Visit
          </a>
          <a
            href={returning ? PWA_DASHBOARD_LINK : PWA_BOOK_LINK}
            className={styles.navCta}
          >
            {returning ? "Dashboard" : "Book"}
          </a>
        </nav>
      </div>
    </header>
  );
}
