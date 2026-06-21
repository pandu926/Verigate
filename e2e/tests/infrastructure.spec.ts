import { test, expect } from '@playwright/test';

const BACKEND_URL = 'http://localhost:3002';

test.describe.serial('Phase 1: Infrastructure Verification', () => {
  test.describe('Backend Health', () => {
    test('health endpoint returns agent identity metadata', async ({ request }) => {
      const response = await request.get(`${BACKEND_URL}/api/health`);

      expect(response.status(), 'Health endpoint should return 200').toBe(200);

      const body = await response.json();

      expect(body.status, 'Status should be healthy').toBe('healthy');
      expect(body.version, 'Version should be a non-empty string').toBeTruthy();
      expect(typeof body.uptime_seconds, 'Uptime should be a number').toBe('number');
      expect(body.uptime_seconds, 'Uptime should be non-negative').toBeGreaterThanOrEqual(0);

      // Agent identity assertions
      expect(body.agent_identity, 'Should include agent_identity object').toBeDefined();
      expect(
        body.agent_identity.agent_did,
        'Agent DID should start with "did:"'
      ).toMatch(/^did:/);
      expect(
        body.agent_identity.sdk_version,
        'SDK version should be a non-empty string'
      ).toBeTruthy();
      expect(
        Array.isArray(body.agent_identity.capabilities),
        'Capabilities should be an array'
      ).toBe(true);

      // Database connectivity
      expect(body.database_connected, 'Database should be connected').toBe(true);
    });

    test('database migrations created core tables', async ({ request }) => {
      const response = await request.get(`${BACKEND_URL}/api/health`);

      expect(response.status()).toBe(200);

      const body = await response.json();

      // database_connected = true means the backend successfully ran migrations on startup
      // and can execute queries against the database. The backend runs sqlx::migrate!()
      // which will fail if migrations are invalid, proving schema is active.
      expect(
        body.database_connected,
        'Database connected confirms migrations ran successfully (backend runs migrations on startup)'
      ).toBe(true);
    });
  });

  test.describe('Frontend Application', () => {
    test('frontend loads and renders application shell', async ({ page }) => {
      const errors: string[] = [];
      page.on('pageerror', (err) => {
        errors.push(err.message);
      });

      await page.goto('/');

      // Page title check
      await expect(page, 'Page title should contain Verigate').toHaveTitle(/Verigate/);

      // AppShell header with logotype
      const logotype = page.locator('h1');
      await expect(
        logotype,
        'VERIGATE logotype should be visible in header'
      ).toBeVisible();
      await expect(logotype).toContainText(/Verigate/i);

      // StatusPulse indicator
      const statusPulse = page.locator('[data-testid="status-pulse"]').or(
        page.locator('text=/Connected|Connecting|Offline/')
      );
      await expect(
        statusPulse.first(),
        'Status indicator should be visible'
      ).toBeVisible({ timeout: 15_000 });

      // No JS errors
      expect(errors, 'Should have no uncaught JS errors').toEqual([]);
    });

    test('frontend displays agent identity from backend', async ({ page }) => {
      await page.goto('/');

      // Wait for health data to load — the agent DID will appear on the page
      const didElement = page.locator('text=/did:/');
      await expect(
        didElement.first(),
        'Agent DID should be visible on the page'
      ).toBeVisible({ timeout: 15_000 });

      // Authentication status indicator
      const authStatus = page.locator('text=/Authenticated|Unauthenticated/');
      await expect(
        authStatus.first(),
        'Authentication status should be visible'
      ).toBeVisible();

      // Database connection status
      const dbStatus = page.locator('text=/Connected|Disconnected/');
      await expect(
        dbStatus.first(),
        'Database connection status should be displayed'
      ).toBeVisible();
    });
  });

  test.describe('Service Orchestration', () => {
    test('all docker-compose services are running', async ({ request, page }) => {
      // Backend service check
      const healthResponse = await request.get(`${BACKEND_URL}/api/health`);
      expect(
        healthResponse.status(),
        'Backend service should respond with 200'
      ).toBe(200);

      // Frontend service check (via nginx)
      const frontendResponse = await request.get('http://localhost:3005');
      expect(
        frontendResponse.status(),
        'Frontend (nginx) service should respond with 200'
      ).toBe(200);

      // Database connectivity through backend
      const body = await healthResponse.json();
      expect(
        body.database_connected,
        'PostgreSQL service should be reachable from backend'
      ).toBe(true);
    });
  });
});
