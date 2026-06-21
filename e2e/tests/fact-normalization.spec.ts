import { test, expect, APIRequestContext } from '@playwright/test';
import * as jwt from 'jsonwebtoken';
import * as fs from 'fs';
import * as path from 'path';

const BACKEND_URL = process.env.BACKEND_URL || 'http://localhost:3002';
const JWT_SECRET = 'dev-secret-do-not-use-in-prod';

// =============================================================================
// Helpers
// =============================================================================

function generateCounterpartyToken(): string {
  const now = Math.floor(Date.now() / 1000);
  return jwt.sign(
    { sub: 'test-counterparty-001', role: 'counterparty', iat: now, exp: now + 900 },
    JWT_SECRET,
    { algorithm: 'HS256' }
  );
}

function generateReviewerToken(): string {
  const now = Math.floor(Date.now() / 1000);
  return jwt.sign(
    { sub: 'test-reviewer-001', role: 'reviewer', iat: now, exp: now + 900 },
    JWT_SECRET,
    { algorithm: 'HS256' }
  );
}

interface VpResponse {
  data: {
    vp: Record<string, unknown>;
    issuer_did: string;
    credential_type: string;
  };
}

async function generateTestVp(
  request: APIRequestContext,
  credentialType: string,
  options: { sdJwt?: boolean } = {}
): Promise<Record<string, unknown>> {
  const params = new URLSearchParams({ type: credentialType });
  if (options.sdJwt) params.set('sd_jwt', 'true');

  const response = await request.get(`/api/test/generate-vp?${params.toString()}`);
  expect(response.status()).toBe(200);
  const body: VpResponse = await response.json();
  return body.data.vp;
}

async function createTestCase(request: APIRequestContext, reviewerToken: string): Promise<string> {
  const createRes = await request.post('/api/cases', {
    headers: {
      Authorization: `Bearer ${reviewerToken}`,
      'Content-Type': 'application/json',
    },
    data: {
      workflow_type: 'Onboarding',
      entity_type: 'Corporation',
      relationship_goal: 'phase5_e2e_test',
    },
  });
  expect(createRes.status()).toBe(201);
  const caseId = (await createRes.json()).data.id;

  // Transition Created -> Discovery
  const discRes = await request.post(`/api/cases/${caseId}/transitions`, {
    headers: {
      Authorization: `Bearer ${reviewerToken}`,
      'Content-Type': 'application/json',
    },
    data: {
      target_status: 'Discovery',
      actor_type: 'Reviewer',
      actor_id: 'test-reviewer-001',
      reason: 'Moving to discovery',
    },
  });
  expect(discRes.status()).toBe(200);

  // Transition Discovery -> Collecting
  const collRes = await request.post(`/api/cases/${caseId}/transitions`, {
    headers: {
      Authorization: `Bearer ${reviewerToken}`,
      'Content-Type': 'application/json',
    },
    data: {
      target_status: 'Collecting',
      actor_type: 'Reviewer',
      actor_id: 'test-reviewer-001',
      reason: 'Ready for credential collection',
    },
  });
  expect(collRes.status()).toBe(200);

  return caseId;
}

interface DisclosedFact {
  id: string;
  case_id: string;
  requirement_id: string;
  fact_type: string;
  claim_key: string;
  claim_value: unknown;
  confidence: number;
  source_credential_hash: string;
  verified_at: string;
}

async function getDisclosedFacts(
  request: APIRequestContext,
  caseId: string
): Promise<DisclosedFact[]> {
  const response = await request.get(`/api/test/cases/${caseId}/disclosed-facts`);
  expect(response.status()).toBe(200);
  const body = await response.json();
  return body.data as DisclosedFact[];
}

// =============================================================================
// Tests
// =============================================================================

