use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    pub agent_did: String,
    pub authenticated: bool,
    pub sdk_version: String,
    pub capabilities: Vec<String>,
}

pub struct AgentAuthClient {
    api_url: Option<String>,
    agent_key: Option<String>,
}

impl AgentAuthClient {
    pub fn new(api_url: Option<String>, agent_key: Option<String>) -> Self {
        Self { api_url, agent_key }
    }

    pub async fn authenticate(&self) -> Result<AgentIdentity, String> {
        match (&self.api_url, &self.agent_key) {
            (Some(_url), Some(_key)) => self.authenticate_live().await,
            _ => Ok(Self::degraded_identity()),
        }
    }

    async fn authenticate_live(&self) -> Result<AgentIdentity, String> {
        let bridge_url = std::env::var("T3N_BRIDGE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3310".to_string());

        let client = reqwest::Client::new();
        let endpoint = format!("{bridge_url}/identity");

        let response = client
            .get(&endpoint)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| format!("T3N bridge request failed: {e}"))?;

        if response.status().is_success() {
            let identity: AgentIdentity = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse T3N bridge response: {e}"))?;
            Ok(identity)
        } else {
            tracing::warn!(
                status = %response.status(),
                "T3N bridge auth failed, falling back to degraded mode"
            );
            Ok(Self::degraded_identity())
        }
    }

    fn degraded_identity() -> AgentIdentity {
        AgentIdentity {
            agent_did: "did:t3n:local-dev-agent".to_string(),
            authenticated: false,
            sdk_version: "0.1.0-dev".to_string(),
            capabilities: vec!["mock-mode".to_string()],
        }
    }
}
