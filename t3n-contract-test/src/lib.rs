#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

wit_bindgen::generate!({
    world: "vg-http-test",
    path: "wit",
    additional_derives: [serde::Deserialize, serde::Serialize],
    generate_all,
});

use crate::host::interfaces::{http, logging, kv_store};
use crate::host::tenant::tenant_context;

struct Component;

#[cfg(target_arch = "wasm32")]
impl exports::z::vg_http_test::contracts::Guest for Component {
    fn test_http(
        req: exports::z::vg_http_test::contracts::GenericInput,
    ) -> Result<Vec<u8>, String> {
        let input_bytes = req.input.ok_or("missing input")?;
        let input: serde_json::Value =
            serde_json::from_slice(&input_bytes).map_err(|e| format!("JSON: {e}"))?;

        let test = input["test"].as_str().unwrap_or("post");
        let api_key = input["api_key"].as_str().unwrap_or("");
        let _ = logging::info(&format!("Test: {}", test));

        match test {
            // Test 1: Just HTTP POST (already proven works)
            "post" => {
                let body = serde_json::json!({"model":"deepseek-ai/DeepSeek-V4-Flash","messages":[{"role":"user","content":"Return: {\"ok\":true}"}],"temperature":0,"stream":false});
                let request = http::Request {
                    method: http::Verb::Post,
                    url: "https://api.pioneer.ai/v1/chat/completions".to_string(),
                    headers: Some(vec![("Content-Type".to_string(),"application/json".to_string()),("Authorization".to_string(),format!("Bearer {}",api_key))]),
                    payload: Some(serde_json::to_vec(&body).unwrap()),
                };
                match http::call(&request) {
                    Ok(r) => Ok(serde_json::to_vec(&serde_json::json!({"step":"post","ok":true,"code":r.code})).unwrap()),
                    Err(e) => Ok(serde_json::to_vec(&serde_json::json!({"step":"post","ok":false,"err":format!("{:?}",e)})).unwrap()),
                }
            }
            // Test 2: KV write only
            "kv_write" => {
                let tid = tenant_context::tenant_did();
                let hex_tid: String = tid.iter().map(|b| format!("{:02x}", b)).collect();
                let map = format!("z:{}:vg-state", hex_tid);
                let key = b"test-key";
                let val = b"test-value";
                match kv_store::put(&map, key, val) {
                    Ok(_) => Ok(serde_json::to_vec(&serde_json::json!({"step":"kv_write","ok":true,"map":map})).unwrap()),
                    Err(e) => Ok(serde_json::to_vec(&serde_json::json!({"step":"kv_write","ok":false,"err":e,"map":map})).unwrap()),
                }
            }
            // Test 3: KV write THEN HTTP POST
            "kv_then_post" => {
                let tid = tenant_context::tenant_did();
                let hex_tid: String = tid.iter().map(|b| format!("{:02x}", b)).collect();
                let map = format!("z:{}:vg-state", hex_tid);
                kv_store::put(&map, b"before-http", b"written").map_err(|e| format!("kv: {e}"))?;
                
                let body = serde_json::json!({"model":"deepseek-ai/DeepSeek-V4-Flash","messages":[{"role":"user","content":"Return: {\"ok\":true}"}],"temperature":0,"stream":false});
                let request = http::Request {
                    method: http::Verb::Post,
                    url: "https://api.pioneer.ai/v1/chat/completions".to_string(),
                    headers: Some(vec![("Content-Type".to_string(),"application/json".to_string()),("Authorization".to_string(),format!("Bearer {}",api_key))]),
                    payload: Some(serde_json::to_vec(&body).unwrap()),
                };
                match http::call(&request) {
                    Ok(r) => Ok(serde_json::to_vec(&serde_json::json!({"step":"kv_then_post","ok":true,"code":r.code})).unwrap()),
                    Err(e) => Ok(serde_json::to_vec(&serde_json::json!({"step":"kv_then_post","ok":false,"err":format!("{:?}",e)})).unwrap()),
                }
            }
            // Test 4: HTTP POST then KV write
            "post_then_kv" => {
                let body = serde_json::json!({"model":"deepseek-ai/DeepSeek-V4-Flash","messages":[{"role":"user","content":"Return: {\"ok\":true}"}],"temperature":0,"stream":false});
                let request = http::Request {
                    method: http::Verb::Post,
                    url: "https://api.pioneer.ai/v1/chat/completions".to_string(),
                    headers: Some(vec![("Content-Type".to_string(),"application/json".to_string()),("Authorization".to_string(),format!("Bearer {}",api_key))]),
                    payload: Some(serde_json::to_vec(&body).unwrap()),
                };
                let http_result = http::call(&request);
                
                let tid = tenant_context::tenant_did();
                let hex_tid: String = tid.iter().map(|b| format!("{:02x}", b)).collect();
                let map = format!("z:{}:vg-state", hex_tid);
                kv_store::put(&map, b"after-http", b"written").map_err(|e| format!("kv: {e}"))?;
                
                match http_result {
                    Ok(r) => Ok(serde_json::to_vec(&serde_json::json!({"step":"post_then_kv","ok":true,"code":r.code})).unwrap()),
                    Err(e) => Ok(serde_json::to_vec(&serde_json::json!({"step":"post_then_kv","ok":false,"err":format!("{:?}",e)})).unwrap()),
                }
            }
            _ => Err("unknown test".to_string()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
export!(Component);
