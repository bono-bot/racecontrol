import { test, expect, type Route, type Page } from "@playwright/test";

/**
 * CIRS lookup E2E specs — PACT-20260506-001 §5.4 + §AMEND-1.D.
 *
 * Five scenarios from PACT §5.4 spec list:
 *   1. 9876543210 → ProfilePreview rendered (Found path)
 *   2. 0000000000 → NotFound CTA (with WARN-gate override since digit[0]=0)
 *   3. ٩٨٧٦٥٤٣٢١٠ (Arabic-Indic per §AMEND-1.D) → input strips to empty
 *      via the JS /\D/ sanitizer; submit disabled (defense-in-depth — the
 *      substrate's InvalidPhone path never gets exercised because the UI
 *      input handler intercepts non-ASCII digits before they hit the API).
 *   4. Walk-In Guest 1 selection → Found ProfilePreview short-circuit
 *      (discount_ineligible: true)
 *   5. 1234567890 → Indian-mobile-prefix WARN gate (NF-james-B per
 *      §AMEND-1.B) → Continue-anyway override → submittable
 *
 * Backend: every test mocks `POST /api/v1/cirs/lookup` via `page.route()`
 * so this lane runs without a racecontrol Rust backend.
 *
 * NOTE — phones starting with digits {0,1,2,3,4,5} fire the WARN gate and
 * surface the "Confirm & lookup" submit text; tests that use such phones
 * (specs 2, 4, 5) walk the override path explicitly.
 */

const POS_LOOKUP_URL = "/v2/pos/lookup";
const LOOKUP_API_PATTERN = "**/api/v1/cirs/lookup";

interface FoundResponseProfile {
  customer_id: string;
  full_name: string;
  balance_credits: number;
  arrival_count_30d: number;
  profiles: Array<{
    profile_id: string;
    full_name: string;
    is_default: boolean;
    discount_ineligible: boolean;
  }>;
}

const REGISTERED_CUSTOMER: FoundResponseProfile = {
  customer_id: "00000000-0000-0000-0000-000000000001",
  full_name: "Test Customer",
  balance_credits: 480,
  arrival_count_30d: 4,
  profiles: [
    {
      profile_id: "00000000-0000-0000-0000-000000000001",
      full_name: "Test Customer",
      is_default: true,
      discount_ineligible: false,
    },
  ],
};

const WALK_IN_GUEST_1: FoundResponseProfile = {
  customer_id: "walk_in_guest_1",
  full_name: "Walk-In Guest 1",
  balance_credits: 0,
  arrival_count_30d: 0,
  profiles: [],
};

const fulfillJson = (route: Route, body: unknown, status = 200) =>
  route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(body),
  });

/**
 * Submit a phone that triggers the Indian-mobile-prefix WARN gate
 * (digit[0] ∈ {0,1,2,3,4,5}) by clicking Continue-anyway then
 * the now-relabelled "Confirm & lookup" button.
 */
async function submitWithWarnOverride(page: Page, phone: string) {
  await page.locator("#phone-lookup-input").fill(phone);
  await expect(page.locator("#phone-warn")).toBeVisible();
  await page.getByRole("button", { name: /Continue anyway/ }).click();
  // After acknowledge the submit is enabled with text "Lookup".
  await page.getByRole("button", { name: /^Lookup$/ }).click();
}

