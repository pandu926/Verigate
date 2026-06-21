use crate::host::interfaces::{http_with_placeholders as hwp, logging};
use crate::host::tenant::tenant_context;
use crate::state::{
    kv_read, kv_write, load_decision, record_violation, sha256_hex, ViolationType,
};

fn get_caller_did() -> String {
    tenant_context::calling_user_did()
        .map(|bytes| {
            if bytes.is_empty() {
                "anonymous".to_string()
            } else {
                format!(
                    "did:t3n:{}",
                    bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>()
                )
            }
        })
        .unwrap_or_else(|| "system".to_string())
}

/// Execute a protected action — ONLY after decide=approved.
pub fn execute_action(
    case_id: &str,
    action_type: &str,
    action_payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let ts = tenant_context::cluster_timestamp_secs();
    let caller_did = get_caller_did();

    // 1. Load decision — must exist and be "approved"
    let decision = load_decision(case_id).ok_or_else(|| {
        format!("No decision exists for case {}. Run decide first.", case_id)
    })?;

    if decision.decision != "approved" {
        record_violation(
            case_id,
            ViolationType::PolicyViolation,
            decision.steps_completed,
            &caller_did,
            "decision=approved",
            &format!("decision={}", decision.decision),
            ts,
        )
        .ok();
        return Err(format!(
            "Protected action blocked: decision='{}' (requires 'approved'). Case {}.",
            decision.decision, case_id
        ));
    }

    // 2. Verify evidence exists (at least the decision step was recorded)
    let evidence_exists = kv_read(case_id, "evidence:0").is_some();
    if !evidence_exists {
        return Err(format!(
            "No evidence found for case {}. Pipeline incomplete.",
            case_id
        ));
    }

    let _ = logging::info(&format!(
        "Protected action authorized: case={}, action={}, decision_hash={}",
        case_id,
        action_type,
        &decision.evidence_chain_hash[..decision.evidence_chain_hash.len().min(16)]
    ));

    // 3. Execute action based on type
    let result = match action_type {
        "notify_counterparty" => execute_notification(case_id, action_payload)?,
        "create_account" => execute_account_creation(case_id, action_payload)?,
        "issue_credential" => execute_credential_issuance(case_id, action_payload)?,
        _ => {
            return Err(format!("Unknown action type: {}", action_type));
        }
    };

    // 4. Record action in evidence (per-step key)
    let result_bytes = serde_json::to_vec(&result).unwrap_or_default();
    let step_index = decision.steps_completed + 1;
    let entry = serde_json::json!({
        "step_index": step_index,
        "function_name": format!("protected:{}", action_type),
        "result_hash": sha256_hex(&result_bytes),
        "timestamp": ts,
    });
    kv_write(
        case_id,
        &format!("evidence:{}", step_index),
        &serde_json::to_vec(&entry).unwrap_or_default(),
    )?;

    let _ = logging::info(&format!(
        "Protected action executed: case={}, action={}, caller={}",
        case_id, action_type, caller_did
    ));

    Ok(serde_json::json!({
        "executed": true,
        "action_type": action_type,
        "case_id": case_id,
        "evidence_recorded": true,
        "step_index": step_index,
        "executed_at": ts,
        "executed_by": caller_did,
        "result": result,
    }))
}

fn execute_notification(
    case_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let message = payload["message"]
        .as_str()
        .unwrap_or("Your onboarding assessment has been approved.");
    let status = payload["status"].as_str().unwrap_or("approved");

    let notification = serde_json::json!({
        "to": "{{profile.verified_contacts.email.value}}",
        "recipient_name": "{{profile.first_name}} {{profile.last_name}}",
        "company": "{{profile.company_name}}",
        "subject": format!("Verigate: Assessment {} — Case {}", status, case_id),
        "body": message,
    });

    let body_bytes = serde_json::to_vec(&notification).unwrap_or_default();

    let request = hwp::Request {
        method: hwp::Verb::Post,
        url: "https://api.verigate.io/notifications/send".to_string(),
        headers: Some(vec![(
            "Content-Type".to_string(),
            "application/json".to_string(),
        )]),
        payload: Some(body_bytes),
    };

    match hwp::call(&request) {
        Ok(_) => Ok(serde_json::json!({
            "notified": true,
            "placeholders_resolved_by": "T3N TEE host",
            "pii_never_in_contract": true,
        })),
        Err(e) => Ok(serde_json::json!({
            "notified": false,
            "scope_enforced": true,
            "reason": format!("{:?}", e),
        })),
    }
}

fn execute_account_creation(
    case_id: &str,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let account_type = payload["account_type"].as_str().unwrap_or("standard");

    let request_body = serde_json::json!({
        "case_id": case_id,
        "account_type": account_type,
        "owner_name": "{{profile.first_name}} {{profile.last_name}}",
        "owner_email": "{{profile.verified_contacts.email.value}}",
        "company": "{{profile.company_name}}",
    });

    let body_bytes = serde_json::to_vec(&request_body).unwrap_or_default();

    let request = hwp::Request {
        method: hwp::Verb::Post,
        url: "https://api.verigate.io/accounts/create".to_string(),
        headers: Some(vec![(
            "Content-Type".to_string(),
            "application/json".to_string(),
        )]),
        payload: Some(body_bytes),
    };

    match hwp::call(&request) {
        Ok(resp) => Ok(serde_json::json!({
            "account_created": true,
            "response_code": resp.code,
            "pii_resolved_by_host": true,
        })),
        Err(e) => Ok(serde_json::json!({
            "account_created": false,
            "reason": format!("{:?}", e),
        })),
    }
}

fn execute_credential_issuance(
    case_id: &str,
    _payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let _ = logging::info(&format!(
        "Credential issuance requested for case={} (placeholder)",
        case_id
    ));

    Ok(serde_json::json!({
        "credential_issued": false,
        "reason": "Credential issuance endpoint not yet configured",
        "case_id": case_id,
    }))
}
