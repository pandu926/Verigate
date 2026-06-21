//! Requirement computation engine.
//!
//! Loads JSON requirement definitions from disk and computes which proofs
//! are required for a given case based on workflow type, entity type, and
//! relationship goal.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::domain::types::{EntityType, WorkflowType};
use crate::error::AppError;

/// A computed proof requirement returned to API consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofRequirement {
    pub claim_type: String,
    pub mandatory: bool,
    pub description: String,
    pub acceptable_proof_types: Vec<String>,
    pub status: String,
}

/// Condition that determines whether a requirement applies.
#[derive(Debug, Clone, Deserialize)]
pub struct RequirementCondition {
    /// If set, the case's relationship_goal must contain this substring.
    pub relationship_goal_contains: Option<String>,
    /// If set, the case's entity_type must be one of these values.
    pub entity_types: Option<Vec<String>>,
}

/// A single requirement definition as stored in JSON config files.
#[derive(Debug, Clone, Deserialize)]
pub struct RequirementDefinition {
    pub claim_type: String,
    pub mandatory: bool,
    pub description: String,
    pub acceptable_proof_types: Vec<String>,
    /// The specific claim names that must be disclosed to satisfy this requirement.
    /// Used by the normalizer to filter over-disclosed fields.
    #[serde(default)]
    pub required_claims: Vec<String>,
    pub conditions: Option<RequirementCondition>,
}

/// Top-level JSON config structure for a workflow's requirements.
#[derive(Debug, Clone, Deserialize)]
pub struct RequirementConfig {
    pub workflow_type: String,
    pub requirements: Vec<RequirementDefinition>,
}

/// Engine that loads requirement configs from disk and computes applicable
/// requirements for a given case configuration.
#[derive(Debug)]
pub struct RequirementEngine {
    configs: HashMap<String, RequirementConfig>,
}

