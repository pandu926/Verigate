//! Database operations for the assessments table.
//!
//! Provides insert and query functions for persisting and retrieving
//! AI-generated assessment records.

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::assessment::{Assessment, NewAssessment};
use crate::error::AppError;

/// Insert a new assessment into the database.
pub async fn insert_assessment(
    pool: &PgPool,
    assessment: &NewAssessment,
) -> Result<Assessment, AppError> {
    let row = sqlx::query_as::<_, Assessment>(
        r#"
        INSERT INTO assessments (case_id, summary_text, decision, evidence_links, confidence, agent_outputs, dynamic_requirements)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, case_id, summary_text, decision, evidence_links, confidence, agent_outputs, dynamic_requirements, created_at
        "#,
    )
    .bind(assessment.case_id)
    .bind(&assessment.summary_text)
    .bind(&assessment.decision)
    .bind(&assessment.evidence_links)
    .bind(assessment.confidence)
    .bind(&assessment.agent_outputs)
    .bind(&assessment.dynamic_requirements)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Retrieve the most recent assessment for a given case.
pub async fn get_latest_assessment(
    pool: &PgPool,
    case_id: Uuid,
) -> Result<Option<Assessment>, AppError> {
    let assessment = sqlx::query_as::<_, Assessment>(
        r#"
        SELECT id, case_id, summary_text, decision, evidence_links, confidence, agent_outputs, dynamic_requirements, created_at
        FROM assessments
        WHERE case_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(case_id)
    .fetch_optional(pool)
    .await?;

    Ok(assessment)
}

/// List all assessments for a given case, most recent first.
pub async fn list_assessments(
    pool: &PgPool,
    case_id: Uuid,
) -> Result<Vec<Assessment>, AppError> {
    let assessments = sqlx::query_as::<_, Assessment>(
        r#"
        SELECT id, case_id, summary_text, decision, evidence_links, confidence, agent_outputs, dynamic_requirements, created_at
        FROM assessments
        WHERE case_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    Ok(assessments)
}
