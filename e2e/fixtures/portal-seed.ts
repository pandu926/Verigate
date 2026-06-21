import { APIRequestContext, expect } from '@playwright/test';
import * as jwt from 'jsonwebtoken';

const JWT_SECRET = 'dev-secret-do-not-use-in-prod';

/**
 * Generate a counterparty JWT token for API calls.
 */
export function generateCounterpartyToken(): string {
  const now = Math.floor(Date.now() / 1000);
  return jwt.sign(
    { sub: 'test-counterparty-portal', role: 'counterparty', iat: now, exp: now + 900 },
    JWT_SECRET,
    { algorithm: 'HS256' }
  );
}

/**
 * Generate a reviewer JWT token for case creation and transitions.
 */
export function generateReviewerToken(): string {
  const now = Math.floor(Date.now() / 1000);
  return jwt.sign(
    { sub: 'test-reviewer-portal', role: 'reviewer', iat: now, exp: now + 900 },
    JWT_SECRET,
    { algorithm: 'HS256' }
  );
}

/**
 * A valid VP JSON structure for submission testing.
 * Contains verifiableCredential array with credentialSubject keys.
 */
export const VALID_VP_JSON = {
  '@context': ['https://www.w3.org/2018/credentials/v1'],
  type: 'VerifiablePresentation',
  verifiableCredential: [
    {
      '@context': ['https://www.w3.org/2018/credentials/v1'],
      type: ['VerifiableCredential', 'EntityCredential'],
      issuer: 'did:example:trusted-issuer-001',
      credentialSubject: {
        id: 'did:example:subject-001',
        entity_name: 'Acme Corporation Ltd',
        jurisdiction: 'US-DE',
        registration_number: 'DE-12345678',
      },
      proof: {
        type: 'Ed25519Signature2020',
        created: '2024-01-01T00:00:00Z',
        proofPurpose: 'assertionMethod',
        verificationMethod: 'did:example:trusted-issuer-001#key-1',
        proofValue: 'z5vGFh4Qm...valid-signature',
      },
    },
  ],
};

/**
 * Invalid VP JSON string — not a valid VP structure.
 */
export const INVALID_VP_JSON = '{"name": "not a VP", "random_field": 123}';

/**
 * Malformed JSON string for syntax error testing.
 */
export const MALFORMED_JSON = '{invalid json syntax!!!';

interface SeedResult {
  caseId: string;
  reviewerToken: string;
  counterpartyToken: string;
}

/**
 * Creates a test case, transitions it to "Collecting" status,
 * and returns the case ID with auth tokens.
 */
export async function seedPortalCase(request: APIRequestContext): Promise<SeedResult> {
  const reviewerToken = generateReviewerToken();
  const counterpartyToken = generateCounterpartyToken();

  // Create a case
  const createRes = await request.post('/api/cases', {
    headers: {
      Authorization: `Bearer ${reviewerToken}`,
      'Content-Type': 'application/json',
    },
    data: {
      workflow_type: 'Onboarding',
      entity_type: 'Corporation',
      relationship_goal: 'portal_e2e_test',
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
      actor_id: 'test-reviewer-portal',
      reason: 'Moving to discovery for portal testing',
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
      actor_id: 'test-reviewer-portal',
      reason: 'Ready for credential collection via portal',
    },
  });
  expect(collRes.status()).toBe(200);

  return { caseId, reviewerToken, counterpartyToken };
}

/**
 * Submit a valid VP proof via backend API (bypassing UI for speed).
 * Returns the submission response data.
 */
export async function submitProofViaApi(
  request: APIRequestContext,
  caseId: string,
  counterpartyToken: string,
  credentialType: string = 'entity'
): Promise<{ submissionId: string; status: string }> {
  // Use the test VP generator endpoint for a valid VP
  const vpRes = await request.get(`/api/test/generate-vp?type=${credentialType}`);
  expect(vpRes.status()).toBe(200);
  const vpBody = await vpRes.json();
  const vp = vpBody.data.vp;

  const claimTypeMap: Record<string, string> = {
    entity: 'entity_registration',
    signer: 'authorized_signer',
    region: 'jurisdiction_compliance',
    wallet: 'beneficial_ownership',
  };

  const response = await request.post(`/api/cases/${caseId}/submissions`, {
    headers: {
      Authorization: `Bearer ${counterpartyToken}`,
      'Content-Type': 'application/json',
    },
    data: {
      raw_vp: vp,
      requirement_claim_type: claimTypeMap[credentialType] ?? 'entity_identity',
      credential_type: credentialType,
    },
  });
  expect(response.status()).toBe(200);
  const body = await response.json();

  return {
    submissionId: body.data.submission_id,
    status: body.data.status,
  };
}