impl RequirementEngine {
    /// Load all JSON requirement config files from the given directory.
    ///
    /// Each file should contain a `RequirementConfig` with a `workflow_type`
    /// field used as the lookup key.
    pub fn from_directory(dir: &Path) -> Result<Self, AppError> {
        let mut configs = HashMap::new();

        let entries = std::fs::read_dir(dir).map_err(|e| {
            AppError::Config(format!(
                "Cannot read requirements directory {}: {e}",
                dir.display()
            ))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                AppError::Config(format!("Directory entry error: {e}"))
            })?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "json") {
                let content = std::fs::read_to_string(&path).map_err(|e| {
                    AppError::Config(format!(
                        "Cannot read requirement file {}: {e}",
                        path.display()
                    ))
                })?;

                let config: RequirementConfig =
                    serde_json::from_str(&content).map_err(|e| {
                        AppError::Config(format!(
                            "Cannot parse requirement file {}: {e}",
                            path.display()
                        ))
                    })?;

                let key = config.workflow_type.clone();
                tracing::debug!(
                    file = %path.display(),
                    workflow_type = %key,
                    count = config.requirements.len(),
                    "Loaded requirement config"
                );
                configs.insert(key, config);
            }
        }

        if configs.is_empty() {
            return Err(AppError::Config(
                "No .json requirement files found in requirements directory".to_string(),
            ));
        }

        tracing::info!(
            dir = %dir.display(),
            configs_loaded = configs.len(),
            "Requirement engine initialized"
        );

        Ok(Self { configs })
    }

    /// Compute the list of applicable proof requirements for a case.
    ///
    /// Selects the config matching the workflow type, then filters requirements
    /// by evaluating any conditions against the entity type and relationship goal.
    pub fn compute_requirements(
        &self,
        workflow_type: &WorkflowType,
        entity_type: &EntityType,
        relationship_goal: &str,
    ) -> Vec<ProofRequirement> {
        let workflow_key = workflow_type_to_key(workflow_type);

        let config = match self.configs.get(&workflow_key) {
            Some(c) => c,
            None => {
                tracing::warn!(
                    workflow_type = %workflow_key,
                    "No requirement config found for workflow type"
                );
                return Vec::new();
            }
        };

        let entity_type_str = entity_type_to_string(entity_type);

        config
            .requirements
            .iter()
            .filter(|def| Self::conditions_match(def, &entity_type_str, relationship_goal))
            .map(|def| ProofRequirement {
                claim_type: def.claim_type.clone(),
                mandatory: def.mandatory,
                description: def.description.clone(),
                acceptable_proof_types: def.acceptable_proof_types.clone(),
                status: "pending".to_string(),
            })
            .collect()
    }

    /// Get the number of required_claims for a specific claim_type in a workflow.
    ///
    /// Returns None if the workflow or claim_type is not found.
    pub fn get_required_claims_count(
        &self,
        workflow_key: &str,
        claim_type: &str,
    ) -> Option<usize> {
        let config = self.configs.get(workflow_key)?;
        let def = config
            .requirements
            .iter()
            .find(|r| r.claim_type == claim_type)?;
        Some(if def.required_claims.is_empty() {
            1
        } else {
            def.required_claims.len()
        })
    }

    /// Get the required_claims list for a specific claim_type in a workflow.
    ///
    /// Returns an empty slice if the workflow or claim_type is not found.
    pub fn get_required_claims(
        &self,
        workflow_key: &str,
        claim_type: &str,
    ) -> &[String] {
        let Some(config) = self.configs.get(workflow_key) else {
            return &[];
        };
        let Some(def) = config.requirements.iter().find(|r| r.claim_type == claim_type) else {
            return &[];
        };
        &def.required_claims
    }

    /// Check whether a requirement definition's conditions are satisfied.
    /// If no conditions are set, the requirement always applies.
    fn conditions_match(
        def: &RequirementDefinition,
        entity_type_str: &str,
        relationship_goal: &str,
    ) -> bool {
        let conditions = match &def.conditions {
            Some(c) => c,
            None => return true,
        };

        // Check relationship_goal_contains condition
        if let Some(ref goal_substring) = conditions.relationship_goal_contains {
            if !relationship_goal
                .to_lowercase()
                .contains(&goal_substring.to_lowercase())
            {
                return false;
            }
        }

        // Check entity_types condition
        if let Some(ref allowed_types) = conditions.entity_types {
            if !allowed_types.iter().any(|t| t == entity_type_str) {
                return false;
            }
        }

        true
    }
}

/// Map WorkflowType enum to the string key used in JSON configs.
fn workflow_type_to_key(wt: &WorkflowType) -> String {
    match wt {
        WorkflowType::Onboarding => "Onboarding".to_string(),
        WorkflowType::DueDiligence => "DueDiligence".to_string(),
        WorkflowType::Compliance => "Compliance".to_string(),
        WorkflowType::Revalidation => "Revalidation".to_string(),
    }
}

