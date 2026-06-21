import { z } from 'zod';
import { useAuthStore } from '@/stores/auth';
import type { HealthResponse } from '@/types/domain';
import type { RequirementsResponse, CompletenessResponse, SubmitProofRequest, SubmitProofResponse } from '@/types/portal';
import type { DemoCase, DisclosedFact } from '@/types/demo';

const API_BASE_URL =
  import.meta.env.VITE_API_URL ?? '';

/**
 * Base fetch wrapper with error handling and centralized auth headers.
 * All API calls go through this to centralize error handling and headers.
 */
async function apiFetch<T>(
  path: string,
  schema: z.ZodType<T>,
  options?: RequestInit,
): Promise<T> {
  const url = `${API_BASE_URL}${path}`;
  const token = useAuthStore.getState().getToken();

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options?.headers as Record<string, string>),
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const response = await fetch(url, {
    ...options,
    headers,
  });

  if (response.status === 401) {
    useAuthStore.getState().logout();
    throw new ApiError('Session expired', 401);
  }

  if (!response.ok) {
    throw new ApiError(
      `API request failed: ${response.status} ${response.statusText}`,
      response.status,
    );
  }

  const data: unknown = await response.json();
  const parsed = schema.safeParse(data);

  if (!parsed.success) {
    throw new ApiError(
      `Invalid API response shape: ${parsed.error.message}`,
      response.status,
    );
  }

  return parsed.data;
}

/** Structured API error with status code. */
export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

/** Zod schema for AgentIdentity */
const agentIdentitySchema = z.object({
  agent_did: z.string(),
  authenticated: z.boolean(),
  sdk_version: z.string(),
  capabilities: z.array(z.string()),
});

/** Zod schema for HealthResponse */
const healthResponseSchema = z.object({
  status: z.string(),
  version: z.string(),
  agent_identity: agentIdentitySchema,
  database_connected: z.boolean(),
  uptime_seconds: z.number(),
});

/** Fetch the backend health status. */
export async function fetchHealth(): Promise<HealthResponse> {
  return apiFetch('/api/health', healthResponseSchema);
}

/** Zod schema for a single proof requirement (matches actual backend shape). */
const proofRequirementRawSchema = z.object({
  claim_type: z.string(),
  mandatory: z.boolean().optional(),
  description: z.string(),
  acceptable_proof_types: z.array(z.string()).optional(),
  status: z.enum(['pending', 'submitted', 'verified', 'failed']),
});

/** Zod schema for raw RequirementsResponse from backend. */
const requirementsResponseRawSchema = z.object({
  data: z.array(proofRequirementRawSchema),
  meta: z.object({
    count: z.number(),
    workflow_type: z.string(),
    case_id: z.string(),
  }),
});

/** Zod schema for RequirementCompleteness. */
const requirementCompletenessSchema = z.object({
  requirement_id: z.string(),
  claim_type: z.string(),
  required_claims_count: z.number(),
  verified_claims_count: z.number(),
  status: z.string(),
});

/** Zod schema for CompletenessResponse (backend wraps in data/error/meta envelope). */
const completenessResponseRawSchema = z.object({
  data: z.object({
    total_required: z.number(),
    verified: z.number(),
    pending: z.number(),
    failed: z.number(),
    percentage: z.number(),
    by_requirement: z.array(requirementCompletenessSchema),
  }),
  error: z.string().nullable(),
  meta: z.object({
    case_id: z.string(),
  }),
});

/** Fetch proof requirements for a case (transforms backend shape to UI shape). */
export async function fetchRequirements(caseId: string): Promise<RequirementsResponse> {
  const token = useAuthStore.getState().getToken();
  const url = `${API_BASE_URL}/api/cases/${encodeURIComponent(caseId)}/requirements`;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const response = await fetch(url, { headers });

  if (!response.ok) {
    throw new ApiError(
      `API request failed: ${response.status} ${response.statusText}`,
      response.status,
    );
  }

  const raw: unknown = await response.json();
  const parsed = requirementsResponseRawSchema.safeParse(raw);

  if (!parsed.success) {
    throw new ApiError(
      `Invalid API response shape: ${parsed.error.message}`,
      response.status,
    );
  }

  // Transform backend shape to UI shape
  const transformed: RequirementsResponse = {
    data: parsed.data.data.map((req) => ({
      id: req.claim_type,
      claim_type: req.claim_type,
      description: req.description,
      category: req.claim_type.split('_')[0] ?? 'custom',
      required_claims: req.acceptable_proof_types ?? [],
      status: req.status,
    })),
    meta: parsed.data.meta,
  };

  return transformed;
}

/** Fetch completeness data for a case (unwraps data envelope). */
export async function fetchCompleteness(caseId: string): Promise<CompletenessResponse> {
  const token = useAuthStore.getState().getToken();
  const url = `${API_BASE_URL}/api/cases/${encodeURIComponent(caseId)}/completeness`;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const response = await fetch(url, { headers });

  if (!response.ok) {
    throw new ApiError(
      `API request failed: ${response.status} ${response.statusText}`,
      response.status,
    );
  }

  const raw: unknown = await response.json();
  const parsed = completenessResponseRawSchema.safeParse(raw);

  if (!parsed.success) {
    throw new ApiError(
      `Invalid API response shape: ${parsed.error.message}`,
      response.status,
    );
  }

  // Unwrap the data envelope
  return parsed.data.data;
}

