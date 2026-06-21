//! Tests for all 4 agent roles and pipeline orchestration.
//!
//! Uses MockLlmClient instances that return pre-crafted valid JSON for each
//! agent's output type, proving typed output parsing and pipeline chaining.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

// Output types used indirectly via AgentOutput enum pattern matching.
// Imported to confirm they are publicly accessible from the crate.
#[allow(unused_imports)]
use verigate_backend::ai::agents::interpreter::InterpreterOutput;
#[allow(unused_imports)]
use verigate_backend::ai::agents::planner::PlannerOutput;
#[allow(unused_imports)]
use verigate_backend::ai::agents::recommender::RecommenderOutput;
#[allow(unused_imports)]
use verigate_backend::ai::agents::summarizer::SummarizerOutput;
use verigate_backend::ai::{
    Agent, AgentInput, AgentOutput, AgentPipeline, LlmClient, LlmError,
    LlmResponse, Message, PolicyContext,
};
use verigate_backend::domain::disclosed_fact::{DisclosedFact, FactType};

// ---------------------------------------------------------------------------
// Mock LLM Client
// ---------------------------------------------------------------------------

struct MockLlmClient {
    responses: Arc<Mutex<VecDeque<Result<LlmResponse, LlmError>>>>,
}

impl MockLlmClient {
    fn new(responses: Vec<Result<LlmResponse, LlmError>>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(VecDeque::from(responses))),
        }
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn chat(
        &self,
        _messages: Vec<Message>,
        _schema: Option<&serde_json::Value>,
    ) -> Result<LlmResponse, LlmError> {
        let mut queue = self.responses.lock().unwrap();
        queue.pop_front().unwrap_or(Err(LlmError::NetworkError(
            "No more canned responses".to_string(),
        )))
    }
}

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn ok_response(content: &str) -> Result<LlmResponse, LlmError> {
    Ok(LlmResponse {
        content: content.to_string(),
        model: "test-model".to_string(),
        usage: None,
    })
}

fn sample_fact(requirement_id: &str, fact_type: FactType) -> DisclosedFact {
    DisclosedFact {
        id: Uuid::now_v7(),
        case_id: Uuid::now_v7(),
        requirement_id: requirement_id.to_string(),
        fact_type,
        claim_key: "test_key".to_string(),
        claim_value: serde_json::json!("test_value"),
        confidence: 1.0,
        source_credential_hash: "abc123def456".to_string(),
        verified_at: Utc::now(),
    }
}

fn sample_policy_context() -> PolicyContext {
    PolicyContext {
        workflow_type: "kyb_onboarding".to_string(),
        entity_type: "corporation".to_string(),
        jurisdiction: "US".to_string(),
        requirements: vec![
            "entity_registration".to_string(),
            "authorized_signer".to_string(),
        ],
    }
}

fn sample_agent_input() -> AgentInput {
    AgentInput {
        facts: vec![
            sample_fact("entity_registration", FactType::EntityVerified),
            sample_fact("authorized_signer", FactType::SignerAuthorized),
        ],
        policy_context: sample_policy_context(),
        previous_output: None,
    }
}

// ---------------------------------------------------------------------------
// Valid JSON responses for each agent
// ---------------------------------------------------------------------------

const PLANNER_JSON: &str = r#"{
    "required_proofs": [
        {
            "requirement_id": "wallet_proof",
            "description": "Proof of wallet ownership via on-chain signature",
            "credential_type": "WalletOwnershipCredential",
            "priority": "high"
        }
    ],
    "reasoning": "Entity registration and signer authorization are verified. Wallet ownership proof is still required.",
    "priority_order": ["wallet_proof"]
}"#;

const INTERPRETER_JSON: &str = r#"{
    "interpretations": [
        {
            "fact_id": "entity_registration",
            "meaning": "The entity is a registered corporation in the US jurisdiction",
            "strength": "strong",
            "concerns": []
        },
        {
            "fact_id": "authorized_signer",
            "meaning": "The signer has been authorized to act on behalf of the entity",
            "strength": "strong",
            "concerns": ["Authorization scope not specified"]
        }
    ],
    "gaps": ["No wallet ownership proof provided"],
    "confidence_assessment": "High confidence in entity and signer verification. Wallet proof gap remains."
}"#;

