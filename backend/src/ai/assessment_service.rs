//! Assessment orchestration service.
//!
//! Coordinates the AI agent pipeline with real case data to produce actionable,
//! evidence-traced assessments. Handles both full assessments and lightweight
//! case initialization calls.

use sqlx::PgPool;
use uuid::Uuid;

use crate::ai::agents::{AgentInput, AgentOutput, PolicyContext, RequirementPlannerAgent, Agent};
use crate::ai::llm::LlmClient;
use crate::ai::pipeline::AgentPipeline;
use crate::auth::requirements::RequirementEngine;
use crate::db::{assessments, audit_events, cases, disclosed_facts};
use crate::domain::assessment::{
    AssessmentDecision, DynamicRequirement, Assessment, EvidenceLink, NewAssessment,
};
use crate::domain::audit::NewAuditEvent;
use crate::domain::types::ActorType;
use crate::error::AppError;

/// Orchestrates the AI assessment pipeline for a case.
pub struct AssessmentService;

impl AssessmentService {
    /// Run a full assessment for a case.
    ///
    /// Loads case data and disclosed facts, runs the full agent pipeline,
    /// then persists the assessment with evidence links and dynamic requirements.
    pub async fn run_assessment(
        pool: &PgPool,
        client: &dyn LlmClient,
        case_id: Uuid,
        requirement_engine: &RequirementEngine,
    ) -> Result<Assessment, AppError> {
        // Load case metadata
        let case = cases::get_case(pool, case_id).await?;

        // Load disclosed facts for reasoning
        let facts = disclosed_facts::get_facts_for_case(pool, case_id).await?;

        if facts.is_empty() {
            return Err(AppError::Validation(
                "Cannot assess case with no verified facts".to_string(),
            ));
        }

        // Compute policy requirements for context
        let policy_reqs = requirement_engine.compute_requirements(
            &case.workflow_type,
            &case.entity_type,
            &case.relationship_goal,
        );
        let requirement_names: Vec<String> =
            policy_reqs.iter().map(|r| r.claim_type.clone()).collect();

        // Build pipeline input
        let policy_context = PolicyContext {
            workflow_type: serde_json::to_value(&case.workflow_type)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default(),
            entity_type: serde_json::to_value(&case.entity_type)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default(),
            jurisdiction: case.jurisdiction.clone().unwrap_or_else(|| "unknown".to_string()),
            requirements: requirement_names,
        };

        let agent_input = AgentInput {
            facts: facts.clone(),
            policy_context,
            previous_output: None,
        };

        // Run the full pipeline
        let pipeline = AgentPipeline::new();
        let result = pipeline.run(agent_input, client).await;

        // Extract outputs by type
        let mut planner_output = None;
        let mut summarizer_output = None;
        let mut recommender_output = None;

        for output in &result.outputs {
            match output {
                AgentOutput::Plan(p) => planner_output = Some(p.clone()),
                AgentOutput::Summary(s) => summarizer_output = Some(s.clone()),
                AgentOutput::Recommendation(r) => recommender_output = Some(r.clone()),
                AgentOutput::Interpretation(_) => {}
            }
        }

        // Build summary markdown
        let summary_text = build_summary_markdown(
            summarizer_output.as_ref(),
            recommender_output.as_ref(),
        );

        // Determine decision
        let decision = recommender_output
            .as_ref()
            .map(|r| AssessmentDecision::from_str_value(&r.decision))
            .unwrap_or(AssessmentDecision::NeedsReview);

        let confidence = recommender_output
            .as_ref()
            .map(|r| r.confidence)
            .unwrap_or(0.0);

        // Build evidence links from summarizer risk items
        let evidence_links = build_evidence_links(summarizer_output.as_ref(), &facts);

        // Build dynamic requirements if decision requires more proof
        let dynamic_requirements = if decision == AssessmentDecision::MoreProofRequired {
            build_dynamic_requirements(planner_output.as_ref())
        } else {
            Vec::new()
        };

        // Serialize pipeline outputs for audit
        let agent_outputs_json = serde_json::to_value(&result.outputs).ok();

        // Persist assessment
        let new_assessment = NewAssessment {
            case_id,
            summary_text,
            decision,
            evidence_links: serde_json::to_value(&evidence_links).unwrap_or_default(),
            confidence,
            agent_outputs: agent_outputs_json,
            dynamic_requirements: Some(
                serde_json::to_value(&dynamic_requirements).unwrap_or_default(),
            ),
        };

        let assessment = assessments::insert_assessment(pool, &new_assessment).await?;

        // Emit audit event
        let mut tx = pool.begin().await.map_err(|e| {
            AppError::Internal(format!("Failed to begin transaction: {e}"))
        })?;

        let _event = audit_events::insert_audit_event(
            &mut tx,
            &NewAuditEvent {
                case_id,
                actor_type: ActorType::Ai,
                actor_id: "assessment_service".to_string(),
                action: "assessment_completed".to_string(),
                details: Some(serde_json::json!({
                    "assessment_id": assessment.id,
                    "decision": serde_json::to_value(&assessment.decision).unwrap_or_default(),
                    "confidence": assessment.confidence,
                    "pipeline_completed": result.completed,
                    "errors": result.errors.len(),
                })),
            },
        )
        .await?;

        tx.commit().await.map_err(|e| {
            AppError::Internal(format!("Failed to commit audit event: {e}"))
        })?;

        Ok(assessment)
    }

