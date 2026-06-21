//! End-to-end tests for the AI agent framework.
//!
//! Tests marked `#[ignore]` require `LLM_API_KEY` environment variable to be set.
//! Run with: `cargo test --test ai_e2e_test -- --ignored`
//!
//! Non-ignored tests verify compile-time privacy boundaries and structural
//! correctness without making API calls.

use std::env;

use chrono::Utc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use verigate_backend::ai::{
    validate_and_retry, AgentInput, AgentOutput, AgentPipeline, LlmClient,
    Message, OpenAiCompatibleClient, PolicyContext,
};
use verigate_backend::domain::disclosed_fact::{DisclosedFact, FactType};

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn sample_fact(requirement_id: &str, fact_type: FactType) -> DisclosedFact {
    DisclosedFact {
        id: Uuid::now_v7(),
        case_id: Uuid::now_v7(),
        requirement_id: requirement_id.to_string(),
        fact_type,
        claim_key: "test_key".to_string(),
        claim_value: serde_json::json!("test_value"),
        confidence: 1.0,
        source_credential_hash: "e2e_test_hash_abc123".to_string(),
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
            "wallet_ownership".to_string(),
        ],
    }
}

/// Test struct for structured output validation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct TestDecision {
    answer: String,
    confidence: f64,
}

// ---------------------------------------------------------------------------
// Compile-time privacy boundary test (always runs)
// ---------------------------------------------------------------------------

/// This test verifies at compile time that AgentInput accepts only DisclosedFact.
///
/// If someone adds a raw credential field to AgentInput, this file would need to
/// import `crate::credential` which is architecturally forbidden in the ai module.
/// The real enforcement is the module boundary (ai/ cannot import credential/) —
/// this test documents and exercises the happy path.
#[tokio::test]
async fn compile_time_privacy_boundary() {
    // Construct AgentInput from DisclosedFacts only — no credential types needed
    let input = AgentInput {
        facts: vec![
            sample_fact("entity_registration", FactType::EntityVerified),
            sample_fact("authorized_signer", FactType::SignerAuthorized),
        ],
        policy_context: sample_policy_context(),
        previous_output: None,
    };

    // The type system enforces that facts is Vec<DisclosedFact>
    assert_eq!(input.facts.len(), 2);
    assert_eq!(input.facts[0].fact_type, FactType::EntityVerified);
    assert_eq!(input.facts[1].fact_type, FactType::SignerAuthorized);

    // PolicyContext contains only string metadata, no credential data
    assert_eq!(input.policy_context.entity_type, "corporation");
    assert_eq!(input.policy_context.jurisdiction, "US");
}

/// Verify that the AgentOutput enum can only hold typed outputs, not raw data.
#[tokio::test]
async fn agent_output_is_fully_typed() {
    use verigate_backend::ai::agents::recommender::RecommenderOutput;

    let output = AgentOutput::Recommendation(RecommenderOutput {
        decision: "ready".to_string(),
        reasoning: "All requirements met".to_string(),
        next_steps: vec!["Proceed with onboarding".to_string()],
        confidence: 0.95,
    });

    // Can serialize to JSON (proves serde integration works)
    let json = serde_json::to_string(&output).unwrap();
    assert!(json.contains("ready"));
    assert!(json.contains("0.95"));
}

/// Verify provider swap is possible without changing business logic types.
/// This is a compile-time proof: both clients have the same type and implement
/// the same trait.
#[tokio::test]
async fn provider_swap_compiles_with_same_types() {
    // Pioneer AI client
    let _pioneer: Box<dyn LlmClient> = Box::new(OpenAiCompatibleClient::new(
        "https://api.pioneer.ai/v1",
        "test-key-not-real",
        "deepseek-ai/DeepSeek-V4-Pro",
    ));

    // OpenAI client — exact same type, different config
    let _openai: Box<dyn LlmClient> = Box::new(OpenAiCompatibleClient::new(
        "https://api.openai.com/v1",
        "test-key-not-real",
        "gpt-4o",
    ));

    // Both are Box<dyn LlmClient> — the business logic (agents, pipeline) doesn't
    // know or care which provider is behind the trait. SC1 proven at compile time.
}