const SUMMARIZER_JSON: &str = r#"{
    "established": [
        "Entity is a registered US corporation",
        "Signer is authorized representative"
    ],
    "missing": ["Wallet ownership verification"],
    "risks": [
        {
            "description": "Without wallet proof, fund disbursement cannot be verified",
            "severity": "medium",
            "related_facts": ["entity_registration"]
        }
    ],
    "overall_risk_level": "medium"
}"#;

const RECOMMENDER_JSON: &str = r#"{
    "decision": "more_proof_required",
    "reasoning": "Entity and signer are verified but wallet ownership proof is missing. Cannot proceed without confirming fund disbursement target.",
    "next_steps": ["Request WalletOwnershipCredential from applicant", "Set 48h deadline for submission"],
    "confidence": 0.85
}"#;

// ---------------------------------------------------------------------------
// Agent tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn planner_agent_produces_typed_output() {
    use verigate_backend::ai::agents::planner::RequirementPlannerAgent;

    let client = MockLlmClient::new(vec![ok_response(PLANNER_JSON)]);
    let agent = RequirementPlannerAgent::new();
    let input = sample_agent_input();

    let result = agent.run(&input, &client).await;
    assert!(result.is_ok(), "Planner agent failed: {:?}", result.err());

    match result.unwrap() {
        AgentOutput::Plan(output) => {
            assert_eq!(output.required_proofs.len(), 1);
            assert_eq!(output.required_proofs[0].requirement_id, "wallet_proof");
            assert_eq!(output.required_proofs[0].priority, "high");
            assert!(!output.reasoning.is_empty());
            assert_eq!(output.priority_order, vec!["wallet_proof"]);
        }
        other => panic!("Expected AgentOutput::Plan, got: {:?}", other),
    }
}

#[tokio::test]
async fn interpreter_agent_produces_typed_output() {
    use verigate_backend::ai::agents::interpreter::VerificationInterpreterAgent;

    let client = MockLlmClient::new(vec![ok_response(INTERPRETER_JSON)]);
    let agent = VerificationInterpreterAgent::new();
    let input = sample_agent_input();

    let result = agent.run(&input, &client).await;
    assert!(
        result.is_ok(),
        "Interpreter agent failed: {:?}",
        result.err()
    );

    match result.unwrap() {
        AgentOutput::Interpretation(output) => {
            assert_eq!(output.interpretations.len(), 2);
            assert_eq!(output.interpretations[0].fact_id, "entity_registration");
            assert_eq!(output.interpretations[0].strength, "strong");
            assert_eq!(output.gaps.len(), 1);
            assert!(!output.confidence_assessment.is_empty());
        }
        other => panic!("Expected AgentOutput::Interpretation, got: {:?}", other),
    }
}

#[tokio::test]
async fn summarizer_agent_produces_typed_output() {
    use verigate_backend::ai::agents::summarizer::RiskSummarizerAgent;

    let client = MockLlmClient::new(vec![ok_response(SUMMARIZER_JSON)]);
    let agent = RiskSummarizerAgent::new();
    let input = sample_agent_input();

    let result = agent.run(&input, &client).await;
    assert!(
        result.is_ok(),
        "Summarizer agent failed: {:?}",
        result.err()
    );

    match result.unwrap() {
        AgentOutput::Summary(output) => {
            assert_eq!(output.established.len(), 2);
            assert_eq!(output.missing.len(), 1);
            assert_eq!(output.risks.len(), 1);
            assert_eq!(output.risks[0].severity, "medium");
            assert_eq!(output.overall_risk_level, "medium");
        }
        other => panic!("Expected AgentOutput::Summary, got: {:?}", other),
    }
}

