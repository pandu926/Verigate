/**
 * Domain types mirroring the Rust backend (backend/src/domain/types.rs).
 *
 * These types maintain a 1:1 correspondence with the backend enums and structs
 * to ensure type safety across the full stack.
 */

/** Current status of a case in the workflow lifecycle. */
export type CaseStatus =
  | 'created'
  | 'discovery'
  | 'collecting'
  | 'verifying'
  | 'assessing'
  | 'review'
  | 'approved'
  | 'blocked';

/** The type of actor performing an action within the system. */
export type ActorType =
  | 'ai'
  | 'verifier'
  | 'reviewer'
  | 'counterparty'
  | 'system'
  | 'protected_action';

/** Classification of the entity being onboarded or assessed. */
export type EntityType =
  | 'individual'
  | 'corporation'
  | 'fund'
  | 'trust'
  | 'dao'
  | 'government';

/** The type of workflow governing the case lifecycle. */
export type WorkflowType =
  | 'onboarding'
  | 'due_diligence'
  | 'compliance'
  | 'revalidation';

/** Category of fact disclosed from a verified credential. */
export type FactType =
  | 'entity_verified'
  | 'jurisdiction_confirmed'
  | 'signer_authorized'
  | 'wallet_ownership'
  | 'financial_threshold'
  | 'compliance_status'
  | 'custom';

/**
 * The canonical privacy-safe data structure that the AI layer consumes.
 *
 * A DisclosedFact represents a single verified claim extracted from a credential
 * presentation. The AI reasoning layer operates exclusively on these structured
 * facts rather than raw credential data.
 */
export interface DisclosedFact {
  readonly id: string;
  readonly case_id: string;
  readonly fact_type: FactType;
  readonly source_credential_id: string;
  readonly claim_key: string;
  readonly claim_value: unknown;
  readonly confidence: number;
  readonly verified_at: string;
  readonly expires_at?: string;
}

/** Terminal 3 Agent identity information. */
export interface AgentIdentity {
  readonly agent_did: string;
  readonly authenticated: boolean;
  readonly sdk_version: string;
  readonly capabilities: readonly string[];
}

/** Response shape from the /api/health endpoint. */
export interface HealthResponse {
  readonly status: string;
  readonly version: string;
  readonly agent_identity: AgentIdentity;
  readonly database_connected: boolean;
  readonly uptime_seconds: number;
}
