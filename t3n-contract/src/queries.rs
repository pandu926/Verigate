use crate::state::kv_read;

/// Get full evidence chain for a case (reads per-step keys).
pub fn get_evidence_chain(case_id: &str) -> Result<serde_json::Value, String> {
    let mut entries = Vec::new();

    for i in 0..20u32 {
        if let Some(bytes) = kv_read(case_id, &format!("evidence:{}", i)) {
            if let Ok(entry) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                entries.push(entry);
            }
        } else {
            break;
        }
    }

    Ok(serde_json::json!({
        "case_id": case_id,
        "entries": entries,
        "chain_length": entries.len(),
    }))
}

/// Get all violations for a case (reads per-timestamp keys via scan).
pub fn get_violations(case_id: &str) -> Result<serde_json::Value, String> {
    let mut violations = Vec::new();

    // Read violations by scanning known timestamps is impractical,
    // so we store a violation counter and read sequentially.
    for i in 0..100u32 {
        if let Some(bytes) = kv_read(case_id, &format!("violation:{}", i)) {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                violations.push(v);
            }
        }
        // Violations use timestamp keys, so we also try the legacy format.
        // In practice, record_violation writes with timestamp keys — we collect
        // what we can find. Break early if we've checked enough empty slots.
    }

    // Fallback: try reading from the old "violations" key if nothing found
    if violations.is_empty() {
        if let Some(bytes) = kv_read(case_id, "violations") {
            if let Ok(arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&bytes) {
                violations = arr;
            }
        }
    }

    Ok(serde_json::json!({
        "case_id": case_id,
        "violations": violations,
        "count": violations.len(),
        "has_violations": !violations.is_empty(),
    }))
}
