//! Execution engine for deterministic plan execution
//!
//! This is where plans become reality.
//! LLM is NOT allowed here.

use crate::models::{ExecutionStatus, Observation, Plan, ToolInput};
use crate::tools::ToolRegistry;
use crate::error::OrchestrationError;
use crate::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, warn};
use uuid::Uuid;

/// Maximum steps allowed per plan (defensive guard)
const MAX_STEPS_PER_PLAN: usize = 50;

/// Executes a plan step-by-step deterministically
pub struct ExecutionEngine {
    tool_registry: ToolRegistry,
}

impl ExecutionEngine {
    pub fn new(tool_registry: ToolRegistry) -> Self {
        Self { tool_registry }
    }

    /// Execute all steps in a plan in order (fail-fast, dependency-safe)
    pub async fn execute_plan(
        &self,
        plan: &Plan,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> Result<Vec<Observation>> {
        if plan.steps.len() > MAX_STEPS_PER_PLAN {
            return Err(
                OrchestrationError::InvalidPlan(format!(
                    "Plan exceeds maximum allowed steps ({})",
                    MAX_STEPS_PER_PLAN
                ))
            );

        }

        let mut observations = Vec::with_capacity(plan.steps.len());

        let mut step_outputs: HashMap<u32, serde_json::Value> =
            HashMap::with_capacity(plan.steps.len());

        debug!(plan_id = ?plan.plan_id, "Starting plan execution");

        for step in &plan.steps {
            debug!(
                step_order = step.order,
                tool_name = %step.tool_name,
                "Processing step"
            );

            // -------------------------------------------------
            // 1️⃣ HARD DEPENDENCY VALIDATION
            // -------------------------------------------------
            let missing_dependencies: Vec<u32> = step
                .dependencies
                .iter()
                .filter(|dep| !step_outputs.contains_key(dep))
                .cloned()
                .collect();

            if !missing_dependencies.is_empty() {
                warn!(
                    step_order = step.order,
                    ?missing_dependencies,
                    "Skipping step due to unmet dependencies"
                );

                let observation = Observation {
                    observation_id: Uuid::new_v4(),
                    tenant_id,
                    user_id,
                    plan_id: plan.plan_id,
                    step_id: step.step_id,
                    tool_name: step.tool_name.clone(),
                    tool_input: step.tool_input.clone(),
                    tool_output: serde_json::json!({
                        "error": format!(
                            "Unmet dependencies: {:?}",
                            missing_dependencies
                        )
                    }),
                    execution_time_ms: 0,
                    created_at: Utc::now(),
                    status: ExecutionStatus::Skipped,
                };

                observations.push(observation);

                // DO NOT EXECUTE
                continue;
            }

            let start = Instant::now();

            let tool_input = ToolInput {
                tool_name: step.tool_name.clone(),
                parameters: step.tool_input.clone(),
            };

            let status;
            let tool_output;

            // -------------------------------------------------
            // 2️⃣ TOOL LOOKUP + EXECUTION
            // -------------------------------------------------
            match self.tool_registry.get(&step.tool_name) {
                Some(tool) => match tool.execute(&tool_input).await {
                    Ok(output) => {
                        status = ExecutionStatus::Success;
                        tool_output = output.data;

                        // Store only successful outputs
                        step_outputs.insert(step.order, tool_output.clone());
                    }
                    Err(e) => {
                        status = ExecutionStatus::Failed;
                        tool_output = serde_json::json!({
                            "error": e.to_string()
                        });

                        warn!(
                            step_order = step.order,
                            error = %e,
                            "Tool execution failed"
                        );
                    }
                },
                None => {
                    status = ExecutionStatus::Skipped;
                    tool_output = serde_json::json!({
                        "error": "Tool not registered"
                    });

                    warn!(
                        step_order = step.order,
                        tool_name = %step.tool_name,
                        "Tool not registered"
                    );
                }
            }

            let execution_time_ms = start.elapsed().as_millis() as u64;

            let observation = Observation {
                observation_id: Uuid::new_v4(),
                tenant_id,
                user_id,
                plan_id: plan.plan_id,
                step_id: step.step_id,
                tool_name: step.tool_name.clone(),
                tool_input: step.tool_input.clone(),
                tool_output: tool_output.clone(),
                execution_time_ms,
                created_at: Utc::now(),
                status: status.clone(),
            };

            observations.push(observation);

            // -------------------------------------------------
            // 3️⃣ FAIL-FAST SAFETY
            // -------------------------------------------------
            if status == ExecutionStatus::Failed {
                warn!(
                    plan_id = ?plan.plan_id,
                    step_order = step.order,
                    "Halting execution due to step failure"
                );
                break;
            }
        }

        debug!(
            plan_id = ?plan.plan_id,
            observation_count = observations.len(),
            "Plan execution completed"
        );

        Ok(observations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::create_default_registry;

    #[tokio::test]
    async fn test_execution_engine() {
        let registry = create_default_registry();
        let engine = ExecutionEngine::new(registry);

        use crate::models::{Goal, GoalContext, PlanStep};

        let goal = Goal {
            goal_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            influencer_id: None,
            description: "Test goal".to_string(),
            created_at: Utc::now(),
            context: GoalContext {
                current_portfolio: None,
                constraints: vec![],
                risk_tolerance: "medium".to_string(),
                time_horizon: "5 years".to_string(),
            },
        };

        let plan = Plan {
            plan_id: Uuid::new_v4(),
            tenant_id: goal.tenant_id,
            user_id: goal.user_id,
            influencer_id: None,
            goal_id: goal.goal_id,
            steps: vec![PlanStep {
                step_id: Uuid::new_v4(),
                order: 1,
                tool_name: "fetch_market_data".to_string(),
                tool_input: serde_json::json!({"symbol": "AAPL"}),
                expected_output: "Market data".to_string(),
                dependencies: vec![],
            }],
            success_criteria: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            replans_count: 0,
            failure_reason: None,
        };

        let result = engine
            .execute_plan(&plan, goal.tenant_id, goal.user_id)
            .await;

        assert!(result.is_ok());

        let observations = result.unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].status, ExecutionStatus::Success);
    }
}
