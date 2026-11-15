//! AI Orchestration state machine, hybrid memory injection, and LLM/TTS tool loop.

#![forbid(unsafe_code)]

pub mod error;
pub mod orchestrator;
pub mod state;
pub mod turn_machine;

pub use error::{OrchestratorError, Result};
pub use orchestrator::{Orchestrator, OrchestratorConfig, TurnSummary};
pub use state::{InputMode, OrchestratorEvent, TurnInput, TurnState};
pub use turn_machine::{TurnMetrics, TurnStateMachine};
