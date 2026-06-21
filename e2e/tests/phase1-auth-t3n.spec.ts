import { test, expect } from '@playwright/test';

const API_BASE = 'http://localhost:3302';
const FRONTEND_BASE = 'http://localhost:3309';

test.describe('Phase 1: Auth + T3N Integration Flow', () => {

  test('protected routes redirect to login when unauthenticated', async ({ page }) => {
    await page.goto(`${FRONTEND_BASE}/dashboard`);
    await page.waitForURL('**/login');
    await expect(page).toHaveURL(/\/login/);
  });

  test('reviewer login → dashboard → create case → logout', async ({ page }) => {
    // Login as reviewer
    await page.goto(`${FRONTEND_BASE}/login`);
    await page.fill('input[name="email"]', 'reviewer@verigate.io');
    await page.fill('input[name="password"]', 'reviewer123');
    await page.click('button[type="submit"]');
    await page.waitForURL('**/dashboard');
    await expect(page).toHaveURL(/\/dashboard/);
    await page.screenshot({ path: 'screenshots/phase1/01-reviewer-dashboard.png' });

    // Create a new case
    await page.click('text=New Case');
    await page.waitForTimeout(500);
    await page.fill('input[placeholder*="Meridian"]', 'Playwright Test Corp');
    await page.click('button:has-text("Create Case")');
    // Wait for modal to close and list to refresh
    await page.waitForTimeout(3000);
    await page.screenshot({ path: 'screenshots/phase1/02-case-created.png' });

    // Logout
    await page.click('[data-testid="logout-btn"]');
    await page.waitForURL('**/login');
    await expect(page).toHaveURL(/\/login/);
  });

  test('counterparty login → portal (not dashboard)', async ({ page }) => {
    await page.goto(`${FRONTEND_BASE}/login`);
    await page.fill('input[name="email"]', 'counterparty@verigate.io');
    await page.fill('input[name="password"]', 'counterparty123');
    await page.click('button[type="submit"]');
    await page.waitForURL('**/portal');
    await expect(page).toHaveURL(/\/portal/);
    await page.screenshot({ path: 'screenshots/phase1/03-counterparty-portal.png' });
  });

  test('counterparty cannot access /dashboard', async ({ page }) => {
    await page.goto(`${FRONTEND_BASE}/login`);
    await page.fill('input[name="email"]', 'counterparty@verigate.io');
    await page.fill('input[name="password"]', 'counterparty123');
    await page.click('button[type="submit"]');
    await page.waitForURL('**/portal');

    // Try navigating to dashboard directly
    await page.goto(`${FRONTEND_BASE}/dashboard`);
    // Should be redirected away (to portal)
    await expect(page).not.toHaveURL(/\/dashboard/);
  });

  test('invalid credentials → error message shown', async ({ page }) => {
    await page.goto(`${FRONTEND_BASE}/login`);
    await page.fill('input[name="email"]', 'reviewer@verigate.io');
    await page.fill('input[name="password"]', 'wrongpassword');
    await page.click('button[type="submit"]');
    await page.waitForTimeout(1000);
    await expect(page.locator('.login-error')).toBeVisible();
    await expect(page.locator('.login-error')).toContainText('Invalid credentials');
    await expect(page).toHaveURL(/\/login/);
  });

  test('T3N integration: submission includes execution_id', async ({ request }) => {
    // Login as reviewer and create a case
    const loginRes = await request.post(`${API_BASE}/api/auth/login`, {
      data: { email: 'reviewer@verigate.io', password: 'reviewer123' },
    });
    const { token: reviewerToken } = await loginRes.json();

    // Create case
    const caseRes = await request.post(`${API_BASE}/api/cases`, {
      headers: { 'Authorization': `Bearer ${reviewerToken}` },
      data: {
        entity_name: 'T3N Integration Test',
        entity_type: 'Corporation',
        workflow_type: 'Onboarding',
        jurisdiction: 'Singapore',
        relationship_goal: 'Investment Partnership',
      },
    });
    const caseData = await caseRes.json();
    const caseId = caseData.data?.id || caseData.id;

    // Transition to collecting
    await request.post(`${API_BASE}/api/cases/${caseId}/transitions`, {
      headers: { 'Authorization': `Bearer ${reviewerToken}` },
      data: { target_status: 'Discovery', actor_type: 'Reviewer', actor_id: 'reviewer-001', reason: 'test' },
    });
    await request.post(`${API_BASE}/api/cases/${caseId}/transitions`, {
      headers: { 'Authorization': `Bearer ${reviewerToken}` },
      data: { target_status: 'Collecting', actor_type: 'Reviewer', actor_id: 'reviewer-001', reason: 'test' },
    });

    // Login as counterparty and submit VP
    const cpLogin = await request.post(`${API_BASE}/api/auth/login`, {
      data: { email: 'counterparty@verigate.io', password: 'counterparty123' },
    });
    const { token: cpToken } = await cpLogin.json();

    // Generate test VP
    const vpRes = await request.get(`${API_BASE}/api/test/generate-vp?type=entity`);
    const vpWrapper = await vpRes.json();
    const vp = vpWrapper.data.vp;

    // Submit VP
    const submitRes = await request.post(`${API_BASE}/api/cases/${caseId}/submissions`, {
      headers: { 'Authorization': `Bearer ${cpToken}` },
      data: {
        credential_type: 'entity',
        requirement_claim_type: 'incorporation_certificate',
        raw_vp: vp,
      },
    });
    const submitData = await submitRes.json();

    // Assert T3N execution_id is present
    expect(submitData.data.t3n_execution_id).toBeTruthy();
    expect(submitData.data.t3n_execution_id).toContain('t3n-exec-');
    expect(submitData.data.status).toBe('Verified');
  });

  test('T3N bridge health shows all endpoints', async ({ request }) => {
    const res = await request.get('http://localhost:3310/health');
    const data = await res.json();
    expect(data.status).toBe('healthy');
    expect(data.authenticated).toBe(true);
    expect(data.endpoints.length).toBeGreaterThanOrEqual(7);
  });

  test('T3N audit push receives events', async ({ request }) => {
    const res = await request.post('http://localhost:3310/audit/push', {
      data: {
        event_type: 'test_event',
        case_id: 'test-case-e2e',
        actor_did: 'did:t3n:ede53f4ac2149d9c6e663e47d5b5727ccd851e80',
        action: 'e2e_test',
        details: { test: true },
      },
    });
    const data = await res.json();
    expect(data.pushed).toBe(true);
    expect(data.t3n_event_id).toContain('t3n-audit-');
  });
});
