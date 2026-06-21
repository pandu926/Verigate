-- Revoke UPDATE and DELETE permissions on audit_events table.
-- This adds SQL-permission-level protection on top of the trigger-based
-- immutability enforcement from migration 001.
REVOKE UPDATE, DELETE ON audit_events FROM current_user;
