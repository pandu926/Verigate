//! Database operations for credential submissions.

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::submission::{CreateSubmissionRequest, Submission, SubmissionStatus};
use crate::error::AppError;

/// Create a new submission record with status "submitted".
pub async fn create_submission(
    pool: &PgPool,
    case_id: Uuid,
    req: &CreateSubmissionRequest,
    submitted_by: &str,
) -> Result<Submission, AppError> {
    let row = sqlx::query_as::<_, Submission>(
        r#"
        INSERT INTO submissions (case_id, requirement_claim_type, credential_type, status, raw_vp, submitted_by)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, case_id, requirement_claim_type, credential_type, status,
                  raw_vp, extracted_claims, failure_reason, submitted_at, verified_at, submitted_by
        "#,
    )
    .bind(case_id)
    .bind(&req.requirement_claim_type)
    .bind(&req.credential_type)
    .bind(SubmissionStatus::Submitted)
    .bind(&req.raw_vp)
    .bind(submitted_by)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Update submission status, optionally setting extracted_claims and failure_reason.
pub async fn update_submission_status(
    pool: &PgPool,
    submission_id: Uuid,
    new_status: SubmissionStatus,
    extracted_claims: Option<&serde_json::Value>,
    failure_reason: Option<&str>,
) -> Result<Submission, AppError> {
    let verified_at = if new_status == SubmissionStatus::Verified {
        Some(Utc::now())
    } else {
        None
    };

    let row = sqlx::query_as::<_, Submission>(
        r#"
        UPDATE submissions
        SET status = $2, extracted_claims = $3, failure_reason = $4, verified_at = $5
        WHERE id = $1
        RETURNING id, case_id, requirement_claim_type, credential_type, status,
                  raw_vp, extracted_claims, failure_reason, submitted_at, verified_at, submitted_by
        "#,
    )
    .bind(submission_id)
    .bind(new_status)
    .bind(extracted_claims)
    .bind(failure_reason)
    .bind(verified_at)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Get all submissions for a given case.
pub async fn get_submissions_for_case(
    pool: &PgPool,
    case_id: Uuid,
) -> Result<Vec<Submission>, AppError> {
    let rows = sqlx::query_as::<_, Submission>(
        r#"
        SELECT id, case_id, requirement_claim_type, credential_type, status,
               raw_vp, extracted_claims, failure_reason, submitted_at, verified_at, submitted_by
        FROM submissions
        WHERE case_id = $1
        ORDER BY submitted_at DESC
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Get a single submission by ID.
pub async fn get_submission_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<Submission>, AppError> {
    let row = sqlx::query_as::<_, Submission>(
        r#"
        SELECT id, case_id, requirement_claim_type, credential_type, status,
               raw_vp, extracted_claims, failure_reason, submitted_at, verified_at, submitted_by
        FROM submissions
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}
