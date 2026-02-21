//! Context Window Management
//!
//! Manages context window size, triggers summarization when needed,
//! and maintains optimal context for LLM interactions

use crate::memory::store::{ConversationHistory, ConversationMessage};
use tracing::info;

/// Configuration for context window management
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Maximum tokens before triggering summarization
    pub max_context_tokens: usize,
    /// Threshold percentage to trigger summarization (e.g., 80%)
    pub summarization_threshold: f32,
    /// Minimum messages to keep before summarization
    pub min_messages_to_keep: usize,
    /// Number of recent messages to always preserve
    pub preserve_recent_count: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 100_000,
            summarization_threshold: 0.8,
            min_messages_to_keep: 5,
            preserve_recent_count: 10,
        }
    }
}

/// Manages context window and triggers summarization
pub struct ContextManager {
    config: ContextConfig,
}

impl ContextManager {
    pub fn new() -> Self {
        Self {
            config: ContextConfig::default(),
        }
    }

    pub fn with_config(config: ContextConfig) -> Self {
        Self { config }
    }

    /// Check if context window is getting full
    pub fn should_summarize(&self, history: &ConversationHistory) -> bool {
        let current_tokens = history.total_tokens();
        let threshold_tokens =
            (self.config.max_context_tokens as f32 * self.config.summarization_threshold) as usize;

        let should = current_tokens >= threshold_tokens
            && history.message_count() > self.config.min_messages_to_keep;

        if should {
            info!(
                "Context window at {}/{} tokens (threshold: {}). Summarization needed.",
                current_tokens,
                self.config.max_context_tokens,
                threshold_tokens
            );
        }

        should
    }

    /// Get percentage of context window used
    pub fn get_context_usage_percent(&self, history: &ConversationHistory) -> f32 {
        (history.total_tokens() as f32 / self.config.max_context_tokens as f32) * 100.0
    }

    /// Prepare messages for LLM context (includes summaries + recent messages)
    ///
    /// Optimized: avoids HashSet allocation
    pub fn prepare_context_for_llm<'a>(
        &self,
        history: &'a ConversationHistory,
    ) -> Vec<&'a ConversationMessage> {
        let mut context = Vec::with_capacity(self.config.preserve_recent_count + 4);

        // 1️⃣ Include summaries first
        for msg in history.messages() {
            if msg.is_summary {
                context.push(msg);
            }
        }

        // 2️⃣ Include recent messages if not already present
        for msg in history.recent_messages(self.config.preserve_recent_count) {
            let exists = context.iter().any(|m| m.message_id == msg.message_id);
            if !exists {
                context.push(msg);
            }
        }

        context
    }

    /// Get messages that should be archived/summarized
    pub fn get_messages_to_archive(
        &self,
        history: &ConversationHistory,
    ) -> Vec<ConversationMessage> {
        let recent_count = self.config.preserve_recent_count;
        let total = history.message_count();

        if total <= recent_count {
            return Vec::new();
        }

        let archive_until = total - recent_count;
        history.get_messages_to_summarize(archive_until)
    }

    /// Estimate if summarization is needed
    pub fn estimate_need_for_summary(
        &self,
        history: &ConversationHistory,
    ) -> SummarizationNeeds {
        let current_tokens = history.total_tokens();
        let percent_used = self.get_context_usage_percent(history);
        let messages_to_archive = self.get_messages_to_archive(history);

        SummarizationNeeds {
            should_summarize: self.should_summarize(history),
            percent_used,
            current_tokens,
            messages_to_archive_count: messages_to_archive.len(),
            tokens_to_save_estimate: messages_to_archive
                .iter()
                .map(|m| m.token_count)
                .sum(),
        }
    }

    pub fn config(&self) -> &ContextConfig {
        &self.config
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Information about summarization needs
#[derive(Debug, Clone)]
pub struct SummarizationNeeds {
    pub should_summarize: bool,
    pub percent_used: f32,
    pub current_tokens: usize,
    pub messages_to_archive_count: usize,
    pub tokens_to_save_estimate: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::store::{ConversationMessage, MessageRole};
    use uuid::Uuid;

    #[test]
    fn test_context_manager_creation() {
        let manager = ContextManager::new();
        assert_eq!(manager.config().max_context_tokens, 100_000);
    }

    #[test]
    fn test_should_summarize() {
        let config = ContextConfig {
            max_context_tokens: 1000,
            summarization_threshold: 0.8,
            min_messages_to_keep: 2,
            preserve_recent_count: 3,
        };

        let manager = ContextManager::with_config(config);
        let mut history = ConversationHistory::new(Uuid::new_v4(), Uuid::new_v4());

        for i in 0..50 {
            let msg = ConversationMessage::new(
                MessageRole::User,
                format!("Question {}. ", i).repeat(20),
                Some("query".to_string()),
            );
            history.add_message(msg);
        }

        let should_summarize = manager.should_summarize(&history);
        assert!(should_summarize);
    }

    #[test]
    fn test_prepare_context_for_llm() {
        let manager = ContextManager::new();
        let mut history = ConversationHistory::new(Uuid::new_v4(), Uuid::new_v4());

        for i in 0..15 {
            let msg = ConversationMessage::new(
                MessageRole::User,
                format!("Question {}", i),
                Some("query".to_string()),
            );
            history.add_message(msg);
        }

        let context = manager.prepare_context_for_llm(&history);
        assert!(!context.is_empty());
        assert!(context.len() <= 15);
    }
}
