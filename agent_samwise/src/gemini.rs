//! Gemini API client for conversational mode
//!
//! Provides direct LLM integration for Q&A style queries
//! Uses a long-lived reqwest::Client for connection pooling.

use serde::{Deserialize, Serialize};
use tracing::{info, error};
use reqwest::Client;
use std::time::Duration;
use crate::error::OrchestrationError;

/// Reusable Gemini client (connection-pooled)
pub struct GeminiClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl GeminiClient {
    pub fn new(api_key: String) -> Self {
        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(8)
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            api_key,
            base_url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent".to_string(),
        }
    }

    /// Generate response from Gemini
    pub async fn generate(
        &self,
        query: &str,
        tools: Option<Vec<String>>,
    ) -> crate::Result<(String, f32)> {

        if self.api_key.is_empty() {
            return Err(crate::error::OrchestrationError::PlanningError(
                "GEMINI_API_KEY not configured".to_string(),
            ));
        }

        let url = format!("{}?key={}", self.base_url, self.api_key);

        let system_prompt = build_system_prompt(tools);

        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: query.to_string(),
                }],
            }],
            generation_config: GenerationConfig {
                temperature: 0.3,
                top_p: 0.9,
                top_k: 40,
                max_output_tokens: 1024,
            },
            system_instruction: SystemInstruction {
                parts: vec![Part {
                    text: system_prompt,
                }],
            },
        };

        info!("Calling Gemini API");

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!("Gemini API request failed: {}", e);
                crate::error::OrchestrationError::PlanningError(
                    format!("Gemini API error: {}", e)
                )
            })?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Gemini API error response: {}", error_text);
            return Err(crate::error::OrchestrationError::PlanningError(
                format!("Gemini API error: {}", error_text)
            ));
        }

        let gemini_response: GeminiResponse = response.json().await.map_err(|e| {
            error!("Failed to parse Gemini response: {}", e);
            OrchestrationError::LlmError(format!("Gemini parse error: {}", e))
        })?;


        if gemini_response.candidates.is_empty() {
            return Err(crate::error::OrchestrationError::PlanningError(
                "No response from Gemini API".to_string(),
            ));
        }

        let answer = gemini_response.candidates[0]
            .content
            .parts
            .first()
            .ok_or_else(|| {
                crate::error::OrchestrationError::PlanningError(
                    "Empty response from Gemini".to_string()
                )
            })?
            .text
            .clone();

        let confidence = calculate_confidence(&gemini_response);

        info!("Gemini response received (confidence: {})", confidence);

        Ok((answer, confidence))
    }
}

/// Backward-compatible wrapper (optional)
pub async fn call_gemini_api(
    query: &str,
    api_key: &str,
) -> crate::Result<(String, f32)> {
    let client = GeminiClient::new(api_key.to_string());
    client.generate(query, None).await
}

/// Build system prompt with optional tool descriptions
fn build_system_prompt(tools: Option<Vec<String>>) -> String {
    let base_prompt = r#"You are a professional financial advisor and analyst.

Guidelines:
- Provide accurate and educational financial information
- Be structured and concise
- Explain technical indicators mathematically when relevant
- Emphasize research and risk awareness
- Use professional financial language

Format: Provide structured answers suitable for financial decision-making."#;

    if let Some(tool_list) = tools {
        format!(
            "{}\n\nAvailable tools:\n- {}",
            base_prompt,
            tool_list.join("\n- ")
        )
    } else {
        base_prompt.to_string()
    }
}

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    generation_config: GenerationConfig,
    system_instruction: SystemInstruction,
}

#[derive(Debug, Serialize, Deserialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    top_p: f32,
    top_k: i32,
    max_output_tokens: i32,
}

#[derive(Debug, Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Content,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsageMetadata {
    prompt_token_count: i32,
    candidates_token_count: i32,
}

/// Calculate response confidence
fn calculate_confidence(response: &GeminiResponse) -> f32 {
    let base_confidence: f32 = 0.85;

    let finish_confidence = match response.candidates[0].finish_reason.as_deref() {
        Some("STOP") => 1.0,
        Some("LENGTH") => 0.8,
        Some("SAFETY") => 0.6,
        _ => 0.7,
    };

    let response_length = response.candidates[0]
        .content
        .parts
        .first()
        .map(|p| p.text.len())
        .unwrap_or(0);

    let length_confidence = if response_length < 50 {
        0.6
    } else if response_length > 2000 {
        0.8
    } else {
        1.0
    };

    (base_confidence * finish_confidence * length_confidence)
        .min(0.98)
        .max(0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: "What is RSI?".to_string(),
                }],
            }],
            generation_config: GenerationConfig {
                temperature: 0.3,
                top_p: 0.9,
                top_k: 40,
                max_output_tokens: 1024,
            },
            system_instruction: SystemInstruction {
                parts: vec![Part {
                    text: "You are a financial advisor".to_string(),
                }],
            },
        };

        let json = serde_json::to_string(&request);
        assert!(json.is_ok());
        assert!(json.unwrap().contains("What is RSI?"));
    }
}
