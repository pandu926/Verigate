import { test, expect, APIRequestContext } from '@playwright/test';
import * as jwt from 'jsonwebtoken';

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
  options: { tamperSignature?: boolean; untrustedIssuer?: boolean } = {}
): Promise<Record<string, unknown>> {
  const params = new URLSearchParams({ type: credentialType });
  if (options.tamperSignature) params.set('tamper_signature', 'true');
  if (options.untrustedIssuer) params.set('untrusted_issuer', 'true');

  const response = await request.get(`/api/test/generate-vp?${params.toString()}`);
  expect(response.status()).toBe(200);
  const body: VpResponse = await response.json();
  return body.data.vp;
}

async function createTestCase(request: APIRequestContext, reviewerToken: string): Promise<string> {
  // Create a case
  const createRes = await request.post('/api/cases', {
    headers: {
      Authorization: `Bearer ${reviewerToken}`,
      'Content-Type': 'application/json',
    },
    data: {
      workflow_type: 'Onboarding',
      entity_type: 'Corporation',
      relationship_goal: 'credential_e2e_test',
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
      reason: 'Moving to discovery for credential collection',
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

// =============================================================================
// Tests
// =============================================================================

test.describe('Phase 4 — Credential Intake & Verification E2E', () => {
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
  // Success Criterion 1: System accepts VP and parses into constituent credentials
  // ==========================================================================

  test.describe('SC1 — VP acceptance and parsing', () => {
    test('submit valid entity VP returns 200 with submission_id and extracted claims', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'entity');

      const response = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });

      expect(response.status()).toBe(200);
      const body = await response.json();

      // Submission ID present
      expect(body.data.submission_id).toBeTruthy();
      // Status should be Verified (valid VP with trusted issuer)
      expect(body.data.status).toBe('Verified');
      // Extracted claims should contain entity fields
      expect(body.data.extracted_claims).toBeTruthy();
      const claims = body.data.extracted_claims;
      expect(Array.isArray(claims)).toBe(true);
      expect(claims.length).toBeGreaterThan(0);
      expect(claims[0].legal_name).toBe('Acme Corporation Ltd');
    });
  });

  // ==========================================================================
  // Success Criterion 2: Validates format, signatures, and issuer trust
  // ==========================================================================

  test.describe('SC2 — Signature validation and issuer trust', () => {
    test('valid VP with trusted issuer gets status "verified"', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'signer');

      const response = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'authorized_signer',
          credential_type: 'signer',
        },
      });

      expect(response.status()).toBe(200);
      const body = await response.json();
      expect(body.data.status).toBe('Verified');
      expect(body.data.failure_reason).toBeNull();
    });

    test('VP with tampered signature gets status "failed" with signature error', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'entity', { tamperSignature: true });

      const response = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });

      expect(response.status()).toBe(200);
      const body = await response.json();
      expect(body.data.status).toBe('Failed');
      expect(body.data.failure_reason).toBeTruthy();
      // Failure reason should mention signature or format
      const reason = body.data.failure_reason.toLowerCase();
      expect(reason.includes('signature') || reason.includes('format')).toBe(true);
    });

    test('VP from untrusted issuer gets status "failed" with issuer error', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'entity', { untrustedIssuer: true });

      const response = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });

      expect(response.status()).toBe(200);
      const body = await response.json();
      expect(body.data.status).toBe('Failed');
      expect(body.data.failure_reason).toBeTruthy();
      expect(body.data.failure_reason.toLowerCase()).toContain('untrusted issuer');
    });
  });

  // ==========================================================================
  // Success Criterion 3: Submission status tracks per requirement
  // ==========================================================================

  test.describe('SC3 — Submission status lifecycle', () => {
    test('GET /submissions returns submissions with correct status', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'region');

      // Submit
      const submitRes = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'jurisdiction_compliance',
          credential_type: 'region',
        },
      });
      expect(submitRes.status()).toBe(200);

      // List submissions
      const listRes = await request.get(`/api/cases/${caseId}/submissions`, {
        headers: { Authorization: `Bearer ${counterpartyToken}` },
      });
      expect(listRes.status()).toBe(200);
      const listBody = await listRes.json();

      expect(listBody.data.length).toBeGreaterThanOrEqual(1);
      const submission = listBody.data[0];
      expect(submission.status).toBe('Verified');
      expect(submission.credential_type).toBe('region');
      // verified_at should be populated for verified submissions
      expect(submission.verified_at).toBeTruthy();
    });

    test('failed submission has failure_reason set', async () => {
      const caseId = await createTestCase(request, reviewerToken);
      const vp = await generateTestVp(request, 'entity', { untrustedIssuer: true });

      // Submit (will fail)
      await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });

      // List and check
      const listRes = await request.get(`/api/cases/${caseId}/submissions`, {
        headers: { Authorization: `Bearer ${counterpartyToken}` },
      });
      const listBody = await listRes.json();
      const failed = listBody.data.find(
        (s: { status: string }) => s.status === 'Failed'
      );
      expect(failed).toBeTruthy();
      expect(failed.failure_reason).toBeTruthy();
    });

    test('resubmission after failure creates new record, old remains failed', async () => {
      const caseId = await createTestCase(request, reviewerToken);

      // First submission: fails (untrusted issuer)
      const badVp = await generateTestVp(request, 'entity', { untrustedIssuer: true });
      const failRes = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: badVp,
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });
      expect(failRes.status()).toBe(200);
      expect((await failRes.json()).data.status).toBe('Failed');

      // Second submission: succeeds (valid VP)
      const goodVp = await generateTestVp(request, 'entity');
      const passRes = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: goodVp,
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });
      expect(passRes.status()).toBe(200);
      expect((await passRes.json()).data.status).toBe('Verified');

      // List: should have both records
      const listRes = await request.get(`/api/cases/${caseId}/submissions`, {
        headers: { Authorization: `Bearer ${counterpartyToken}` },
      });
      const listBody = await listRes.json();
      expect(listBody.data.length).toBe(2);

      const statuses = listBody.data.map((s: { status: string }) => s.status);
      expect(statuses).toContain('Failed');
      expect(statuses).toContain('Verified');
    });
  });

  // ==========================================================================
  // Success Criterion 4: All 4 credential types work in one case
  // ==========================================================================

  test.describe('SC4 — Multiple credential types in single case', () => {
    test('submit all 4 credential types to one case and verify each', async () => {
      const caseId = await createTestCase(request, reviewerToken);

      const types = [
        { type: 'entity', claim: 'entity_identity' },
        { type: 'signer', claim: 'authorized_signer' },
        { type: 'region', claim: 'jurisdiction_compliance' },
        { type: 'wallet', claim: 'wallet_proof' },
      ];

      for (const { type: credType, claim } of types) {
        const vp = await generateTestVp(request, credType);
        const res = await request.post(`/api/cases/${caseId}/submissions`, {
          headers: {
            Authorization: `Bearer ${counterpartyToken}`,
            'Content-Type': 'application/json',
          },
          data: {
            raw_vp: vp,
            requirement_claim_type: claim,
            credential_type: credType,
          },
        });
        expect(res.status()).toBe(200);
        const body = await res.json();
        expect(body.data.status).toBe('Verified');
      }

      // List all submissions for this case
      const listRes = await request.get(`/api/cases/${caseId}/submissions`, {
        headers: { Authorization: `Bearer ${counterpartyToken}` },
      });
      expect(listRes.status()).toBe(200);
      const listBody = await listRes.json();

      expect(listBody.data.length).toBe(4);
      expect(listBody.meta.count).toBe(4);

      // Verify each credential type is present
      const credentialTypes = listBody.data.map(
        (s: { credential_type: string }) => s.credential_type
      );
      expect(credentialTypes).toContain('entity');
      expect(credentialTypes).toContain('signer');
      expect(credentialTypes).toContain('region');
      expect(credentialTypes).toContain('wallet');

      // Verify extracted claims differ per type
      const entitySub = listBody.data.find(
        (s: { credential_type: string }) => s.credential_type === 'entity'
      );
      const walletSub = listBody.data.find(
        (s: { credential_type: string }) => s.credential_type === 'wallet'
      );
      expect(entitySub.extracted_claims).toBeTruthy();
      expect(walletSub.extracted_claims).toBeTruthy();
      // Entity has legal_name, wallet has wallet_address
      const entityClaims = entitySub.extracted_claims;
      const walletClaims = walletSub.extracted_claims;
      expect(entityClaims[0].legal_name).toBeTruthy();
      expect(walletClaims[0].wallet_address).toBeTruthy();
    });
  });

  // ==========================================================================
  // Negative Tests
  // ==========================================================================

  test.describe('Negative cases', () => {
    test('unauthenticated request returns 401', async () => {
      const response = await request.post('/api/cases/00000000-0000-0000-0000-000000000001/submissions', {
        headers: {
          Authorization: 'Bearer invalid-token-garbage',
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: {},
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });
      expect(response.status()).toBe(401);
    });

    test('reviewer role attempting submission returns 403', async () => {
      const caseId = await createTestCase(request, reviewerToken);

      const vp = await generateTestVp(request, 'entity');
      const response = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${reviewerToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });
      expect(response.status()).toBe(403);
    });

    test('submission to non-existent case returns 404', async () => {
      const fakeId = '00000000-0000-0000-0000-000000000099';
      const vp = await generateTestVp(request, 'entity');
      const response = await request.post(`/api/cases/${fakeId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: {
          raw_vp: vp,
          requirement_claim_type: 'entity_identity',
          credential_type: 'entity',
        },
      });
      expect(response.status()).toBe(404);
    });

    test('malformed JSON body returns 400 or 422', async () => {
      const caseId = await createTestCase(request, reviewerToken);

      const response = await request.post(`/api/cases/${caseId}/submissions`, {
        headers: {
          Authorization: `Bearer ${counterpartyToken}`,
          'Content-Type': 'application/json',
        },
        data: '{"invalid json missing fields',
      });
      // Axum returns 400 for JSON parse errors or 422 for missing fields
      expect([400, 422]).toContain(response.status());
    });
  });
});
