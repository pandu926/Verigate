-- Core schema for Santiora
-- Cases, audit events, credentials, and disclosed facts

-- Cases: the primary workflow entity
CREATE TABLE cases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    workflow_type TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    relationship_goal TEXT NOT NULL,
    jurisdiction TEXT,
    requested_outcome TEXT,
    status TEXT NOT NULL DEFAULT 'created',
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_cases_status ON cases(status);

-- Audit events: append-only immutable timeline
CREATE TABLE audit_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases(id),
    actor_type TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    action TEXT NOT NULL,
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_audit_events_case_id ON audit_events(case_id);
CREATE INDEX idx_audit_events_case_timeline ON audit_events(case_id, created_at);

-- Enforce append-only on audit_events: block UPDATE and DELETE
CREATE OR REPLACE FUNCTION audit_events_immutable()
RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'audit_events table is append-only: % operations are not permitted', TG_OP;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_audit_events_no_update
    BEFORE UPDATE ON audit_events
    FOR EACH ROW
    EXECUTE FUNCTION audit_events_immutable();

CREATE TRIGGER trg_audit_events_no_delete
    BEFORE DELETE ON audit_events
    FOR EACH ROW
    EXECUTE FUNCTION audit_events_immutable();

-- Credentials: verifiable presentations submitted by counterparties
CREATE TABLE credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases(id),
    credential_type TEXT NOT NULL,
    issuer TEXT,
    subject TEXT,
    raw_presentation JSONB NOT NULL,
    submission_status TEXT NOT NULL DEFAULT 'pending',
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    verified_at TIMESTAMPTZ
);

CREATE INDEX idx_credentials_case_id ON credentials(case_id);

-- Disclosed facts: privacy-safe structured claims extracted from credentials
CREATE TABLE disclosed_facts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases(id),
    source_credential_id UUID REFERENCES credentials(id),
    fact_type TEXT NOT NULL,
    claim_key TEXT NOT NULL,
    claim_value JSONB NOT NULL,
    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    verified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ
);

CREATE INDEX idx_disclosed_facts_case_id ON disclosed_facts(case_id);
