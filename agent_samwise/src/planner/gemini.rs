//! Gemini-powered planner for goal decomposition
//!
//! Uses Google's Gemini API to intelligently create structured plans

use crate::gemini::GeminiClient;
use crate::models::{ContextSnapshot, Goal, Plan, PlanStep};
use crate::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

pub struct GeminiPlanner {
    client: GeminiClient,
}

impl GeminiPlanner {
    pub fn new(api_key: String) -> Self {
        Self {
            client: GeminiClient::new(api_key),
        }
    }

    fn build_single_step_plan(
        goal: &Goal,
        tool_name: &str,
        tool_input: serde_json::Value,
        expected_output: &str,
    ) -> Plan {
        Plan {
            plan_id: Uuid::new_v4(),
            tenant_id: goal.tenant_id,
            user_id: goal.user_id,
            influencer_id: goal.influencer_id,
            goal_id: goal.goal_id,
            steps: vec![PlanStep {
                step_id: Uuid::new_v4(),
                order: 1,
                tool_name: tool_name.to_string(),
                tool_input,
                expected_output: expected_output.to_string(),
                dependencies: vec![],
            }],
            success_criteria: vec![format!("{} executed successfully", tool_name)],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            replans_count: 0,
            failure_reason: None,
        }
    }

    /// Try to extract a JSON object from a markdown ```json ... ``` fence
    /// inside the description. Returns `Some(Value)` when the block parses
    /// successfully and contains an `"ast"` key (i.e. it is a run-config payload).
    fn extract_json_config(description: &str) -> Option<serde_json::Value> {
        // Look for ```json ... ``` fenced block
        let start = description.find("```json")?;
        let after_fence = &description[start + 7..]; // skip "```json"
        let end = after_fence.find("```")?;
        let json_str = after_fence[..end].trim();

        let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
        if parsed.is_object() && parsed.get("ast").is_some() {
            Some(parsed)
        } else {
            None
        }
    }

    fn select_tool_by_intent(goal: &Goal) -> Option<Plan> {
        let description = goal.description.trim();
        let lowered = description.to_lowercase();

        let has_any = |keywords: &[&str]| keywords.iter().any(|k| lowered.contains(k));

        let has_backtest = has_any(&["backtest", "back test", "run-config", "run config"]);
        let has_strategy = has_any(&[
            "strategy",
            "buy when",
            "sell when",
            "entry",
            "exit",
            "stop loss",
            "take profit",
        ]);

        // ── Priority 1: Backtest with a ready-made JSON config (has "ast" key)
        // This is the path the frontend "Backtest Strategy" button takes.
        if has_backtest {
            if let Some(config) = Self::extract_json_config(description) {
                return Some(Self::build_single_step_plan(
                    goal,
                    "backtester",
                    config,
                    "Backtest summary metrics",
                ));
            }
        }

        // ── Priority 2: Strategy definition keywords present
        // Even if the user also says "backtest", we must parse the strategy
        // first so the backtester receives a proper AST later.
        if has_strategy {
            return Some(Self::build_single_step_plan(
                goal,
                "strategy_builder",
                json!({ "strategy_text": description }),
                "Parsed strategy AST",
            ));
        }

        // ── Priority 3: Backtest keyword without strategy keywords or JSON config
        if has_backtest {
            return Some(Self::build_single_step_plan(
                goal,
                "backtester",
                json!({ "strategy_text": description }),
                "Backtest summary metrics",
            ));
        }

        if has_any(&[
            "screener",
            "screen",
            "find stocks",
            "show me stocks",
            "oversold",
            "momentum stocks",
            "macd",
            "ema",
            "rsi below",
            "rsi above",
        ]) {
            return Some(Self::build_single_step_plan(
                goal,
                "screener",
                json!({
                    "query": description,
                    "limit": 10,
                    "data_source": "yfinance",
                    "force_database": false
                }),
                "Filtered stock candidates",
            ));
        }

        if has_any(&["news", "sentiment", "headline"]) {
            return Some(Self::build_single_step_plan(
                goal,
                "news",
                json!({ "query": description }),
                "Market news summary",
            ));
        }

        if has_any(&["insight", "insights", "opportunity", "risk analysis"]) {
            return Some(Self::build_single_step_plan(
                goal,
                "insights",
                json!({ "query": description }),
                "Actionable investment insights",
            ));
        }

        if has_any(&[
            "macro",
            "inflation",
            "yield",
            "interest rate",
            "cpi",
            "gdp",
            "fomc",
        ]) {
            return Some(Self::build_single_step_plan(
                goal,
                "web_search",
                json!({ "query": description }),
                "Macro market data",
            ));
        }

        None
    }

