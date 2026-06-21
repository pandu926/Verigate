import { test, expect, APIRequestContext } from '@playwright/test';

const BACKEND_URL = process.env.BACKEND_URL || 'http://localhost:3002';

test.describe('Case Lifecycle — Phase 2 Success Criteria', () => {
  let request: APIRequestContext;

  test.beforeAll(async ({ playwright }) => {
    request = await playwright.request.newContext({
      baseURL: BACKEND_URL,
    });
  });

  test.afterAll(async () => {
    await request.dispose();
  });

  test('Success Criteria 1 — Reviewer can create a case with all fields', async () => {
    const payload = {
      workflow_type: 'Onboarding',
      entity_type: 'Corporation',
      relationship_goal: 'supplier qualification',
      jurisdiction: 'US-DE',
      requested_outcome: 'approved_vendor',
    };

    const response = await request.post('/api/cases', { data: payload });

    expect(response.status()).toBe(201);

    const body = await response.json();
    const data = body.data;

    expect(data.id).toBeTruthy();
    expect(data.status).toBe('Created');
    expect(data.workflow_type).toBe('Onboarding');
    expect(data.entity_type).toBe('Corporation');
    expect(data.relationship_goal).toBe('supplier qualification');
    expect(data.jurisdiction).toBe('US-DE');
    expect(data.requested_outcome).toBe('approved_vendor');
    expect(data.created_at).toBeTruthy();
  });

  test('Success Criteria 2 — Case progresses through lifecycle with valid transitions enforced', async () => {
    // Create a case
    const createRes = await request.post('/api/cases', {
      data: {
        workflow_type: 'Onboarding',
        entity_type: 'Corporation',
        relationship_goal: 'full lifecycle test',
        jurisdiction: 'US',
        requested_outcome: 'approved',
      },
    });
    expect(createRes.status()).toBe(201);
    const caseId = (await createRes.json()).data.id;

    // Transition through the full happy path
    const transitions = ['Discovery', 'Collecting', 'Verifying', 'Assessing', 'Review', 'Approved'];

    for (const target of transitions) {
      const transRes = await request.post(`/api/cases/${caseId}/transitions`, {
        data: {
          target_status: target,
          actor_type: 'Reviewer',
          actor_id: 'reviewer-e2e',
          reason: `transition to ${target}`,
        },
      });

      expect(transRes.status()).toBe(200);
      const transBody = await transRes.json();
      expect(transBody.data.case.status).toBe(target);
    }

    // Verify invalid transition is rejected: create a fresh case and try created -> approved
    const createRes2 = await request.post('/api/cases', {
      data: {
        workflow_type: 'DueDiligence',
        entity_type: 'Individual',
        relationship_goal: 'invalid transition test',
      },
    });
    expect(createRes2.status()).toBe(201);
    const caseId2 = (await createRes2.json()).data.id;

    const invalidRes = await request.post(`/api/cases/${caseId2}/transitions`, {
      data: {
        target_status: 'Approved',
        actor_type: 'Reviewer',
        actor_id: 'reviewer-e2e',
        reason: 'attempt invalid jump',
      },
    });

    expect(invalidRes.status()).toBe(409);
    const errorBody = await invalidRes.json();
    expect(errorBody.error).toContain('Invalid state transition');
    expect(errorBody.data.allowed_transitions).toBeTruthy();
  });

  test('Success Criteria 3 — Every state transition emits event in timeline', async () => {
    // Create case
    const createRes = await request.post('/api/cases', {
      data: {
        workflow_type: 'Compliance',
        entity_type: 'Fund',
        relationship_goal: 'timeline event verification',
        jurisdiction: 'EU',
      },
    });
    expect(createRes.status()).toBe(201);
    const caseId = (await createRes.json()).data.id;

    // Transition through 3 states
    const steps = ['Discovery', 'Collecting', 'Verifying'];
    for (const step of steps) {
      const res = await request.post(`/api/cases/${caseId}/transitions`, {
        data: {
          target_status: step,
          actor_type: 'Reviewer',
          actor_id: 'rev-timeline',
          reason: `testing timeline for ${step}`,
        },
      });
      expect(res.status()).toBe(200);
    }

    // Fetch timeline
    const timelineRes = await request.get(`/api/cases/${caseId}/timeline`);
    expect(timelineRes.status()).toBe(200);

    const timeline = await timelineRes.json();
    const events = timeline.data;

    // 1 case_created + 3 transitions = 4 events
    expect(events.length).toBe(4);

    // Verify each event has required fields
    for (const event of events) {
      expect(event.created_at).toBeTruthy();
      expect(event.actor_type).toBeTruthy();
      expect(event.actor_id).toBeTruthy();
      expect(event.action).toBeTruthy();
    }

    // Verify transition events have from_status and to_status in details
    const transitionEvents = events.filter(
      (e: { action: string }) => e.action === 'state_transition'
    );
    expect(transitionEvents.length).toBe(3);

    for (const te of transitionEvents) {
      expect(te.details).toBeTruthy();
      expect(te.details.from_status).toBeTruthy();
      expect(te.details.to_status).toBeTruthy();
    }
  });

  test('Success Criteria 4 — Audit log is append-only', async () => {
    // Create case and do a transition to generate audit events
    const createRes = await request.post('/api/cases', {
      data: {
        workflow_type: 'Revalidation',
        entity_type: 'Trust',
        relationship_goal: 'immutability test',
      },
    });
    expect(createRes.status()).toBe(201);
    const caseId = (await createRes.json()).data.id;

    await request.post(`/api/cases/${caseId}/transitions`, {
      data: {
        target_status: 'Discovery',
        actor_type: 'System',
        actor_id: 'sys-immutability',
        reason: 'test immutability',
      },
    });

    // Capture timeline snapshot
    const timeline1Res = await request.get(`/api/cases/${caseId}/timeline`);
    expect(timeline1Res.status()).toBe(200);
    const snapshot1 = await timeline1Res.json();
    const events1 = snapshot1.data;
    expect(events1.length).toBeGreaterThan(0);

    // Verify no PUT/PATCH/DELETE endpoints exist for timeline
    const deleteRes = await request.delete(`/api/cases/${caseId}/timeline`);
    expect([404, 405].includes(deleteRes.status())).toBeTruthy();

    const putRes = await request.put(`/api/cases/${caseId}/timeline`, {
      data: { action: 'tampered' },
    });
    expect([404, 405].includes(putRes.status())).toBeTruthy();

    const patchRes = await request.patch(`/api/cases/${caseId}/timeline`, {
      data: { action: 'tampered' },
    });
    expect([404, 405].includes(patchRes.status())).toBeTruthy();

    // Verify timeline is unchanged after mutation attempts
    const timeline2Res = await request.get(`/api/cases/${caseId}/timeline`);
    expect(timeline2Res.status()).toBe(200);
    const snapshot2 = await timeline2Res.json();
    const events2 = snapshot2.data;

    // Same number of events, same IDs and timestamps
    expect(events2.length).toBe(events1.length);
    for (let i = 0; i < events1.length; i++) {
      expect(events2[i].id).toBe(events1[i].id);
      expect(events2[i].created_at).toBe(events1[i].created_at);
      expect(events2[i].action).toBe(events1[i].action);
      expect(events2[i].actor_id).toBe(events1[i].actor_id);
    }
  });

  test('Timeline supports cursor-based pagination', async () => {
    // Create case and transition through all states to approved (6 transitions + 1 create = 7 events)
    const createRes = await request.post('/api/cases', {
      data: {
        workflow_type: 'Onboarding',
        entity_type: 'Corporation',
        relationship_goal: 'pagination test',
        jurisdiction: 'SG',
        requested_outcome: 'full_approval',
      },
    });
    expect(createRes.status()).toBe(201);
    const caseId = (await createRes.json()).data.id;

    const steps = ['Discovery', 'Collecting', 'Verifying', 'Assessing', 'Review', 'Approved'];
    for (const step of steps) {
      const res = await request.post(`/api/cases/${caseId}/transitions`, {
        data: {
          target_status: step,
          actor_type: 'Reviewer',
          actor_id: 'rev-pagination',
          reason: 'pagination test',
        },
      });
      expect(res.status()).toBe(200);
    }

    // Fetch first page with limit=3
    const page1Res = await request.get(`/api/cases/${caseId}/timeline?limit=3`);
    expect(page1Res.status()).toBe(200);
    const page1 = await page1Res.json();

    expect(page1.data.length).toBe(3);
    expect(page1.meta.has_more).toBe(true);
    expect(page1.meta.next_cursor).toBeTruthy();

    // Fetch subsequent pages and collect all event IDs
    const allEventIds: string[] = page1.data.map((e: { id: string }) => e.id);
    let cursor = page1.meta.next_cursor;

    while (cursor) {
      const pageRes = await request.get(
        `/api/cases/${caseId}/timeline?limit=3&cursor=${encodeURIComponent(cursor)}`
      );
      expect(pageRes.status()).toBe(200);
      const page = await pageRes.json();

      for (const event of page.data) {
        allEventIds.push(event.id);
      }

      cursor = page.meta.has_more ? page.meta.next_cursor : null;
    }

    // Should have all 7 events with no duplicates
    expect(allEventIds.length).toBe(7);
    const uniqueIds = new Set(allEventIds);
    expect(uniqueIds.size).toBe(7);
  });
});
