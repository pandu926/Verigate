//! AI reasoning module.
//!
//! This module MUST NOT import from `crate::credential` or `crate::domain::credential`.
//! It operates exclusively on `Vec<DisclosedFact>` — the privacy-safe data structure
//! produced by the normalization layer.
//!
//! # Architectural Invariant
//!
//! The AI module sits behind a strict privacy boundary. It never receives raw
//! VerifiablePresentation, VerifiableCredential, or JWT data. All functions in
//! this module accept only `DisclosedFact` vectors as their credential-related
//! input. This ensures the AI reasoning layer cannot leak or over-process
//! sensitive credential content.
//!
//! ## Allowed imports from crate:
//! - `crate::domain::disclosed_fact::DisclosedFact`
//! - `crate::domain::disclosed_fact::FactType`
//!
//! ## Forbidden imports (enforced by code review and grep CI check):
//! - `crate::credential::*`
//! - `crate::domain::credential::*`
//! - Any raw VP/VC types

pub mod agents;
pub mod assessment_service;
pub mod llm;
pub mod pipeline;
pub mod provider;
pub mod schema;

use crate::domain::disclosed_fact::{DisclosedFact, FactType};

// Re-export key types for convenient access.
pub use agents::{Agent, AgentError, AgentInput, AgentOutput, PolicyContext};
pub use assessment_service::AssessmentService;
pub use llm::{LlmClient, LlmError, LlmResponse, Message, Role};
pub use pipeline::{AgentPipeline, PipelineResult};
pub use provider::{create_llm_client, OpenAiCompatibleClient};
pub use schema::validate_and_retry;

/// Input type for AI reasoning functions.
///
/// Wraps a set of disclosed facts for a case, providing the only data
/// the AI layer is allowed to reason over from the verification subsystem.
#[derive(Debug, Clone)]
pub struct AiInput {
    /// The disclosed facts available for reasoning.
    pub facts: Vec<DisclosedFact>,
}

impl AiInput {
    /// Create a new AI input from a vector of disclosed facts.
    pub fn new(facts: Vec<DisclosedFact>) -> Self {
        Self { facts }
    }

    /// Filter facts by type.
    pub fn facts_of_type(&self, fact_type: &FactType) -> Vec<&DisclosedFact> {
        self.facts
            .iter()
            .filter(|f| &f.fact_type == fact_type)
            .collect()
    }

    /// Check whether facts exist for a given requirement.
    pub fn has_requirement(&self, requirement_id: &str) -> bool {
        self.facts
            .iter()
            .any(|f| f.requirement_id == requirement_id)
    }

    /// Get all unique requirement IDs present in the facts.
    pub fn requirement_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self
            .facts
            .iter()
            .map(|f| f.requirement_id.as_str())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    fn sample_fact(requirement_id: &str, fact_type: FactType) -> DisclosedFact {
        DisclosedFact {
            id: Uuid::now_v7(),
            case_id: Uuid::now_v7(),
            requirement_id: requirement_id.to_string(),
            fact_type,
            claim_key: "test_key".to_string(),
            claim_value: serde_json::json!("test_value"),
            confidence: 1.0,
            source_credential_hash: "abc123".to_string(),
            verified_at: Utc::now(),
        }
    }

    #[test]
    fn ai_input_filters_by_fact_type() {
        let facts = vec![
            sample_fact("entity_registration", FactType::EntityVerified),
            sample_fact("authorized_signer", FactType::SignerAuthorized),
            sample_fact("entity_registration", FactType::EntityVerified),
        ];

        let input = AiInput::new(facts);
        let entity_facts = input.facts_of_type(&FactType::EntityVerified);
        assert_eq!(entity_facts.len(), 2);
    }

    #[test]
    fn ai_input_checks_requirement_presence() {
        let facts = vec![
            sample_fact("entity_registration", FactType::EntityVerified),
        ];

        let input = AiInput::new(facts);
        assert!(input.has_requirement("entity_registration"));
        assert!(!input.has_requirement("wallet_proof"));
    }

    #[test]
    fn ai_input_lists_unique_requirements() {
        let facts = vec![
            sample_fact("entity_registration", FactType::EntityVerified),
            sample_fact("entity_registration", FactType::EntityVerified),
            sample_fact("authorized_signer", FactType::SignerAuthorized),
        ];

        let input = AiInput::new(facts);
        let ids = input.requirement_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"entity_registration"));
        assert!(ids.contains(&"authorized_signer"));
    }
}
