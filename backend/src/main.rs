use verigate_backend::ai;
use verigate_backend::auth::cedar::PolicyEngine;
use verigate_backend::auth::middleware::DEV_JWT_SECRET;
use verigate_backend::auth::requirements::RequirementEngine;
use verigate_backend::config::AppConfig;
use verigate_backend::credential::issuer_trust::TrustedIssuerRegistry;
use verigate_backend::credential::verifier::{
    CredentialVerifier, EntityVerifier, RegionVerifier, SignerVerifier, WalletVerifier,
};
use verigate_backend::db::connect_db;
use verigate_backend::routes::assessments;
use verigate_backend::routes::cases;
use verigate_backend::routes::completeness;
use verigate_backend::routes::events;
use verigate_backend::routes::health::health_check;
use verigate_backend::routes::override_action;
use verigate_backend::routes::requirements;
use verigate_backend::routes::submissions;
use verigate_backend::routes::timeline;
use verigate_backend::t3::agent_identity::AgentAuthClient;
use verigate_backend::t3::protected_action::{
    DevProtectedActionExecutor, T3ProtectedActionExecutor,
};
use verigate_backend::AppState;

use axum::{middleware, routing::get, routing::post, Router};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Load .env file if present (development convenience)
    let _ = dotenvy::dotenv();

    // Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    // Load configuration
    let config = AppConfig::from_env().unwrap_or_else(|e| {
        tracing::error!("Failed to load configuration: {e}");
        std::process::exit(1);
    });

    // Connect to database and run migrations
    let pool = connect_db(&config.database_url).await.unwrap_or_else(|e| {
        tracing::error!("Failed to connect to database: {e}");
        std::process::exit(1);
    });
    tracing::info!("Database connected and migrations applied");

    // Auto-seed demo data if database is empty
    if let Err(e) = verigate_backend::seed::auto_seed(&pool).await {
        tracing::warn!("Auto-seed failed (non-fatal): {e}");
    }

    // Create LLM client (before T3 auth which moves config fields)
    let llm_client: Arc<dyn verigate_backend::ai::LlmClient> =
        Arc::from(ai::create_llm_client(&config));
    if config.llm_api_key.is_some() {
        tracing::info!(provider = %config.llm_provider, model = %config.llm_model, "LLM client initialized");
    } else {
        tracing::warn!("LLM_API_KEY not set — AI features will return errors on first call");
    }

    // Authenticate with Terminal 3
    let t3_client = AgentAuthClient::new(config.t3_api_url, config.t3_agent_key);
    let agent_identity = t3_client.authenticate().await.unwrap_or_else(|e| {
        tracing::error!("Terminal 3 authentication failed: {e}");
        std::process::exit(1);
    });

    if agent_identity.authenticated {
        tracing::info!(
            agent_did = %agent_identity.agent_did,
            "Terminal 3 agent authenticated"
        );
    } else {
        tracing::warn!(
            agent_did = %agent_identity.agent_did,
            "Running in degraded mode — Terminal 3 credentials not configured"
        );
    }

    // Load JWT secret (dev default if not configured)
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        tracing::warn!(
            "JWT_SECRET not set — using insecure dev default. Do NOT use in production."
        );
        DEV_JWT_SECRET.to_string()
    });

    // Initialize Cedar policy engine
    let policies_dir = PathBuf::from(
        std::env::var("POLICIES_DIR").unwrap_or_else(|_| "policies".to_string()),
    );
    let policy_engine = PolicyEngine::from_directory(&policies_dir).unwrap_or_else(|e| {
        tracing::error!("Failed to initialize Cedar policy engine: {e}");
        std::process::exit(1);
    });

    // Initialize requirement engine
    let requirements_dir = policies_dir.join("requirements");
    let requirement_engine = RequirementEngine::from_directory(&requirements_dir).unwrap_or_else(|e| {
        tracing::error!("Failed to initialize requirement engine: {e}");
        std::process::exit(1);
    });

    // Load trusted issuer registry
    let issuers_path = PathBuf::from(
        std::env::var("TRUSTED_ISSUERS_PATH")
            .unwrap_or_else(|_| "config/trusted_issuers.json".to_string()),
    );
    let issuer_registry = TrustedIssuerRegistry::from_file(&issuers_path).unwrap_or_else(|e| {
        tracing::error!("Failed to load trusted issuers: {e}");
        std::process::exit(1);
    });
    tracing::info!("Trusted issuer registry loaded from {:?}", issuers_path);

    // Initialize credential verifiers
    let credential_verifiers: Vec<Box<dyn CredentialVerifier>> = vec![
        Box::new(EntityVerifier),
        Box::new(SignerVerifier),
        Box::new(RegionVerifier),
        Box::new(WalletVerifier),
    ];

    // Initialize protected action executor (dev-mock or real T3 TEE)
    let protected_action_executor: Arc<dyn verigate_backend::t3::protected_action::ProtectedActionExecutor> =
        match std::env::var("T3_API_URL") {
            Ok(url) if !url.is_empty() => {
                tracing::info!(url = %url, "Protected action executor: T3 TEE (live)");
                Arc::new(T3ProtectedActionExecutor::new(url))
            }
            _ => {
                tracing::info!("Protected action executor: dev-mock (T3_API_URL not set)");
                Arc::new(DevProtectedActionExecutor)
            }
        };

    // Initialize T3N verification client (if bridge URL configured)
    let t3n_client = config.t3n_bridge_url.as_ref().map(|url| {
        tracing::info!(url = %url, "T3N verification client: connected");
        Arc::new(verigate_backend::t3::verification::T3nVerificationClient::new(url))
    });

    let state = AppState {
        pool,
        agent_identity,
        start_time: Arc::new(Instant::now()),
        policy_engine: Arc::new(policy_engine),
        requirement_engine: Arc::new(requirement_engine),
        jwt_secret,
        issuer_registry: Arc::new(issuer_registry),
        credential_verifiers: Arc::new(credential_verifiers),
        llm_client,
        llm_api_key: config.llm_api_key.clone(),
        protected_action_executor,
        t3n_client,
    };

    // Build router
    // Public routes — no auth middleware
    let public_routes = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/auth/login", post(verigate_backend::auth::login::login))
        .route("/api/seed", post(verigate_backend::routes::seed::seed_database))
        .route(
            "/api/test/token",
            get(verigate_backend::routes::test_helpers::generate_demo_token),
        )
        .route(
            "/api/test/generate-vp",
            get(verigate_backend::routes::test_helpers::generate_test_vp),
        )
        .route(
            "/api/test/cases/:id/disclosed-facts",
            get(verigate_backend::routes::test_helpers::get_test_disclosed_facts),
        );

    // Protected routes — auth middleware enforced
    let protected_routes = Router::new()
        .route("/api/cases/:id/timeline", get(timeline::get_timeline))
        .route(
            "/api/cases/:id/requirements",
            get(requirements::get_requirements),
        )
        .route(
            "/api/cases/:id/completeness",
            get(completeness::get_case_completeness),
        )
        .route(
            "/api/cases/:id/submissions",
            post(submissions::submit_presentation).get(submissions::get_case_submissions),
        )
        .route(
            "/api/cases/:id/assess",
            post(assessments::trigger_assessment),
        )
        .route(
            "/api/cases/:id/assessment",
            get(assessments::get_assessment),
        )
        .route(
            "/api/cases/:id/override",
            post(override_action::override_decision),
        )
        .route(
            "/api/cases/:id/events/stream",
            get(events::stream_case_events),
        )
        // State machine routes (Phase A)
        .route(
            "/api/cases/:id/evidence",
            get(verigate_backend::routes::evidence::get_evidence_chain),
        )
        .route(
            "/api/cases/:id/violations",
            get(verigate_backend::routes::evidence::get_violations),
        )
        .route(
            "/api/cases/:id/plan-status",
            get(verigate_backend::routes::evidence::get_plan_status),
        )
        .route(
            "/api/cases/:id/policy",
            post(verigate_backend::routes::evidence::set_policy),
        )
        .route(
            "/api/cases/:id/decide",
            post(verigate_backend::routes::evidence::trigger_decide),
        )
        .route(
            "/api/cases/:id/protected-action",
            post(verigate_backend::routes::evidence::execute_protected_action),
        )
        .merge(cases::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            verigate_backend::auth::middleware::auth_middleware,
        ));

    let app = public_routes
        .merge(protected_routes)
        .with_state(state);

    // Start server
    let addr = SocketAddr::new(
        config.server_host.parse().unwrap_or([0, 0, 0, 0].into()),
        config.server_port,
    );
    tracing::info!("Server starting on {addr}");

    let listener = TcpListener::bind(addr).await.unwrap_or_else(|e| {
        tracing::error!("Failed to bind to {addr}: {e}");
        std::process::exit(1);
    });

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Server error: {e}");
            std::process::exit(1);
        });
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
    tracing::info!("Shutdown signal received");
}
