use financial_agent_orchestrator::{
    agent::Orchestrator,
    api::start_server,
    audit::AuditLog,
    execution::ExecutionEngine,
    planner::GeminiPlanner,
    state::InMemoryStateStore,
    tools::create_default_registry,
    verification::create_default_verification_engine,
};
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Load environment variables
    dotenv::dotenv().ok();
    
    let gemini_api_key = std::env::var("GEMINI_API_KEY")
        .unwrap_or_else(|_| {
            eprintln!("⚠️  GEMINI_API_KEY not set in .env");
            eprintln!("📌 See .env.example for setup instructions");
            "mock_key".to_string()
        });

    let api_port: u16 = std::env::var("PORT")
        .or_else(|_| std::env::var("API_PORT"))
        .unwrap_or_else(|_| "8080".to_string())
        .parse()?;

    info!("🚀 Financial Agent Orchestrator - API Server");
    info!("📍 Port: {}", api_port);

    // Create components
    let planner = Box::new(GeminiPlanner::new(gemini_api_key));
    let registry = create_default_registry();
    let execution_engine = ExecutionEngine::new(registry);
    let verification_engine = create_default_verification_engine();
    let state_store = Box::new(InMemoryStateStore::new());
    let audit_log = AuditLog::new();

    // Create orchestrator
    let orchestrator = Arc::new(Orchestrator::new(
        planner,
        execution_engine,
        verification_engine,
        state_store,
        audit_log,
    ));

    info!("✅ Orchestrator initialized");
    info!("📡 Starting API server...");

    // Start API server
    start_server(orchestrator, api_port).await?;

    Ok(())
}
