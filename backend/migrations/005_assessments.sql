-- Assessment results from the AI decision engine.
-- Each assessment captures a point-in-time evaluation of a case's completeness,
-- risks, and recommended next steps.

CREATE TABLE assessments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases(id),
    summary_text TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('ready', 'more_proof_required', 'needs_review', 'blocked')),
    evidence_links JSONB NOT NULL DEFAULT '[]',
    confidence DOUBLE PRECISION NOT NULL,
    agent_outputs JSONB,
    dynamic_requirements JSONB DEFAULT '[]',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Optimized lookup for latest assessment per case.
CREATE INDEX idx_assessments_case_created ON assessments (case_id, created_at DESC);
