//! Verification Interpreter agent.
//!
//! Interprets the significance of each disclosed fact, assesses evidence
//! strength, and identifies gaps in the verification record.

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ai::agents::{Agent, AgentError, AgentInput, AgentOutput};
use crate::ai::llm::{LlmClient, Message};
use crate::ai::schema::validate_and_retry_default;

/// Output of the VerificationInterpreterAgent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InterpreterOutput {
    /// Per-fact interpretation of meaning and strength.
    pub interpretations: Vec<FactInterpretation>,
    /// Identified gaps in the evidence record.
    pub gaps: Vec<String>,
    /// Overall confidence assessment of the disclosed facts.
    pub confidence_assessment: String,
}

/// Interpretation of a single disclosed fact.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FactInterpretation {
    /// The fact ID being interpreted.
    pub fact_id: String,
    /// What this fact means in the compliance context.
    pub meaning: String,
    /// Strength of evidence: "strong", "moderate", or "weak".
    pub strength: String,
    /// Any concerns about this fact.
    pub concerns: Vec<String>,
}

/// Agent that interprets the significance of disclosed facts.
pub struct VerificationInterpreterAgent;

impl VerificationInterpreterAgent {
    pub fn new() -> Self {
        Self
    }

    fn build_messages(&self, input: &AgentInput) -> Vec<Message> {
        let system_prompt = format!(
            "You are a verification interpreter. Your role is to analyze each \
             disclosed fact and assess its significance for compliance.\n\n\
             Workflow type: {}\n\
             Entity type: {}\n\
             Jurisdiction: {}\n\n\
             For each fact, determine:\n\
             - Its meaning in the compliance context\n\
             - The strength of evidence (strong/moderate/weak)\n\
             - Any concerns or limitations\n\n\
             Also identify gaps where expected evidence is missing.\n\n\
             Respond with ONLY valid JSON.",
            input.policy_context.workflow_type,
            input.policy_context.entity_type,
            input.policy_context.jurisdiction,
        );

        let facts_json = serde_json::to_string_pretty(&input.facts)
            .unwrap_or_else(|_| "[]".to_string());

        let mut messages = vec![
            Message::system(system_prompt),
            Message::user(format!("Disclosed facts:\n{facts_json}")),
        ];

        if let Some(prev) = &input.previous_output {
            messages.push(Message::user(format!(
                "Previous agent (Planner) output:\n{}",
                serde_json::to_string_pretty(prev).unwrap_or_default()
            )));
        }

        messages
    }
}

impl Default for VerificationInterpreterAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for VerificationInterpreterAgent {
    fn name(&self) -> &'static str {
        "VerificationInterpreter"
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
        let output: InterpreterOutput =
            validate_and_retry_default(client, messages).await?;
        Ok(AgentOutput::Interpretation(output))
    }
}
