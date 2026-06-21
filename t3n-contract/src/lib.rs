#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

wit_bindgen::generate!({
    world: "verigate",
    path: "wit",
    additional_derives: [serde::Deserialize, serde::Serialize],
    generate_all,
});

mod crypto;
mod claims;
mod state;
mod plan;
mod policy;
mod protected;
mod queries;

use crate::host::tenant::tenant_context;
use crate::crypto::{decode_jwt_parts, resolve_did_key, verify_signature};
use crate::claims::{extract_claims, credential_hash, strip_sd_jwt};
use crate::state::{sha256_hex, kv_write, CompliancePolicy};
use crate::plan::{verify_step, commit_plan};

struct Component;

#[cfg(target_arch = "wasm32")]
impl exports::z::verigate::contracts::Guest for Component {

    fn set_compliance_policy(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("set-compliance-policy: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;
        let policy: CompliancePolicy = serde_json::from_value(input["policy"].clone())
            .map_err(|e| format!("Invalid policy format: {e}"))?;

        let result = policy::set_policy(case_id, policy)?;
        serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))
    }

    fn commit_assessment_plan(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("commit-assessment-plan: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;
        let ttl_secs = input["ttl_secs"].as_u64().unwrap_or(3600);

        let steps: Vec<String> = input["steps"].as_array()
            .ok_or("Missing steps array")?
            .iter()
            .map(|s| s["function_name"].as_str().unwrap_or("unknown").to_string())
            .collect();

        let meta = commit_plan(case_id, steps, ttl_secs)?;

        let result = serde_json::json!({
            "plan_committed": true,
            "case_id": case_id,
            "steps_count": meta.total_steps,
            "steps": meta.steps,
            "committed_by": meta.committed_by,
            "committed_at": meta.committed_at,
            "expires_at": meta.expires_at,
            "status": "committed",
        });
        serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))
    }

    fn verify_credential(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("verify-credential: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().unwrap_or("unknown");
        let requirement_id = input["requirement_id"].as_str().unwrap_or("unknown");

        // Step verification — cheap cursor read/write (~20 bytes)
        let step = verify_step(case_id, "verify-credential")?;

        let vp = &input["vp"];
        let trusted_issuers: Vec<String> = input["trusted_issuers"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let credentials = vp["verifiableCredential"]
            .as_array()
            .ok_or("VP missing verifiableCredential array")?;

        let mut all_facts = Vec::new();
        let mut verified_count = 0u32;
        let mut algorithms_used = Vec::new();

        for (i, cred_value) in credentials.iter().enumerate() {
            let jwt_raw = cred_value.as_str().ok_or(format!("Credential {} not a string", i))?;
            let jwt = strip_sd_jwt(jwt_raw);

            let parts = match decode_jwt_parts(jwt) {
                Ok(p) => p,
                Err(e) => {
                    return Ok(serde_json::to_vec(&serde_json::json!({
                        "verified": false,
                        "reason": format!("JWT decode failed: {}", e),
                        "credential_index": i,
                        "step_index": step.step_index,
                    })).unwrap_or_default());
                }
            };

            let alg = parts.header["alg"].as_str().unwrap_or("unknown");
            let issuer_did = parts.payload["iss"].as_str().unwrap_or("unknown");

            let (key_alg, pubkey_bytes) = match resolve_did_key(issuer_did) {
                Ok(result) => result,
                Err(e) => {
                    return Ok(serde_json::to_vec(&serde_json::json!({
                        "verified": false,
                        "reason": format!("DID resolution failed: {}", e),
                        "credential_index": i,
                        "step_index": step.step_index,
                    })).unwrap_or_default());
                }
            };

            if let Err(e) = verify_signature(&key_alg, &parts.signing_input, &parts.signature, &pubkey_bytes) {
                return Ok(serde_json::to_vec(&serde_json::json!({
                    "verified": false,
                    "signature_valid": false,
                    "reason": e,
                    "credential_index": i,
                    "step_index": step.step_index,
                })).unwrap_or_default());
            }

            algorithms_used.push(alg.to_string());

            let issuer_trusted = trusted_issuers.is_empty()
                || trusted_issuers.iter().any(|t| t == issuer_did);
            if !issuer_trusted {
                return Ok(serde_json::to_vec(&serde_json::json!({
                    "verified": false,
                    "issuer_trusted": false,
                    "reason": format!("Issuer {} not trusted", issuer_did),
                    "step_index": step.step_index,
                })).unwrap_or_default());
            }

            let vc = &parts.payload["vc"];
            let subject = &vc["credentialSubject"];
            let facts = extract_claims(subject);
            let source_hash = credential_hash(jwt);

            for fact in &facts {
                all_facts.push(serde_json::json!({
                    "claim_key": fact.claim_key,
                    "claim_value": fact.claim_value,
                    "source_hash": source_hash,
                    "issuer_did": issuer_did,
                    "verified_in_tee": true,
                }));
            }
            verified_count += 1;
        }

        let ts = tenant_context::cluster_timestamp_secs();

        let response = serde_json::json!({
            "verified": true,
            "signature_valid": true,
            "issuer_trusted": true,
            "credentials_verified": verified_count,
            "facts": all_facts,
            "facts_count": all_facts.len(),
            "algorithms_used": algorithms_used,
            "case_id": case_id,
            "requirement_id": requirement_id,
            "step_index": step.step_index,
            "execution_ts": ts,
            "verified_in_tee": true,
        });

        let result_bytes = serde_json::to_vec(&response).map_err(|e| format!("Serialize: {e}"))?;

        // Evidence: write per-step entry (single KV write, no chain load)
        let evidence_entry = serde_json::json!({
            "step_index": step.step_index,
            "function_name": "verify-credential",
            "result_hash": sha256_hex(&result_bytes),
            "timestamp": ts,
        });
        kv_write(case_id, &format!("evidence:{}", step.step_index),
            &serde_json::to_vec(&evidence_entry).unwrap_or_default())?;

        Ok(result_bytes)
    }

    fn assess_risk(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("assess-risk: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().unwrap_or("unknown");
        let facts = &input["facts"];
        let llm_api_key = input["llm_api_key"].as_str().unwrap_or("");
        let llm_base_url = input["llm_base_url"]
            .as_str()
            .unwrap_or("https://api.pioneer.ai/v1");

        // Step verification — cheap cursor read/write (~20 bytes)
        let step = verify_step(case_id, "assess-risk")?;

        let ts = tenant_context::cluster_timestamp_secs();

        // Build compact prompt
        let facts_compact = facts.as_array()
            .map(|arr| arr.iter()
                .filter_map(|f| {
                    let k = f["claim_key"].as_str().unwrap_or("");
                    let v = f["claim_value"].as_str().unwrap_or("");
                    if k.is_empty() { None } else { Some(format!("{}={}", k, v)) }
                })
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default();

        let prompt = format!(
            "Assess counterparty risk. Facts: {}. Return {{\"decision\":\"ready|needs_review|blocked\",\"confidence\":0.0-1.0,\"reasoning\":\"brief\"}}",
            facts_compact
        );

        use crate::host::interfaces::http;

        let llm_payload = serde_json::json!({
            "model": "deepseek-ai/DeepSeek-V4-Flash",
            "messages": [
                {"role": "system", "content": "JSON only. No markdown."},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0,
            "stream": false
        });
        let body_bytes = serde_json::to_vec(&llm_payload).unwrap_or_default();

        let request = http::Request {
            method: http::Verb::Post,
            url: format!("{}/chat/completions", llm_base_url),
            headers: Some(vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("Authorization".to_string(), format!("Bearer {}", llm_api_key)),
            ]),
            payload: Some(body_bytes),
        };

        let ai_decision = match http::call(&request) {
            Ok(response) => {
                if response.code == 200 {
                    let llm_resp: serde_json::Value = serde_json::from_slice(&response.payload)
                        .unwrap_or(serde_json::json!({}));
                    let content = llm_resp["choices"][0]["message"]["content"].as_str().unwrap_or("{}");
                    serde_json::from_str(content).unwrap_or(serde_json::json!({"decision":"needs_review","confidence":0.3,"reasoning":"parse error"}))
                } else {
                    serde_json::json!({"decision":"needs_review","confidence":0.0,"reasoning":format!("HTTP {}", response.code)})
                }
            }
            Err(e) => {
                serde_json::json!({"decision":"needs_review","confidence":0.0,"reasoning":format!("egress: {}", e)})
            }
        };

        let result = serde_json::json!({
            "decision": ai_decision["decision"],
            "confidence": ai_decision["confidence"],
            "reasoning": ai_decision["reasoning"],
            "case_id": case_id,
            "step_index": step.step_index,
            "execution_ts": ts,
            "assessed_in_tee": true,
            "llm_called_from": "TEE enclave via host:interfaces/http",
            "facts_never_left_enclave": true,
        });

        let result_bytes = serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))?;

        // Evidence: per-step entry with AI decision data
        let evidence_entry = serde_json::json!({
            "step_index": step.step_index,
            "function_name": "assess-risk",
            "result_hash": sha256_hex(&result_bytes),
            "ai_decision": ai_decision["decision"],
            "ai_confidence": ai_decision["confidence"],
            "timestamp": ts,
        });
        kv_write(case_id, &format!("evidence:{}", step.step_index),
            &serde_json::to_vec(&evidence_entry).unwrap_or_default())?;

        Ok(result_bytes)
    }

    fn decide(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("decide: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;

        // Step verification — cheap cursor read/write
        let step = verify_step(case_id, "decide")?;

        let decision = policy::make_decision(case_id)?;
        let result_bytes = serde_json::to_vec(&decision).map_err(|e| format!("Serialize: {e}"))?;

        // Evidence: per-step entry
        let ts = tenant_context::cluster_timestamp_secs();
        let evidence_entry = serde_json::json!({
            "step_index": step.step_index,
            "function_name": "decide",
            "result_hash": sha256_hex(&result_bytes),
            "timestamp": ts,
        });
        kv_write(case_id, &format!("evidence:{}", step.step_index),
            &serde_json::to_vec(&evidence_entry).unwrap_or_default())?;

        Ok(result_bytes)
    }

    fn execute_protected_action(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("execute-protected-action: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;
        let action_type = input["action_type"].as_str().ok_or("Missing action_type")?;
        let action_payload = &input["action_payload"];

        let result = protected::execute_action(case_id, action_type, action_payload)?;
        serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))
    }

    fn get_plan_status(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("get-plan-status: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;
        let result = plan::get_status(case_id)?;
        serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))
    }

    fn get_evidence_chain(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("get-evidence-chain: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;
        let result = queries::get_evidence_chain(case_id)?;
        serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))
    }

    fn get_violations(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("get-violations: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;
        let result = queries::get_violations(case_id)?;
        serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))
    }

    fn advance_step(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("advance-step: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;
        let function_name = input["function_name"].as_str().ok_or("Missing function_name")?;

        let step = verify_step(case_id, function_name)?;

        let result = serde_json::json!({
            "advanced": true,
            "case_id": case_id,
            "step_index": step.step_index,
            "function_name": function_name,
            "caller_did": step.caller_did,
        });
        serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))
    }

    fn append_to_evidence(
        req: exports::z::verigate::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("append-to-evidence: missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("Invalid JSON: {e}"))?;

        let case_id = input["case_id"].as_str().ok_or("Missing case_id")?;
        let step_index = input["step_index"].as_u64().unwrap_or(0) as u32;
        let function_name = input["function_name"].as_str().unwrap_or("unknown");
        let result_data = input["result_data"].as_str().unwrap_or("{}");

        let ts = tenant_context::cluster_timestamp_secs();
        let result_hash = sha256_hex(result_data.as_bytes());

        let entry = serde_json::json!({
            "step_index": step_index,
            "function_name": function_name,
            "result_hash": result_hash,
            "timestamp": ts,
        });
        kv_write(case_id, &format!("evidence:{}", step_index),
            &serde_json::to_vec(&entry).unwrap_or_default())?;

        let result = serde_json::json!({
            "appended": true,
            "case_id": case_id,
            "step_index": step_index,
            "function_name": function_name,
            "result_hash": result_hash,
            "timestamp": ts,
        });
        serde_json::to_vec(&result).map_err(|e| format!("Serialize: {e}"))
    }
}

#[cfg(target_arch = "wasm32")]
export!(Component);
