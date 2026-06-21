use serde::{Deserialize, Serialize};

/// Current status of a case in the workflow lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum CaseStatus {
    Created,
    Discovery,
    Collecting,
    Verifying,
    Assessing,
    Review,
    Approved,
    Blocked,
}

/// The type of actor performing an action within the system.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ActorType {
    Ai,
    Verifier,
    Reviewer,
    Counterparty,
    System,
    ProtectedAction,
}

/// Classification of the entity being onboarded or assessed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum EntityType {
    Individual,
    Corporation,
    Fund,
    Trust,
    Dao,
    Government,
}

/// The type of workflow governing the case lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum WorkflowType {
    Onboarding,
    DueDiligence,
    Compliance,
    Revalidation,
}

/// Classification of credential types submitted for verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum CredentialType {
    Entity,
    Signer,
    Region,
    Wallet,
}
