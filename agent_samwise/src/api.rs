//! REST API Server for the Financial Agent Orchestrator
//!
//! Exposes the orchestrator via HTTP endpoints
//! Integrates with frontend UI

use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::agent::Orchestrator;
use crate::classifier::{InteractionClassifier, InteractionType};
use crate::conversational;
use crate::models::{Goal, GoalContext, RiskTolerance, TimeHorizon};

/// =============================
/// Request Models
/// =============================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrchestrationRequest {
    pub goal_description: String,
    pub current_portfolio: Option<String>,
    pub constraints: Vec<String>,
    pub risk_tolerance: String,
    pub time_horizon: String,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub chat_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub chat_id: Option<String>,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub messages: Vec<ChatMessage>,
}

/// =============================
/// Response Wrapper
/// =============================

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
    pub timestamp: String,
}

impl ApiResponse {
    pub fn success<T: Serialize>(data: T) -> Self {
        Self {
            success: true,
            data: serde_json::to_value(data).ok(),
            error: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// =============================
/// API State
/// =============================

#[derive(Clone)]
pub struct ApiState {
    pub orchestrator: Arc<Orchestrator>,
}

/// =============================
/// Helpers — String → Enum Parsing
/// =============================

fn parse_risk(r: String) -> RiskTolerance {
    match r.to_lowercase().as_str() {
        "low" => RiskTolerance::Low,
        "medium" | "moderate" => RiskTolerance::Medium,
        "high" => RiskTolerance::High,
        _ => RiskTolerance::Medium,
    }
}

fn parse_horizon(h: String) -> TimeHorizon {
    match h.to_lowercase().as_str() {
        "short" | "short_term" | "short-term" => TimeHorizon::ShortTerm,
        "medium" | "medium_term" | "medium-term" => TimeHorizon::MediumTerm,
        "long" | "long_term" | "long-term" => TimeHorizon::LongTerm,
        _ => TimeHorizon::MediumTerm,
    }
}

fn stable_uuid_from_string(input: &str) -> uuid::Uuid {
    use sha2::{Digest, Sha256};

    let hash = Sha256::digest(input.as_bytes());
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash[..16]);

    // Set UUID version (4) and variant (RFC4122) bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    uuid::Uuid::from_bytes(bytes)
}

fn parse_or_stable_uuid(value: Option<&str>, fallback_seed: &str) -> uuid::Uuid {
    match value {
        Some(v) if !v.trim().is_empty() => {
            uuid::Uuid::parse_str(v).unwrap_or_else(|_| stable_uuid_from_string(v))
        }
        _ => stable_uuid_from_string(fallback_seed),
    }
}

/// =============================
/// Health Endpoint
/// =============================

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// =============================
/// Main Orchestration Endpoint
/// =============================

async fn run_orchestration(
    State(state): State<ApiState>,
    Json(req): Json<OrchestrationRequest>,
) -> (StatusCode, Json<ApiResponse>) {
    info!("Received orchestration request: {}", req.goal_description);

    let tenant_id = parse_or_stable_uuid(req.tenant_id.as_deref(), "default-tenant");
    let user_id = parse_or_stable_uuid(req.user_id.as_deref(), "anonymous-user");

    // Build Goal
    let goal = Goal {
        goal_id: uuid::Uuid::new_v4(),
        tenant_id,
        user_id,
        influencer_id: req
            .chat_id
            .as_deref()
            .map(|value| parse_or_stable_uuid(Some(value), "chat-fallback")),
        description: req.goal_description.clone(),
        created_at: chrono::Utc::now(),
        context: GoalContext {
            current_portfolio: req.current_portfolio,
            constraints: req.constraints,
            risk_tolerance: parse_risk(req.risk_tolerance),
            time_horizon: parse_horizon(req.time_horizon),
        },
    };

    // Classify interaction
    let interaction_type = InteractionClassifier::classify(&goal);
    info!("Interaction type: {:?}", interaction_type);

    match interaction_type {
        InteractionType::Conversational => {
            info!("Handling as conversational query");

            match conversational::handle_conversational(&goal).await {
                Ok(response) => (
                    StatusCode::OK,
                    Json(ApiResponse::success(serde_json::json!({
                        "type": "conversational",
                        "answer": response.answer,
                        "source": response.source,
                        "confidence": response.confidence,
                    }))),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiResponse::error(format!(
                        "Conversational handler failed: {}",
                        e
                    ))),
                ),
            }
        }

        InteractionType::GoalDriven => {
            info!("Handling as goal-driven orchestration");

            match state.orchestrator.run(goal).await {
                Ok(result) => {
                    let summary = result
                        .result
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Goal-driven execution completed.")
                        .to_string();
                    (
                        StatusCode::OK,
                        Json(ApiResponse::success(serde_json::json!({
                            "type": "goal_driven",
                            "answer": summary,
                            "summary": summary,
                            "result": result.result,
                            "risk_summary": result.risk_summary,
                            "compliance_statement": result.compliance_statement,
                            "audit_id": result.audit_id,
                        }))),
                    )
                }
                Err(e) => (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::error(format!("Orchestration failed: {}", e))),
                ),
            }
        }
    }
}

