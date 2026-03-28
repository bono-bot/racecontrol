# Phase 255: Legal Compliance - Context

**Gathered:** 2026-03-29
**Status:** Ready for planning
**Mode:** Auto-generated (discuss skipped)

<domain>
## Phase Boundary

Every session is legally auditable: GST is correctly separated, waivers are enforced, and minor protections are active. India-specific compliance for Consumer Protection Act, DPDP Act, and Indian Contract Act.

Requirements: LEGAL-01 (18% GST separation), LEGAL-02 (GST invoice), LEGAL-03 (waiver gate), LEGAL-04 (minor guardian OTP), LEGAL-05 (guardian presence toggle), LEGAL-06 (minor liability disclosure), LEGAL-07 (pricing/refund display), LEGAL-08 (data retention policy), LEGAL-09 (parental consent revocation)

Depends on: Phase 252 (invoicing requires atomically-committed session data), Phase 254 (RBAC gates legal workflows)

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
Key guidance from MMA audit (India-specific research by Gemini Pro with web sources):

- LEGAL-01: GST is 18% for sim racing (amusement facility, 28% slab removed per user confirmation). In the accounting.rs journal posting, split every session charge into two lines: Revenue = price / 1.18, GST Payable = price - Revenue. Use inclusive GST calculation.
- LEGAL-02: Generate a GST-compliant invoice struct with: venue GSTIN, HSN/SAC code (SAC 999692 for amusement services), customer name, invoice number (sequential), date, taxable value, GST amount, total. Store in invoices table. Expose via GET /api/v1/billing/sessions/{id}/invoice.
- LEGAL-03: In start_billing handler (routes.rs), check driver.waiver_signed before allowing billing. If false, return 400 with "Waiver required". POS kiosk must show waiver flow before billing.
- LEGAL-04: For drivers with age < 18 (from DOB), require guardian_phone + guardian OTP verified before billing. Add guardian_otp_verified BOOLEAN to auth_tokens or drivers.
- LEGAL-05: Add guardian_present BOOLEAN to billing_sessions. Staff must toggle this in kiosk UI for minor sessions. Server rejects billing start if minor + guardian_present=false.
- LEGAL-06: When guardian signs waiver for minor, show disclosure text: "Under Indian Contract Act 1872, this waiver may not be legally enforceable for minors. Racing Point maintains additional insurance coverage for participants under 18."
- LEGAL-07: Add pricing_policy TEXT field to pricing display response. Kiosk shows refund policy before wallet top-up acceptance.
- LEGAL-08: Add data_retention_config table or TOML config: financial_records_years=8, pii_inactive_months=24. Background job marks inactive drivers for anonymization.
- LEGAL-09: PWA endpoint POST /customer/revoke-consent that anonymizes driver PII and marks account as revoked.

</decisions>

<code_context>
## Existing Code Insights

### Key Files
- `crates/racecontrol/src/accounting.rs` — journal entry posting (post_topup, post_refund)
- `crates/racecontrol/src/api/routes.rs` — billing start, pricing display
- `crates/racecontrol/src/auth/mod.rs` — OTP, waiver status
- `crates/racecontrol/src/db/mod.rs` — drivers table schema, migrations
- `kiosk/src/components/SetupWizard.tsx` — kiosk billing flow (would need waiver UI)
- `pwa/src/app/` — PWA customer-facing pages

### Established Patterns
- Journal entries: double-entry in accounting.rs with acc_cash, acc_wallet, acc_racing_rev accounts
- Driver schema: waiver_signed, dob, guardian_name, guardian_phone fields exist
- OTP: argon2id hashing from Phase 254

</code_context>

<specifics>
## Specific Ideas

User confirmed: GST is 18% (28% slab removed). Hostinger Mumbai datacenter OK for data localization. Need operational solution for minor waiver non-enforceability.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
