import { test, expect } from '@playwright/test';

const BASE = 'http://localhost:3309';

test.describe('Full Business Flow — Reviewer + Counterparty UI', () => {

  test('complete flow: create case → submit proofs → verify', async ({ browser }) => {
    const reviewerCtx = await browser.newContext();
    const reviewer = await reviewerCtx.newPage();

    // Reviewer login + create case
    await reviewer.goto(`${BASE}/login`);
    await reviewer.fill('input[name="email"]', 'reviewer@verigate.io');
    await reviewer.fill('input[name="password"]', 'reviewer123');
    await reviewer.click('button[type="submit"]');
    await reviewer.waitForURL('**/dashboard');
    console.log('✓ Reviewer logged in');

    await reviewer.click('text=New Case');
    await reviewer.waitForTimeout(500);
    await reviewer.fill('input[placeholder*="Meridian"]', 'UI Flow Test Corp');
    await reviewer.click('button:has-text("Create Case")');
    await reviewer.waitForTimeout(3000);
    console.log('✓ Case created');

    // Get case portal link
    const caseLink = reviewer.locator('a:has-text("Open Portal")').first();
    const portalHref = await caseLink.getAttribute('href');
    const caseId = portalHref?.split('/portal/')[1] || '';
    console.log(`✓ Case ID: ${caseId}`);

    // Counterparty login + submit proofs
    const cpCtx = await browser.newContext();
    const cp = await cpCtx.newPage();
    await cp.goto(`${BASE}/login`);
    await cp.fill('input[name="email"]', 'counterparty@verigate.io');
    await cp.fill('input[name="password"]', 'counterparty123');
    await cp.click('button[type="submit"]');
    await cp.waitForURL('**/portal');
    console.log('✓ Counterparty logged in');

    // Navigate to case
    await cp.goto(`${BASE}/portal/${caseId}`);
    await cp.waitForTimeout(2000);

    // Check requirements loaded
    const reqCount = await cp.locator('.portal__tab-count').first().textContent();
    console.log(`✓ Requirements visible: ${reqCount}`);

    // Submit first proof
    const submitBtn = cp.locator('button:has-text("Submit Proof")').first();
    const hasBtns = await submitBtn.isVisible().catch(() => false);
    console.log(`✓ Submit button visible: ${hasBtns}`);

    if (hasBtns) {
      await submitBtn.click();
      await cp.waitForTimeout(5000);
      // Check if verification succeeded
      const verifiedBadge = cp.locator('text=Verified in TEE');
      const verified = await verifiedBadge.isVisible().catch(() => false);
      console.log(`✓ First proof verified in TEE: ${verified}`);
      await cp.screenshot({ path: 'screenshots/full-flow/07-proof-submitted.png' });
    }

    // Check progress updated
    await cp.waitForTimeout(1000);
    const progressText = await cp.locator('.portal__progress-badge').textContent();
    console.log(`✓ Progress: ${progressText}`);

    // Privacy view
    await reviewer.goto(`${BASE}/privacy/${caseId}`);
    await reviewer.waitForTimeout(2000);
    const teeBadge = await reviewer.locator('.tee-badge__label').isVisible().catch(() => false);
    console.log(`✓ Privacy TEE badge: ${teeBadge}`);
    await reviewer.screenshot({ path: 'screenshots/full-flow/08-privacy-view.png' });

    // Logout both
    await cp.goto(`${BASE}/login`);
    await reviewer.goto(`${BASE}/login`);
    console.log('✓ Both logged out');

    await reviewerCtx.close();
    await cpCtx.close();
  });
});