/// Map EntityType enum to its string representation for condition matching.
fn entity_type_to_string(et: &EntityType) -> String {
    match et {
        EntityType::Individual => "Individual".to_string(),
        EntityType::Corporation => "Corporation".to_string(),
        EntityType::Fund => "Fund".to_string(),
        EntityType::Trust => "Trust".to_string(),
        EntityType::Dao => "Dao".to_string(),
        EntityType::Government => "Government".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn requirements_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("policies/requirements")
    }

    fn engine() -> RequirementEngine {
        RequirementEngine::from_directory(&requirements_dir())
            .expect("RequirementEngine should load from policies/requirements")
    }

    #[test]
    fn loads_all_configs() {
        let engine = engine();
        assert_eq!(engine.configs.len(), 4);
        assert!(engine.configs.contains_key("Onboarding"));
        assert!(engine.configs.contains_key("DueDiligence"));
        assert!(engine.configs.contains_key("Compliance"));
        assert!(engine.configs.contains_key("Revalidation"));
    }

    #[test]
    fn onboarding_corporation_web3_gets_all_requirements() {
        let engine = engine();
        let reqs = engine.compute_requirements(
            &WorkflowType::Onboarding,
            &EntityType::Corporation,
            "web3_partner_integration",
        );

        let claim_types: Vec<&str> = reqs.iter().map(|r| r.claim_type.as_str()).collect();
        assert!(claim_types.contains(&"entity_registration"));
        assert!(claim_types.contains(&"authorized_signer"));
        assert!(claim_types.contains(&"jurisdiction_compliance"));
        assert!(claim_types.contains(&"wallet_proof"));
        assert!(claim_types.contains(&"beneficial_ownership"));
        assert_eq!(reqs.len(), 5);
    }

    #[test]
    fn onboarding_individual_standard_excludes_conditional() {
        let engine = engine();
        let reqs = engine.compute_requirements(
            &WorkflowType::Onboarding,
            &EntityType::Individual,
            "standard_partner",
        );

        let claim_types: Vec<&str> = reqs.iter().map(|r| r.claim_type.as_str()).collect();
        assert!(claim_types.contains(&"entity_registration"));
        assert!(claim_types.contains(&"authorized_signer"));
        assert!(claim_types.contains(&"jurisdiction_compliance"));
        // wallet_proof requires web3 in relationship_goal
        assert!(!claim_types.contains(&"wallet_proof"));
        // beneficial_ownership requires Corporation/Fund/Trust/Dao
        assert!(!claim_types.contains(&"beneficial_ownership"));
        assert_eq!(reqs.len(), 3);
    }

    #[test]
    fn due_diligence_includes_all_mandatory() {
        let engine = engine();
        let reqs = engine.compute_requirements(
            &WorkflowType::DueDiligence,
            &EntityType::Corporation,
            "standard_financial_review",
        );

        let claim_types: Vec<&str> = reqs.iter().map(|r| r.claim_type.as_str()).collect();
        assert!(claim_types.contains(&"entity_registration"));
        assert!(claim_types.contains(&"financial_standing"));
        assert!(claim_types.contains(&"regulatory_history"));
        assert!(claim_types.contains(&"beneficial_ownership"));
        assert!(claim_types.contains(&"sanctions_screening"));
        // No web3 in relationship_goal
        assert!(!claim_types.contains(&"wallet_proof"));
        assert_eq!(reqs.len(), 5);
    }

    #[test]
    fn compliance_always_returns_four_requirements() {
        let engine = engine();
        let reqs = engine.compute_requirements(
            &WorkflowType::Compliance,
            &EntityType::Individual,
            "anything",
        );

        assert_eq!(reqs.len(), 4);
        let claim_types: Vec<&str> = reqs.iter().map(|r| r.claim_type.as_str()).collect();
        assert!(claim_types.contains(&"regulatory_license"));
        assert!(claim_types.contains(&"aml_program"));
        assert!(claim_types.contains(&"data_protection"));
        assert!(claim_types.contains(&"jurisdiction_compliance"));
    }

    #[test]
    fn revalidation_corporation_includes_change_of_control() {
        let engine = engine();
        let reqs = engine.compute_requirements(
            &WorkflowType::Revalidation,
            &EntityType::Corporation,
            "periodic_review",
        );

        let claim_types: Vec<&str> = reqs.iter().map(|r| r.claim_type.as_str()).collect();
        assert!(claim_types.contains(&"entity_registration"));
        assert!(claim_types.contains(&"current_standing"));
        assert!(claim_types.contains(&"change_of_control"));
        assert_eq!(reqs.len(), 3);
    }

    #[test]
    fn revalidation_individual_excludes_change_of_control() {
        let engine = engine();
        let reqs = engine.compute_requirements(
            &WorkflowType::Revalidation,
            &EntityType::Individual,
            "periodic_review",
        );

        let claim_types: Vec<&str> = reqs.iter().map(|r| r.claim_type.as_str()).collect();
        assert!(claim_types.contains(&"entity_registration"));
        assert!(claim_types.contains(&"current_standing"));
        assert!(!claim_types.contains(&"change_of_control"));
        assert_eq!(reqs.len(), 2);
    }

    #[test]
    fn all_requirements_have_pending_status() {
        let engine = engine();
        let reqs = engine.compute_requirements(
            &WorkflowType::Onboarding,
            &EntityType::Corporation,
            "web3_partner",
        );

        for req in &reqs {
            assert_eq!(req.status, "pending");
        }
    }
}
