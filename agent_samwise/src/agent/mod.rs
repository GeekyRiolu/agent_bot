//! Main orchestrator - implements the unified loop
//!
//! INPUT → PLAN → EXECUTE → OBSERVE → VERIFY → REPLAN? → COMPLETE

use crate::audit::compute_context_hash;
use crate::audit::AuditLog;
use crate::execution::ExecutionEngine;
use crate::models::{ExecutionRecord, ExecutionStatus, Goal, OrchestrationResult};
use crate::planner::Planner;
use crate::state::StateStore;
use crate::verification::VerificationEngine;
use crate::Result;
use chrono::Utc;
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

const MAX_REPLAN_ATTEMPTS: u32 = 5;
const MAX_STEPS_PER_PLAN: u32 = 20;

fn output_data(output: &Value) -> &Value {
    output.get("data").unwrap_or(output)
}

/// Build a rich markdown summary for a backtest result, including
/// a summary section, per-stock metrics table, and individual trade tables.
fn summarize_backtest(data: &Value) -> String {
    let mut out = String::new();

    // ── Summary header ──
    let summary = data.get("summary");
    let successful = summary
        .and_then(|v| v.get("successful"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let total = summary
        .and_then(|v| v.get("total_stocks"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let failed = summary
        .and_then(|v| v.get("failed"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let execution_time = summary
        .and_then(|v| v.get("execution_time"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    out.push_str("### Backtest Results\n\n");
    out.push_str(&format!(
        "**{}** / **{}** stock(s) successful",
        successful, total
    ));
    if failed > 0 {
        out.push_str(&format!(" • {} failed", failed));
    }
    out.push_str(&format!(" • Execution: {:.2}s\n\n", execution_time));

    // ── Per-stock metrics table ──
    if let Some(results_obj) = data.get("results").and_then(Value::as_object) {
        if !results_obj.is_empty() {
            out.push_str("| Stock | Return % | Total Return | Trades | Win Rate |\n");
            out.push_str("|-------|----------|-------------|--------|----------|\n");

            for (stock, result) in results_obj {
                let metrics = result.get("metrics");
                let return_pct = metrics
                    .and_then(|m| m.get("return_pct"))
                    .and_then(Value::as_f64);
                let total_return = metrics
                    .and_then(|m| m.get("total_return"))
                    .and_then(Value::as_f64);
                let total_trades = metrics
                    .and_then(|m| m.get("total_trades"))
                    .and_then(Value::as_i64);
                let win_rate = metrics
                    .and_then(|m| m.get("win_rate"))
                    .and_then(Value::as_f64);
                let success = result
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);

                let status_icon = if success { "" } else { " ⚠️" };

                out.push_str(&format!(
                    "| {}{} | {} | {} | {} | {} |\n",
                    stock,
                    status_icon,
                    return_pct
                        .map(|v| format!("{:.2}%", v))
                        .unwrap_or_else(|| "—".into()),
                    total_return
                        .map(|v| format!("₹{:.0}", v))
                        .unwrap_or_else(|| "—".into()),
                    total_trades
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "—".into()),
                    win_rate
                        .map(|v| format!("{:.2}%", v))
                        .unwrap_or_else(|| "—".into()),
                ));
            }

            out.push('\n');

            // ── Per-stock trade details ──
            for (stock, result) in results_obj {
                if let Some(trades) = result.get("trades").and_then(Value::as_array) {
                    if trades.is_empty() {
                        continue;
                    }
                    out.push_str(&format!("**Trades — {}**\n\n", stock));
                    out.push_str("| # | Entry Date | Exit Date | PnL | PnL % |\n");
                    out.push_str("|---|-----------|----------|-----|-------|\n");

                    for (i, trade) in trades.iter().enumerate() {
                        let entry_date = trade
                            .get("entry_date")
                            .and_then(Value::as_str)
                            .unwrap_or("—");
                        let exit_date = trade
                            .get("exit_date")
                            .and_then(Value::as_str)
                            .unwrap_or("—");
                        let pnl = trade.get("pnl").and_then(Value::as_f64);
                        let pnl_pct = trade.get("pnl_pct").and_then(Value::as_f64);

                        out.push_str(&format!(
                            "| {} | {} | {} | {} | {} |\n",
                            i + 1,
                            entry_date,
                            exit_date,
                            pnl.map(|v| format!("₹{:.0}", v))
                                .unwrap_or_else(|| "—".into()),
                            pnl_pct
                                .map(|v| format!("{:.2}%", v))
                                .unwrap_or_else(|| "—".into()),
                        ));
                    }

                    out.push('\n');
                }
            }
        }
    }

    // If completely empty (no results), fallback to a simple message
    if out.trim().is_empty() {
        return "Backtest completed but returned no results.".to_string();
    }

    // Append the full structured data so the frontend can render a richer UI.
    if let Ok(pretty) = serde_json::to_string_pretty(data) {
        out.push_str("```backtest-results\n");
        out.push_str(&pretty);
        out.push_str("\n```\n");
    }

    out
}

fn summarize_tool_output(observation: &crate::models::Observation) -> String {
    let output = &observation.tool_output;
    let data = output_data(output);

    match observation.tool_name.as_str() {
        "strategy_builder" => {
            let pretty = serde_json::to_string_pretty(data).unwrap_or_else(|_| data.to_string());
            format!("```json\n{}\n```", pretty)
        }
        "backtester" => {
            summarize_backtest(data)
        }
        "screener" => {
            let matched = data
                .get("total_matched")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let scanned = data
                .get("total_scanned")
                .and_then(Value::as_i64)
                .unwrap_or(0);

            format!(
                "Screener completed: {} match(es) out of {} scanned.",
                matched, scanned
            )
        }
        _ => format!("{} completed successfully.", observation.tool_name),
    }
}

/// Main orchestrator that coordinates the entire workflow
pub struct Orchestrator {
    planner: Box<dyn Planner>,
    execution_engine: ExecutionEngine,
    verification_engine: VerificationEngine,
    state_store: Box<dyn StateStore>,
    audit_log: AuditLog,
}

impl Orchestrator {
    pub fn new(
        planner: Box<dyn Planner>,
        execution_engine: ExecutionEngine,
        verification_engine: VerificationEngine,
        state_store: Box<dyn StateStore>,
        audit_log: AuditLog,
    ) -> Self {
        Self {
            planner,
            execution_engine,
            verification_engine,
            state_store,
            audit_log,
        }
    }

    /// Run the unified orchestration loop
    pub async fn run(&self, goal: Goal) -> Result<OrchestrationResult> {
        let start_time = Instant::now();
        let mut reasoning_trace = Vec::new();
        let mut all_observations;
        let mut replan_count = 0;

        info!(
            goal_id = ?goal.goal_id,
            user_id = ?goal.user_id,
            description = %goal.description,
            "Orchestrator: starting execution"
        );

        reasoning_trace.push("INPUT: Goal received".to_string());

        // === PLAN ===
        reasoning_trace.push("PLAN: Creating execution plan".to_string());
        debug!("Loading context for user");

        let mut context = self.state_store.load_context(goal.user_id).await?;
        context.plan_id = Uuid::new_v4();

        loop {
            if replan_count > MAX_REPLAN_ATTEMPTS {
                return Err(crate::error::OrchestrationError::MaxReplansExceeded(
                    format!("Exceeded {} replan attempts", MAX_REPLAN_ATTEMPTS),
                ));
            }

            let failure_reason = if replan_count > 0 {
                Some("Previous execution failed verification")
            } else {
                None
            };

            let plan = self
                .planner
                .create_plan(&goal, &context, failure_reason)
                .await?;

            debug!(
                plan_id = ?plan.plan_id,
                step_count = plan.steps.len(),
                replans = replan_count,
                "Plan created"
            );

            if plan.steps.len() > MAX_STEPS_PER_PLAN as usize {
                return Err(crate::error::OrchestrationError::InvalidPlan(format!(
                    "Plan exceeds {} steps",
                    MAX_STEPS_PER_PLAN
                )));
            }

            reasoning_trace.push(format!("PLAN: {} steps in plan", plan.steps.len()));

            // === EXECUTE ===
            reasoning_trace.push("EXECUTE: Running plan steps".to_string());
            debug!("Executing plan");

            let observations = self
                .execution_engine
                .execute_plan(&plan, goal.tenant_id, goal.user_id)
                .await?;

            // For single-tool mode, fail fast if the selected tool does not succeed.
            if plan.steps.len() == 1 {
                if let Some(failed) = observations
                    .iter()
                    .find(|obs| obs.status != ExecutionStatus::Success)
                {
                    let detail = failed
                        .tool_output
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("tool execution failed");
                    return Err(crate::error::OrchestrationError::ExecutionError(format!(
                        "{} failed: {}",
                        failed.tool_name, detail
                    )));
                }
            }

            all_observations = observations.clone();

            for (i, obs) in observations.iter().enumerate() {
                reasoning_trace.push(format!(
                    "OBSERVE: Step {} ({}) - {} ms",
                    i + 1,
                    obs.tool_name,
                    obs.execution_time_ms
                ));

                self.state_store.persist_observation(obs.clone()).await?;
            }

            debug!(
                plan_id = ?plan.plan_id,
                observation_count = observations.len(),
                "Execution complete"
            );

            // === VERIFY ===
            reasoning_trace.push("VERIFY: Running compliance checks".to_string());
            debug!("Running verification");

            let verification_result =
                self.verification_engine
                    .verify(&plan, &observations, &context)?;

            reasoning_trace.push(format!(
                "VERIFY: {} / {} rules passed",
                verification_result
                    .compliance_checks
                    .iter()
                    .filter(|c| c.passed)
                    .count(),
                verification_result.compliance_checks.len()
            ));

            if verification_result.verified {
                reasoning_trace.push("COMPLETE: Verification passed".to_string());

                info!(
                    plan_id = ?plan.plan_id,
                    "Verification passed - execution complete"
                );
                let context_hash = compute_context_hash(&goal);
                // Store in audit log
                let execution_record = ExecutionRecord {
                    audit_id: Uuid::new_v4(),
                    tenant_id: goal.tenant_id,
                    user_id: goal.user_id,
                    goal: Arc::new(goal.clone()),
                    context_snapshot_hash: context_hash,
                    plan: Arc::new(plan),
                    observations: Arc::new(all_observations.clone()),
                    verification_result: Arc::new(verification_result.clone()),
                    final_output: serde_json::json!({
                        "status": "success",
                        "observations": all_observations.len(),
                    }),
                    reasoning_trace: Arc::new(reasoning_trace.clone()),
                    created_at: Utc::now(),
                    execution_time_ms: start_time.elapsed().as_millis() as u64,
                };

                let audit_id = execution_record.audit_id;
                self.audit_log.record(execution_record).await?;

                let primary_observation = all_observations
                    .iter()
                    .find(|obs| obs.status == crate::models::ExecutionStatus::Success)
                    .or_else(|| all_observations.first());

                let result_json = if let Some(obs) = primary_observation {
                    serde_json::json!({
                        "status": "success",
                        "observations": all_observations.len(),
                        "tool_name": obs.tool_name,
                        "tool_output": obs.tool_output,
                        "summary": summarize_tool_output(obs),
                    })
                } else {
                    serde_json::json!({
                        "status": "success",
                        "observations": all_observations.len(),
                        "summary": "Execution completed successfully.",
                    })
                };

                return Ok(OrchestrationResult {
                    result: result_json,
                    risk_summary: format!("{:?}", verification_result.risk_level),
                    compliance_statement: format!(
                        "{} checks passed",
                        verification_result
                            .compliance_checks
                            .iter()
                            .filter(|c| c.passed)
                            .count()
                    ),
                    audit_id,
                    reasoning_trace,
                });
            } else {
                // === REPLAN ===
                reasoning_trace.push(format!(
                    "REPLAN: Verification failed - attempt {}",
                    replan_count + 1
                ));

                warn!(
                    plan_id = plan.plan_id.to_string(),
                    issues = ?verification_result.issues,
                    "Verification failed - replanning"
                );

                replan_count += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::ExecutionEngine;
    use crate::models::GoalContext;
    use crate::planner::MockPlanner;
    use crate::state::InMemoryStateStore;
    use crate::tools::create_default_registry;
    use crate::verification::create_default_verification_engine;

    #[tokio::test]
    async fn test_orchestrator_run() {
        let planner = Box::new(MockPlanner);
        let registry = create_default_registry();
        let execution_engine = ExecutionEngine::new(registry);
        let verification_engine = create_default_verification_engine();
        let state_store = Box::new(InMemoryStateStore::new());
        let audit_log = AuditLog::new();

        let orchestrator = Orchestrator::new(
            planner,
            execution_engine,
            verification_engine,
            state_store,
            audit_log,
        );

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

        let result = orchestrator.run(goal).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.reasoning_trace.is_empty());
    }
}