#[tokio::test]
async fn recommender_agent_produces_typed_output() {
    use verigate_backend::ai::agents::recommender::DecisionRecommenderAgent;

    let client = MockLlmClient::new(vec![ok_response(RECOMMENDER_JSON)]);
    let agent = DecisionRecommenderAgent::new();
    let input = sample_agent_input();

    let result = agent.run(&input, &client).await;
    assert!(
        result.is_ok(),
        "Recommender agent failed: {:?}",
        result.err()
    );

    match result.unwrap() {
        AgentOutput::Recommendation(output) => {
            assert_eq!(output.decision, "more_proof_required");
            assert!(!output.reasoning.is_empty());
            assert_eq!(output.next_steps.len(), 2);
            assert!(output.confidence >= 0.0 && output.confidence <= 1.0);
            assert_eq!(output.confidence, 0.85);
        }
        other => panic!("Expected AgentOutput::Recommendation, got: {:?}", other),
    }
}

#[tokio::test]
async fn agent_input_contains_only_disclosed_facts() {
    // This test verifies at compile time that AgentInput.facts is Vec<DisclosedFact>.
    // If someone changes the type to a raw credential, this file would fail to compile
    // because DisclosedFact is the only type imported from the domain layer.
    let input = sample_agent_input();

    // Verify the facts field holds DisclosedFact instances
    assert_eq!(input.facts.len(), 2);
    assert_eq!(input.facts[0].fact_type, FactType::EntityVerified);
    assert_eq!(input.facts[1].fact_type, FactType::SignerAuthorized);

    // Verify policy context is accessible
    assert_eq!(input.policy_context.workflow_type, "kyb_onboarding");
}

#[tokio::test]
async fn pipeline_returns_partial_results_on_failure() {
    // First 2 agents succeed (planner + interpreter), then LLM error for summarizer
    let client = MockLlmClient::new(vec![
        ok_response(PLANNER_JSON),
        ok_response(INTERPRETER_JSON),
        Err(LlmError::NetworkError("Connection refused".to_string())),
        // Recommender gets None as previous_output but still needs valid response
        ok_response(RECOMMENDER_JSON),
    ]);

    let pipeline = AgentPipeline::new();
    let input = sample_agent_input();

    let result = pipeline.run(input, &client).await;

    // Should have 3 successful outputs (planner, interpreter, recommender)
    // and 1 error (summarizer)
    assert_eq!(result.outputs.len(), 3);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.errors[0].0, "RiskSummarizer");
    assert!(!result.completed);
}

#[tokio::test]
async fn pipeline_completes_successfully_with_all_agents() {
    let client = MockLlmClient::new(vec![
        ok_response(PLANNER_JSON),
        ok_response(INTERPRETER_JSON),
        ok_response(SUMMARIZER_JSON),
        ok_response(RECOMMENDER_JSON),
    ]);

    let pipeline = AgentPipeline::new();
    let input = sample_agent_input();

    let result = pipeline.run(input, &client).await;

    assert_eq!(result.outputs.len(), 4);
    assert!(result.errors.is_empty());
    assert!(result.completed);

    // Verify output types in order
    assert!(matches!(result.outputs[0], AgentOutput::Plan(_)));
    assert!(matches!(result.outputs[1], AgentOutput::Interpretation(_)));
    assert!(matches!(result.outputs[2], AgentOutput::Summary(_)));
    assert!(matches!(result.outputs[3], AgentOutput::Recommendation(_)));
}

#[tokio::test]
async fn pipeline_handles_empty_input() {
    let client = MockLlmClient::new(vec![]);

    let pipeline = AgentPipeline::new();
    let input = AgentInput {
        facts: vec![], // Empty facts
        policy_context: sample_policy_context(),
        previous_output: None,
    };

    let result = pipeline.run(input, &client).await;

    // All agents should error with InputEmpty (planner checks requirements too)
    // Planner has requirements so it may not error, but others will
    assert!(!result.completed);
    assert!(!result.errors.is_empty());
}
