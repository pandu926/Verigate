import { test, expect, APIRequestContext } from '@playwright/test';
import {
  seedPortalCase,
  submitProofViaApi,
  VALID_VP_JSON,
  MALFORMED_JSON,
  INVALID_VP_JSON,
} from '../fixtures/portal-seed';

const BACKEND_URL = process.env.BACKEND_URL || 'http://localhost:3002';

test.describe('Phase 9 — Counterparty Portal E2E', () => {
  let apiCtx: APIRequestContext;

  test.beforeAll(async ({ playwright }) => {
    apiCtx = await playwright.request.newContext({ baseURL: BACKEND_URL });
  });

  test.afterAll(async () => {
    await apiCtx.dispose();
  });

  // ============================================================================
  // SUCCESS CRITERIA 1: Step-by-step checklist tailored to case
  // ============================================================================

  test('SC1: counterparty sees step-by-step checklist tailored to case', async ({ page }) => {
    const { caseId } = await seedPortalCase(apiCtx);

    await page.goto(`/portal/${caseId}`);

    // Wait for loading to finish — checklist container should appear
    const checklist = page.locator('.proof-checklist');
    await expect(checklist).toBeVisible({ timeout: 15000 });

    // Verify multiple requirement steps are rendered
    const steps = checklist.locator('.proof-checklist__step');
    const stepCount = await steps.count();
    expect(stepCount).toBeGreaterThanOrEqual(2);

    // Each step should have: description text, status badge, claim type
    const firstStep = steps.first();
    await expect(firstStep.locator('.proof-checklist__type')).toBeVisible();
    await expect(firstStep.locator('.proof-checklist__description')).toBeVisible();
    await expect(firstStep.locator('.proof-checklist__badge')).toBeVisible();

    // Verify journey format: one step should be "current" (highlighted)
    const currentCards = checklist.locator('.proof-checklist__card--current');
    await expect(currentCards.first()).toBeVisible();

    // Future pending steps should exist (dimmed styling via CSS class)
    const pendingCards = checklist.locator('.proof-checklist__card--pending');
    const pendingCount = await pendingCards.count();
    // If there are multiple requirements, at least one should be pending
    if (stepCount > 1) {
      expect(pendingCount).toBeGreaterThanOrEqual(1);
    }

    // Verify "Submit Proof" action button on the current step
    const actionBtn = checklist.locator('.proof-checklist__action');
    await expect(actionBtn.first()).toBeVisible();
    await expect(actionBtn.first()).toHaveText(/Submit Proof/);

    // Screenshot: initial checklist state
    await page.screenshot({ path: 'e2e/test-results/portal-checklist-initial.png', fullPage: true });
  });

  // ============================================================================
  // SUCCESS CRITERIA 2: Submit proof with real-time format validation
  // ============================================================================

  test('SC2: submission interface with real-time format validation', async ({ page }) => {
    const { caseId } = await seedPortalCase(apiCtx);

    await page.goto(`/portal/${caseId}`);

    // Wait for checklist
    const checklist = page.locator('.proof-checklist');
    await expect(checklist).toBeVisible({ timeout: 15000 });

    // Click "Submit Proof" on the current step
    const actionBtn = checklist.locator('.proof-checklist__action').first();
    await actionBtn.click();

    // Verify submission card expands — textarea should be visible
    const submissionCard = page.locator('.submission-card--expanded');
    await expect(submissionCard).toBeVisible({ timeout: 5000 });

    const textarea = submissionCard.locator('textarea');
    await expect(textarea).toBeVisible();

    // Type INVALID JSON → verify error appears
    await textarea.fill(MALFORMED_JSON);
    // Wait for debounced validation (300ms + buffer)
    await page.waitForTimeout(500);
    const errorMsg = submissionCard.locator('.submission-card__error');
    await expect(errorMsg).toBeVisible();
    await expect(errorMsg).toContainText('Invalid JSON');

    // Verify textarea has error styling
    const textareaWithError = submissionCard.locator('.submission-card__textarea--error');
    await expect(textareaWithError).toBeVisible();

    // Clear and type structurally invalid VP (valid JSON, not VP)
    await textarea.fill(INVALID_VP_JSON);
    await page.waitForTimeout(500);
    await expect(errorMsg).toBeVisible();
    await expect(errorMsg).toContainText('Missing VP structure');

    // Clear and paste VALID VP JSON → error clears, disclosure preview shows
    await textarea.fill(JSON.stringify(VALID_VP_JSON));
    await page.waitForTimeout(500);

    // Error should be gone
    await expect(submissionCard.locator('.submission-card__error')).not.toBeVisible();

    // Textarea should have valid styling
    const textareaValid = submissionCard.locator('.submission-card__textarea--valid');
    await expect(textareaValid).toBeVisible();

    // Disclosure preview should show field chips
    const disclosureSection = submissionCard.locator('.submission-card__disclosure');
    await expect(disclosureSection).toBeVisible();
    await expect(disclosureSection).toContainText('Fields to be disclosed');

    // Chips should contain known field names from our VP
    const chips = disclosureSection.locator('.submission-card__chip--disclosed');
    const chipCount = await chips.count();
    expect(chipCount).toBeGreaterThan(0);

    // Screenshot: submission card with valid VP
    await page.screenshot({ path: 'e2e/test-results/portal-submission-valid.png', fullPage: true });
  });

  // ============================================================================
  // SUCCESS CRITERIA 3: Progress tracking updates
  // ============================================================================

  test('SC3: progress tracking updates after proof submission', async ({ page }) => {
    const { caseId, counterpartyToken } = await seedPortalCase(apiCtx);

    // Submit a proof via API to create verified state
    await submitProofViaApi(apiCtx, caseId, counterpartyToken, 'entity');

    // Navigate to portal
    await page.goto(`/portal/${caseId}`);

    // Wait for progress ring to render
    const progressRing = page.locator('.progress-ring');
    await expect(progressRing).toBeVisible({ timeout: 15000 });

    // Progress percentage should be > 0%
    const percentageText = progressRing.locator('.progress-ring__percentage');
    await expect(percentageText).toBeVisible();
    const pctValue = await percentageText.textContent();
    expect(pctValue).toBeTruthy();
    // Strip % and check it's not 0
    const pctNum = parseInt(pctValue!.replace('%', ''), 10);
    expect(pctNum).toBeGreaterThan(0);

    // Fraction text should show verified count
    const fractionText = progressRing.locator('.progress-ring__fraction');
    await expect(fractionText).toBeVisible();
    const fraction = await fractionText.textContent();
    expect(fraction).toMatch(/\d+ of \d+ verified/);
    // At least 1 verified
    expect(fraction).not.toBe('0 of 0 verified');

    // Category breakdown should show at least one category with status
    const categoryBreakdown = page.locator('.category-breakdown');
    await expect(categoryBreakdown).toBeVisible();
    const categoryItems = categoryBreakdown.locator('.category-breakdown__item');
    const catCount = await categoryItems.count();
    expect(catCount).toBeGreaterThanOrEqual(1);

    // Screenshot: progress after submission
    await page.screenshot({ path: 'e2e/test-results/portal-progress-updated.png', fullPage: true });
  });

  // ============================================================================
  // SUCCESS CRITERIA 4: Disclosure score displays privacy metric
  // ============================================================================

  test('SC4: disclosure score shows minimal privacy preservation metric', async ({ page }) => {
    const { caseId, counterpartyToken } = await seedPortalCase(apiCtx);

    // Submit a proof to have disclosure data
    await submitProofViaApi(apiCtx, caseId, counterpartyToken, 'entity');

    // Navigate to portal
    await page.goto(`/portal/${caseId}`);

    // Wait for disclosure score component
    const disclosureScore = page.locator('.disclosure-score');
    await expect(disclosureScore).toBeVisible({ timeout: 15000 });

    // Should show "Minimal Disclosure Score" label
    const label = disclosureScore.locator('.disclosure-score__label');
    await expect(label).toBeVisible();
    await expect(label).toContainText('Minimal Disclosure Score');

    // Should show "X of Y fields disclosed"
    const ratio = disclosureScore.locator('.disclosure-score__ratio');
    await expect(ratio).toBeVisible();
    const ratioText = await ratio.textContent();
    expect(ratioText).toMatch(/\d+ of \d+ fields disclosed/);

    // Should show percentage
    const percentage = disclosureScore.locator('.disclosure-score__percentage');
    await expect(percentage).toBeVisible();

    // Privacy shield SVG should be present
    const shield = disclosureScore.locator('.disclosure-score__shield');
    await expect(shield).toBeVisible();

    // "Lower is better" hint text
    const hint = disclosureScore.locator('.disclosure-score__hint');
    await expect(hint).toBeVisible();
    await expect(hint).toContainText('Lower is better');

    // Screenshot: disclosure score
    await page.screenshot({ path: 'e2e/test-results/portal-disclosure-score.png', fullPage: true });
  });
});
