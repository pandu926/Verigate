use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn auto_seed(pool: &PgPool) -> anyhow::Result<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM cases")
        .fetch_one(pool)
        .await?;

    if count.0 > 0 {
        tracing::debug!("Database not empty — skipping auto-seed");
        return Ok(());
    }

    tracing::info!("Empty database detected — seeding demo data");
    seed_demo_data(pool).await?;
    tracing::info!("Demo data seeded — 3 scenarios created");
    Ok(())
}

pub async fn seed_demo_data(pool: &PgPool) -> anyhow::Result<()> {
    let now = Utc::now();

    // Deterministic UUIDs for demo consistency
    let case_a_id = Uuid::parse_str("a0000001-0000-0000-0000-000000000001")?;
    let case_b_id = Uuid::parse_str("b0000002-0000-0000-0000-000000000002")?;
    let case_c_id = Uuid::parse_str("c0000003-0000-0000-0000-000000000003")?;

    // Scenario A: Meridian Capital Partners — APPROVED
    seed_case_approved(pool, case_a_id, now - Duration::days(3)).await?;

    // Scenario B: Atlas Protocol Foundation — COLLECTING (in progress)
    seed_case_collecting(pool, case_b_id, now - Duration::days(1)).await?;

    // Scenario C: Nightfall Trading Ltd — BLOCKED
    seed_case_blocked(pool, case_c_id, now - Duration::days(2)).await?;

    Ok(())
}

