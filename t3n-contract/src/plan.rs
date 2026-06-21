use crate::host::interfaces::logging;
use crate::host::tenant::tenant_context;
use crate::state::{kv_read, kv_write, record_violation, ViolationType};
use serde::{Deserialize, Serialize};

pub struct VerifyStepOk {
    pub step_index: u32,
    pub caller_did: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanMeta {
    pub case_id: String,
    pub steps: Vec<String>,
    pub committed_by: String,
    pub committed_at: u64,
    pub expires_at: u64,
    pub total_steps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanCursor {
    pub current_index: u32,
    pub status: u8, // 0=committed, 1=in_progress, 2=completed, 3=violated
}

impl PlanCursor {
    pub fn status_str(&self) -> &'static str {
        match self.status {
            0 => "committed",
            1 => "in_progress",
            2 => "completed",
            3 => "violated",
            _ => "unknown",
        }
    }
}

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

fn load_cursor(case_id: &str) -> Option<PlanCursor> {
    kv_read(case_id, "plan:cursor")
        .and_then(|b| serde_json::from_slice(&b).ok())
}

fn save_cursor(case_id: &str, cursor: &PlanCursor) -> Result<(), String> {
    let bytes = serde_json::to_vec(cursor).map_err(|e| format!("ser cursor: {e}"))?;
    kv_write(case_id, "plan:cursor", &bytes)
}

fn load_meta(case_id: &str) -> Option<PlanMeta> {
    kv_read(case_id, "plan:meta")
        .and_then(|b| serde_json::from_slice(&b).ok())
}

fn save_meta(case_id: &str, meta: &PlanMeta) -> Result<(), String> {
    let bytes = serde_json::to_vec(meta).map_err(|e| format!("ser meta: {e}"))?;
    kv_write(case_id, "plan:meta", &bytes)
}

/// Verify step ordering — reads tiny cursor (~20 bytes), writes cursor back.
/// This is cheap enough to combine with HTTP calls within fuel budget.
pub fn verify_step(case_id: &str, function_name: &str) -> Result<VerifyStepOk, String> {
    let ts = tenant_context::cluster_timestamp_secs();
    let caller_did = get_caller_did();

    // Read cursor (tiny: ~20 bytes)
    let mut cursor = match load_cursor(case_id) {
        Some(c) => c,
        None => {
            record_violation(case_id, ViolationType::NoPlan, 0, &caller_did, "plan exists", "no plan", ts).ok();
            return Err(format!("No plan for case {}", case_id));
        }
    };

    // Check status
    if cursor.status >= 2 {
        return Err(format!("Plan {} (status={})", case_id, cursor.status_str()));
    }

    // Read meta for step validation (only need steps[current_index])
    let meta = load_meta(case_id).ok_or("Plan meta missing")?;

    // Check expiry
    if ts > meta.expires_at {
        record_violation(case_id, ViolationType::Expired, cursor.current_index, &caller_did,
            &format!("expires_at={}", meta.expires_at), &format!("now={}", ts), ts).ok();
        cursor.status = 3;
        save_cursor(case_id, &cursor).ok();
        return Err(format!("Plan expired for case {}", case_id));
    }

    // Check ordering
    let idx = cursor.current_index as usize;
    if idx >= meta.steps.len() {
        return Err(format!("All steps completed for case {}", case_id));
    }

    let expected = &meta.steps[idx];
    if expected != function_name {
        record_violation(case_id, ViolationType::OutOfOrder, cursor.current_index, &caller_did,
            expected, function_name, ts).ok();
        cursor.status = 3;
        save_cursor(case_id, &cursor).ok();
        return Err(format!("Out of order: expected '{}', got '{}'", expected, function_name));
    }

    // Advance cursor
    cursor.current_index += 1;
    cursor.status = if cursor.current_index >= meta.total_steps { 2 } else { 1 };
    save_cursor(case_id, &cursor)?;

    Ok(VerifyStepOk { step_index: idx as u32, caller_did })
}

/// Commit plan — write meta (once) + cursor (initial state).
pub fn commit_plan(case_id: &str, steps: Vec<String>, ttl_secs: u64) -> Result<PlanMeta, String> {
    let ts = tenant_context::cluster_timestamp_secs();
    let caller_did = get_caller_did();

    // Check no active plan
    if let Some(cursor) = load_cursor(case_id) {
        if cursor.status < 2 {
            return Err(format!("Active plan exists for case {} (status={})", case_id, cursor.status_str()));
        }
    }

    if steps.is_empty() || steps.len() > 20 {
        return Err("Steps must be 1-20".to_string());
    }

    let meta = PlanMeta {
        case_id: case_id.to_string(),
        steps: steps.clone(),
        committed_by: caller_did,
        committed_at: ts,
        expires_at: ts + ttl_secs,
        total_steps: steps.len() as u32,
    };

    save_meta(case_id, &meta)?;
    save_cursor(case_id, &PlanCursor { current_index: 0, status: 0 })?;

    let _ = logging::info(&format!("Plan committed: case={}, steps={}", case_id, steps.len()));
    Ok(meta)
}

/// Get plan status (for query function).
pub fn get_status(case_id: &str) -> Result<serde_json::Value, String> {
    let cursor = load_cursor(case_id).ok_or("No plan")?;
    let meta = load_meta(case_id).ok_or("No plan meta")?;

    Ok(serde_json::json!({
        "case_id": case_id,
        "status": cursor.status_str(),
        "current_index": cursor.current_index,
        "total_steps": meta.total_steps,
        "steps": meta.steps,
        "committed_by": meta.committed_by,
        "committed_at": meta.committed_at,
        "expires_at": meta.expires_at,
    }))
}
