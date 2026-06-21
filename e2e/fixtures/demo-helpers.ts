import { APIRequestContext } from '@playwright/test';
import * as jwt from 'jsonwebtoken';

/**
 * Shared test utilities for demo E2E tests.
 * Provides JWT generation, expected seed data constants, and API helpers.
 */

// Matches JWT_SECRET in docker-compose.yml for demo environment
const DEV_JWT_SECRET = process.env.JWT_SECRET || 'demo-jwt-secret-not-for-production';
const BACKEND_URL = process.env.BACKEND_URL || 'http://localhost:3002';

/** Screenshot output directory for demo tests (relative to e2e/ working dir). */
export const SCREENSHOT_DIR = 'screenshots/demo/';

/** Expected entity names from seed data. */
export const DEMO_CASE_NAMES = [
  'Meridian Capital Partners',
  'Atlas Protocol Foundation',
  'Nightfall Trading Ltd',
] as const;

/** Expected statuses for each seeded entity. */
export const EXPECTED_STATUSES: Record<string, string> = {
  'Meridian Capital Partners': 'approved',
  'Atlas Protocol Foundation': 'collecting',
  'Nightfall Trading Ltd': 'blocked',
};

/** Deterministic case IDs from the seed script. */
export const SEED_CASE_IDS = {
  meridian: 'a0000001-0000-0000-0000-000000000001',
  atlas: 'b0000002-0000-0000-0000-000000000002',
  nightfall: 'c0000003-0000-0000-0000-000000000003',
} as const;

/** Generate a reviewer JWT token for API calls. */
export function generateDemoJwt(): string {
  const now = Math.floor(Date.now() / 1000);
  return jwt.sign(
    { sub: 'demo-reviewer', role: 'reviewer', iat: now, exp: now + 3600 },
    DEV_JWT_SECRET,
    { algorithm: 'HS256' }
  );
}

/** DemoCase shape returned from GET /api/cases (mapped for tests). */
export interface DemoCaseResponse {
  id: string;
  entity_name: string;
  workflow_type: string;
  status: string;
  entity_type: string;
  jurisdiction: string;
  relationship_goal: string;
  created_at: string;
  updated_at: string;
}

/** Fetch all demo cases from the backend with auth. */
export async function getDemoCases(request: APIRequestContext): Promise<DemoCaseResponse[]> {
  const token = generateDemoJwt();
  const response = await request.get(`${BACKEND_URL}/api/cases`, {
    headers: { Authorization: `Bearer ${token}` },
  });

  if (!response.ok()) {
    throw new Error(`Failed to fetch cases: ${response.status()} ${response.statusText()}`);
  }

  const body = await response.json();
  // Backend may wrap in { data: [...] } or return array directly
  const raw: Array<Record<string, unknown>> = Array.isArray(body) ? body : (body.data ?? []);

  // Map relationship_goal to entity_name (backend schema uses relationship_goal for entity names)
  return raw.map((c) => ({
    id: String(c.id ?? ''),
    entity_name: String(c.entity_name ?? c.relationship_goal ?? 'Unknown Entity'),
    workflow_type: String(c.workflow_type ?? 'onboarding'),
    status: String(c.status ?? 'created').toLowerCase(),
    entity_type: String(c.entity_type ?? 'corporation').toLowerCase(),
    jurisdiction: String(c.jurisdiction ?? '--'),
    relationship_goal: String(c.relationship_goal ?? ''),
    created_at: String(c.created_at ?? ''),
    updated_at: String(c.updated_at ?? ''),
  }));
}

/** Trigger database seeding via POST /api/seed. */
export async function triggerSeed(request: APIRequestContext): Promise<{ status: number; body: unknown }> {
  const response = await request.post(`${BACKEND_URL}/api/seed`);
  const body = await response.json();
  return { status: response.status(), body };
}

/** Timeline event shape from GET /api/cases/:id/timeline. */
export interface TimelineEvent {
  id: string;
  case_id: string;
  event_type: string;
  actor_type: string;
  actor_id: string;
  details: Record<string, unknown>;
  created_at: string;
}

/** Fetch timeline events for a case. */
export async function getCaseTimeline(
  request: APIRequestContext,
  caseId: string
): Promise<TimelineEvent[]> {
  const token = generateDemoJwt();
  const response = await request.get(`${BACKEND_URL}/api/cases/${caseId}/timeline?limit=100`, {
    headers: { Authorization: `Bearer ${token}` },
  });

  if (!response.ok()) {
    throw new Error(`Failed to fetch timeline: ${response.status()} ${response.statusText()}`);
  }

  const body = await response.json();
  return body.data ?? [];
}

/** Fetch backend health check (no auth). */
export async function getHealth(request: APIRequestContext): Promise<Record<string, unknown>> {
  const response = await request.get(`${BACKEND_URL}/api/health`);

  if (!response.ok()) {
    throw new Error(`Health check failed: ${response.status()}`);
  }

  return await response.json();
}
