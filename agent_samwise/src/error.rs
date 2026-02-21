//! Error types for the financial agent orchestrator

use thiserror::Error;

/// Result type alias for orchestrator operations
pub type Result<T> = std::result::Result<T, OrchestrationError>;

#[derive(Error, Debug)]
pub enum OrchestrationError {

    // =============================
    // Core Pipeline Errors
    // =============================

    #[error("Planning error: {0}")]
    PlanningError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Tool error: {0}")]
    ToolError(String),

    #[error("Verification error: {0}")]
    VerificationError(String),

    #[error("State persistence error: {0}")]
    StateError(String),

    #[error("Compliance violation: {0}")]
    ComplianceViolation(String),

    #[error("Max replans exceeded: {0}")]
    MaxReplansExceeded(String),

    #[error("Invalid plan: {0}")]
    InvalidPlan(String),

    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    #[error("Invalid tool input: {0}")]
    InvalidToolInput(String),

    #[error("LLM error: {0}")]
    LlmError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Audit error: {0}")]
    AuditError(String),

    #[error("Unknown error: {0}")]
    Unknown(String),

    // =============================
    // External Library Conversions
    // =============================

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("HTTP client error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("UUID parse error: {0}")]
    UuidError(#[from] uuid::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