test.describe("CIRS lookup E2E (PACT-20260506-001 §5.4)", () => {
  test("phone Found path renders ProfilePreview with name + balance + repeat badge", async ({
    page,
  }) => {
    await page.route(LOOKUP_API_PATTERN, (route) =>
      fulfillJson(route, { status: "found", profile: REGISTERED_CUSTOMER }),
    );

    await page.goto(POS_LOOKUP_URL);

    await page.locator("#phone-lookup-input").fill("9876543210");
    await page.getByRole("button", { name: /^Lookup$/ }).click();

    // h2 in ProfilePreviewCard renders profile.full_name; specific role+name
    // avoids collision with the ProfileSummary span "Test Customer (default)".
    await expect(
      page.getByRole("heading", { name: "Test Customer" }),
    ).toBeVisible();
    // Wallet balance is rendered as the integer 480 immediately followed by
    // a "credits" unit span. The 480 text node lives inside a span whose
    // full text is "480credits" — `getByText("480")` (substring) matches the
    // text node itself, which is unambiguous because 480 appears only here
    // in the REGISTERED_CUSTOMER fixture.
    await expect(page.getByText("Wallet balance")).toBeVisible();
    await expect(page.getByText("480")).toBeVisible();
    // arrival_count_30d > 0 surfaces the Repeat badge (aria-label set per
    // ProfilePreviewCard's isNew check).
    await expect(page.getByLabel("Repeat customer")).toBeVisible();
  });

  test("phone NotFound shows NotFound CTA with searched phone (WARN override path)", async ({
    page,
  }) => {
    await page.route(LOOKUP_API_PATTERN, (route) =>
      fulfillJson(route, { status: "not_found", reason: "unknown_phone" }),
    );

    await page.goto(POS_LOOKUP_URL);
    // 0000000000 fires the Indian-mobile-prefix WARN — walk the override.
    await submitWithWarnOverride(page, "0000000000");

    await expect(
      page.getByRole("heading", { name: "No customer found" }),
    ).toBeVisible();
    await expect(page.getByText("0000000000")).toBeVisible();
    await expect(
      page.getByRole("button", { name: /Register new customer/ }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: /Use Walk-In Guest/ }),
    ).toBeVisible();
  });

  test("Arabic-Indic codepoints (٩٨٧٦٥٤٣٢١٠) are stripped by /\\D/ sanitizer; submit disabled", async ({
    page,
  }) => {
    // §AMEND-1.D codepoint annotation (U+0660–U+0669 ARABIC-INDIC DIGITS,
    // value=9876543210). The PhoneLookupInput onChange handler strips
    // non-ASCII digits via String.prototype.replace(/\D/g, ""), so paste
    // results in an empty string — defense-in-depth before substrate.
    let lookupCalled = false;
    await page.route(LOOKUP_API_PATTERN, (route) => {
      lookupCalled = true;
      return fulfillJson(route, { status: "found", profile: REGISTERED_CUSTOMER });
    });

    await page.goto(POS_LOOKUP_URL);

    const arabicIndic = "٩٨٧٦٥٤٣٢١٠";
    const input = page.locator("#phone-lookup-input");
    await input.fill(arabicIndic);

    await expect(input).toHaveValue("");

    const submit = page.getByRole("button", { name: /^Lookup$/ });
    await expect(submit).toBeDisabled();

    expect(lookupCalled).toBe(false);
  });

  test("Walk-In Guest 1 short-circuits to discount-ineligible Found preview", async ({
    page,
  }) => {
    await page.route(LOOKUP_API_PATTERN, async (route) => {
      const body = route.request().postDataJSON();
      // Inner field is `guest_id` (Rust canonical wire — Session 7 MMA fix).
      if (body?.method === "walk_in_guest_id" && body?.guest_id === 1) {
        return fulfillJson(route, { status: "found", profile: WALK_IN_GUEST_1 });
      }
      // NotFound on the seed phone path so the WalkInGuestDropdown surfaces.
      return fulfillJson(route, { status: "not_found", reason: "unknown_phone" });
    });

    await page.goto(POS_LOOKUP_URL);
    // 0000000000 fires WARN — walk the override.
    await submitWithWarnOverride(page, "0000000000");

    // WalkInGuestDropdown renders below NotFoundCTA per page state machine.
    await page
      .getByRole("button", { name: "Select Walk-In Guest 1" })
      .click();

    // After selection, view flips to `found` with WALK_IN_GUEST_1 profile
    // → ProfilePreviewCard renders with h2 "Walk-In Guest 1".
    await expect(
      page.getByRole("heading", { name: "Walk-In Guest 1" }),
    ).toBeVisible();
  });

  test("Indian-mobile-prefix WARN gate fires for digit[0]=1; Continue-anyway unblocks submit", async ({
    page,
  }) => {
    // NF-james-B per §AMEND-1.B — digit[0] ∈ {0,1,2,3,4,5} fires WARN-with-override.
    let lookupCalled = false;
    await page.route(LOOKUP_API_PATTERN, (route) => {
      lookupCalled = true;
      return fulfillJson(route, { status: "found", profile: REGISTERED_CUSTOMER });
    });

    await page.goto(POS_LOOKUP_URL);

    const input = page.locator("#phone-lookup-input");
    await input.fill("1234567890");

    await expect(page.locator("#phone-warn")).toBeVisible();

    const submitBefore = page.getByRole("button", { name: /Confirm & lookup/ });
    await expect(submitBefore).toBeDisabled();

    await page.getByRole("button", { name: /Continue anyway/ }).click();

    const submitAfter = page.getByRole("button", { name: /^Lookup$/ });
    await expect(submitAfter).toBeEnabled();
    await submitAfter.click();

    expect(lookupCalled).toBe(true);
    await expect(
      page.getByRole("heading", { name: "Test Customer" }),
    ).toBeVisible();
  });
});
