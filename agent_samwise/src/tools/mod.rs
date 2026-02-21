//! Tool trait and registry
//!
//! Tools are deterministic, side-effect-free operations.
//! HTTP-backed tools call your financial API service.

use crate::error::OrchestrationError;
use crate::gemini::GeminiClient;
use crate::models::{ToolInput, ToolOutput};
use crate::Result;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

/// Trait for a single tool (deterministic execution)
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn execute(&self, input: &ToolInput) -> Result<ToolOutput>;
}

/// Tool registry for looking up and executing tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    pub fn list(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct FinancialApiClient {
    client: Client,
    base_url: String,
}

impl FinancialApiClient {
    fn from_env() -> Option<Self> {
        let base_url = env::var("FINANCIAL_API_BASE_URL")
            .or_else(|_| env::var("TOOLS_API_BASE_URL"))
            .ok()?;

        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(8)
            .timeout(Duration::from_secs(30))
            .build()
            .ok()?;

        Some(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
        })
    }

    async fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}{}", self.base_url, path);

        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| {
                OrchestrationError::ToolError(format!(
                    "Financial API request failed for {}: {}",
                    path, e
                ))
            })?;

        let status = response.status();
        let body = response
            .json::<Value>()
            .await
            .map_err(|e| OrchestrationError::ToolError(format!("Invalid JSON response: {}", e)))?;

        if !status.is_success() {
            return Err(OrchestrationError::ToolError(format!(
                "Financial API returned {} for {}: {}",
                status, path, body
            )));
        }

        Ok(body)
    }
}

/// Try to extract a JSON config object (with an "ast" key) that is embedded
/// inside the "strategy_text" field.  The planner sometimes wraps an entire
/// user message — including a ```json fenced block — as a single string.
fn extract_json_from_strategy_text(params: &Value) -> Option<Value> {
    let text = params.get("strategy_text")?.as_str()?;

    // 1) Try to find a ```json ... ``` fenced block
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            let json_str = after[..end].trim();
            if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                if parsed.is_object() && parsed.get("ast").is_some() {
                    return Some(parsed);
                }
            }
        }
    }

    // 2) Fallback: try parsing the largest { ... } block in the text
    if let Some(brace_start) = text.find('{') {
        if let Some(brace_end) = text.rfind('}') {
            let json_str = &text[brace_start..=brace_end];
            if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                if parsed.is_object() && parsed.get("ast").is_some() {
                    return Some(parsed);
                }
            }
        }
    }

    None
}

fn require_strategy_text(input: &ToolInput) -> Result<String> {
    input
        .parameters
        .get("strategy_text")
        .and_then(|v| v.as_str())
        .or_else(|| input.parameters.get("query").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .ok_or_else(|| {
            OrchestrationError::InvalidToolInput(
                "Expected 'strategy_text' (or 'query') in tool_input".to_string(),
            )
        })
}

fn ensure_object_parameters(input: &ToolInput) -> Result<()> {
    if input.parameters.is_object() {
        Ok(())
    } else {
        Err(OrchestrationError::InvalidToolInput(
            "tool_input must be a JSON object".to_string(),
        ))
    }
}

pub struct StrategyBuilderTool {
    api: Option<FinancialApiClient>,
}

impl StrategyBuilderTool {
    fn new(api: Option<FinancialApiClient>) -> Self {
        Self { api }
    }
}

#[async_trait::async_trait]
impl Tool for StrategyBuilderTool {
    fn name(&self) -> &'static str {
        "strategy_builder"
    }

    fn description(&self) -> &'static str {
        "Parse natural language strategy into AST config using /api/v1/strategy/parse"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolOutput> {
        let api = self.api.as_ref().ok_or_else(|| {
            OrchestrationError::ToolError(
                "FINANCIAL_API_BASE_URL (or TOOLS_API_BASE_URL) is not configured".to_string(),
            )
        })?;

        ensure_object_parameters(input)?;
        let strategy_text = require_strategy_text(input)?;
        let response = api
            .post_json(
                "/api/v1/strategy/parse",
                &json!({
                    "strategy_text": strategy_text
                }),
            )
            .await?;

        Ok(ToolOutput {
            success: true,
            data: response,
            error: None,
        })
    }
}

pub struct BacktesterTool {
    api: Option<FinancialApiClient>,
}

impl BacktesterTool {
    fn new(api: Option<FinancialApiClient>) -> Self {
        Self { api }
    }
}

#[async_trait::async_trait]
impl Tool for BacktesterTool {
    fn name(&self) -> &'static str {
        "backtester"
    }

    fn description(&self) -> &'static str {
        "Run backtests using /api/v1/backtest/run-config or /api/v1/backtest/run"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolOutput> {
        let api = self.api.as_ref().ok_or_else(|| {
            OrchestrationError::ToolError(
                "FINANCIAL_API_BASE_URL (or TOOLS_API_BASE_URL) is not configured".to_string(),
            )
        })?;

        ensure_object_parameters(input)?;
        let params = input.parameters.clone();

        // If there's already a top-level "ast" key we can use run-config directly.
        // Otherwise, try to extract a JSON config embedded inside "strategy_text"
        // (the planner may have wrapped the whole user message as strategy_text).
        let (endpoint, body) = if params.get("ast").is_some() {
            ("/api/v1/backtest/run-config", params)
        } else if let Some(extracted) = extract_json_from_strategy_text(&params) {
            ("/api/v1/backtest/run-config", extracted)
        } else {
            let body = if params.get("strategy_text").is_none() && params.get("query").is_some() {
                let mut updated = params.clone();
                if let Some(query) = updated.get("query").cloned() {
                    updated["strategy_text"] = query;
                }
                updated
            } else {
                params
            };
            ("/api/v1/backtest/run", body)
        };

        let response = api.post_json(endpoint, &body).await?;

        Ok(ToolOutput {
            success: true,
            data: response,
            error: None,
        })
    }
}

