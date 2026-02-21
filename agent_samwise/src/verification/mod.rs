//! Verification engine for compliance and risk checks
//!
//! Rules-based verification before final output.
//! Deterministic enforcement.

use crate::models::{
    ComplianceCheck, ContextSnapshot, Observation, Plan, RiskLevel, VerificationResult,
};
use crate::Result;
use chrono::Utc;
use tracing::info;
use std::cmp::Ordering;

/// Trait for verification rules
pub trait VerificationRule: Send + Sync {
    fn name(&self) -> &'static str;

    /// Risk severity if this rule fails
    fn risk_level(&self) -> RiskLevel;

    fn verify(
        &self,
        plan: &Plan,
        observations: &[Observation],
        context: &ContextSnapshot,
    ) -> VerificationCheckResult;
}

pub struct VerificationCheckResult {
    pub passed: bool,
    pub details: String,
}

/// Verification engine that enforces rules
pub struct VerificationEngine {
    rules: Vec<Box<dyn VerificationRule>>,
}

impl VerificationEngine {
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
        }
    }

    pub fn add_rule(&mut self, rule: Box<dyn VerificationRule>) {
        self.rules.push(rule);
    }

    /// Verify a complete execution (SYNC — no async overhead)
    pub fn verify(
        &self,
        plan: &Plan,
        observations: &[Observation],
        context: &ContextSnapshot,
    ) -> Result<VerificationResult> {
        let mut compliance_checks =
            Vec::with_capacity(self.rules.len());

        let mut issues = Vec::new();
        let mut max_risk = RiskLevel::Low;

        for rule in &self.rules {
            let result = rule.verify(plan, observations, context);

            let check = ComplianceCheck {
                rule_name: rule.name().to_string(),
                passed: result.passed,
                details: result.details.clone(),
            };

            if !result.passed {
                issues.push(format!(
                    "{}: {}",
                    rule.name(),
                    result.details
                ));

                // escalate based on rule severity
                max_risk = std::cmp::max(max_risk, rule.risk_level());
            }

            compliance_checks.push(check);
        }

        let verified = issues.is_empty();

        info!(
            rule_count = self.rules.len(),
            verified = verified,
            "Verification completed"
        );

        Ok(VerificationResult {
            verified,
            risk_level: max_risk,
            compliance_checks,
            issues,
            verified_at: Utc::now(),
        })
    }
}

impl Default for VerificationEngine {
    fn default() -> Self {
        Self::new()
    }
}

//
// ================= RiskLevel Ordering =================
//

impl PartialOrd for RiskLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.rank().cmp(&other.rank()))
    }
}

impl Ord for RiskLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

impl RiskLevel {
    fn rank(&self) -> u8 {
        match self {
            RiskLevel::Low => 0,
            RiskLevel::Medium => 1,
            RiskLevel::High => 2,
            RiskLevel::Critical => 3,
        }
    }
}

//
// ========== Mock Verification Rules ==========
//

/// Rule: All observations must succeed
pub struct AllObservationsSuccessRule;

impl VerificationRule for AllObservationsSuccessRule {
    fn name(&self) -> &'static str {
        "all_observations_success"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::High
    }

    fn verify(
        &self,
        _plan: &Plan,
        observations: &[Observation],
        _context: &ContextSnapshot,
    ) -> VerificationCheckResult {
        let success_count = observations
            .iter()
            .filter(|obs| {
                obs.status == crate::models::ExecutionStatus::Success
            })
            .count();

        let all_success = success_count == observations.len();

        VerificationCheckResult {
            passed: all_success,
            details: format!(
                "Success observations: {}/{}",
                success_count,
                observations.len()
            ),
        }
    }
}

/// Rule: Portfolio risk constraints
pub struct PortfolioRiskRule;

impl VerificationRule for PortfolioRiskRule {
    fn name(&self) -> &'static str {
        "portfolio_risk_constraint"
    }

    fn risk_level(&self) -> RiskLevel {
        RiskLevel::Medium
    }

    fn verify(
        &self,
        _plan: &Plan,
        _observations: &[Observation],
        context: &ContextSnapshot,
    ) -> VerificationCheckResult {
        let has_portfolio = context.portfolio_state.is_some();

        VerificationCheckResult {
            passed: true,
            details: if has_portfolio {
                "Portfolio state verified".to_string()
            } else {
                "No portfolio state available (acceptable for planning phase)"
                    .to_string()
            },
        }
    }
}

/// Create a default verification engine with standard rules
pub fn create_default_verification_engine() -> VerificationEngine {
    let mut engine = VerificationEngine::new();
    engine.add_rule(Box::new(AllObservationsSuccessRule));
    engine.add_rule(Box::new(PortfolioRiskRule));
    engine
}

//
// ================= Tests =================
//

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ExecutionStatus;
    use uuid::Uuid;

    #[test]
    fn test_verification() {
        let engine = create_default_verification_engine();

        let plan = Plan {
            plan_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            influencer_id: None,
            goal_id: Uuid::new_v4(),
            steps: vec![],
            success_criteria: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            replans_count: 0,
            failure_reason: None,
        };

        let observations = vec![Observation {
            observation_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            plan_id: plan.plan_id,
            step_id: Uuid::new_v4(),
            tool_name: "test".to_string(),
            tool_input: serde_json::json!({}),
            tool_output: serde_json::json!({}),
            execution_time_ms: 100,
            created_at: Utc::now(),
            status: ExecutionStatus::Success,
        }];

        let context = ContextSnapshot {
            snapshot_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            plan_id: plan.plan_id,
            observations: vec![],
            portfolio_state: None,
            created_at: Utc::now(),
            context_hash: "hash".to_string(),
        };

        let result = engine.verify(&plan, &observations, &context);
        assert!(result.is_ok());
        assert!(result.unwrap().verified);
    }
}
