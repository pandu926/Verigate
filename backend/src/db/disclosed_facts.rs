//! Database operations for the disclosed_facts table.
//!
//! Provides insert and query functions for persisting and retrieving
//! normalized DisclosedFact records.

use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::disclosed_fact::DisclosedFact;
use crate::error::AppError;

/// Insert a single disclosed fact into the database.
pub async fn insert_disclosed_fact(
    pool: &PgPool,
    fact: &DisclosedFact,
) -> Result<DisclosedFact, AppError> {
    let row = sqlx::query_as::<_, DisclosedFact>(
        r#"
        INSERT INTO disclosed_facts (id, case_id, requirement_id, fact_type, claim_key, claim_value, confidence, source_credential_hash, verified_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING id, case_id, requirement_id, fact_type, claim_key, claim_value, confidence, source_credential_hash, verified_at
        "#,
    )
    .bind(fact.id)
    .bind(fact.case_id)
    .bind(&fact.requirement_id)
    .bind(&fact.fact_type)
    .bind(&fact.claim_key)
    .bind(&fact.claim_value)
    .bind(fact.confidence)
    .bind(&fact.source_credential_hash)
    .bind(fact.verified_at)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Insert multiple disclosed facts in a single transaction.
pub async fn insert_disclosed_facts(
    pool: &PgPool,
    facts: &[DisclosedFact],
) -> Result<Vec<DisclosedFact>, AppError> {
    if facts.is_empty() {
        return Ok(Vec::new());
    }

    let mut tx = pool.begin().await.map_err(|e| {
        AppError::Internal(format!("Failed to begin transaction: {e}"))
    })?;

    let mut inserted = Vec::with_capacity(facts.len());

    for fact in facts {
        let row = sqlx::query_as::<_, DisclosedFact>(
            r#"
            INSERT INTO disclosed_facts (id, case_id, requirement_id, fact_type, claim_key, claim_value, confidence, source_credential_hash, verified_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id, case_id, requirement_id, fact_type, claim_key, claim_value, confidence, source_credential_hash, verified_at
            "#,
        )
        .bind(fact.id)
        .bind(fact.case_id)
        .bind(&fact.requirement_id)
        .bind(&fact.fact_type)
        .bind(&fact.claim_key)
        .bind(&fact.claim_value)
        .bind(fact.confidence)
        .bind(&fact.source_credential_hash)
        .bind(fact.verified_at)
        .fetch_one(&mut *tx)
        .await?;

        inserted.push(row);
    }

    tx.commit().await.map_err(|e| {
        AppError::Internal(format!("Failed to commit disclosed facts: {e}"))
    })?;

    Ok(inserted)
}

/// Retrieve all disclosed facts for a given case.
pub async fn get_facts_for_case(
    pool: &PgPool,
    case_id: Uuid,
) -> Result<Vec<DisclosedFact>, AppError> {
    let facts = sqlx::query_as::<_, DisclosedFact>(
        r#"
        SELECT id, case_id, requirement_id, fact_type, claim_key, claim_value, confidence, source_credential_hash, verified_at
        FROM disclosed_facts
        WHERE case_id = $1
        ORDER BY verified_at ASC
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    Ok(facts)
}

/// Retrieve disclosed facts for a specific requirement within a case.
pub async fn get_facts_by_requirement(
    pool: &PgPool,
    case_id: Uuid,
    requirement_id: &str,
) -> Result<Vec<DisclosedFact>, AppError> {
    let facts = sqlx::query_as::<_, DisclosedFact>(
        r#"
        SELECT id, case_id, requirement_id, fact_type, claim_key, claim_value, confidence, source_credential_hash, verified_at
        FROM disclosed_facts
        WHERE case_id = $1 AND requirement_id = $2
        ORDER BY verified_at ASC
        "#,
    )
    .bind(case_id)
    .bind(requirement_id)
    .fetch_all(pool)
    .await?;

    Ok(facts)
}

/// Count disclosed facts grouped by requirement_id for a case.
///
/// Used by the completeness endpoint to compute verification progress.
pub async fn count_facts_by_requirement(
    pool: &PgPool,
    case_id: Uuid,
) -> Result<Vec<(String, i64)>, AppError> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        r#"
        SELECT requirement_id, COUNT(*) as count
        FROM disclosed_facts
        WHERE case_id = $1
        GROUP BY requirement_id
        "#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
