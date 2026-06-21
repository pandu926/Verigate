use crate::host::interfaces::logging;
use crate::state::{
    save_policy, save_decision,
    CompliancePolicy, DecisionResult,
};
use crate::host::tenant::tenant_context;

fn get_caller_did() -> String {
    tenant_context::calling_user_did()
        .map(|bytes| {
            if bytes.is_empty() {
                "anonymous".to_string()
            } else {
                format!("did:t3n:{}", bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>())
            }
        })
        .unwrap_or_else(|| "system".to_string())
}

/// Set compliance policy for a case.
pub fn set_policy(case_id: &str, policy: CompliancePolicy) -> Result<serde_json::Value, String> {
    if policy.auto_approve_threshold < 0.0 || policy.auto_approve_threshold > 1.0 {
        return Err("auto_approve_threshold must be between 0.0 and 1.0".to_string());
    }
    if policy.require_human_review_below < 0.0 || policy.require_human_review_below > 1.0 {
        return Err("require_human_review_below must be between 0.0 and 1.0".to_string());
    }
    if policy.max_risk_tolerance < 0.0 || policy.max_risk_tolerance > 1.0 {
        return Err("max_risk_tolerance must be between 0.0 and 1.0".to_string());
    }

    save_policy(case_id, &policy)?;

    let _ = logging::info(&format!(
        "Policy set: case={}, types={}, threshold={}",
        case_id, policy.required_credential_types.len(), policy.auto_approve_threshold,
    ));

    Ok(serde_json::json!({
        "policy_set": true,
        "case_id": case_id,
        "required_credential_types": policy.required_credential_types,
        "auto_approve_threshold": policy.auto_approve_threshold,
        "require_human_review_below": policy.require_human_review_below,
        "max_risk_tolerance": policy.max_risk_tolerance,
    }))
}

/// Make final decision — reads AI confidence from evidence entries.
pub fn make_decision(case_id: &str) -> Result<DecisionResult, String> {
    let ts = tenant_context::cluster_timestamp_secs();
    let caller_did = get_caller_did();

    let mut verified_credentials = 0u32;
    let mut ai_confidence: Option<f64> = None;
    let mut ai_decision: Option<String> = None;
    let mut last_hash = String::new();

    for i in 0..10u32 {
        if let Some(bytes) = crate::state::kv_read(case_id, &format!("evidence:{}", i)) {
            if let Ok(entry) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                let fn_name = entry["function_name"].as_str().unwrap_or("");
                if fn_name == "verify-credential" { verified_credentials += 1; }
                if fn_name == "assess-risk" {
                    ai_confidence = entry["ai_confidence"].as_f64();
                    ai_decision = entry["ai_decision"].as_str().map(String::from);
                }
                if let Some(h) = entry["result_hash"].as_str() { last_hash = h.to_string(); }
            }
        } else {
            break;
        }
    }

    if verified_credentials == 0 && ai_confidence.is_none() {
        return Err("No evidence to decide on".to_string());
    }

    // Use ACTUAL AI confidence — not hardcoded
    let confidence = ai_confidence.unwrap_or(if verified_credentials > 0 { 0.5 } else { 0.0 });

    // Decision based on AI assessment result
    let decision = match ai_decision.as_deref() {
        Some("ready") if confidence >= 0.7 => "approved",
        Some("blocked") => "blocked",
        Some("needs_review") => "needs_review",
        Some(_) if confidence >= 0.7 => "approved",
        Some(_) if confidence >= 0.4 => "needs_review",
        Some(_) => "blocked",
        None if verified_credentials > 0 => "needs_review",
        None => "blocked",
    };

    let reasoning = format!(
        "ai_decision={}, ai_confidence={:.2}, credentials_verified={}",
        ai_decision.as_deref().unwrap_or("none"), confidence, verified_credentials
    );

    let result = DecisionResult {
        case_id: case_id.to_string(),
        decision: decision.to_string(),
        confidence,
        reasoning,
        evidence_chain_hash: last_hash,
        policy_applied: false,
        steps_completed: verified_credentials + if ai_confidence.is_some() { 1 } else { 0 },
        violations_count: 0,
        decided_at: ts,
        decided_by: caller_did,
    };

    save_decision(&result)?;
    Ok(result)
}

