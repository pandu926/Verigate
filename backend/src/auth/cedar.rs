//! Cedar policy engine wrapper for RBAC authorization decisions.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;

use cedar_policy::{
    Authorizer, Context, Entities, Entity, EntityId, EntityTypeName, EntityUid, PolicySet,
    Request, RestrictedExpression, Schema,
};

use crate::error::AppError;

/// Result of a Cedar authorization evaluation.
#[derive(Debug, Clone)]
pub struct AuthzDecision {
    pub allowed: bool,
    pub reason: Option<String>,
}

/// Cedar policy engine that loads schema and policies from disk.
#[derive(Debug)]
pub struct PolicyEngine {
    schema: Schema,
    policies: PolicySet,
    authorizer: Authorizer,
}

impl PolicyEngine {
    /// Construct a new PolicyEngine by loading all `.cedarschema` and `.cedar`
    /// files from the given directory path.
    pub fn from_directory(dir: &Path) -> Result<Self, AppError> {
        let schema = Self::load_schema(dir)?;
        let policies = Self::load_policies(dir)?;
        let authorizer = Authorizer::new();

        tracing::info!(
            dir = %dir.display(),
            "Cedar policy engine initialized"
        );

        Ok(Self {
            schema,
            policies,
            authorizer,
        })
    }

    /// Evaluate an authorization request against loaded policies.
    ///
    /// Constructs a Cedar entity for the principal with appropriate attributes,
    /// then evaluates the request against the policy set.
    pub fn is_authorized(
        &self,
        principal_type: &str,
        principal_id: &str,
        role_or_type: &str,
        action_name: &str,
        resource_type: &str,
        resource_id: &str,
    ) -> AuthzDecision {
        let principal_uid = match Self::make_entity_uid(principal_type, principal_id) {
            Ok(uid) => uid,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to construct principal UID");
                return AuthzDecision {
                    allowed: false,
                    reason: Some(format!("Invalid principal: {e}")),
                };
            }
        };

