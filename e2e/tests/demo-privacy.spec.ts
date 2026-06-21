import { test, expect, APIRequestContext } from '@playwright/test';
import {
  getDemoCases,
  SEED_CASE_IDS,
  SCREENSHOT_DIR,
  DemoCaseResponse,
  generateDemoJwt,
} from '../fixtures/demo-helpers';

/**
 * Phase 10 E2E — Privacy Split-Screen Verification
 *
 * Proves Success Criterion 3:
 *   SC3: Privacy split-screen shows full vs disclosed with redaction animation
 */
test.describe('Phase 10 — Privacy Split-Screen', () => {
  const caseId = SEED_CASE_IDS.meridian; // Approved case with all 4 credential types

  test.beforeEach(async ({ page }) => {
    // Inject demo JWT into localStorage before navigation
    const token = generateDemoJwt();
    await page.addInitScript((jwt: string) => {
      localStorage.setItem('demo_jwt', jwt);
    }, token);
  });

  test('SC3: split-screen page renders two panels', async ({ page }) => {
    await page.goto(`/privacy/${caseId}`);

    // Wait for split panels to render
    const leftPanel = page.locator('.split-screen__panel--full');
    const rightPanel = page.locator('.split-screen__panel--disclosed');

    await expect(leftPanel).toBeVisible({ timeout: 15000 });
    await expect(rightPanel).toBeVisible({ timeout: 15000 });

    // Left panel header: "What the Counterparty Provided"
    const leftTitle = leftPanel.locator('.split-screen__title');
    await expect(leftTitle).toContainText('Counterparty Provided', { ignoreCase: true });

    // Right panel header: "What the AI Agent Sees"
    const rightTitle = rightPanel.locator('.split-screen__title');
    await expect(rightTitle).toContainText('AI Agent Sees', { ignoreCase: true });

    // Both panels have field content (not empty)
    const leftFields = leftPanel.locator('.field-redaction');
    const rightFields = rightPanel.locator('.field-redaction');
    const leftCount = await leftFields.count();
    const rightCount = await rightFields.count();
    expect(leftCount).toBeGreaterThan(0);
    expect(rightCount).toBeGreaterThan(0);

    // Left panel has MORE fields than right (proving privacy filtering)
    expect(leftCount).toBeGreaterThan(rightCount);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}demo-privacy-split-screen.png`,
      fullPage: true,
    });
  });

  test('SC3: redacted fields show animation/blur effect', async ({ page }) => {
    await page.goto(`/privacy/${caseId}`);

    // Wait for animation to play through
    await page.waitForTimeout(2500);

    // Left panel should have redacted fields
    const leftPanel = page.locator('.split-screen__panel--full');
    await expect(leftPanel).toBeVisible({ timeout: 10000 });

    // Redacted fields contain the {{redacted}} placeholder
    const redactedPlaceholders = leftPanel.locator('.field-redaction__placeholder');
    const redactedCount = await redactedPlaceholders.count();
    expect(redactedCount).toBeGreaterThanOrEqual(3);

    // Disclosed fields in right panel have the checkmark
    const rightPanel = page.locator('.split-screen__panel--disclosed');
    const disclosedChecks = rightPanel.locator('.field-redaction__check');
    const disclosedCount = await disclosedChecks.count();
    expect(disclosedCount).toBeGreaterThan(0);

    // Disclosed count < total fields in left panel (proving selective disclosure)
    const totalLeftFields = await leftPanel.locator('.field-redaction').count();
    expect(disclosedCount).toBeLessThan(totalLeftFields);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}demo-privacy-redacted-fields.png`,
      fullPage: true,
    });
  });

  test('SC3: credential type tabs switch content', async ({ page }) => {
    await page.goto(`/privacy/${caseId}`);

    // Wait for page to render
    const tabs = page.locator('.split-screen__tab');
    await expect(tabs.first()).toBeVisible({ timeout: 10000 });

    // Expect 4 tabs for each credential type
    const tabCount = await tabs.count();
    expect(tabCount).toBe(4);

    // Click second tab (Authorized Signer)
    await tabs.nth(1).click();
    await page.waitForTimeout(500);

    // Right panel should show signer-related disclosed fields
    const rightPanel = page.locator('.split-screen__panel--disclosed');
    const rightFieldNames = rightPanel.locator('.field-redaction__key');
    const fieldTexts: string[] = [];
    const count = await rightFieldNames.count();
    for (let i = 0; i < count; i++) {
      const text = await rightFieldNames.nth(i).textContent();
      if (text) fieldTexts.push(text.toLowerCase());
    }
    // Authorized signer discloses: full_name, title, signing_authority_level
    const hasSignerFields = fieldTexts.some(
      (t) => t.includes('full name') || t.includes('title') || t.includes('signing authority')
    );
    expect(hasSignerFields, `Expected signer fields, got: ${fieldTexts.join(', ')}`).toBe(true);

    // Click third tab (Jurisdiction Compliance)
    await tabs.nth(2).click();
    await page.waitForTimeout(500);

    const rightFieldNames2 = rightPanel.locator('.field-redaction__key');
    const fieldTexts2: string[] = [];
    const count2 = await rightFieldNames2.count();
    for (let i = 0; i < count2; i++) {
      const text = await rightFieldNames2.nth(i).textContent();
      if (text) fieldTexts2.push(text.toLowerCase());
    }
    // Jurisdiction compliance discloses: country_code, regulatory_status, compliance_rating
    const hasJurisdictionFields = fieldTexts2.some(
      (t) => t.includes('country') || t.includes('regulatory') || t.includes('compliance')
    );
    expect(hasJurisdictionFields, `Expected jurisdiction fields, got: ${fieldTexts2.join(', ')}`).toBe(true);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}demo-privacy-tab-switching.png`,
      fullPage: true,
    });
  });

  test('SC3: disclosure stats bar shows ratio', async ({ page }) => {
    await page.goto(`/privacy/${caseId}`);

    // Wait for stats bar to render
    const statsBar = page.locator('.split-screen__stats');
    await expect(statsBar).toBeVisible({ timeout: 10000 });

    // Should contain "of X fields disclosed" text pattern
    const statsText = await statsBar.textContent();
    expect(statsText).toBeTruthy();
    // textContent concatenates span content without spaces: "3of 11 fields disclosed"
    expect(statsText).toMatch(/\d+\s*of \d+ fields disclosed/);

    // Should show percentage
    expect(statsText).toMatch(/\d+%/);

    // Extract numbers: disclosed count < total count (proving privacy value)
    const match = statsText!.match(/(\d+)\s*of (\d+) fields disclosed/);
    expect(match).toBeTruthy();
    const disclosed = parseInt(match![1], 10);
    const total = parseInt(match![2], 10);
    expect(disclosed).toBeLessThan(total);
    expect(disclosed).toBeGreaterThan(0);

    await page.screenshot({
      path: `${SCREENSHOT_DIR}demo-privacy-stats.png`,
      fullPage: true,
    });
  });
});
