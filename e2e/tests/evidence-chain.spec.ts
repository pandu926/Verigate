import { test, expect } from '@playwright/test';

const BASE_URL = process.env.BASE_URL || 'http://verigate.rbexp.com';
const BRIDGE_URL = process.env.BRIDGE_URL || 'http://verigate.rbexp.com/bridge';

test.describe.serial('Verigate — Live E2E Verification', () => {
  test('frontend loads correctly', async ({ page }) => {
    await page.goto(BASE_URL);
    await expect(page).toHaveTitle(/Verigate/i, { timeout: 10000 });
    await expect(page.locator('body')).toBeVisible();
  });

  test('backend API health check', async ({ request }) => {
    const res = await request.get(`${BASE_URL}/api/health`);
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(body.status).toBe('healthy');
    expect(body.agent_identity).toBeDefined();
    expect(body.agent_identity.agent_did).toMatch(/^did:/);
    expect(body.database_connected).toBe(true);
  });

  test('T3N bridge health check', async ({ request }) => {
    const res = await request.get(`${BRIDGE_URL}/health`);
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(body.status).toBe('healthy');
    expect(body.authenticated).toBe(true);
    expect(body.did).toMatch(/^did:t3n:/);
    expect(body.contract_tail).toBe('verigate');
  });

  test('evidence chain page loads', async ({ page }) => {
    await page.goto(`${BASE_URL}/evidence`);
    await expect(page.locator('.evidence-chain__title')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('.scenario-selector')).toBeVisible();
    await expect(page.locator('.scenario-card')).toHaveCount(3);
  });

  test('evidence chain — good entity scenario (live T3N)', async ({ page }) => {
    test.setTimeout(120000);
    await page.goto(`${BASE_URL}/evidence`);
    await expect(page.locator('.scenario-card').first()).toBeVisible({ timeout: 5000 });

    // Click "Good Entity"
    await page.locator('.scenario-card').first().click();

    // Should show loading
    await expect(page.locator('.evidence-chain__loading')).toBeVisible({ timeout: 3000 });

    // Wait for pipeline to complete (up to 90s — T3N testnet can be slow)
    await expect(page.locator('.decision-badge')).toBeVisible({ timeout: 90000 });

    // Verify delegation card
    await expect(page.locator('.delegation-card--create')).toBeVisible();
    await expect(page.locator('.delegation-card--create')).toContainText('Delegation Created');
    await expect(page.locator('.delegation-card--create')).toContainText('EIP-191');

    // Verify timeline steps exist (commit-plan + verify-credentials + assess-risk + decide)
    const stepCount = await page.locator('.timeline-step').count();
    expect(stepCount).toBeGreaterThanOrEqual(4);

    // Verify AI assessment step
    const assessStep = page.locator('.timeline-step').filter({ hasText: 'AI Risk Assessment' });
    await expect(assessStep).toBeVisible();
    await expect(assessStep.locator('.timeline-step__reasoning')).toBeVisible();
    await expect(assessStep.locator('.timeline-step__badge--tee')).toContainText('TEE');

    // Verify decision badge shows APPROVED
    await expect(page.locator('.decision-badge__label')).toContainText('APPROVED');

    // Verify delegation revoked
    await expect(page.locator('.delegation-card--revoke')).toBeVisible();
    await expect(page.locator('.delegation-card--revoke')).toContainText('Delegation Revoked');

    // Screenshot for evidence
    await page.screenshot({ path: 'e2e/screenshots/evidence-chain-approved.png', fullPage: true });
  });

  test('delegation status endpoint works', async ({ request }) => {
    // Create delegation
    const createRes = await request.post(`${BRIDGE_URL}/delegation/create`, {
      data: {
        case_id: `e2e-deleg-${Date.now()}`,
        functions: ['verify-credential', 'assess-risk'],
        ttl_secs: 60,
      },
    });
    expect(createRes.status()).toBe(200);
    const created = await createRes.json();
    expect(created.delegation_created).toBe(true);
    expect(created.vc_id).toBeTruthy();
    expect(created.counterparty_did).toMatch(/^did:t3n:/);
    expect(created.signature.type).toBe('EIP-191');
  });

  test('scenario endpoint — sanctioned entity returns blocked', async ({ request }) => {
    test.setTimeout(120000);
    // Wait for any rate limit from previous pipeline test
    await new Promise(r => setTimeout(r, 60000));
    const res = await request.post(`${BRIDGE_URL}/scenarios/run`, {
      data: { scenario: 'sanctioned' },
    });
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(body.success).toBe(true);
    expect(body.decision).toBe('blocked');
    expect(body.confidence).toBeGreaterThanOrEqual(0.9);
    expect(body.delegation).toBeDefined();
    expect(body.delegation.lifecycle).toContain('revoke');
  });
});
