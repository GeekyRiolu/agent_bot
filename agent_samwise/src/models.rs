//! Core data models for the financial agent

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;
use std::fmt;

//
// ================= Enums =================
//

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RiskTolerance {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TimeHorizon {
    ShortTerm,
    MediumTerm,
    LongTerm,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

//
// ================= Goal =================
//

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Goal {
    pub goal_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub influencer_id: Option<Uuid>,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub context: GoalContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalContext {
    pub current_portfolio: Option<String>,
    pub constraints: Vec<String>,
    pub risk_tolerance: RiskTolerance,
    pub time_horizon: TimeHorizon,
}

//
// ================= Plan =================
//

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub plan_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub influencer_id: Option<Uuid>,
    pub goal_id: Uuid,
    pub steps: Vec<PlanStep>,
    pub success_criteria: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub replans_count: u32,
    #[serde(default)]
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_id: Uuid,
    pub order: u32,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub expected_output: String,
    pub dependencies: Vec<u32>,
}

//
// ================= Execution =================
//

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub observation_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub plan_id: Uuid,
    pub step_id: Uuid,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
    pub tool_output: serde_json::Value,
    pub execution_time_ms: u64,
    pub created_at: DateTime<Utc>,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    Success,
    Failed,
    Skipped,
}

//
// ================= Context =================
//

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSnapshot {
    pub snapshot_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub plan_id: Uuid,
    pub observations: Vec<Observation>,
    pub portfolio_state: Option<String>,
    pub created_at: DateTime<Utc>,
    pub context_hash: String,
}

//
// ================= Verification =================
//

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub verified: bool,
    pub risk_level: RiskLevel,
    pub compliance_checks: Vec<ComplianceCheck>,
    pub issues: Vec<String>,
    pub verified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceCheck {
    pub rule_name: String,
    pub passed: bool,
    pub details: String,
}

//
// ================= Execution Record =================
//

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub audit_id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,

    pub goal: Arc<Goal>,
    pub context_snapshot_hash: String,
    pub plan: Arc<Plan>,
    pub observations: Arc<Vec<Observation>>,
    pub verification_result: Arc<VerificationResult>,

    pub final_output: serde_json::Value,
    pub reasoning_trace: Arc<Vec<String>>,

    pub created_at: DateTime<Utc>,
    pub execution_time_ms: u64,
}

//
// ================= Tool I/O =================
//

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInput {
    pub tool_name: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
}

//
// ================= Final Result =================
//

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestrationResult {
    pub result: serde_json::Value,
    pub risk_summary: String,
    pub compliance_statement: String,
    pub audit_id: Uuid,
    pub reasoning_trace: Vec<String>,
}

impl fmt::Display for RiskTolerance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            RiskTolerance::Low => "Low",
            RiskTolerance::Medium => "Medium",
            RiskTolerance::High => "High",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for TimeHorizon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            TimeHorizon::ShortTerm => "Short-Term",
            TimeHorizon::MediumTerm => "Medium-Term",
            TimeHorizon::LongTerm => "Long-Term",
        };
        write!(f, "{}", s)
    }
}
