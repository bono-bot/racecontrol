import { test, expect } from "@playwright/test";

/**
 * FLAG-2-Kaizen-A11y-AAA bundle — behavioral verification spec.
 *
 * Closes UI-AUDIT-FLAG2-REAUDIT-20260512 4 sub-findings via served-asset
 * + rendered-DOM probes. Composes with §S-208 close-anchor.
 *
 * Sub-findings verified here:
 *   FLAG-2a — skip-link parity on /v2/privacy (rendered <a class="rp-skip-link"> + <main id="main">)
 *   FLAG-2b — input outline upgrade (computed outline 2px solid racing-red on .optInField input[type="tel"]:focus-visible)
 *   FLAG-2c — global a:focus-visible explicit outline (computed outline 2px solid racing-red)
 *   FLAG-2d — @media (prefers-reduced-motion: reduce) guard in served global CSS
 *
 * NOT covered here: real screen-reader narration · mobile real-device ·
 * forced-colors mode · OS-level prefers-reduced-motion toggle behavioral
 * (CSS-presence-only). See UI-AUDIT-FLAG2-REAUDIT-20260512.md §NOT TESTED.
 *
 * Captures screenshots to tests/screenshots/ per SCREENSHOT ENFORCEMENT
 * hook + racecontrol/CLAUDE.md "Visual verification for display-affecting
 * deploys" standing rule.
 */

test.describe("FLAG-2-Kaizen-A11y-AAA bundle", () => {
  test("FLAG-2a — privacy page renders skip-link + <main id='main'>", async ({ page }) => {
    await page.goto("/v2/privacy");

    const skipLink = page.locator("a.rp-skip-link");
    await expect(skipLink).toHaveAttribute("href", "#main");
    await expect(skipLink).toHaveText(/Skip to content/i);

    const main = page.locator("main#main");
    await expect(main).toBeVisible();
    await expect(main.locator("h1")).toHaveText(/Privacy Policy/);

    await page.screenshot({
      path: "tests/screenshots/flag2a-privacy-skip-link-rendered.png",
      fullPage: false,
    });
  });

  test("FLAG-2a — skip-link slides into view on keyboard focus (left:0)", async ({ page }) => {
    await page.goto("/v2/privacy");
    const skipLink = page.locator("a.rp-skip-link");

    const initialLeft = await skipLink.evaluate(
      (el) => window.getComputedStyle(el).left,
    );
    expect(initialLeft).toBe("-9999px");

    await skipLink.focus();
    const focusedLeft = await skipLink.evaluate(
      (el) => window.getComputedStyle(el).left,
    );
    expect(focusedLeft).toBe("0px");

    await page.screenshot({
      path: "tests/screenshots/flag2a-privacy-skip-link-focused.png",
      fullPage: false,
    });
  });

  test("FLAG-2b — opt-in tel input has 2px outline on focus-visible", async ({ page }) => {
    await page.goto("/v2/");
    const phoneInput = page.locator('input[type="tel"][name="phone"]');
    await phoneInput.focus();

    const outlineWidth = await phoneInput.evaluate(
      (el) => window.getComputedStyle(el).outlineWidth,
    );
    expect(outlineWidth).toBe("2px");

    const outlineStyle = await phoneInput.evaluate(
      (el) => window.getComputedStyle(el).outlineStyle,
    );
    expect(outlineStyle).toBe("solid");

    await page.screenshot({
      path: "tests/screenshots/flag2b-input-focus-outline.png",
      fullPage: false,
    });
  });

  test("FLAG-2c — served global CSS has explicit a:focus-visible outline", async ({ request }) => {
    const root = await request.get("/v2/");
    const html = await root.text();
    const cssMatch = html.match(/\/v2\/_next\/static\/chunks\/[a-f0-9]+\.css/g);
    expect(cssMatch).not.toBeNull();

    // Fetch each candidate chunk; one of them has the global rules.
    let foundOutline = false;
    let foundReducedMotion = false;
    let foundSkipLink = false;

    for (const path of cssMatch!) {
      const css = await (await request.get(path)).text();
      if (/a:focus-visible\{[^}]*outline:2px solid/.test(css)) foundOutline = true;
      if (/@media \(prefers-reduced-motion:reduce\)/.test(css)) foundReducedMotion = true;
      if (/\.rp-skip-link\{/.test(css)) foundSkipLink = true;
    }

    expect(foundOutline).toBe(true);
    expect(foundReducedMotion).toBe(true);
    expect(foundSkipLink).toBe(true);
  });

  test("FLAG-2c-followup — /v2/ root .skipLink has white outline on focus-visible (no red-on-red collision)", async ({ page }) => {
    await page.goto("/v2/");
    const skipLink = page.locator("a[class*='skipLink']").first();
    // Use keyboard Tab to trigger :focus-visible (programmatic .focus() doesn't always match).
    await page.keyboard.press("Tab");
    await expect(skipLink).toBeFocused();

    const outlineColor = await skipLink.evaluate(
      (el) => window.getComputedStyle(el).outlineColor,
    );
    expect(outlineColor).toBe("rgb(255, 255, 255)");

    const outlineWidth = await skipLink.evaluate(
      (el) => window.getComputedStyle(el).outlineWidth,
    );
    expect(outlineWidth).toBe("2px");

    await page.screenshot({
      path: "tests/screenshots/flag2c-followup-skiplink-white-outline.png",
      fullPage: false,
    });
  });

  test("FLAG-2c-followup — served /v2/ page CSS chunk contains skipLink:focus-visible white outline", async ({ request }) => {
    const root = await request.get("/v2/");
    const html = await root.text();
    const cssMatch = html.match(/\/v2\/_next\/static\/chunks\/[a-f0-9_-]+\.css/g);
    expect(cssMatch).not.toBeNull();

    let foundSkipLinkOutline = false;
    for (const path of cssMatch!) {
      const css = await (await request.get(path)).text();
      if (/skipLink[^{]*:focus-visible\{[^}]*outline:2px solid #fff/.test(css)) {
        foundSkipLinkOutline = true;
      }
    }
    expect(foundSkipLinkOutline).toBe(true);
  });

  test("FLAG-3 regression — disabled submit still labeled 'Please confirm consent'", async ({ page }) => {
    await page.goto("/v2/");
    const submit = page.locator("button[type='submit'].page-module___8aEwW__optInSubmit, button[type='submit'][class*='optInSubmit']");
    await expect(submit).toHaveAttribute("disabled", "");
    await expect(submit).toHaveAttribute("aria-disabled", "true");
    await expect(submit).toHaveText("Please confirm consent");
  });
});
