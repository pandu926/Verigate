//! Agent framework: trait definition, shared types, and role implementations.
//!
//! Each agent operates exclusively on `DisclosedFact` collections and policy
//! context. No raw credential types are accessible from this module.

pub mod interpreter;
pub mod planner;
pub mod recommender;
pub mod summarizer;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::ai::llm::{LlmClient, LlmError};
use crate::domain::disclosed_fact::DisclosedFact;

pub use interpreter::{InterpreterOutput, VerificationInterpreterAgent};
pub use planner::{PlannerOutput, RequirementPlannerAgent};
pub use recommender::{DecisionRecommenderAgent, RecommenderOutput};
pub use summarizer::{RiskSummarizerAgent, SummarizerOutput};

/// Policy context provided to agents for reasoning.
///
/// Contains the high-level parameters of the compliance workflow without
/// exposing any raw credential data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyContext {
    pub workflow_type: String,
    pub entity_type: String,
    pub jurisdiction: String,
    pub requirements: Vec<String>,
}

/// Input provided to each agent in the pipeline.
///
/// Contains only privacy-safe data: disclosed facts and policy context.
/// The `previous_output` field enables pipeline chaining by passing the
/// serialized output of the prior agent.
#[derive(Debug, Clone)]
pub struct AgentInput {
    pub facts: Vec<DisclosedFact>,
    pub policy_context: PolicyContext,
    pub previous_output: Option<serde_json::Value>,
}

/// Errors that can occur during agent execution.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Output validation failed: {parse_error}")]
    OutputValidation {
        raw_response: String,
        parse_error: String,
    },

    #[error("LLM communication failure: {0}")]
    LlmFailure(#[from] LlmError),

    #[error("Agent input is empty — no facts provided")]
    InputEmpty,
}

/// Unified output envelope for all agent roles.
///
/// Each variant wraps the typed output of a specific agent role.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "agent_type", content = "data")]
pub enum AgentOutput {
    Plan(PlannerOutput),
    Interpretation(InterpreterOutput),
    Summary(SummarizerOutput),
    Recommendation(RecommenderOutput),
}

/// Trait implemented by all agent roles.
///
/// Each agent transforms an `AgentInput` into a typed `AgentOutput` using
/// the provided LLM client for reasoning.
#[async_trait]
pub trait Agent: Send + Sync {
    /// Human-readable name of this agent role.
    fn name(&self) -> &'static str;

    /// Execute the agent's reasoning step.
    async fn run(
        &self,
        input: &AgentInput,
        client: &dyn LlmClient,
    ) -> Result<AgentOutput, AgentError>;
}