    /// Build structured planning prompt
    fn build_prompt(
        &self,
        goal: &Goal,
        context: &ContextSnapshot,
        failure_reason: Option<&str>,
    ) -> String {
        let tool_descriptions = vec![
            "web_search – Retrieve live financial data or macro information",
            "screener – Filter stocks by financial criteria",
            "strategy_builder – Construct trading strategies",
            "backtester – Backtest trading strategies on historical data",
            "news – Retrieve financial news and sentiment",
            "insights – Generate analytical financial insights",
        ];

        let base_prompt = format!(
            r#"You are a financial planning engine.

Create a deterministic, executable financial plan.
IMPORTANT: for now, return exactly ONE tool step.

GOAL:
{}

PORTFOLIO:
{:?}

CONSTRAINTS:
{:?}

RISK TOLERANCE:
{}

TIME HORIZON:
{}

Available tools:
- {}

Tool input guidance:
- strategy_builder: {{ "strategy_text": "<natural language strategy>" }}
- backtester:
  - if config/AST is already available: pass it directly in tool_input
  - if only natural language is available: {{ "strategy_text": "<strategy text>" }}
- screener:
  - NLP mode: {{ "query": "<nl query>", "limit": 10 }}
  - Structured mode: {{ "strategy": "...", "filters": {{...}}, "indicators": {{...}}, "limit": 10 }}
- web_search/news/insights: {{ "query": "<user query>" }}

Rules:
- Exactly 1 step
- Each step must reference only available tools
- dependencies must be []
- Return ONLY valid JSON
- No explanation text
- JSON format:

{{
  "steps": [
    {{
      "order": 1,
      "tool_name": "web_search",
      "tool_input": {{ ... }},
      "expected_output": "...",
      "dependencies": []
    }}
  ],
  "success_criteria": ["..."]
}}
"#,
            goal.description,
            goal.context.current_portfolio,
            goal.context.constraints,
            goal.context.risk_tolerance,
            goal.context.time_horizon,
            tool_descriptions.join("\n- "),
        );

        if let Some(reason) = failure_reason {
            format!(
                "Previous plan failed verification:\n{}\n\nGenerate a DIFFERENT improved plan.\n\n{}",
                reason,
                base_prompt
            )
        } else {
            base_prompt
        }
    }
}

#[async_trait]
impl crate::planner::Planner for GeminiPlanner {
    async fn create_plan(
        &self,
        goal: &Goal,
        context: &ContextSnapshot,
        failure_reason: Option<&str>,
    ) -> Result<Plan> {
        // Deterministic intent routing for high-confidence single-tool requests.
        if let Some(plan) = Self::select_tool_by_intent(goal) {
            return Ok(plan);
        }

        let prompt = self.build_prompt(goal, context, failure_reason);

        let (response, _confidence) = self.client.generate(&prompt, None).await?;

        parse_plan_response(&response, goal)
    }
}

/// Parse plan response from Gemini
fn parse_plan_response(response: &str, goal: &Goal) -> Result<Plan> {
    let cleaned = response
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let json: serde_json::Value = serde_json::from_str(cleaned).map_err(|e| {
        crate::error::OrchestrationError::LlmError(format!(
            "Failed to parse Gemini plan response: {} | raw={}",
            e, response
        ))
    })?;

    let steps_json = json
        .get("steps")
        .ok_or_else(|| {
            crate::error::OrchestrationError::InvalidPlan("No steps in response".to_string())
        })?
        .as_array()
        .ok_or_else(|| {
            crate::error::OrchestrationError::InvalidPlan("Steps is not an array".to_string())
        })?;

    let mut steps = Vec::new();

    for step_json in steps_json.iter().take(1) {
        let order = step_json
            .get("order")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| {
                crate::error::OrchestrationError::InvalidPlan("Missing order".to_string())
            })? as u32;

        let tool_name = step_json
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::error::OrchestrationError::InvalidPlan("Missing tool_name".to_string())
            })?
            .to_string();

        let tool_input = step_json
            .get("tool_input")
            .ok_or_else(|| {
                crate::error::OrchestrationError::InvalidPlan("Missing tool_input".to_string())
            })?
            .clone();

        let expected_output = step_json
            .get("expected_output")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                crate::error::OrchestrationError::InvalidPlan("Missing expected_output".to_string())
            })?
            .to_string();

        let dependencies: Vec<u32> = vec![];

        steps.push(PlanStep {
            step_id: Uuid::new_v4(),
            order,
            tool_name,
            tool_input,
            expected_output,
            dependencies,
        });
    }

    steps.sort_by_key(|s| s.order);

    let success_criteria: Vec<String> = json
        .get("success_criteria")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(Plan {
        plan_id: Uuid::new_v4(),
        tenant_id: goal.tenant_id,
        user_id: goal.user_id,
        influencer_id: goal.influencer_id,
        goal_id: goal.goal_id,
        steps,
        success_criteria,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        replans_count: 0,
        failure_reason: None,
    })
}
