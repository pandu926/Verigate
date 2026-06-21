//! Decision Recommender agent.
//!
//! Makes a final recommendation based on the complete risk analysis,
//! selecting one of four possible decisions with supporting rationale.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ai::agents::{Agent, AgentError, AgentInput, AgentOutput};
use crate::ai::llm::{LlmClient, Message};
use crate::ai::schema::validate_and_retry_default;

/// Output of the DecisionRecommenderAgent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RecommenderOutput {
    /// Decision: "ready", "more_proof_required", "needs_review", or "blocked".
    pub decision: String,
    /// Detailed reasoning for the decision.
    pub reasoning: String,
    /// Recommended next steps.
    pub next_steps: Vec<String>,
    /// Confidence in the decision (0.0 to 1.0).
    pub confidence: f64,
}

/// Agent that makes a final compliance decision recommendation.
pub struct DecisionRecommenderAgent;

impl DecisionRecommenderAgent {
    pub fn new() -> Self {
        Self
    }

    fn build_messages(&self, input: &AgentInput) -> Vec<Message> {
        let total_requirements = input.policy_context.requirements.len();
        let total_facts = input.facts.len();

        let system_prompt = format!(
            "You are an autonomous counterparty due diligence decision agent.\n\n\
             Context:\n\
             - Workflow: {}\n\
             - Entity type: {}\n\
             - Jurisdiction: {}\n\
             - Requirements defined: {}\n\
             - Facts verified: {}\n\n\
             DECISION MATRIX (follow strictly):\n\
             - \"ready\": completeness > 70% AND no high/critical risks → APPROVE autonomously\n\
             - \"more_proof_required\": completeness < 50% OR specific critical evidence missing → REQUEST more\n\
             - \"needs_review\": completeness 50-70% with risk signals, OR conflicting evidence → ESCALATE to human\n\
             - \"blocked\": sanctions match, fraud indicators, or critical compliance failure → BLOCK immediately\n\n\
             CONFIDENCE SCORING:\n\
             - 0.0-0.3: Very low evidence, mostly guessing\n\
             - 0.3-0.5: Partial evidence, significant gaps\n\
             - 0.5-0.7: Moderate evidence, some concerns\n\
             - 0.7-0.9: Strong evidence, minor gaps only\n\
             - 0.9-1.0: Complete evidence, no concerns\n\n\
             RULES:\n\
             - Decision MUST be exactly one of: ready, more_proof_required, needs_review, blocked\n\
             - Reasoning MUST cite specific claim_keys from the facts\n\
             - next_steps MUST be actionable (e.g. \"Request UBO declaration from counterparty\")\n\
             - If more_proof_required: next_steps MUST list the specific missing credentials\n\
             - confidence MUST be a float between 0.0 and 1.0\n\n\
             Respond with ONLY valid JSON.",
            input.policy_context.workflow_type,
            input.policy_context.entity_type,
            input.policy_context.jurisdiction,
            total_requirements,
            total_facts,
        );

        let facts_json = serde_json::to_string_pretty(&input.facts)
            .unwrap_or_else(|_| "[]".to_string());

        let mut messages = vec![
            Message::system(system_prompt),
            Message::user(format!("Disclosed facts:\n{facts_json}")),
        ];

        if let Some(prev) = &input.previous_output {
            messages.push(Message::user(format!(
                "Previous agent (Summarizer) output:\n{}",
                serde_json::to_string_pretty(prev).unwrap_or_default()
            )));
        }

        messages
    }
}

impl Default for DecisionRecommenderAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for DecisionRecommenderAgent {
    fn name(&self) -> &'static str {
        "DecisionRecommender"
    }

    async fn run(
        &self,
        input: &AgentInput,
        client: &dyn LlmClient,
    ) -> Result<AgentOutput, AgentError> {
        if input.facts.is_empty() {
            return Err(AgentError::InputEmpty);
        }

        let messages = self.build_messages(input);
        let output: RecommenderOutput =
            validate_and_retry_default(client, messages).await?;
        Ok(AgentOutput::Recommendation(output))
    }
}
