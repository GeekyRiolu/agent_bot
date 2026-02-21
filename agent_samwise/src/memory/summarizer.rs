//! Context Summarization
//!
//! Uses Gemini API to intelligently summarize old conversations
//! when context window gets too large

use crate::memory::store::{ConversationMessage, MessageRole};
use crate::gemini;
use std::env;
use tracing::{info, warn};

/// Summarizes conversation context using Gemini API
pub struct ContextSummarizer;

impl ContextSummarizer {
    /// Summarize a set of messages into a concise summary
    pub async fn summarize_messages(
        messages: Vec<ConversationMessage>,
    ) -> crate::Result<ConversationMessage> {
        if messages.is_empty() {
            return Err(crate::error::OrchestrationError::PlanningError(
                "Cannot summarize empty message list".to_string(),
            ));
        }

        // Build conversation text
        let conversation_text = Self::format_messages_for_summary(&messages);

        // Get Gemini API key
        let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| String::new());

        if api_key.is_empty() {
            warn!("Gemini API key not configured, cannot summarize context");
            return Err(crate::error::OrchestrationError::PlanningError(
                "API key not configured for context summarization".to_string(),
            ));
        }

        // Call Gemini with summarization prompt
        let prompt = format!(
            r#"You are an expert financial conversation summarizer. 
            
Your task is to create a concise, informative summary of the following conversation.
Focus on:
1. Key financial concepts discussed
2. Important questions and answers
3. Key insights or conclusions
4. Any decisions or recommendations made

Keep the summary structured, professional, and about 20-30% of the original length.
Format as bullet points for clarity.

CONVERSATION:
---
{}
---

SUMMARY (focus on key insights and decisions):"#,
            conversation_text
        );

        info!("Calling Gemini API to summarize {} messages", messages.len());

        match gemini::call_gemini_api(&prompt, &api_key).await {
            Ok((summary_text, _confidence)) => {
                info!(
                    "Successfully summarized {} messages into summary",
                    messages.len()
                );
                Ok(ConversationMessage::summary(summary_text))
            }
            Err(e) => {
                warn!("Failed to summarize context: {}", e);
                Err(e)
            }
        }
    }

    /// Format messages into readable text for summarization
    fn format_messages_for_summary(messages: &[ConversationMessage]) -> String {
        let mut text = String::new();

        for msg in messages {
            let role_str = match msg.role {
                MessageRole::User => "User",
                MessageRole::Agent => "Agent",
                MessageRole::System => "System",
            };

            text.push_str(&format!("{}: {}\n", role_str, msg.content));
        }

        text
    }

    /// Create a summary of conversation focused on financial context
    pub async fn create_financial_context_summary(
        messages: Vec<ConversationMessage>,
    ) -> crate::Result<String> {
        if messages.is_empty() {
            return Ok("No previous financial context".to_string());
        }

        let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| String::new());

        if api_key.is_empty() {
            warn!("Cannot create financial context summary without API key");
            return Err(crate::error::OrchestrationError::PlanningError(
                "API key not configured".to_string(),
            ));
        }

        let conversation_text = Self::format_messages_for_summary(&messages);

        let prompt = format!(
            r#"Create a financial context summary from this conversation.
            
Extract and summarize:
1. Financial goals mentioned
2. Portfolio information or preferences
3. Risk tolerance or constraints
4. Technical analysis concepts discussed
5. Investment strategies or recommendations
6. Market data or specific securities mentioned

Format as a structured summary with sections. Keep it concise and actionable.

CONVERSATION:
---
{}
---

FINANCIAL CONTEXT SUMMARY:"#,
            conversation_text
        );

        match gemini::call_gemini_api(&prompt, &api_key).await {
            Ok((summary, _)) => {
                info!("Created financial context summary");
                Ok(summary)
            }
            Err(e) => {
                warn!("Failed to create context summary: {}", e);
                Err(e)
            }
        }
    }
}

/// Helper to estimate if a message is important enough to preserve
pub fn is_message_important(msg: &ConversationMessage) -> bool {
    // Important messages are:
    // 1. System messages
    // 2. Messages with specific types (goal, decision, summary)
    // 3. Very long messages (likely containing important info)

    if msg.role == MessageRole::System {
        return true;
    }

    if let Some(msg_type) = &msg.message_type {
        matches!(msg_type.as_str(), "goal" | "decision" | "summary" | "plan")
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_messages_for_summary() {
        let messages = vec![
            ConversationMessage::new(
                MessageRole::User,
                "What is RSI?".to_string(),
                Some("query".to_string()),
            ),
            ConversationMessage::new(
                MessageRole::Agent,
                "RSI is a momentum indicator...".to_string(),
                Some("answer".to_string()),
            ),
        ];

        let formatted = ContextSummarizer::format_messages_for_summary(&messages);
        assert!(formatted.contains("User:"));
        assert!(formatted.contains("Agent:"));
        assert!(formatted.contains("RSI"));
    }

    #[test]
    fn test_is_message_important() {
        let system_msg = ConversationMessage::new(
            MessageRole::System,
            "Important notice".to_string(),
            None,
        );
        assert!(is_message_important(&system_msg));

        let goal_msg = ConversationMessage::new(
            MessageRole::User,
            "Build a portfolio".to_string(),
            Some("goal".to_string()),
        );
        assert!(is_message_important(&goal_msg));

        let trivial_msg = ConversationMessage::new(
            MessageRole::User,
            "Hello".to_string(),
            None,
        );
        assert!(!is_message_important(&trivial_msg));
    }
}
