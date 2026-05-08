/**
 * ProfilePreviewCard — renders ProfilePreview returned by /api/v1/cirs/lookup.
 *
 * PACT-20260506-001 Phase 1 wire-up Session 4.
 *
 * Absorbs gap #1 (UI evaluation 2026-05-08 ~05:15 IST): NEW vs REPEAT badge
 *   driven by arrival_count_30d (0 → NEW; ≥1 → REPEAT). Helps staff route
 *   to gaming-hall education step (Scenario 1 step 4 — "education NOT optional").
 *
 * Absorbs gap #5 (same evaluation): tier/discount surface placeholder.
 *   The slot renders Wave 0 with a placeholder when Wave 4 MI substrate
 *   (experience-score + discount-tier formula) is not yet wired. When Wave 4
 *   ships, the response shape extends with `tier` + `discount_pct` and the
 *   slot renders live values without a component-level structural change.
 *
 * Synthetic single-element profiles[] is a known Wave 0 limitation
 * (Q-CUSTOMER-PROFILES-1 deferred to multi-profile sub-PACT). Component
 * renders the profiles list as-given; multi-profile UX (gap #2) is V2.1+.
 */

import type { ProfilePreview } from "@/lib/types/cirs-lookup";
import styles from "./ProfilePreviewCard.module.css";

/**
 * Wave 4 placeholder. When MI substrate ships, the API response will extend
 * with these fields and they'll thread through. Optional in TS so Wave 0
 * compiles + renders without them.
 */
interface ProfilePreviewWithTier extends ProfilePreview {
  tier?: "new" | "building" | "loyal" | "vip" | "lapsed";
  discount_pct?: number;
}

interface Props {
  profile: ProfilePreviewWithTier;
}

const TIER_LABEL: Record<NonNullable<ProfilePreviewWithTier["tier"]>, string> = {
  new: "New",
  building: "Building",
  loyal: "Loyal",
  vip: "VIP",
  lapsed: "Lapsed (comeback)",
};

export default function ProfilePreviewCard({ profile }: Props) {
  const isNew = profile.arrival_count_30d === 0;
  const tierKnown = profile.tier !== undefined;

  return (
    <article className={styles.card}>
      <header className={styles.header}>
        <div className={styles.identity}>
          <h2 className={styles.name}>{profile.full_name}</h2>
          <p className={styles.customerId}>ID {profile.customer_id}</p>
        </div>
        <span
          className={`${styles.badge} ${isNew ? styles.badgeNew : styles.badgeRepeat}`}
          aria-label={isNew ? "New customer" : "Repeat customer"}
        >
          {isNew ? "New" : "Repeat"}
        </span>
      </header>

      <div className={styles.metrics}>
        <div className={styles.metric}>
          <span className={styles.metricLabel}>Wallet balance</span>
          <span className={styles.metricValue}>
            {profile.balance_credits}
            <span className={styles.metricUnit}>credits</span>
          </span>
        </div>
        <div className={styles.metric}>
          <span className={styles.metricLabel}>Visits (30d)</span>
          <span className={styles.metricValue}>{profile.arrival_count_30d}</span>
        </div>
      </div>

      {/* Gap #5 absorb — tier/discount surface (Wave 4 substrate; Wave 0 = placeholder) */}
      <div className={styles.tierSlot}>
        <span className={styles.tierLabel}>Customer tier &amp; offer</span>
        {tierKnown && profile.tier ? (
          <span className={styles.tierValue}>
            {TIER_LABEL[profile.tier]}
            {profile.discount_pct !== undefined && (
              <span className={styles.metricUnit}>
                {" "}
                — {profile.discount_pct}% offer
              </span>
            )}
          </span>
        ) : (
          <span className={styles.tierPlaceholder}>
            Tier display pending Wave 4 MI substrate (experience-score)
          </span>
        )}
      </div>

      {profile.profiles.length > 0 && (
        <section className={styles.profilesSection}>
          <span className={styles.profilesLabel}>
            Profiles ({profile.profiles.length})
          </span>
          {profile.profiles.map((p) => (
            <div key={p.profile_id} className={styles.profile}>
              <span className={styles.profileName}>
                {p.full_name}
                {p.is_default && " (default)"}
              </span>
              <span className={styles.profileMeta}>
                {p.discount_ineligible ? "Discount ineligible" : "Eligible"}
              </span>
            </div>
          ))}
        </section>
      )}
    </article>
  );
}