        let action_uid = match Self::make_entity_uid("Action", action_name) {
            Ok(uid) => uid,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to construct action UID");
                return AuthzDecision {
                    allowed: false,
                    reason: Some(format!("Invalid action: {e}")),
                };
            }
        };

        let resource_uid = match Self::make_entity_uid(resource_type, resource_id) {
            Ok(uid) => uid,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to construct resource UID");
                return AuthzDecision {
                    allowed: false,
                    reason: Some(format!("Invalid resource: {e}")),
                };
            }
        };

        // Build principal entity with role/type attribute
        let attr_key = if principal_type == "Agent" {
            "agent_type"
        } else {
            "role"
        };
        let mut attrs = HashMap::new();
        attrs.insert(
            attr_key.to_string(),
            RestrictedExpression::new_string(role_or_type.to_string()),
        );
        let principal_entity =
            Entity::new(principal_uid.clone(), attrs, HashSet::new()).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Failed to create principal entity, using uid-only");
                Entity::new_no_attrs(principal_uid.clone(), HashSet::new())
            });

        let entities = Entities::from_entities([principal_entity], None).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "Failed to create entities set");
            Entities::empty()
        });

        let context = Context::empty();

        let request = match Request::new(
            principal_uid.clone(),
            action_uid,
            resource_uid,
            context,
            Some(&self.schema),
        ) {
            Ok(req) => req,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to construct Cedar request");
                return AuthzDecision {
                    allowed: false,
                    reason: Some(format!("Request construction failed: {e}")),
                };
            }
        };

        let response = self.authorizer.is_authorized(&request, &self.policies, &entities);

        let allowed = response.decision() == cedar_policy::Decision::Allow;
        let reason = if !allowed {
            let errors: Vec<String> = response
                .diagnostics()
                .errors()
                .map(|e| e.to_string())
                .collect();
            if errors.is_empty() {
                Some("No matching permit policy".to_string())
            } else {
                Some(errors.join("; "))
            }
        } else {
            None
        };

        tracing::debug!(
            principal = %principal_id,
            action = %action_name,
            resource_type = %resource_type,
            resource_id = %resource_id,
            allowed = %allowed,
            "Cedar authorization decision"
        );

        AuthzDecision { allowed, reason }
    }

    fn load_schema(dir: &Path) -> Result<Schema, AppError> {
        let mut schema_src = String::new();

        for entry in std::fs::read_dir(dir).map_err(|e| {
            AppError::Config(format!("Cannot read policies directory {}: {e}", dir.display()))
        })? {
            let entry = entry.map_err(|e| AppError::Config(format!("Directory entry error: {e}")))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "cedarschema") {
                let content = std::fs::read_to_string(&path).map_err(|e| {
                    AppError::Config(format!("Cannot read schema file {}: {e}", path.display()))
                })?;
                schema_src.push_str(&content);
                schema_src.push('\n');
            }
        }

        if schema_src.is_empty() {
            return Err(AppError::Config(
                "No .cedarschema files found in policies directory".to_string(),
            ));
        }

        let (schema, warnings) =
            Schema::from_cedarschema_str(&schema_src).map_err(|e| {
                AppError::Config(format!("Cedar schema parse error: {e}"))
            })?;

        for warning in warnings {
            tracing::warn!(warning = %warning, "Cedar schema warning");
        }

        Ok(schema)
    }

    fn load_policies(dir: &Path) -> Result<PolicySet, AppError> {
        let mut policy_src = String::new();

        for entry in std::fs::read_dir(dir).map_err(|e| {
            AppError::Config(format!("Cannot read policies directory {}: {e}", dir.display()))
        })? {
            let entry = entry.map_err(|e| AppError::Config(format!("Directory entry error: {e}")))?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "cedar") {
                let content = std::fs::read_to_string(&path).map_err(|e| {
                    AppError::Config(format!("Cannot read policy file {}: {e}", path.display()))
                })?;
                policy_src.push_str(&content);
                policy_src.push('\n');
            }
        }

        if policy_src.is_empty() {
            return Err(AppError::Config(
                "No .cedar policy files found in policies directory".to_string(),
            ));
        }

        PolicySet::from_str(&policy_src)
            .map_err(|e| AppError::Config(format!("Cedar policy parse error: {e}")))
    }

    fn make_entity_uid(type_name: &str, id: &str) -> Result<EntityUid, String> {
        let entity_type = EntityTypeName::from_str(type_name)
            .map_err(|e| format!("Invalid entity type '{type_name}': {e}"))?;
        Ok(EntityUid::from_type_name_and_id(
            entity_type,
            EntityId::new(id),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn policies_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("policies")
    }

    fn engine() -> PolicyEngine {
        PolicyEngine::from_directory(&policies_dir()).expect("PolicyEngine should load")
    }

    #[test]
    fn test_reviewer_can_view_case() {
        let engine = engine();
        let decision =
            engine.is_authorized("User", "alice", "reviewer", "view", "Case", "case-123");
        assert!(decision.allowed, "Reviewer should be allowed to view cases");
    }

    #[test]
    fn test_reviewer_can_create_case() {
        let engine = engine();
        let decision = engine.is_authorized(
            "User",
            "alice",
            "reviewer",
            "create",
            "Application",
            "verigate",
        );
        assert!(
            decision.allowed,
            "Reviewer should be allowed to create cases"
        );
    }

    #[test]
    fn test_counterparty_cannot_create_case() {
        let engine = engine();
        let decision = engine.is_authorized(
            "User",
            "bob",
            "counterparty",
            "create",
            "Application",
            "verigate",
        );
        assert!(
            !decision.allowed,
            "Counterparty should NOT be allowed to create cases"
        );
    }

    #[test]
    fn test_counterparty_can_submit_proof() {
        let engine = engine();
        let decision = engine.is_authorized(
            "User",
            "bob",
            "counterparty",
            "submit_proof",
            "Case",
            "case-456",
        );
        assert!(
            decision.allowed,
            "Counterparty should be allowed to submit proof"
        );
    }

    #[test]
    fn test_counterparty_can_view_case() {
        let engine = engine();
        let decision = engine.is_authorized(
            "User",
            "bob",
            "counterparty",
            "view",
            "Case",
            "case-456",
        );
        assert!(
            decision.allowed,
            "Counterparty should be allowed to view cases"
        );
    }

    #[test]
    fn test_system_agent_full_access() {
        let engine = engine();
        for action in &["view", "create", "transition", "override", "submit_proof"] {
            let decision =
                engine.is_authorized("Agent", "system-bot", "system", action, "Case", "case-789");
            assert!(
                decision.allowed,
                "System agent should be allowed to {action}"
            );
        }
    }

    #[test]
    fn test_unknown_principal_denied() {
        let engine = engine();
        let decision = engine.is_authorized(
            "User",
            "unknown",
            "unknown_role",
            "view",
            "Case",
            "case-123",
        );
        assert!(
            !decision.allowed,
            "Unknown role should be denied"
        );
    }

    #[test]
    fn test_counterparty_cannot_transition() {
        let engine = engine();
        let decision = engine.is_authorized(
            "User",
            "bob",
            "counterparty",
            "transition",
            "Case",
            "case-456",
        );
        assert!(
            !decision.allowed,
            "Counterparty should NOT be allowed to transition cases"
        );
    }
}
