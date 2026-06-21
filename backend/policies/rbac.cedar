// =============================================================================
// Santiora RBAC Policies
// =============================================================================
// Cedar uses default-deny. Only actions explicitly permitted here are allowed.
// Policies are evaluated at request time by the auth middleware.
// =============================================================================

// -----------------------------------------------------------------------------
// Reviewer permissions
// Reviewers are internal users who manage cases through the workflow.
// -----------------------------------------------------------------------------

// Reviewer can view any case or the application resource (list operations)
permit(
    principal is User,
    action == Action::"view",
    resource
) when {
    principal.role == "reviewer"
};

// Reviewer can create new cases
permit(
    principal is User,
    action == Action::"create",
    resource
) when {
    principal.role == "reviewer"
};

// Reviewer can transition cases between workflow states
permit(
    principal is User,
    action == Action::"transition",
    resource
) when {
    principal.role == "reviewer"
};

// Reviewer can override automated decisions (escalation path)
permit(
    principal is User,
    action == Action::"override",
    resource
) when {
    principal.role == "reviewer"
};

// -----------------------------------------------------------------------------
// Counterparty permissions
// Counterparties are external entities being onboarded. Limited access.
// -----------------------------------------------------------------------------

// Counterparty can view cases (resource-level scoping enforced at handler level)
permit(
    principal is User,
    action == Action::"view",
    resource
) when {
    principal.role == "counterparty"
};

// Counterparty can submit proof documents for their case
permit(
    principal is User,
    action == Action::"submit_proof",
    resource
) when {
    principal.role == "counterparty"
};

// -----------------------------------------------------------------------------
// System agent permissions
// Automated agents (AI, verifiers) have full access for workflow automation.
// -----------------------------------------------------------------------------

// System agent can perform any action
permit(
    principal is Agent,
    action,
    resource
) when {
    principal.agent_type == "system"
};
