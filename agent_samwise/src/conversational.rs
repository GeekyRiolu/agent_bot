//! Conversational interaction handler
//! 
//! Handles quick Q&A style interactions that don't require orchestration
//! Routes directly to Gemini API for fast LLM responses
//! Integrates with memory system to remember context across conversations

use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use crate::models::Goal;
use crate::gemini;
use crate::memory::{ConversationHistory, ConversationMessage, MessageRole, ContextManager, ContextSummarizer};
use std::env;
use std::sync::Arc;
use tokio::sync::{OnceCell, RwLock};
use std::collections::HashMap;
use uuid::Uuid;
use std::sync::OnceLock;
use sqlx::{PgPool, Row};

/// Response for conversational interactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationalResponse {
    pub answer: String,
    pub source: String,
    pub confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_used: Option<String>,
}

enum MemoryBackend {
    InMemory {
        histories: Arc<RwLock<HashMap<(Uuid, Uuid), ConversationHistory>>>,
    },
    Postgres {
        pool: PgPool,
        schema_ready: Arc<OnceCell<()>>,
    },
}

/// Global memory store for conversations.
pub struct ConversationMemory {
    backend: MemoryBackend,
    context_manager: ContextManager,
}

impl ConversationMemory {
    pub fn new() -> Self {
        let backend = build_backend();

        Self {
            backend,
            context_manager: ContextManager::new(),
        }
    }

    async fn ensure_schema_if_needed(&self) -> crate::Result<()> {
        let MemoryBackend::Postgres {
            pool,
            schema_ready,
        } = &self.backend
        else {
            return Ok(());
        };

        schema_ready
            .get_or_try_init(|| async {
                sqlx::query(
                    r#"
                    CREATE TABLE IF NOT EXISTS conversation_messages (
                      message_id UUID PRIMARY KEY,
                      user_id UUID NOT NULL,
                      tenant_id UUID NOT NULL,
                      conversation_id UUID NOT NULL,
                      role TEXT NOT NULL,
                      content TEXT NOT NULL,
                      token_count INTEGER NOT NULL,
                      is_summary BOOLEAN NOT NULL DEFAULT FALSE,
                      message_type TEXT,
                      created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                    );
                    "#,
                )
                .execute(pool)
                .await?;

                sqlx::query(
                    r#"
                    CREATE INDEX IF NOT EXISTS idx_conversation_messages_scope_time
                    ON conversation_messages (user_id, conversation_id, created_at);
                    "#,
                )
                .execute(pool)
                .await?;

                Ok::<(), sqlx::Error>(())
            })
            .await
            .map_err(|e| {
                crate::error::OrchestrationError::DatabaseError(format!(
                    "Failed to initialize conversation memory schema: {}",
                    e
                ))
            })?;

        Ok(())
    }