    /// Lightweight case initialization call.
    ///
    /// Runs only the Planner agent with empty facts to get initial suggested
    /// requirements and workflow guidance. Stores result as an audit event.
    pub async fn initialize_case(
        pool: &PgPool,
        client: &dyn LlmClient,
        case_id: Uuid,
        requirement_engine: &RequirementEngine,
    ) -> Result<(), AppError> {
        // Load case metadata
        let case = cases::get_case(pool, case_id).await?;

        // Compute policy requirements for context
        let policy_reqs = requirement_engine.compute_requirements(
            &case.workflow_type,
            &case.entity_type,
            &case.relationship_goal,
        );
        let requirement_names: Vec<String> =
            policy_reqs.iter().map(|r| r.claim_type.clone()).collect();

        // Build minimal context (empty facts for initialization)
        let policy_context = PolicyContext {
            workflow_type: serde_json::to_value(&case.workflow_type)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default(),
            entity_type: serde_json::to_value(&case.entity_type)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default(),
            jurisdiction: case.jurisdiction.clone().unwrap_or_else(|| "unknown".to_string()),
            requirements: requirement_names,
        };

        let agent_input = AgentInput {
            facts: Vec::new(),
            policy_context,
            previous_output: None,
        };

        // Run only the planner agent
        let planner = RequirementPlannerAgent::new();
        let planner_result = planner.run(&agent_input, client).await;

        // Extract planner output for the audit event
        let details = match planner_result {
            Ok(AgentOutput::Plan(plan)) => {
                serde_json::json!({
                    "brief": plan.reasoning,
                    "suggested_requirements": plan.required_proofs,
                    "priority_order": plan.priority_order,
                })
            }
            Ok(_) => serde_json::json!({ "brief": "Planner returned unexpected output type" }),
            Err(e) => serde_json::json!({ "brief": format!("Planner initialization skipped: {e}") }),
        };

        // Emit audit event
        let mut tx = pool.begin().await.map_err(|e| {
            AppError::Internal(format!("Failed to begin transaction: {e}"))
        })?;

        let _event = audit_events::insert_audit_event(
            &mut tx,
            &NewAuditEvent {
                case_id,
                actor_type: ActorType::Ai,
                actor_id: "assessment_service".to_string(),
                action: "case_initialized".to_string(),
                details: Some(details),
            },
        )
        .await?;

        tx.commit().await.map_err(|e| {
            AppError::Internal(format!("Failed to commit audit event: {e}"))
        })?;

        Ok(())
    }
}

/// Build structured markdown summary from pipeline outputs.
fn build_summary_markdown(
    summarizer: Option<&crate::ai::agents::SummarizerOutput>,
    recommender: Option<&crate::ai::agents::RecommenderOutput>,
) -> String {
    let mut md = String::new();

    // Established section
    md.push_str("## Established\n\n");
    if let Some(s) = summarizer {
        for item in &s.established {
            md.push_str(&format!("- {item}\n"));
        }
    } else {
        md.push_str("- No data available\n");
    }

    // Missing section
    md.push_str("\n## Missing\n\n");
    if let Some(s) = summarizer {
        for item in &s.missing {
            md.push_str(&format!("- {item}\n"));
        }
    } else {
        md.push_str("- No data available\n");
    }

    // Risks section
    md.push_str("\n## Risks\n\n");
    if let Some(s) = summarizer {
        for risk in &s.risks {
            md.push_str(&format!("- **[{}]** {}\n", risk.severity, risk.description));
        }
        md.push_str(&format!("\nOverall risk level: **{}**\n", s.overall_risk_level));
    } else {
        md.push_str("- No risk data available\n");
    }

    // Recommendation section
    md.push_str("\n## Recommendation\n\n");
    if let Some(r) = recommender {
        md.push_str(&format!("{}\n\n", r.reasoning));
        if !r.next_steps.is_empty() {
            md.push_str("**Next steps:**\n");
            for step in &r.next_steps {
                md.push_str(&format!("- {step}\n"));
            }
        }
    } else {
        md.push_str("- No recommendation available\n");
    }

    md
}

/// Build evidence links by mapping risk-related fact references to actual fact IDs.
fn build_evidence_links(
    summarizer: Option<&crate::ai::agents::SummarizerOutput>,
    facts: &[crate::domain::disclosed_fact::DisclosedFact],
) -> Vec<EvidenceLink> {
    let Some(summary) = summarizer else {
        return Vec::new();
    };

    let mut links = Vec::new();

    for risk in &summary.risks {
        for fact_ref in &risk.related_facts {
            // Try to match the reference to an actual fact by claim_key or ID
            let matched_fact = facts.iter().find(|f| {
                f.claim_key == *fact_ref
                    || f.id.to_string() == *fact_ref
                    || f.requirement_id == *fact_ref
            });

            if let Some(fact) = matched_fact {
                links.push(EvidenceLink {
                    fact_id: fact.id,
                    claim_key: fact.claim_key.clone(),
                    relevance: risk.description.clone(),
                });
            }
        }
    }

    // Also link all established facts as supporting evidence
    if let Some(summary) = summarizer {
        for established in &summary.established {
            let matched_fact = facts.iter().find(|f| {
                established.contains(&f.claim_key) || established.contains(&f.requirement_id)
            });

            if let Some(fact) = matched_fact {
                // Avoid duplicates
                if !links.iter().any(|l| l.fact_id == fact.id) {
                    links.push(EvidenceLink {
                        fact_id: fact.id,
                        claim_key: fact.claim_key.clone(),
                        relevance: "Supporting established fact".to_string(),
                    });
                }
            }
        }
    }

    links
}

/// Build dynamic requirements from planner output.
fn build_dynamic_requirements(
    planner: Option<&crate::ai::agents::PlannerOutput>,
) -> Vec<DynamicRequirement> {
    let Some(plan) = planner else {
        return Vec::new();
    };

    plan.required_proofs
        .iter()
        .map(|proof| {
            DynamicRequirement::new(
                proof.requirement_id.clone(),
                proof.credential_type.clone(),
                proof.description.clone(),
            )
        })
        .collect()
}
