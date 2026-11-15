//! AI Orchestration state machine, hybrid memory injection, policy-enforced tool loop,
//! and interactive user confirmation management.

#![forbid(unsafe_code)]

pub mod confirmation;
pub mod error;
pub mod interpreter;
pub mod orchestrator;
pub mod state;
pub mod tool_dispatcher;
pub mod turn_machine;

pub use confirmation::{ConfirmationId, ConfirmationManager, PendingConfirmation};
pub use error::{OrchestratorError, Result};
pub use interpreter::{
    InterpreterSession, InterpreterSessionConfig, InterpreterSpeaker, InterpreterTurn,
};
pub use orchestrator::{Orchestrator, OrchestratorConfig, TurnExecution, TurnSummary};
pub use state::{InputMode, OrchestratorEvent, TurnInput, TurnState};
pub use tool_dispatcher::{redact_sensitive_json, ToolDispatcher, ToolHandler};
pub use turn_machine::{TurnMetrics, TurnStateMachine};
