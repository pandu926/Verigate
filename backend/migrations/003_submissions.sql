-- Submissions table: tracks verifiable presentation submissions per case/requirement.
-- Separate from the Phase 1 placeholder `credentials` table.

CREATE TABLE submissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases(id),
    requirement_claim_type TEXT NOT NULL,
    credential_type TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    raw_vp JSONB NOT NULL,
    extracted_claims JSONB,
    failure_reason TEXT,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    verified_at TIMESTAMPTZ,
    submitted_by TEXT NOT NULL
);

CREATE INDEX idx_submissions_case_id ON submissions(case_id);
CREATE INDEX idx_submissions_case_requirement ON submissions(case_id, requirement_claim_type);