    fn role_to_db(role: MessageRole) -> &'static str {
        match role {
            MessageRole::User => "user",
            MessageRole::Agent => "agent",
            MessageRole::System => "system",
        }
    }

    fn role_from_db(role: &str) -> MessageRole {
        match role.to_lowercase().as_str() {
            "user" => MessageRole::User,
            "agent" => MessageRole::Agent,
            "system" => MessageRole::System,
            _ => MessageRole::User,
        }
    }

    async fn get_or_create_history(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        conversation_id: Uuid,
    ) -> crate::Result<ConversationHistory> {
        match &self.backend {
            MemoryBackend::InMemory { histories } => {
                let key = (user_id, conversation_id);

                {
                    let locked = histories.read().await;
                    if let Some(history) = locked.get(&key) {
                        return Ok(history.clone());
                    }
                }

                let mut locked = histories.write().await;
                let history = locked
                    .entry(key)
                    .or_insert_with(|| ConversationHistory::new(user_id, tenant_id))
                    .clone();

                Ok(history)
            }
            MemoryBackend::Postgres { pool, .. } => {
                self.ensure_schema_if_needed().await?;

                let rows = sqlx::query(
                    r#"
                    SELECT message_id, role, content, token_count, is_summary, message_type, created_at
                    FROM conversation_messages
                    WHERE user_id = $1 AND conversation_id = $2
                    ORDER BY created_at ASC
                    "#,
                )
                .bind(user_id)
                .bind(conversation_id)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    crate::error::OrchestrationError::DatabaseError(format!(
                        "Failed to load conversation history: {}",
                        e
                    ))
                })?;

                let mut history = ConversationHistory::new(user_id, tenant_id);

                for row in rows {
                    let db_role: String = row.try_get("role").unwrap_or_else(|_| "user".to_string());
                    let token_count: i32 = row.try_get("token_count").unwrap_or(0);

                    let message = ConversationMessage {
                        message_id: row.try_get("message_id").unwrap_or_else(|_| Uuid::new_v4()),
                        timestamp: row
                            .try_get("created_at")
                            .unwrap_or_else(|_| chrono::Utc::now()),
                        role: Self::role_from_db(&db_role),
                        content: row.try_get("content").unwrap_or_default(),
                        token_count: token_count.max(0) as usize,
                        is_summary: row.try_get("is_summary").unwrap_or(false),
                        message_type: row.try_get("message_type").ok(),
                    };

                    history.add_existing_message(message);
                }

                Ok(history)
            }
        }
    }

    async fn save_history(
        &self,
        user_id: Uuid,
        conversation_id: Uuid,
        tenant_id: Uuid,
        history: &ConversationHistory,
    ) -> crate::Result<()> {
        match &self.backend {
            MemoryBackend::InMemory { histories } => {
                let key = (user_id, conversation_id);
                let mut locked = histories.write().await;
                locked.insert(key, history.clone());
                Ok(())
            }
            MemoryBackend::Postgres { pool, .. } => {
                self.ensure_schema_if_needed().await?;

                let mut tx = pool.begin().await.map_err(|e| {
                    crate::error::OrchestrationError::DatabaseError(format!(
                        "Failed to begin transaction for saving conversation history: {}",
                        e
                    ))
                })?;

                sqlx::query(
                    "DELETE FROM conversation_messages WHERE user_id = $1 AND conversation_id = $2",
                )
                .bind(user_id)
                .bind(conversation_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| {
                    crate::error::OrchestrationError::DatabaseError(format!(
                        "Failed to clear old conversation history: {}",
                        e
                    ))
                })?;

                for msg in history.messages() {
                    sqlx::query(
                        r#"
                        INSERT INTO conversation_messages
                          (message_id, user_id, tenant_id, conversation_id, role, content, token_count, is_summary, message_type, created_at)
                        VALUES
                          ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                        "#,
                    )
                    .bind(msg.message_id)
                    .bind(user_id)
                    .bind(tenant_id)
                    .bind(conversation_id)
                    .bind(Self::role_to_db(msg.role))
                    .bind(&msg.content)
                    .bind(msg.token_count as i32)
                    .bind(msg.is_summary)
                    .bind(&msg.message_type)
                    .bind(msg.timestamp)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        crate::error::OrchestrationError::DatabaseError(format!(
                            "Failed to insert conversation message: {}",
                            e
                        ))
                    })?;
                }

                tx.commit().await.map_err(|e| {
                    crate::error::OrchestrationError::DatabaseError(format!(
                        "Failed to commit conversation history transaction: {}",
                        e
                    ))
                })?;

                Ok(())
            }
        }
    }
}

impl Default for ConversationMemory {
    fn default() -> Self {
        Self::new()
    }
}

// Global conversation memory (can be initialized once at startup)
static CONVERSATION_MEMORY: OnceLock<ConversationMemory> = OnceLock::new();

fn get_memory() -> &'static ConversationMemory {
    CONVERSATION_MEMORY.get_or_init(ConversationMemory::new)
}