/** Zod schema for submission response. */
const submitProofResponseSchema = z.object({
  data: z.object({
    submission_id: z.string(),
    status: z.string(),
    extracted_claims: z.unknown().optional(),
    failure_reason: z.string().nullable(),
  }),
  error: z.string().nullable(),
  meta: z.object({
    case_id: z.string(),
  }),
});

/** Submit a verifiable presentation for a case requirement. */
export async function submitPresentation(
  caseId: string,
  request: SubmitProofRequest,
): Promise<SubmitProofResponse> {
  return apiFetch(
    `/api/cases/${encodeURIComponent(caseId)}/submissions`,
    submitProofResponseSchema,
    {
      method: 'POST',
      body: JSON.stringify(request),
    },
  );
}

/* ─── T3N TEE Integration APIs ─────────────────────────────────────── */

/** Generate a real signed VP from the test endpoint. */
export async function generateTestVp(type: string): Promise<object> {
  const token = useAuthStore.getState().getToken();
  const url = `${API_BASE_URL}/api/test/generate-vp?type=${encodeURIComponent(type)}`;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const response = await fetch(url, { headers });
  if (!response.ok) throw new ApiError(`Generate VP failed: ${response.status}`, response.status);
  const data = await response.json();
  return data.data.vp;
}

/** Trigger AI assessment (async — returns 202). */
export async function triggerAssessment(caseId: string): Promise<{ status: string }> {
  const token = useAuthStore.getState().getToken();
  const url = `${API_BASE_URL}/api/cases/${encodeURIComponent(caseId)}/assess`;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const response = await fetch(url, { method: 'POST', headers });
  if (!response.ok) throw new ApiError(`Assess failed: ${response.status}`, response.status);
  const data = await response.json();
  return data.data;
}

/** Fetch latest assessment result. */
export async function fetchAssessment(caseId: string): Promise<unknown> {
  const token = useAuthStore.getState().getToken();
  const url = `${API_BASE_URL}/api/cases/${encodeURIComponent(caseId)}/assessment`;
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const response = await fetch(url, { headers });
  if (!response.ok) throw new ApiError(`Fetch assessment failed: ${response.status}`, response.status);
  const data = await response.json();
  return data.data;
}

/* ─── Demo / Privacy Split-Screen APIs ─────────────────────────────── */

/** Zod schema for a single DisclosedFact from the test endpoint. */
const disclosedFactSchema = z.object({
  id: z.string(),
  case_id: z.string(),
  claim_type: z.string(),
  claim_key: z.string(),
  claim_value: z.string(),
  source_credential_id: z.string(),
  verified_at: z.string(),
});

/** Zod schema for the disclosed-facts response (array envelope). */
const disclosedFactsResponseSchema = z.array(disclosedFactSchema);

/** Fetch disclosed facts for a case (privacy-filtered claims the AI sees). */
export async function fetchDisclosedFacts(caseId: string): Promise<DisclosedFact[]> {
  const url = `${API_BASE_URL}/api/test/cases/${encodeURIComponent(caseId)}/disclosed-facts`;
  const response = await fetch(url, {
    headers: { 'Content-Type': 'application/json' },
  });

  if (!response.ok) {
    throw new ApiError(
      `API request failed: ${response.status} ${response.statusText}`,
      response.status,
    );
  }

  const raw: unknown = await response.json();
  const parsed = disclosedFactsResponseSchema.safeParse(raw);

  if (!parsed.success) {
    // Gracefully return empty if shape doesn't match (demo fallback)
    return [];
  }

  return parsed.data;
}

/** Zod schema for a case item from the cases list. */
const demoCaseRawSchema = z.object({
  id: z.string(),
  entity_name: z.string().optional().nullable(),
  workflow_type: z.string().optional().nullable().default('onboarding'),
  status: z.string(),
  entity_type: z.string().optional().nullable().default('corporation'),
  jurisdiction: z.string().optional().nullable().default('--'),
  relationship_goal: z.string().optional().nullable().default('counterparty assessment'),
  created_at: z.string(),
  updated_at: z.string(),
});

/** Transform raw case: use relationship_goal as entity_name when entity_name is missing. */
const demoCaseSchema = demoCaseRawSchema.transform((raw) => ({
  ...raw,
  entity_name: raw.entity_name || raw.relationship_goal || 'Unknown Entity',
  workflow_type: raw.workflow_type || 'onboarding',
  entity_type: raw.entity_type || 'corporation',
  jurisdiction: raw.jurisdiction || '--',
  relationship_goal: raw.relationship_goal || 'counterparty assessment',
  status: (raw.status || 'created').toLowerCase(),
}));

/** Zod schema for cases list response (may be wrapped in data envelope or bare array). */
const casesResponseSchema = z.union([
  z.object({ data: z.array(demoCaseSchema) }),
  z.array(demoCaseSchema),
]);

/** Fetch all cases visible to the current user. */
export async function fetchCases(): Promise<DemoCase[]> {
  const token = useAuthStore.getState().getToken();
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const url = `${API_BASE_URL}/api/cases`;
  const response = await fetch(url, { headers });

  if (!response.ok) {
    throw new ApiError(
      `API request failed: ${response.status} ${response.statusText}`,
      response.status,
    );
  }

  const raw: unknown = await response.json();
  const parsed = casesResponseSchema.safeParse(raw);

  if (!parsed.success) {
    return [];
  }

  // Unwrap envelope if present
  const cases = Array.isArray(parsed.data) ? parsed.data : parsed.data.data;
  return cases;
}