test.describe('Phase 5 — Fact Normalization & Selective Disclosure E2E', () => {
  let request: APIRequestContext;
  let counterpartyToken: string;
  let reviewerToken: string;

  test.beforeAll(async ({ playwright }) => {
    request = await playwright.request.newContext({ baseURL: BACKEND_URL });
    counterpartyToken = generateCounterpartyToken();
    reviewerToken = generateReviewerToken();
  });

  test.afterAll(async () => {
    await request.dispose();
  });

  // ==========================================================================
  // SC1 — DisclosedFact normalization (VRFY-01)
  // ==========================================================================

  test.describe('SC1 — DisclosedFact normalization', () => {
    test('standard VP submission produces DisclosedFacts with only required claims', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'entity');

      // Submit the VP
      const submitRes = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_registration',
          credential_type: 'entity',
        },
      });
      expect(submitRes.status()).toBe(200);
      const submitBody = await submitRes.json();
      expect(submitBody.data.status).toBe('Verified');

      // Query disclosed facts for this case
      const facts = await getDisclosedFacts(request, caseId);

      // Should have exactly 4 entity claims
      expect(facts.length).toBe(4);

      const claimKeys = facts.map((f) => f.claim_key).sort();
      expect(claimKeys).toEqual(['entity_type', 'jurisdiction', 'legal_name', 'registration_number']);

      // No extra claims present
      const disallowed = ['extra_field', 'ssn', 'internal_score', 'date_of_birth'];
      for (const key of disallowed) {
        expect(facts.find((f) => f.claim_key === key)).toBeUndefined();
      }

      // All facts have correct metadata
      for (const fact of facts) {
        expect(fact.case_id).toBe(caseId);
        expect(fact.requirement_id).toBe('entity_registration');
        expect(fact.fact_type).toBe('EntityVerified');
        expect(fact.confidence).toBe(1.0);
        expect(fact.source_credential_hash).toHaveLength(64);
      }
    });
  });

  // ==========================================================================
  // SC2 — SD-JWT selective disclosure (CRED-05)
  // ==========================================================================

  test.describe('SC2 — SD-JWT selective disclosure', () => {
    test('SD-JWT VP reveals only policy-required fields', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'entity', { sdJwt: true });

      // Submit SD-JWT VP
      const submitRes = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_registration',
          credential_type: 'entity',
        },
      });
      expect(submitRes.status()).toBe(200);
      const submitBody = await submitRes.json();
      expect(submitBody.data.status).toBe('Verified');

      // Query disclosed facts
      const facts = await getDisclosedFacts(request, caseId);

      // Should contain only the 4 required entity claims
      expect(facts.length).toBe(4);
      const claimKeys = facts.map((f) => f.claim_key).sort();
      expect(claimKeys).toEqual(['entity_type', 'jurisdiction', 'legal_name', 'registration_number']);

      // Over-disclosed fields must NOT be present
      const overDisclosed = ['internal_score', 'ssn', 'date_of_birth'];
      for (const key of overDisclosed) {
        const found = facts.find((f) => f.claim_key === key);
        expect(found).toBeUndefined();
      }
    });

    test('over-disclosed fields are never stored in database', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'entity', { sdJwt: true });

      // Submit SD-JWT VP with over-disclosed fields
      const submitRes = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_registration',
          credential_type: 'entity',
        },
      });
      expect(submitRes.status()).toBe(200);

      // Query ALL disclosed facts for this case
      const facts = await getDisclosedFacts(request, caseId);

      // Explicitly verify sensitive fields are absent
      const sensitiveKeys = ['ssn', 'date_of_birth', 'internal_score'];
      const sensitiveFound = facts.filter((f) => sensitiveKeys.includes(f.claim_key));
      expect(sensitiveFound).toHaveLength(0);
    });
  });

  // ==========================================================================
  // SC3 — Requirement completeness tracking (VRFY-03)
  // ==========================================================================

  test.describe('SC3 — Requirement completeness tracking', () => {
    test('completeness starts at 0% before any submissions', async () => {
      const caseId = await createTestCase(request, reviewerToken);

      const compRes = await request.get(`/api/cases/${caseId}/completeness`, {
        headers: { Authorization: `Bearer ${reviewerToken}` },
      });
      expect(compRes.status()).toBe(200);
      const body = await compRes.json();

      expect(body.data.percentage).toBe(0);
      expect(body.data.verified).toBe(0);
      expect(body.data.total_required).toBeGreaterThan(0);
    });

    test('completeness increases after successful verification', async () => {
      const caseId = await createTestCase(request, reviewerToken);

      // Submit entity VP
      const entityVp = await generateTestVp(request, 'entity');
      const entityRes = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: entityVp,
          requirement_claim_type: 'entity_registration',
          credential_type: 'entity',
        },
      });
      expect(entityRes.status()).toBe(200);

      // Check completeness after first submission
      const comp1Res = await request.get(`/api/cases/${caseId}/completeness`, {
        headers: { Authorization: `Bearer ${reviewerToken}` },
      });
      const comp1 = (await comp1Res.json()).data;
      expect(comp1.verified).toBeGreaterThan(0);
      expect(comp1.percentage).toBeGreaterThan(0);

      const firstVerified = comp1.verified;
      const firstPercentage = comp1.percentage;

      // Submit signer VP
      const signerVp = await generateTestVp(request, 'signer');
      const signerRes = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: signerVp,
          requirement_claim_type: 'authorized_signer',
          credential_type: 'signer',
        },
      });
      expect(signerRes.status()).toBe(200);

      // Check completeness after second submission
      const comp2Res = await request.get(`/api/cases/${caseId}/completeness`, {
        headers: { Authorization: `Bearer ${reviewerToken}` },
      });
      const comp2 = (await comp2Res.json()).data;

      expect(comp2.verified).toBeGreaterThan(firstVerified);
      expect(comp2.percentage).toBeGreaterThan(firstPercentage);
    });

    test('completeness shows per-requirement breakdown', async () => {
      const caseId = await createTestCase(request, reviewerToken);

      // Submit entity + signer VPs
      const entityVp = await generateTestVp(request, 'entity');
      await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: entityVp,
          requirement_claim_type: 'entity_registration',
          credential_type: 'entity',
        },
      });

      const signerVp = await generateTestVp(request, 'signer');
      await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: signerVp,
          requirement_claim_type: 'authorized_signer',
          credential_type: 'signer',
        },
      });

      // Check completeness breakdown
      const compRes = await request.get(`/api/cases/${caseId}/completeness`, {
        headers: { Authorization: `Bearer ${reviewerToken}` },
      });
      const body = await compRes.json();
      const byReq = body.data.by_requirement;

      expect(Array.isArray(byReq)).toBe(true);
      expect(byReq.length).toBeGreaterThan(0);

      // Find entity_registration and authorized_signer entries
      const entityReq = byReq.find(
        (r: { requirement_id: string }) => r.requirement_id === 'entity_registration'
      );
      const signerReq = byReq.find(
        (r: { requirement_id: string }) => r.requirement_id === 'authorized_signer'
      );

      // Submitted requirements should be complete
      if (entityReq) {
        expect(entityReq.status).toBe('complete');
        expect(entityReq.verified_claims_count).toBeGreaterThan(0);
      }
      if (signerReq) {
        expect(signerReq.status).toBe('complete');
        expect(signerReq.verified_claims_count).toBeGreaterThan(0);
      }

      // There should be at least one requirement with "missing" status
      // (since we haven't submitted all required credentials)
      const missingReqs = byReq.filter(
        (r: { status: string }) => r.status === 'missing'
      );
      expect(missingReqs.length).toBeGreaterThan(0);
    });
  });

  // ==========================================================================
  // SC4 — AI boundary enforcement (VRFY-06)
  // ==========================================================================

  test.describe('SC4 — AI module boundary enforcement', () => {
    test('AI module source code does not import raw credential types', async () => {
      // Read the ai/mod.rs file directly
      const aiModPath = path.resolve(__dirname, '../../backend/src/ai/mod.rs');
      const content = fs.readFileSync(aiModPath, 'utf-8');

      // Filter out comment lines for accurate check
      const nonCommentLines = content
        .split('\n')
        .filter((line) => {
          const trimmed = line.trim();
          return !trimmed.startsWith('//') && !trimmed.startsWith('///') && !trimmed.startsWith('//!');
        })
        .join('\n');

      // Must NOT contain raw credential types in non-comment code
      expect(nonCommentLines).not.toContain('VerifiablePresentation');
      expect(nonCommentLines).not.toContain('VerifiableCredential');
      expect(nonCommentLines).not.toContain('crate::credential::');

      // MUST contain DisclosedFact (the only allowed type)
      expect(content).toContain('DisclosedFact');
      expect(content).toContain('FactType');
    });
  });
});
