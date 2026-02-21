use financial_agent_orchestrator::{
    agent::Orchestrator,
    audit::AuditLog,
    execution::ExecutionEngine,
    models::{Goal, GoalContext},
    planner::MockPlanner,
    state::InMemoryStateStore,
    tools::create_default_registry,
    verification::create_default_verification_engine,
};
use chrono::Utc;
use uuid::Uuid;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Financial Agent Orchestrator starting");

    // Create components
    let planner = Box::new(MockPlanner);
    let registry = create_default_registry();
    let execution_engine = ExecutionEngine::new(registry);
    let verification_engine = create_default_verification_engine();
    let state_store = Box::new(InMemoryStateStore::new());
    let audit_log = AuditLog::new();

    // Create orchestrator
    let orchestrator = Orchestrator::new(
        planner,
        execution_engine,
        verification_engine,
        state_store,
        audit_log,
    );

    // Create a sample goal
    let goal = Goal {
        goal_id: Uuid::new_v4(),
        tenant_id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        influencer_id: None,
        description: "Analyze my portfolio and recommend rebalancing".to_string(),
        created_at: Utc::now(),
        context: GoalContext {
            current_portfolio: Some("AAPL, MSFT, GOOGL".to_string()),
            constraints: vec!["Max 30% in tech".to_string()],
            risk_tolerance: "medium".to_string(),
            time_horizon: "5 years".to_string(),
        },
    };

    info!(
        goal_id = ?goal.goal_id,
        description = %goal.description,
        "Running orchestrator"
    );

    // Run orchestration
    match orchestrator.run(goal).await {
        Ok(result) => {
            info!("Orchestration successful");
            println!("\n=== ORCHESTRATION RESULT ===");
            println!("Audit ID: {}", result.audit_id);
            println!("Risk Level: {}", result.risk_summary);
            println!("Compliance: {}", result.compliance_statement);
            println!("\nReasoning Trace:");
            for (i, trace) in result.reasoning_trace.iter().enumerate() {
                println!("  {}: {}", i + 1, trace);
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Orchestration failed: {}", e);
            Err(Box::new(e) as Box<dyn std::error::Error>)
        }
    }
}
