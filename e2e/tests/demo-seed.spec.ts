import { test, expect } from '@playwright/test';
import {
  getDemoCases,
  triggerSeed,
  getCaseTimeline,
  getHealth,
  DEMO_CASE_NAMES,
  EXPECTED_STATUSES,
  SEED_CASE_IDS,
} from '../fixtures/demo-helpers';

/**
 * Phase 10 E2E — Seed Data & Zero-Config Startup Verification
 *
 * Proves Success Criteria 1 and 2:
 *   SC1: Single command seeds database with realistic demo scenarios
 *   SC2: docker-compose up starts everything with no manual config
 */
test.describe('Phase 10 — Demo Seed & Startup', () => {

  test('SC1: seed data creates 3+ realistic demo cases', async ({ request }) => {
    const cases = await getDemoCases(request);

    // At least 3 cases from seed
    expect(cases.length).toBeGreaterThanOrEqual(3);

    // All expected entity names present
    for (const expectedName of DEMO_CASE_NAMES) {
      const found = cases.find((c) =>
        c.entity_name.toLowerCase().includes(expectedName.toLowerCase())
      );
      expect(found, `Expected case "${expectedName}" not found`).toBeTruthy();
    }

    // Each case has correct status
    for (const [name, expectedStatus] of Object.entries(EXPECTED_STATUSES)) {
      const caseItem = cases.find((c) =>
        c.entity_name.toLowerCase().includes(name.toLowerCase())
      );
      expect(caseItem, `Case "${name}" not found`).toBeTruthy();
      expect(caseItem!.status).toBe(expectedStatus);
    }
  });

  test('SC1: seeded cases have full audit trails', async ({ request }) => {
    // The approved case (Meridian Capital) should have a rich timeline
    const timeline = await getCaseTimeline(request, SEED_CASE_IDS.meridian);

    // Expect many events (the approved case has 18 seed events)
    expect(timeline.length).toBeGreaterThan(5);

    // Multiple actor types present (API returns PascalCase from Rust enum serialization)
    const actorTypes = new Set(timeline.map((e) => e.actor_type));

    // At minimum: System events exist (state transitions)
    expect(actorTypes.has('System')).toBe(true);

    // Should have at least 3 distinct actor types in full timeline
    expect(actorTypes.size).toBeGreaterThanOrEqual(3);

    // Multiple event types (action field)
    const eventTypes = new Set(timeline.map((e) => e.action));
    expect(eventTypes.size).toBeGreaterThan(3);
  });

  test('SC2: system starts with no manual configuration', async ({ request }) => {
    // Health endpoint should respond without any auth
    const health = await getHealth(request);

    expect(health.status).toBe('healthy');
    expect(health.database_connected).toBe(true);

    // Agent identity should be present (T3 integration)
    expect(health.agent_identity).toBeTruthy();
    const identity = health.agent_identity as Record<string, unknown>;
    expect(identity.agent_did).toBeTruthy();

    // Cases API should work with auth (confirms full stack functional)
    const cases = await getDemoCases(request);
    expect(Array.isArray(cases)).toBe(true);
    expect(cases.length).toBeGreaterThan(0);
  });

  test('SC1: explicit seed endpoint works for re-seeding', async ({ request }) => {
    // Call the seed endpoint
    const seedResult = await triggerSeed(request);
    expect(seedResult.status).toBe(200);

    // Verify cases still exist after re-seed
    const cases = await getDemoCases(request);
    expect(cases.length).toBeGreaterThanOrEqual(3);

    // All 3 expected cases should still be present
    for (const expectedName of DEMO_CASE_NAMES) {
      const found = cases.find((c) =>
        c.entity_name.toLowerCase().includes(expectedName.toLowerCase())
      );
      expect(found, `Expected case "${expectedName}" not found after re-seed`).toBeTruthy();
    }
  });
});
