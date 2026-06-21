-- Migration: Add requirement_id and source_credential_hash to disclosed_facts table.
-- These columns support the fact normalization and selective disclosure phase.

ALTER TABLE disclosed_facts
    ADD COLUMN IF NOT EXISTS requirement_id TEXT NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS source_credential_hash TEXT NOT NULL DEFAULT '';

-- Index for completeness queries: count facts grouped by requirement per case
CREATE INDEX IF NOT EXISTS idx_disclosed_facts_case_requirement
    ON disclosed_facts (case_id, requirement_id);