/// =============================
/// Chat Endpoint
/// =============================

async fn chat_handler(
    State(state): State<ApiState>,
    Json(req): Json<ChatRequest>,
) -> (StatusCode, Json<ApiResponse>) {
    let last_user_message_index = req.messages.iter().rposition(|m| m.role == "user");

    let Some(last_user_message_index) = last_user_message_index else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::error("No user message found".into())),
        );
    };
    let user_msg = &req.messages[last_user_message_index];

    let chat_id = req
        .chat_id
        .as_deref()
        .map(|value| parse_or_stable_uuid(Some(value), "chat-fallback"));

    let tenant_id = parse_or_stable_uuid(
        req.tenant_id.as_deref(),
        req.chat_id.as_deref().unwrap_or("default-tenant"),
    );
    let user_id = parse_or_stable_uuid(
        req.user_id.as_deref(),
        req.chat_id.as_deref().unwrap_or("anonymous-user"),
    );

    let mut constraints = vec![];
    if let Some(chat_id) = chat_id {
        constraints.push(format!("chat_id={}", chat_id));
    }

    let orchestration_req = OrchestrationRequest {
        // Keep only the current user turn here. Conversational memory already
        // tracks context by (user_id, chat_id) and adding full transcript on
        // every request causes prompt growth and stalls after early turns.
        goal_description: user_msg.content.clone(),
        current_portfolio: None,
        constraints,
        risk_tolerance: "moderate".into(),
        time_horizon: "long-term".into(),
        tenant_id: Some(tenant_id.to_string()),
        user_id: Some(user_id.to_string()),
        chat_id: chat_id.map(|v| v.to_string()),
    };
    info!(
        "chat_handler ids => chat_id={:?} tenant_id={} user_id={}",
        chat_id, tenant_id, user_id
    );

    let (status, Json(mut response)) =
        run_orchestration(State(state), Json(orchestration_req)).await;
    if response.success {
        if let Some(data) = response.data.as_mut() {
            if let Some(chat_id) = chat_id {
                data["chat_id"] = serde_json::json!(chat_id.to_string());
            }
            data["tenant_id"] = serde_json::json!(tenant_id.to_string());
            data["user_id"] = serde_json::json!(user_id.to_string());
        }
    }
    (status, Json(response))
}

/// =============================
/// Router
/// =============================

pub fn create_router(orchestrator: Arc<Orchestrator>) -> Router {
    let state = ApiState { orchestrator };

    Router::new()
        .route("/health", axum::routing::get(health))
        .route("/api/orchestrate", post(run_orchestration))
        .route("/api/chat", post(chat_handler))
        .with_state(state)
        .layer(CorsLayer::permissive())
}

/// =============================
/// Server Startup
/// =============================

pub async fn start_server(
    orchestrator: Arc<Orchestrator>,
    port: u16,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let router = create_router(orchestrator);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;

    info!("API Server listening on http://0.0.0.0:{}", port);
    info!("Local: http://127.0.0.1:{}", port);

    axum::serve(listener, router).await?;

    Ok(())
}
