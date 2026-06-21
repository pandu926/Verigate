/**
 * Demo-specific types and static data for the privacy split-screen
 * and reviewer dashboard pages.
 */

/** A single field within a full credential, marking whether it's disclosed to the AI. */
export interface CredentialField {
  readonly value: string;
  readonly disclosed: boolean;
}

/** A full credential with all fields (counterparty-provided data). */
export interface FullCredential {
  readonly type: string;
  readonly label: string;
  readonly fields: Record<string, CredentialField>;
}

/** A case as returned by the reviewer dashboard API. */
export interface DemoCase {
  readonly id: string;
  readonly entity_name: string;
  readonly workflow_type: string;
  readonly status: string;
  readonly entity_type: string;
  readonly jurisdiction: string;
  readonly relationship_goal: string;
  readonly created_at: string;
  readonly updated_at: string;
}

/** A single disclosed fact visible to the AI reasoning layer. */
export interface DisclosedFact {
  readonly id: string;
  readonly case_id: string;
  readonly claim_type: string;
  readonly claim_key: string;
  readonly claim_value: string;
  readonly source_credential_id: string;
  readonly verified_at: string;
}

/** A step in the demo narrative walkthrough. */
export interface DemoStep {
  readonly step: number;
  readonly title: string;
  readonly description: string;
  readonly route: string;
}

/**
 * Static full credential data representing what the counterparty provided.
 * Fields marked disclosed:true are the only ones the AI agent sees.
 */
export const FULL_CREDENTIALS: Record<string, FullCredential> = {
  entity_registration: {
    type: 'entity_registration',
    label: 'Entity Registration',
    fields: {
      legal_name: { value: 'Meridian Capital Holdings Ltd', disclosed: true },
      registration_number: { value: 'HE-392847', disclosed: false },
      jurisdiction: { value: 'Cyprus', disclosed: true },
      entity_type: { value: 'corporation', disclosed: true },
      incorporation_date: { value: '2019-03-14', disclosed: false },
      registered_address: { value: '42 Makarios III Ave, Nicosia 1065', disclosed: false },
      directors: { value: 'Elena Vasiliou, Christos Andreou', disclosed: false },
      shareholders: { value: 'Meridian Trust (87%), E. Vasiliou (13%)', disclosed: false },
      tax_id: { value: 'CY-10293847', disclosed: false },
      operating_status: { value: 'Active', disclosed: false },
      last_annual_filing: { value: '2025-12-01', disclosed: false },
    },
  },
  authorized_signer: {
    type: 'authorized_signer',
    label: 'Authorized Signer',
    fields: {
      full_name: { value: 'Elena Vasiliou', disclosed: true },
      title: { value: 'Managing Director', disclosed: true },
      signing_authority_level: { value: 'full', disclosed: true },
      date_of_birth: { value: '1984-07-22', disclosed: false },
      nationality: { value: 'Cypriot', disclosed: false },
      passport_number: { value: 'K08294731', disclosed: false },
      residential_address: { value: '18 Aphrodite Hills, Paphos', disclosed: false },
      appointment_date: { value: '2019-03-14', disclosed: false },
      authority_limitations: { value: 'None', disclosed: false },
    },
  },
  jurisdiction_compliance: {
    type: 'jurisdiction_compliance',
    label: 'Jurisdiction Compliance',
    fields: {
      country_code: { value: 'CY', disclosed: true },
      regulatory_status: { value: 'Licensed', disclosed: true },
      compliance_rating: { value: 'A+', disclosed: true },
      license_numbers: { value: 'CySEC CIF 284/15, AMLD5 REG-4829', disclosed: false },
      last_audit_date: { value: '2025-09-15', disclosed: false },
      regulator_name: { value: 'Cyprus Securities and Exchange Commission', disclosed: false },
      sanctions_screening: { value: 'Clear - OFAC/EU/UN', disclosed: false },
      pep_status: { value: 'No PEP associations', disclosed: false },
      risk_classification: { value: 'Standard', disclosed: false },
    },
  },
  beneficial_ownership: {
    type: 'beneficial_ownership',
    label: 'Beneficial Ownership',
    fields: {
      wallet_address: { value: '0x7a3B...9f2E', disclosed: true },
      chain: { value: 'Ethereum', disclosed: true },
      kyc_status: { value: 'Verified', disclosed: true },
      balance: { value: '2,847.3 ETH', disclosed: false },
      transaction_history: { value: '1,247 transactions since 2020', disclosed: false },
      risk_score: { value: '12/100 (Low)', disclosed: false },
      linked_entities: { value: 'Meridian Capital Holdings Ltd, Meridian DeFi Fund', disclosed: false },
      source_of_funds: { value: 'Corporate treasury operations', disclosed: false },
      last_activity: { value: '2026-06-17T14:23:00Z', disclosed: false },
    },
  },
};

/**
 * Demo walkthrough narrative — 8 steps matching the 5-minute demo flow.
 */
export const DEMO_STEPS: readonly DemoStep[] = [
  {
    step: 1,
    title: 'Reviewer Dashboard',
    description: 'Overview of active counterparty cases awaiting review.',
    route: '/dashboard',
  },
  {
    step: 2,
    title: 'Case Details',
    description: 'Select a case to see proof collection progress.',
    route: '/portal/__CASE_ID__',
  },
  {
    step: 3,
    title: 'Counterparty Portal',
    description: 'The guided proof submission journey for the counterparty.',
    route: '/portal/__CASE_ID__',
  },
  {
    step: 4,
    title: 'Privacy Architecture',
    description: 'See which fields the AI agent actually receives vs. full credentials.',
    route: '/privacy/__CASE_ID__',
  },
  {
    step: 5,
    title: 'Selective Disclosure',
    description: 'Watch fields redact in real-time as privacy filtering is applied.',
    route: '/privacy/__CASE_ID__',
  },
  {
    step: 6,
    title: 'Credential Types',
    description: 'Switch between entity, signer, compliance, and ownership credentials.',
    route: '/privacy/__CASE_ID__',
  },
  {
    step: 7,
    title: 'Verification Flow',
    description: 'Observe how disclosed facts feed into AI reasoning.',
    route: '/portal/__CASE_ID__',
  },
  {
    step: 8,
    title: 'System Health',
    description: 'Agent identity, uptime, and infrastructure status.',
    route: '/system',
  },
] as const;