async fn seed_case_approved(pool: &PgPool, case_id: Uuid, created_at: chrono::DateTime<Utc>) -> anyhow::Result<()> {
    // Schema: id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at
    // Entity name stored in relationship_goal for demo display (frontend reads this)
    sqlx::query(
        r#"INSERT INTO cases (id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at)
           VALUES ($1, 'onboarding', 'corporation', 'Meridian Capital Partners', 'US', 'investment_partner', 'approved', 'system', $2, $2)
           ON CONFLICT (id) DO UPDATE SET status = 'approved', updated_at = $2"#
    )
    .bind(case_id)
    .bind(created_at)
    .execute(pool)
    .await?;

    let events = vec![
        ("case_created", "system", created_at, r#"{"workflow_type":"onboarding","entity_name":"Meridian Capital Partners"}"#),
        ("state_transition", "system", created_at + Duration::minutes(1), r#"{"from":"created","to":"discovery"}"#),
        ("state_transition", "system", created_at + Duration::minutes(5), r#"{"from":"discovery","to":"collecting"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(2), r#"{"credential_type":"entity_registration"}"#),
        ("submission_verified", "system", created_at + Duration::hours(2) + Duration::minutes(1), r#"{"credential_type":"entity_registration","status":"verified"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(3), r#"{"credential_type":"authorized_signer"}"#),
        ("submission_verified", "system", created_at + Duration::hours(3) + Duration::minutes(1), r#"{"credential_type":"authorized_signer","status":"verified"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(5), r#"{"credential_type":"jurisdiction_compliance"}"#),
        ("submission_verified", "system", created_at + Duration::hours(5) + Duration::minutes(1), r#"{"credential_type":"jurisdiction_compliance","status":"verified"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(6), r#"{"credential_type":"beneficial_ownership"}"#),
        ("submission_verified", "system", created_at + Duration::hours(6) + Duration::minutes(1), r#"{"credential_type":"beneficial_ownership","status":"verified"}"#),
        ("state_transition", "system", created_at + Duration::hours(6) + Duration::minutes(2), r#"{"from":"collecting","to":"verifying"}"#),
        ("state_transition", "system", created_at + Duration::hours(6) + Duration::minutes(3), r#"{"from":"verifying","to":"assessing"}"#),
        ("assessment_complete", "ai", created_at + Duration::hours(7), r#"{"decision":"ready","summary":"All credentials verified. Entity is compliant."}"#),
        ("state_transition", "system", created_at + Duration::hours(7) + Duration::minutes(1), r#"{"from":"assessing","to":"review"}"#),
        ("override_decision", "reviewer", created_at + Duration::days(1), r#"{"action":"approve","rationale":"All documentation verified, counterparty meets all criteria for investment partnership."}"#),
        ("state_transition", "system", created_at + Duration::days(1) + Duration::minutes(1), r#"{"from":"review","to":"approved"}"#),
        ("protected_action_executed", "protected_action", created_at + Duration::days(1) + Duration::minutes(2), r#"{"template":"issue_onboarding_token","placeholders_present":["profile.legal_name","profile.wallet_address","profile.jurisdiction"],"execution_status":"success"}"#),
    ];

    for (action, actor_type, ts, details) in events {
        sqlx::query(
            r#"INSERT INTO audit_events (id, case_id, actor_type, actor_id, action, details, created_at)
               VALUES ($1, $2, $3, 'system', $4, $5::jsonb, $6)
               ON CONFLICT DO NOTHING"#
        )
        .bind(Uuid::new_v4())
        .bind(case_id)
        .bind(actor_type)
        .bind(action)
        .bind(details)
        .bind(ts)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_case_collecting(pool: &PgPool, case_id: Uuid, created_at: chrono::DateTime<Utc>) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO cases (id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at)
           VALUES ($1, 'onboarding', 'corporation', 'Atlas Protocol Foundation', 'SG', 'protocol_integration', 'collecting', 'system', $2, $2)
           ON CONFLICT (id) DO UPDATE SET status = 'collecting', updated_at = $2"#
    )
    .bind(case_id)
    .bind(created_at)
    .execute(pool)
    .await?;

    let events = vec![
        ("case_created", "system", created_at, r#"{"workflow_type":"onboarding","entity_name":"Atlas Protocol Foundation"}"#),
        ("state_transition", "system", created_at + Duration::minutes(1), r#"{"from":"created","to":"discovery"}"#),
        ("state_transition", "system", created_at + Duration::minutes(3), r#"{"from":"discovery","to":"collecting"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(4), r#"{"credential_type":"entity_registration"}"#),
        ("submission_verified", "system", created_at + Duration::hours(4) + Duration::minutes(1), r#"{"credential_type":"entity_registration","status":"verified"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(6), r#"{"credential_type":"authorized_signer"}"#),
    ];

    for (action, actor_type, ts, details) in events {
        sqlx::query(
            r#"INSERT INTO audit_events (id, case_id, actor_type, actor_id, action, details, created_at)
               VALUES ($1, $2, $3, 'system', $4, $5::jsonb, $6)
               ON CONFLICT DO NOTHING"#
        )
        .bind(Uuid::new_v4())
        .bind(case_id)
        .bind(actor_type)
        .bind(action)
        .bind(details)
        .bind(ts)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn seed_case_blocked(pool: &PgPool, case_id: Uuid, created_at: chrono::DateTime<Utc>) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO cases (id, workflow_type, entity_type, relationship_goal, jurisdiction, requested_outcome, status, created_by, created_at, updated_at)
           VALUES ($1, 'onboarding', 'corporation', 'Nightfall Trading Ltd', 'KY', 'trading_counterparty', 'blocked', 'system', $2, $2)
           ON CONFLICT (id) DO UPDATE SET status = 'blocked', updated_at = $2"#
    )
    .bind(case_id)
    .bind(created_at)
    .execute(pool)
    .await?;

    let events = vec![
        ("case_created", "system", created_at, r#"{"workflow_type":"onboarding","entity_name":"Nightfall Trading Ltd"}"#),
        ("state_transition", "system", created_at + Duration::minutes(1), r#"{"from":"created","to":"discovery"}"#),
        ("state_transition", "system", created_at + Duration::minutes(2), r#"{"from":"discovery","to":"collecting"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(1), r#"{"credential_type":"entity_registration"}"#),
        ("submission_verified", "system", created_at + Duration::hours(1) + Duration::minutes(1), r#"{"credential_type":"entity_registration","status":"verified"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(2), r#"{"credential_type":"authorized_signer"}"#),
        ("submission_verified", "system", created_at + Duration::hours(2) + Duration::minutes(1), r#"{"credential_type":"authorized_signer","status":"verified"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(3), r#"{"credential_type":"jurisdiction_compliance"}"#),
        ("submission_failed", "system", created_at + Duration::hours(3) + Duration::minutes(1), r#"{"credential_type":"jurisdiction_compliance","status":"failed","reason":"Untrusted issuer: did:key:z6Mk_unknown_issuer"}"#),
        ("submission_received", "counterparty", created_at + Duration::hours(4), r#"{"credential_type":"beneficial_ownership"}"#),
        ("submission_verified", "system", created_at + Duration::hours(4) + Duration::minutes(1), r#"{"credential_type":"beneficial_ownership","status":"verified"}"#),
        ("state_transition", "system", created_at + Duration::hours(5), r#"{"from":"collecting","to":"verifying"}"#),
        ("state_transition", "system", created_at + Duration::hours(5) + Duration::minutes(1), r#"{"from":"verifying","to":"assessing"}"#),
        ("assessment_complete", "ai", created_at + Duration::hours(6), r#"{"decision":"blocked","summary":"Jurisdiction compliance credential from unrecognized issuer. Cannot verify regulatory standing."}"#),
        ("state_transition", "system", created_at + Duration::hours(6) + Duration::minutes(1), r#"{"from":"assessing","to":"review"}"#),
        ("override_decision", "reviewer", created_at + Duration::hours(8), r#"{"action":"reject","rationale":"Jurisdiction compliance credential from unrecognized issuer. Counterparty must resubmit from an accredited registrar."}"#),
        ("state_transition", "system", created_at + Duration::hours(8) + Duration::minutes(1), r#"{"from":"review","to":"blocked"}"#),
    ];

    for (action, actor_type, ts, details) in events {
        sqlx::query(
            r#"INSERT INTO audit_events (id, case_id, actor_type, actor_id, action, details, created_at)
               VALUES ($1, $2, $3, 'system', $4, $5::jsonb, $6)
               ON CONFLICT DO NOTHING"#
        )
        .bind(Uuid::new_v4())
        .bind(case_id)
        .bind(actor_type)
        .bind(action)
        .bind(details)
        .bind(ts)
        .execute(pool)
        .await?;
    }

    Ok(())
}
