/**
 * Portal domain types for the counterparty proof submission flow.
 *
 * These types mirror the backend response shapes from:
 * - GET /api/cases/:id/requirements
 * - GET /api/cases/:id/completeness
 */

/** Status of a single proof requirement in the checklist. */
export type RequirementStatus = 'pending' | 'submitted' | 'verified' | 'failed';

/** Category grouping for proof requirements. */
export type RequirementCategory =
  | 'entity'
  | 'signer'
  | 'region'
  | 'wallet'
  | 'financial'
  | 'compliance'
  | 'custom';

/** A single proof requirement in the counterparty checklist. */
export interface ProofRequirement {
  readonly id: string;
  readonly claim_type: string;
  readonly description: string;
  readonly category: string;
  readonly required_claims: readonly string[];
  readonly status: RequirementStatus;
}

/** Metadata returned alongside the requirements list. */
export interface RequirementsMeta {
  readonly count: number;
  readonly workflow_type: string;
  readonly case_id: string;
}

/** Full response shape from GET /api/cases/:id/requirements. */
export interface RequirementsResponse {
  readonly data: readonly ProofRequirement[];
  readonly meta: RequirementsMeta;
}

/** Per-requirement completeness breakdown. */
export interface RequirementCompleteness {
  readonly requirement_id: string;
  readonly claim_type: string;
  readonly required_claims_count: number;
  readonly verified_claims_count: number;
  readonly status: string;
}

/** Full response shape from GET /api/cases/:id/completeness. */
export interface CompletenessResponse {
  readonly total_required: number;
  readonly verified: number;
  readonly pending: number;
  readonly failed: number;
  readonly percentage: number;
  readonly by_requirement: readonly RequirementCompleteness[];
}

/* --- SSE Event Types --- */

/** Known SSE event type names from the backend stream. */
export type SSEEventType =
  | 'requirement_added'
  | 'submission_verified'
  | 'assessment_complete'
  | 'status_changed';

/** Generic SSE event envelope from the stream. */
export interface SSEEvent {
  readonly type: SSEEventType;
  readonly data: unknown;
  readonly timestamp: string;
}

/** Data payload for requirement_added events. */
export interface RequirementAddedData {
  readonly requirement_id: string;
  readonly claim_type: string;
  readonly description: string;
}

/** Data payload for submission_verified events. */
export interface SubmissionVerifiedData {
  readonly submission_id: string;
  readonly requirement_id: string;
  readonly status: string;
}

/** Data payload for assessment_complete events. */
export interface AssessmentCompleteData {
  readonly case_id: string;
  readonly decision: string;
}

/** Data payload for status_changed events. */
export interface StatusChangedData {
  readonly case_id: string;
  readonly old_status: string;
  readonly new_status: string;
}

/** Submission request body for POST /api/cases/:id/submissions. */
export interface SubmitProofRequest {
  readonly requirement_id: string;
  readonly credential_type: string;
  readonly requirement_claim_type: string;
  readonly raw_vp: object;
}

/** Submission response from POST /api/cases/:id/submissions. */
export interface SubmitProofResponse {
  readonly data: {
    readonly submission_id: string;
    readonly status: string;
    readonly extracted_claims?: unknown;
    readonly failure_reason: string | null;
  };
  readonly error: string | null;
  readonly meta: {
    readonly case_id: string;
  };
}
