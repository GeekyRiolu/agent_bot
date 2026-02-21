//! Interaction Classifier
//! 
//! Classifies user inputs as either:
//! - Conversational: Quick LLM responses (e.g., "what is RSI?", "how is Reliance performing?")
//! - Goal-Driven: Multi-step tasks requiring orchestration (e.g., "build a portfolio", "find profitable stocks")

use crate::models::{Goal, GoalContext, RiskTolerance, TimeHorizon};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionType {
    Conversational,
    GoalDriven,
}

/// Static keyword lists — zero allocation
const GOAL_KEYWORDS: &[&str] = &[
    // Portfolio tasks
    "portfolio", "rebalance", "allocate", "diversify",
    // Search/screening
    "find", "screen", "search", "identify", "discover",
    // Analysis tasks
    "analyze", "compare", "backtest", "simulate", "optimize",
    // Action tasks
    "build", "create", "construct", "suggest", "recommend", "generate",
    // Monitoring
    "monitor", "track", "watch",
    // Time-based
    "month", "week", "day", "year", "period", "long-term", "short-term",
    // Quantitative
    "return", "gain", "profit", "risk", "ratio", "percent", "%",
];

const CONVERSATIONAL_KEYWORDS: &[&str] = &[
    // Questions
    "what", "how", "explain", "tell me", "what is", "what are",
    // Info seeking
    "is", "are", "definition", "meaning", "concept",
    // Quick checks
    "performing", "price", "status", "current", "today", "now",
    // Indicators
    "rsi", "ma", "ema", "macd", "bollinger", "stochastic",
    // Stocks
    "reliance", "tcs", "infy", "wipro", "bajaj", "hdfc",
];

/// Interaction classifier
pub struct InteractionClassifier;

impl InteractionClassifier {
    /// Classify user input as conversational or goal-driven
    pub fn classify(goal: &Goal) -> InteractionType {
        let description = goal.description.to_lowercase();

        let goal_score = GOAL_KEYWORDS
            .iter()
            .filter(|kw| description.contains(**kw))
            .count();

        let conversational_score = CONVERSATIONAL_KEYWORDS
            .iter()
            .filter(|kw| description.contains(**kw))
            .count();

        // Heuristics
        if goal_score >= 2
            || (goal_score > 0 && description.len() > 50)
            || contains_action_verbs(&description)
        {
            InteractionType::GoalDriven
        } else if conversational_score >= 1
            || (description.len() < 30
                && !GOAL_KEYWORDS.iter().any(|kw| description.contains(*kw)))
        {
            InteractionType::Conversational
        } else {
            InteractionType::Conversational
        }
    }
}

/// Fast path action verb detection
fn contains_action_verbs(text: &str) -> bool {
    text.contains("build")
        || text.contains("create")
        || text.contains("generate")
        || text.contains("recommend")
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use chrono::Utc;

    fn create_test_goal(description: &str) -> Goal {
        Goal {
            goal_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            influencer_id: None,
            description: description.to_string(),
            created_at: Utc::now(),
            context: GoalContext {
                current_portfolio: None,
                constraints: vec![],
                risk_tolerance: RiskTolerance::Medium,
                time_horizon: TimeHorizon::LongTerm,
            },
        }
    }

    #[test]
    fn test_conversational_questions() {
        let cases = vec![
            "what is RSI?",
            "how is Reliance performing?",
            "explain moving average",
            "what does MACD mean?",
        ];

        for c in cases {
            let goal = create_test_goal(c);
            assert_eq!(
                InteractionClassifier::classify(&goal),
                InteractionType::Conversational
            );
        }
    }

    #[test]
    fn test_goal_driven_tasks() {
        let cases = vec![
            "build a 6-month portfolio",
            "find profitable stocks",
            "create a diversified portfolio",
            "recommend stocks for long-term investment",
        ];

        for c in cases {
            let goal = create_test_goal(c);
            assert_eq!(
                InteractionClassifier::classify(&goal),
                InteractionType::GoalDriven
            );
        }
    }

    #[test]
    fn test_edge_cases() {
        let short_goal = create_test_goal("hi");
        assert_eq!(
            InteractionClassifier::classify(&short_goal),
            InteractionType::Conversational
        );

        let unclear_goal = create_test_goal("stock market");
        assert_eq!(
            InteractionClassifier::classify(&unclear_goal),
            InteractionType::Conversational
        );
    }
}
