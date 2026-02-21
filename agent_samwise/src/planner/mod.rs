//! Planner trait and implementations
//!
//! The Planner uses LLM to decompose goals into structured plans.
//! It generates deterministic-executable plans.

use crate::models::{Goal, Plan, ContextSnapshot};
use crate::Result;
use async_trait::async_trait;

pub mod gemini;
pub use gemini::GeminiPlanner;

/// Trait for plan generation (LLM controlled)
#[async_trait]
pub trait Planner: Send + Sync {
    /// Create a plan for a goal, optionally with failure context for replanning
    async fn create_plan(
        &self,
        goal: &Goal,
        context: &ContextSnapshot,
        failure_reason: Option<&str>,
    ) -> Result<Plan>;
}

/// Mock planner for development & testing
/// Keeps system functional without LLM dependency
pub struct MockPlanner;

#[async_trait]
impl Planner for MockPlanner {
    async fn create_plan(
        &self,
        goal: &Goal,
        context: &ContextSnapshot,
        _failure_reason: Option<&str>,
    ) -> Result<Plan> {

        use crate::models::PlanStep;
        use uuid::Uuid;
        use chrono::Utc;

        let plan = Plan {
            plan_id: Uuid::new_v4(),
            tenant_id: goal.tenant_id,
            user_id: goal.user_id,
            influencer_id: goal.influencer_id,
            goal_id: goal.goal_id,
            steps: vec![
                PlanStep {
                    step_id: Uuid::new_v4(),
                    order: 1,
                    tool_name: "web_search".to_string(),
                    tool_input: serde_json::json!({
                        "query": goal.description
                    }),
                    expected_output: "Relevant financial data".to_string(),
                    dependencies: vec![],
                },
                PlanStep {
                    step_id: Uuid::new_v4(),
                    order: 2,
                    tool_name: "insights".to_string(),
                    tool_input: serde_json::json!({
                        "analysis_target": "portfolio"
                    }),
                    expected_output: "Generated financial insights".to_string(),
                    dependencies: vec![1],
                },
            ],
            success_criteria: vec![
                "Relevant data retrieved".to_string(),
                "Insights generated successfully".to_string(),
            ],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            replans_count: 0,
            failure_reason: None,
        };

        Ok(plan)
    }
}
