//! Pipeline orchestrator for chaining agents sequentially.
//!
//! Runs agents in a fixed order (Planner -> Interpreter -> Summarizer -> Recommender),
//! passing each agent's output as context to the next. Supports partial results
//! on failure — downstream agents continue even if an upstream agent fails.

use crate::ai::agents::{
    Agent, AgentError, AgentInput, AgentOutput, DecisionRecommenderAgent,
    RequirementPlannerAgent, RiskSummarizerAgent, VerificationInterpreterAgent,
};
use crate::ai::llm::LlmClient;

/// Result of a pipeline execution, containing all outputs and any errors.
#[derive(Debug)]
pub struct PipelineResult {
    /// Successfully produced outputs in execution order.
    pub outputs: Vec<AgentOutput>,
    /// Errors encountered during execution: (agent_name, error).
    pub errors: Vec<(String, AgentError)>,
    /// Whether all agents completed successfully.
    pub completed: bool,
}

/// Orchestrates a sequence of agents, chaining outputs as inputs.
pub struct AgentPipeline {
    agents: Vec<Box<dyn Agent>>,
}

impl AgentPipeline {
    /// Create a new pipeline with full 4-agent chain.
    pub fn new() -> Self {
        let agents: Vec<Box<dyn Agent>> = vec![
            Box::new(RequirementPlannerAgent::new()),
            Box::new(VerificationInterpreterAgent::new()),
            Box::new(RiskSummarizerAgent::new()),
            Box::new(DecisionRecommenderAgent::new()),
        ];
        Self { agents }
    }

    /// Run the full pipeline sequentially.
    ///
    /// Each agent receives the original input augmented with the previous
    /// agent's output as `previous_output`. On agent failure, the error is
    /// recorded and the next agent receives `previous_output = None`.
    pub async fn run(
        &self,
        input: AgentInput,
        client: &dyn LlmClient,
    ) -> PipelineResult {
        let mut outputs: Vec<AgentOutput> = Vec::new();
        let mut errors: Vec<(String, AgentError)> = Vec::new();
        let mut previous_output: Option<serde_json::Value> = input.previous_output.clone();

        for agent in &self.agents {
            let agent_input = AgentInput {
                facts: input.facts.clone(),
                policy_context: input.policy_context.clone(),
                previous_output: previous_output.clone(),
            };

            match agent.run(&agent_input, client).await {
                Ok(output) => {
                    // Serialize the output for the next agent in the chain
                    previous_output = serde_json::to_value(&output).ok();
                    outputs.push(output);
                }
                Err(err) => {
                    errors.push((agent.name().to_string(), err));
                    // Clear previous output so next agent knows upstream failed
                    previous_output = None;
                }
            }
        }

        let completed = errors.is_empty();
        PipelineResult {
            outputs,
            errors,
            completed,
        }
    }
}

impl Default for AgentPipeline {
    fn default() -> Self {
        Self::new()
    }
}