fn build_backend() -> MemoryBackend {
    let database_url = env::var("POSTGRES_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .ok();

    if let Some(url) = database_url {
        match sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(&url)
        {
            Ok(pool) => {
                info!("Conversation memory backend: postgres");
                return MemoryBackend::Postgres {
                    pool,
                    schema_ready: Arc::new(OnceCell::new()),
                };
            }
            Err(error) => {
                warn!(
                    "Failed to initialize postgres memory backend, falling back to in-memory: {}",
                    error
                );
            }
        }
    }

    info!("Conversation memory backend: in-memory");
    MemoryBackend::InMemory {
        histories: Arc::new(RwLock::new(HashMap::new())),
    }
}

/// Handle conversational queries directly via Gemini API with memory
/// 
/// Makes direct calls to Google's Gemini API without orchestration
/// Remembers conversation history for context awareness
pub async fn handle_conversational_with_memory(goal: &Goal) -> crate::Result<ConversationalResponse> {
    let description = &goal.description;
    let user_id = goal.user_id;
    let tenant_id = goal.tenant_id;
    let conversation_id = goal.influencer_id.unwrap_or(goal.user_id);
    
    // Get or create conversation history
    let memory = get_memory();
    let mut history = match memory
        .get_or_create_history(user_id, tenant_id, conversation_id)
        .await
    {
        Ok(history) => history,
        Err(error) => {
            warn!("Conversation memory load failed, continuing without persisted context: {}", error);
            ConversationHistory::new(user_id, tenant_id)
        }
    };

    // Add user query to history
    let user_message = ConversationMessage::new(
        MessageRole::User,
        description.clone(),
        Some("query".to_string()),
    );
    history.add_message(user_message);

    // Check if we need to summarize old context
    let memory = get_memory();
    let context_manager = &memory.context_manager;
    let summarization_needs = context_manager.estimate_need_for_summary(&history);
    
    if summarization_needs.should_summarize {
        info!(
            "Context at {}% - Summarizing {} old messages to save tokens",
            summarization_needs.percent_used as u32, summarization_needs.messages_to_archive_count
        );

        // Get messages to summarize
        let messages_to_archive = context_manager.get_messages_to_archive(&history);
        
        if !messages_to_archive.is_empty() {
            // Create summary using Gemini
            match ContextSummarizer::summarize_messages(messages_to_archive).await {
                Ok(summary_msg) => {
                    info!("Successfully created context summary");
                    history.add_message(summary_msg);
                    // Trim old messages, keeping only recent ones
                    history.trim_to_recent(context_manager.config().preserve_recent_count);
                }
                Err(e) => {
                    warn!("Failed to summarize context: {}. Continuing without summarization", e);
                }
            }
        }
    }

    // Prepare context for LLM (includes summaries + recent messages)
    let context_messages = context_manager.prepare_context_for_llm(&history);
    let has_context = context_messages.len() > 1;
    let context_message_count = context_messages.len();
    
    let mut enhanced_prompt = String::new();
    if has_context {
        enhanced_prompt.push_str("Based on our conversation history:\n\n");
        for msg in &context_messages[0..context_messages.len().saturating_sub(1)] {
            enhanced_prompt.push_str(&format!(
                "- {}: {}\n",
                if msg.role == MessageRole::User { "User" } else { "Agent" },
                &msg.content
            ));
        }
        enhanced_prompt.push_str("\n---\n\n");
    }
    enhanced_prompt.push_str("Answer this question: ");
    enhanced_prompt.push_str(description);

    // Drop the context_messages borrow before we mutate history
    drop(context_messages);

    let api_key = env::var("GEMINI_API_KEY").unwrap_or_else(|_| String::new());
    
    // Try real API first
    match gemini::call_gemini_api(&enhanced_prompt, &api_key).await {
        Ok((answer, confidence)) => {
            info!("Conversational response from Gemini API (confidence: {})", confidence);
            
            // Add agent response to history
            let agent_message = ConversationMessage::new(
                MessageRole::Agent,
                answer.clone(),
                Some("answer".to_string()),
            );
            history.add_message(agent_message);
            
            // Save updated history
            if let Err(error) = memory
                .save_history(user_id, conversation_id, tenant_id, &history)
                .await
            {
                warn!(
                    "Conversation memory save failed, response will still be returned: {}",
                    error
                );
            }

            Ok(ConversationalResponse {
                answer,
                source: "Gemini API".to_string(),
                confidence,
                context_used: if has_context {
                    Some(format!("{} previous messages included", context_message_count - 1))
                } else {
                    None
                },
            })
        }
        Err(e) => {
            warn!("Gemini API call failed: {}", e);
            
            if api_key.is_empty() || api_key == "your_gemini_api_key_here" {
                Err(crate::error::OrchestrationError::PlanningError(
                    "⚠️ Gemini API key not configured. Please set GEMINI_API_KEY in your .env file. See .env.example for details.".to_string()
                ))
            } else {
                Err(e)
            }
        }
    }
}

/// Handle conversational queries directly via Gemini API (backward compatible, without memory)
/// 
/// Makes direct calls to Google's Gemini API without orchestration
pub async fn handle_conversational(goal: &Goal) -> crate::Result<ConversationalResponse> {
    handle_conversational_with_memory(goal).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use chrono::Utc;
    use crate::models::GoalContext;

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
                risk_tolerance: "medium".to_string(),
                time_horizon: "5 years".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_missing_api_key() {
        // Temporarily clear API key
        env::remove_var("GEMINI_API_KEY");
        
        let goal = create_test_goal("what is RSI?");
        let result = handle_conversational(&goal).await;
        
        // Should return error about missing API key
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.to_lowercase().contains("api key"));
    }
}
