use std::env;

/// Application configuration loaded from environment variables.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub server_host: String,
    pub server_port: u16,
    pub t3_agent_key: Option<String>,
    pub t3_api_url: Option<String>,
    pub t3n_bridge_url: Option<String>,
    /// LLM provider identifier: "pioneer" or "openai".
    pub llm_provider: String,
    /// API key for the LLM provider.
    pub llm_api_key: Option<String>,
    /// Model identifier (e.g. "deepseek-ai/DeepSeek-V4-Pro").
    pub llm_model: String,
    /// Base URL for the LLM API (without trailing /chat/completions).
    pub llm_base_url: String,
}

impl AppConfig {
    /// Load configuration from environment variables.
    ///
    /// Fails fast with a clear error if DATABASE_URL is not set.
    pub fn from_env() -> Result<Self, String> {
        let database_url = env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL environment variable is required but not set")?;

        let server_host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

        let server_port = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3001".to_string())
            .parse::<u16>()
            .map_err(|e| format!("SERVER_PORT must be a valid u16: {e}"))?;

        let t3_agent_key = env::var("T3_AGENT_KEY").ok().filter(|s| !s.is_empty());
        let t3_api_url = env::var("T3_API_URL").ok().filter(|s| !s.is_empty());
        let t3n_bridge_url = env::var("T3N_BRIDGE_URL").ok().filter(|s| !s.is_empty());

        let llm_provider =
            env::var("LLM_PROVIDER").unwrap_or_else(|_| "pioneer".to_string());
        let llm_api_key = env::var("LLM_API_KEY").ok().filter(|s| !s.is_empty());
        let llm_model = env::var("LLM_MODEL")
            .unwrap_or_else(|_| "deepseek-ai/DeepSeek-V4-Pro".to_string());
        let llm_base_url = env::var("LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.pioneer.ai/v1".to_string());

        Ok(Self {
            database_url,
            server_host,
            server_port,
            t3_agent_key,
            t3_api_url,
            t3n_bridge_url,
            llm_provider,
            llm_api_key,
            llm_model,
            llm_base_url,
        })
    }
}
