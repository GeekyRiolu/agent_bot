//! Conversation history storage
//!
//! Stores and manages conversation messages with timestamps and metadata

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

/// Role of a message sender
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Agent,
    System,
}

/// A single message in the conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub message_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub role: MessageRole,
    pub content: String,
    /// Approximate token count for context window management
    pub token_count: usize,
    /// If this message is part of a summary
    pub is_summary: bool,
    /// Optional metadata about message type (query, answer, goal, etc.)
    pub message_type: Option<String>,
}

impl ConversationMessage {
    /// Create a new conversation message
    pub fn new(
        role: MessageRole,
        content: String,
        message_type: Option<String>,
    ) -> Self {
        let token_count = (content.len() + 3) / 4;

        Self {
            message_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            role,
            content,
            token_count,
            is_summary: false,
            message_type,
        }
    }

    /// Create a summary message
    pub fn summary(summary_content: String) -> Self {
        let token_count = (summary_content.len() + 3) / 4;

        Self {
            message_id: Uuid::new_v4(),
            timestamp: Utc::now(),
            role: MessageRole::System,
            content: summary_content,
            token_count,
            is_summary: true,
            message_type: Some("summary".to_string()),
        }
    }
}

/// Conversation history for a user session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationHistory {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Messages in conversation (VecDeque for efficient queue operations)
    messages: VecDeque<ConversationMessage>,
    /// Total token count (approximate)
    total_tokens: usize,
}

impl ConversationHistory {
    /// Create a new conversation history
    pub fn new(user_id: Uuid, tenant_id: Uuid) -> Self {
        Self {
            user_id,
            tenant_id,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            messages: VecDeque::new(),
            total_tokens: 0,
        }
    }

    /// Add a message to history
    pub fn add_message(&mut self, message: ConversationMessage) {
        self.total_tokens += message.token_count;
        self.messages.push_back(message);
        self.updated_at = Utc::now();
    }

    /// Add an existing message loaded from persistent storage.
    pub fn add_existing_message(&mut self, message: ConversationMessage) {
        self.total_tokens += message.token_count;
        self.messages.push_back(message);
        self.updated_at = Utc::now();
    }

    // =============================
    // Iterators (ZERO ALLOCATION)
    // =============================

    /// Iterate over all messages
    pub fn messages(&self) -> impl Iterator<Item = &ConversationMessage> {
        self.messages.iter()
    }

    /// Iterate over recent messages (N most recent)
    pub fn recent_messages(
        &self,
        count: usize,
    ) -> impl DoubleEndedIterator<Item = &ConversationMessage> {
        self.messages.iter().rev().take(count)
    }


    /// Get total token count
    pub fn total_tokens(&self) -> usize {
        self.total_tokens
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Recompute token count (prevents drift)
    fn recompute_total_tokens(&mut self) {
        self.total_tokens = self.messages.iter().map(|m| m.token_count).sum();
    }

    /// Get formatted conversation for context (useful for LLM prompts)
    pub fn get_formatted_context(&self) -> String {
        let mut context = String::new();
        context.push_str("## Conversation History\n\n");

        for msg in &self.messages {
            let role_str = match msg.role {
                MessageRole::User => "**User**",
                MessageRole::Agent => "**Agent**",
                MessageRole::System => "**System**",
            };

            if msg.is_summary {
                context.push_str("### [Summary]\n");
            }

            context.push_str(&format!(
                "{}: {} ({})\n\n",
                role_str,
                msg.content,
                msg.timestamp.format("%H:%M:%S")
            ));
        }

        context
    }

    /// Remove oldest message and return it
    pub fn remove_oldest(&mut self) -> Option<ConversationMessage> {
        let msg = self.messages.pop_front();

        if let Some(ref m) = msg {
            self.total_tokens = self.total_tokens.saturating_sub(m.token_count);
        }

        if msg.is_some() {
            self.updated_at = Utc::now();
        }

        msg
    }

    /// Clear all non-summary messages except last N
    pub fn trim_to_recent(&mut self, keep_count: usize) {
        let summary_count = self.messages.iter().filter(|m| m.is_summary).count();
        let target_count = keep_count + summary_count;

        while self.messages.len() > target_count {
            self.messages.pop_front();
        }

        self.recompute_total_tokens();
        self.updated_at = Utc::now();
    }

    /// Get messages that should be archived/summarized
    pub fn get_messages_to_summarize(&self, until_index: usize) -> Vec<ConversationMessage> {
        self.messages
            .iter()
            .take(until_index)
            .filter(|m| !m.is_summary)
            .cloned()
            .collect()
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.messages.clear();
        self.total_tokens = 0;
        self.updated_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_message_creation() {
        let msg = ConversationMessage::new(
            MessageRole::User,
            "What is the S&P 500?".to_string(),
            Some("query".to_string()),
        );
        assert_eq!(msg.role, MessageRole::User);
        assert!(!msg.is_summary);
        assert!(msg.token_count > 0);
    }

    #[test]
    fn test_conversation_history() {
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let mut history = ConversationHistory::new(user_id, tenant_id);

        let msg1 = ConversationMessage::new(
            MessageRole::User,
            "What is RSI?".to_string(),
            Some("query".to_string()),
        );
        let msg2 = ConversationMessage::new(
            MessageRole::Agent,
            "RSI is a momentum oscillator...".to_string(),
            Some("answer".to_string()),
        );

        history.add_message(msg1);
        history.add_message(msg2);

        assert_eq!(history.message_count(), 2);
        assert!(history.total_tokens() > 0);
    }

    #[test]
    fn test_trim_to_recent() {
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let mut history = ConversationHistory::new(user_id, tenant_id);

        for i in 0..10 {
            let msg = ConversationMessage::new(
                MessageRole::User,
                format!("Question {}", i),
                Some("query".to_string()),
            );
            history.add_message(msg);
        }

        history.trim_to_recent(5);
        assert_eq!(history.message_count(), 5);
    }

    #[test]
    fn test_summary_message() {
        let summary = ConversationMessage::summary(
            "Previous conversation covered RSI and MACD indicators".to_string(),
        );
        assert!(summary.is_summary);
        assert_eq!(summary.role, MessageRole::System);
    }
}
