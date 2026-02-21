//! Financial Agent Orchestrator
//! 
//! A production-grade financial agent that:
//! - Supports conversational + goal-driven interactions
//! - Decomposes complex tasks into structured subtasks
//! - Uses deterministic finance engines (LLM excluded from execution)
//! - Persists portfolio and execution state
//! - Enforces compliance rules before output
//! - Is fully auditable and replayable
//!
//! UNIFIED LOOP:
//! INPUT → PLAN → EXECUTE → OBSERVE → VERIFY → REPLAN? → COMPLETE

pub mod agent;
pub mod api;
pub mod audit;
pub mod classifier;
pub mod conversational;
pub mod error;
pub mod execution;
pub mod gemini;
pub mod memory;
pub mod models;
pub mod planner;
pub mod state;
pub mod tools;
pub mod verification;

pub use error::Result;

// Re-export common types
pub use models::*;
pub use classifier::{InteractionClassifier, InteractionType};
