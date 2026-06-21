use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::host::interfaces::kv_store;
use crate::host::interfaces::logging;
use crate::host::tenant::tenant_context;

// --- Violation Type ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ViolationType {
    OutOfOrder,
    Unauthorized,
    Expired,
    PolicyViolation,
    TamperDetected,
    NoPlan,
}

// --- Violation ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub violation_type: ViolationType,
    pub step_index: u32,
    pub caller_did: String,
    pub expected: String,
    pub actual: String,
    pub recorded_at: u64,
}

// --- Compliance Policy ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompliancePolicy {
    pub required_credential_types: Vec<String>,
    pub max_risk_tolerance: f64,
    pub sanctions_check_required: bool,
    pub auto_approve_threshold: f64,
    pub require_human_review_below: f64,
    pub max_steps: u32,
    pub ttl_secs: u64,
}

// --- Decision Result ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionResult {
    pub case_id: String,
    pub decision: String,
    pub confidence: f64,
    pub reasoning: String,
    pub evidence_chain_hash: String,
    pub policy_applied: bool,
    pub steps_completed: u32,
    pub violations_count: u32,
    pub decided_at: u64,
    pub decided_by: String,
}

// --- KV Helpers ---

const MAP_TAIL: &str = "vg-state";

fn map_name() -> String {
    let tid = tenant_context::tenant_did();
    let hex_tid: String = tid.iter().map(|b| format!("{:02x}", b)).collect();
    format!("z:{}:{}", hex_tid, MAP_TAIL)
}

fn case_key(case_id: &str, suffix: &str) -> Vec<u8> {
    format!("case:{}:{}", case_id, suffix).into_bytes()
}

pub fn kv_write(case_id: &str, suffix: &str, data: &[u8]) -> Result<(), String> {
    let key = case_key(case_id, suffix);
    let name = map_name();
    kv_store::put(&name, &key, data)
        .map_err(|e| format!("KV write failed [{}]: {}", name, e))
}

pub fn kv_read(case_id: &str, suffix: &str) -> Option<Vec<u8>> {
    let key = case_key(case_id, suffix);
    let name = map_name();
    match kv_store::get(&name, &key) {
        Ok(data) => data,
        Err(_) => None,
    }
}

// --- Typed Helpers (policy + decision only) ---

pub fn save_policy(case_id: &str, policy: &CompliancePolicy) -> Result<(), String> {
    let bytes = serde_json::to_vec(policy).map_err(|e| format!("Serialize policy: {e}"))?;
    kv_write(case_id, "policy", &bytes)
}

pub fn load_decision(case_id: &str) -> Option<DecisionResult> {
    kv_read(case_id, "decision")
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
}

pub fn save_decision(decision: &DecisionResult) -> Result<(), String> {
    let bytes = serde_json::to_vec(decision).map_err(|e| format!("Serialize decision: {e}"))?;
    kv_write(&decision.case_id, "decision", &bytes)
}

// --- Record Violation ---

pub fn record_violation(
    case_id: &str,
    violation_type: ViolationType,
    step_index: u32,
    caller_did: &str,
    expected: &str,
    actual: &str,
    timestamp: u64,
) -> Result<(), String> {
    let violation = Violation {
        violation_type: violation_type.clone(),
        step_index,
        caller_did: caller_did.to_string(),
        expected: expected.to_string(),
        actual: actual.to_string(),
        recorded_at: timestamp,
    };

    let key = format!("violation:{}", timestamp);
    let bytes = serde_json::to_vec(&violation).map_err(|e| format!("Serialize violation: {e}"))?;
    kv_write(case_id, &key, &bytes)?;

    let _ = logging::error(&format!(
        "VIOLATION [case={}]: {:?} at step {} — expected={}, actual={}, caller={}",
        case_id, violation_type, step_index, expected, actual, caller_did
    ));

    Ok(())
}

// --- SHA-256 Utility ---

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    result.iter().map(|b| format!("{:02x}", b)).collect()
}