pub struct ScreenerTool {
    api: Option<FinancialApiClient>,
}

impl ScreenerTool {
    fn new(api: Option<FinancialApiClient>) -> Self {
        Self { api }
    }
}

#[async_trait::async_trait]
impl Tool for ScreenerTool {
    fn name(&self) -> &'static str {
        "screener"
    }

    fn description(&self) -> &'static str {
        "Run screener using /api/v1/screener/nlp-query or /api/v1/screener/run"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolOutput> {
        let api = self.api.as_ref().ok_or_else(|| {
            OrchestrationError::ToolError(
                "FINANCIAL_API_BASE_URL (or TOOLS_API_BASE_URL) is not configured".to_string(),
            )
        })?;

        ensure_object_parameters(input)?;
        let mut params = input.parameters.clone();

        if params.get("query").is_some() {
            if params.get("limit").is_none() {
                params["limit"] = json!(10);
            }
            if params.get("data_source").is_none() {
                params["data_source"] = json!("yfinance");
            }
            if params.get("force_database").is_none() {
                params["force_database"] = json!(false);
            }

            let response = api.post_json("/api/v1/screener/nlp-query", &params).await?;
            return Ok(ToolOutput {
                success: true,
                data: response,
                error: None,
            });
        }

        let response = api.post_json("/api/v1/screener/run", &params).await?;

        Ok(ToolOutput {
            success: true,
            data: response,
            error: None,
        })
    }
}

pub struct GeminiQueryTool {
    tool_name: &'static str,
    tool_description: &'static str,
    client: GeminiClient,
    system_prefix: &'static str,
}

impl GeminiQueryTool {
    pub fn new(
        tool_name: &'static str,
        tool_description: &'static str,
        system_prefix: &'static str,
        api_key: String,
    ) -> Self {
        Self {
            tool_name,
            tool_description,
            client: GeminiClient::new(api_key),
            system_prefix,
        }
    }
}

#[async_trait::async_trait]
impl Tool for GeminiQueryTool {
    fn name(&self) -> &'static str {
        self.tool_name
    }

    fn description(&self) -> &'static str {
        self.tool_description
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolOutput> {
        let query = input
            .parameters
            .get("query")
            .and_then(|v| v.as_str())
            .or_else(|| input.parameters.get("text").and_then(|v| v.as_str()))
            .unwrap_or_default();

        if query.is_empty() {
            return Err(OrchestrationError::InvalidToolInput(
                "Expected 'query' for this tool".to_string(),
            ));
        }

        let prompt = format!("{}\n\nUser query: {}", self.system_prefix, query);
        let (answer, confidence) = self.client.generate(&prompt, None).await?;

        Ok(ToolOutput {
            success: true,
            data: json!({
                "answer": answer,
                "confidence": confidence,
                "tool": self.tool_name,
            }),
            error: None,
        })
    }
}

/// Legacy mock tool: Fetch market data
pub struct FetchMarketDataTool;

#[async_trait::async_trait]
impl Tool for FetchMarketDataTool {
    fn name(&self) -> &'static str {
        "fetch_market_data"
    }

    fn description(&self) -> &'static str {
        "Fetch current market data for a given symbol"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolOutput> {
        let symbol = input
            .parameters
            .get("symbol")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");

        Ok(ToolOutput {
            success: true,
            data: json!({
                "symbol": symbol,
                "price": 150.50,
                "change": 2.5,
                "volume": 1000000,
            }),
            error: None,
        })
    }
}

/// Legacy mock tool: Analyze portfolio
pub struct AnalyzePortfolioTool;

#[async_trait::async_trait]
impl Tool for AnalyzePortfolioTool {
    fn name(&self) -> &'static str {
        "analyze_portfolio"
    }

    fn description(&self) -> &'static str {
        "Analyze portfolio composition and risk"
    }

    async fn execute(&self, _input: &ToolInput) -> Result<ToolOutput> {
        Ok(ToolOutput {
            success: true,
            data: json!({
                "total_value": 100000.0,
                "diversification_score": 0.75,
                "risk_level": "medium",
                "top_holdings": ["AAPL", "MSFT", "GOOGL"],
            }),
            error: None,
        })
    }
}

/// Create a default registry with HTTP-backed financial tools.
pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    let financial_api = FinancialApiClient::from_env();
    let gemini_api_key = env::var("GEMINI_API_KEY").unwrap_or_default();

    // Tools requested by planner
    registry.register(Arc::new(StrategyBuilderTool::new(financial_api.clone())));
    registry.register(Arc::new(BacktesterTool::new(financial_api.clone())));
    registry.register(Arc::new(ScreenerTool::new(financial_api.clone())));

    registry.register(Arc::new(GeminiQueryTool::new(
        "web_search",
        "Retrieve live financial data or macro information",
        "Provide concise, up-to-date financial information and macro context.",
        gemini_api_key.clone(),
    )));
    registry.register(Arc::new(GeminiQueryTool::new(
        "news",
        "Retrieve financial news and sentiment",
        "Summarize major market news and sentiment relevant to the query.",
        gemini_api_key.clone(),
    )));
    registry.register(Arc::new(GeminiQueryTool::new(
        "insights",
        "Generate analytical financial insights",
        "Provide structured financial insights, opportunities, and risks.",
        gemini_api_key,
    )));

    // Legacy tools to keep old tests/examples functional.
    registry.register(Arc::new(FetchMarketDataTool));
    registry.register(Arc::new(AnalyzePortfolioTool));

    registry
}
