//! Requirement Planner agent.
//!
//! Analyzes policy context and disclosed facts to determine what additional
//! proofs are needed for compliance.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ai::agents::{Agent, AgentError, AgentInput, AgentOutput};
use crate::ai::llm::{LlmClient, Message};
use crate::ai::schema::validate_and_retry_default;

/// Output of the RequirementPlannerAgent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PlannerOutput {
    /// List of proofs still required for compliance.
    pub required_proofs: Vec<RequiredProof>,
    /// The agent's reasoning for its selections.
    pub reasoning: String,
    /// Suggested priority order of requirement IDs.
    pub priority_order: Vec<String>,
}

/// A single proof requirement identified by the planner.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RequiredProof {
    /// The requirement ID this proof satisfies.
    pub requirement_id: String,
    /// Human-readable description of what's needed.
    pub description: String,
    /// The type of credential expected.
    pub credential_type: String,
    /// Priority level: "critical", "high", "medium", or "low".
    pub priority: String,
}

/// Agent that plans what proofs are still required.
pub struct RequirementPlannerAgent;

impl RequirementPlannerAgent {
    pub fn new() -> Self {
        Self
    }

    fn build_messages(&self, input: &AgentInput) -> Vec<Message> {
        let system_prompt = format!(
            "You are a compliance requirement planner. Your role is to analyze the \
             policy requirements and the facts already disclosed, then determine what \
             additional proofs are still needed.\n\n\
             Workflow type: {}\n\
             Entity type: {}\n\
             Jurisdiction: {}\n\
             Policy requirements: {:?}\n\n\
             Analyze the disclosed facts and identify gaps. For each missing proof, \
             specify the requirement ID, a description of what's needed, the expected \
             credential type, and the priority (critical/high/medium/low).\n\n\
             When facts are empty, suggest initial requirements based on policy context \
             alone. This is used for case initialization to provide early guidance on \
             what proofs will likely be needed.\n\n\
             Respond with ONLY valid JSON.",
            input.policy_context.workflow_type,
            input.policy_context.entity_type,
            input.policy_context.jurisdiction,
            input.policy_context.requirements,
        );

        let facts_json = serde_json::to_string_pretty(&input.facts)
            .unwrap_or_else(|_| "[]".to_string());

        vec![
            Message::system(system_prompt),
            Message::user(format!("Disclosed facts:\n{facts_json}")),
        ]
    }
}

impl Default for RequirementPlannerAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for RequirementPlannerAgent {
    fn name(&self) -> &'static str {
        "RequirementPlanner"
    }

    async fn run(
        &self,
        input: &AgentInput,
        client: &dyn LlmClient,
    ) -> Result<AgentOutput, AgentError> {
        if input.facts.is_empty() && input.policy_context.requirements.is_empty() {
            return Err(AgentError::InputEmpty);
        }

        let messages = self.build_messages(input);
        let output: PlannerOutput = validate_and_retry_default(client, messages).await?;
        Ok(AgentOutput::Plan(output))
    }
}