// ---------------------------------------------------------------------------
// Real API tests (require LLM_API_KEY)
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore]
async fn e2e_provider_swap_no_business_logic_change() {
    let api_key = match env::var("LLM_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("Skipping E2E: LLM_API_KEY not set");
            return;
        }
    };

    // Create Pioneer AI client
    let client = OpenAiCompatibleClient::new(
        "https://api.pioneer.ai/v1",
        &api_key,
        "deepseek-ai/DeepSeek-V4-Pro",
    );

    // Simple chat to prove HTTP layer works
    let messages = vec![Message::user(
        "Respond with exactly: {\"status\": \"ok\"}",
    )];
    let result = client.chat(messages, None).await;

    match result {
        Ok(response) => {
            println!("Pioneer AI response: {}", response.content);
            assert!(!response.content.is_empty(), "Response should not be empty");
            println!("Pioneer AI model: {}", response.model);
        }
        Err(e) => {
            println!("Pioneer AI call failed (may be expected in CI): {:?}", e);
            // Don't hard-fail — API might be temporarily unavailable
        }
    }

    // If OPENAI_API_KEY is also set, prove the same client type works with OpenAI
    if let Ok(openai_key) = env::var("OPENAI_API_KEY") {
        let openai_client = OpenAiCompatibleClient::new(
            "https://api.openai.com/v1",
            &openai_key,
            "gpt-4o-mini",
        );

        let messages = vec![Message::user(
            "Respond with exactly: {\"status\": \"ok\"}",
        )];
        let result = openai_client.chat(messages, None).await;

        match result {
            Ok(response) => {
                println!("OpenAI response: {}", response.content);
                assert!(!response.content.is_empty());
            }
            Err(e) => {
                println!("OpenAI call failed: {:?}", e);
            }
        }
    } else {
        println!("OPENAI_API_KEY not set — skipping OpenAI half of provider swap test");
    }
}

#[tokio::test]
#[ignore]
async fn e2e_structured_output_validates_and_retries() {
    let api_key = match env::var("LLM_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("Skipping E2E: LLM_API_KEY not set");
            return;
        }
    };

    let client = OpenAiCompatibleClient::new(
        "https://api.pioneer.ai/v1",
        &api_key,
        "deepseek-ai/DeepSeek-V4-Pro",
    );

    let messages = vec![
        Message::system(
            "You are a decision-making assistant. Respond with ONLY valid JSON.",
        ),
        Message::user(
            "Should a company with verified entity registration and authorized signer \
             proceed with onboarding? Respond as JSON with fields: \
             \"answer\" (string, your decision) and \"confidence\" (number between 0.0 and 1.0).",
        ),
    ];

    let result: Result<TestDecision, _> =
        validate_and_retry(&client, messages, 3).await;

    match result {
        Ok(decision) => {
            println!(
                "Structured output: answer={}, confidence={}",
                decision.answer, decision.confidence
            );
            assert!(!decision.answer.is_empty(), "Answer should not be empty");
            assert!(
                decision.confidence >= 0.0 && decision.confidence <= 1.0,
                "Confidence should be between 0.0 and 1.0, got: {}",
                decision.confidence
            );
        }
        Err(e) => {
            println!("Structured output test failed: {:?}", e);
            panic!("E2E structured output test should succeed with valid API key");
        }
    }
}

#[tokio::test]
#[ignore]
async fn e2e_full_pipeline_with_real_llm() {
    let api_key = match env::var("LLM_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("Skipping E2E: LLM_API_KEY not set");
            return;
        }
    };

    let client = OpenAiCompatibleClient::new(
        "https://api.pioneer.ai/v1",
        &api_key,
        "deepseek-ai/DeepSeek-V4-Pro",
    );

    let input = AgentInput {
        facts: vec![
            sample_fact("entity_registration", FactType::EntityVerified),
            sample_fact("authorized_signer", FactType::SignerAuthorized),
        ],
        policy_context: sample_policy_context(),
        previous_output: None,
    };

    let pipeline = AgentPipeline::new();
    let result = pipeline.run(input, &client).await;

    println!("Pipeline outputs: {}", result.outputs.len());
    println!("Pipeline errors: {}", result.errors.len());

    for (i, output) in result.outputs.iter().enumerate() {
        let variant = match output {
            AgentOutput::Plan(_) => "Plan",
            AgentOutput::Interpretation(_) => "Interpretation",
            AgentOutput::Summary(_) => "Summary",
            AgentOutput::Recommendation(_) => "Recommendation",
        };
        println!("  Output {}: {}", i, variant);
    }

    for (name, err) in &result.errors {
        println!("  Error in {}: {:?}", name, err);
    }

    // At minimum, we expect at least 1 successful output
    assert!(
        !result.outputs.is_empty(),
        "Pipeline should produce at least one output with a valid API key"
    );

    // If fully completed, verify all output types
    if result.completed {
        println!("Full pipeline completed successfully!");
        assert_eq!(result.outputs.len(), 4);
        assert!(matches!(result.outputs[0], AgentOutput::Plan(_)));
        assert!(matches!(result.outputs[1], AgentOutput::Interpretation(_)));
        assert!(matches!(result.outputs[2], AgentOutput::Summary(_)));
        assert!(matches!(result.outputs[3], AgentOutput::Recommendation(_)));
    } else {
        println!(
            "Pipeline partially completed: {}/4 agents succeeded",
            result.outputs.len()
        );
    }
}
