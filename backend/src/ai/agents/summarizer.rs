//! Risk Summarizer agent.
//!
//! Produces a structured risk summary from fact interpretations and policy
//! context, categorizing what has been established, what is missing, and
//! what risks remain.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ai::agents::{Agent, AgentError, AgentInput, AgentOutput};
use crate::ai::llm::{LlmClient, Message};
use crate::ai::schema::validate_and_retry_default;

/// Output of the RiskSummarizerAgent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SummarizerOutput {
    /// Facts and conditions that have been established.
    pub established: Vec<String>,
    /// Evidence or conditions that are still missing.
    pub missing: Vec<String>,
    /// Identified risk items with severity and related facts.
    pub risks: Vec<RiskItem>,
    /// Overall risk level: "low", "medium", "high", or "critical".
    pub overall_risk_level: String,
}

/// A single identified risk.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RiskItem {
    /// Description of the risk.
    pub description: String,
    /// Severity: "low", "medium", "high", or "critical".
    pub severity: String,
    /// IDs of facts related to this risk.
    pub related_facts: Vec<String>,
}

/// Agent that summarizes risks from verification interpretations.
pub struct RiskSummarizerAgent;

impl RiskSummarizerAgent {
    pub fn new() -> Self {
        Self
    }

    fn build_messages(&self, input: &AgentInput) -> Vec<Message> {
        let system_prompt = format!(
            "You are a risk summarizer. Your role is to produce a structured risk \
             assessment based on the verification interpretations and policy context.\n\n\
             Workflow type: {}\n\
             Entity type: {}\n\
             Jurisdiction: {}\n\
             Requirements: {:?}\n\n\
             Summarize:\n\
             - What has been established (verified facts)\n\
             - What is still missing (unmet requirements)\n\
             - Specific risks with severity ratings\n\
             - Overall risk level (low/medium/high/critical)\n\n\
             IMPORTANT: In the `related_facts` array for each risk item, you MUST \
             reference specific `claim_key` values from the input facts JSON. These \
             are used for evidence tracing. Only reference claim_keys that actually \
             appear in the disclosed facts provided.\n\n\
             Respond with ONLY valid JSON.",
            input.policy_context.workflow_type,
            input.policy_context.entity_type,
            input.policy_context.jurisdiction,
            input.policy_context.requirements,
        );

        let facts_json = serde_json::to_string_pretty(&input.facts)
            .unwrap_or_else(|_| "[]".to_string());

        let mut messages = vec![
            Message::system(system_prompt),
            Message::user(format!("Disclosed facts:\n{facts_json}")),
        ];

        if let Some(prev) = &input.previous_output {
            messages.push(Message::user(format!(
                "Previous agent (Interpreter) output:\n{}",
                serde_json::to_string_pretty(prev).unwrap_or_default()
            )));
        }

        messages
    }
}

impl Default for RiskSummarizerAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for RiskSummarizerAgent {
    fn name(&self) -> &'static str {
        "RiskSummarizer"
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
        let output: SummarizerOutput =
            validate_and_retry_default(client, messages).await?;
        Ok(AgentOutput::Summary(output))
    }
}
