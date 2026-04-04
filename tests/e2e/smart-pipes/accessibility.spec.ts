import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

const pages = [
  { name: 'Dashboard', url: 'http://192.168.31.23:3200/' },
  { name: 'Billing', url: 'http://192.168.31.23:3200/billing' },
  { name: 'Cameras', url: 'http://192.168.31.23:3200/cameras' },
];

for (const p of pages) {
  test(`Accessibility: ${p.name}`, async ({ page }) => {
    await page.goto(p.url);
    await page.waitForLoadState('domcontentloaded');
    const results = await new AxeBuilder({ page }).analyze();
    const critical = results.violations.filter(v => v.impact === 'critical' || v.impact === 'serious');
    console.log(`${p.name}: ${results.violations.length} total violations, ${critical.length} critical/serious`);
    for (const v of critical.slice(0, 5)) {
      console.log(`  [${v.impact}] ${v.id}: ${v.description} (${v.nodes.length} nodes)`);
    }
  });
}
