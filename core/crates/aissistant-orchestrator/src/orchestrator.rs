//! Main conversational AI orchestrator coordinating memory retrieval, LLM streaming,
//! policy-enforced tool execution, and user confirmation lifecycle.

use std::sync::{Arc, Mutex, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aissistant_core::{AuditEntryId, ConversationId, SkillId, TurnId};
use aissistant_llm::{
    EmbeddingEngine, FinishReason, LlmEngine, LlmSession, SamplingParams, TokenChunk,
    ToolCallValidator,
};
use aissistant_security::{
    Actor, AdversarialScanner, AuditEntry, AuditExecutionStatus, PolicyContext, PolicyDecision,
    PolicyEngine, TaintTracker,
};
use aissistant_speech::{SentenceChunker, TtsEngine};
use aissistant_storage::{
    AuditStore, ConversationStore, ConversationTurn, HybridSearchEngine, PersonaStore,
    TurnMetadata, TurnRole, VectorMemoryStore, VectorRecord,
};

use crate::confirmation::{ConfirmationId, ConfirmationManager, PendingConfirmation};
use crate::error::{OrchestratorError, Result};
use crate::persona::PersonaPromptInjector;
use crate::state::{OrchestratorEvent, TurnState};
use crate::tool_dispatcher::{redact_sensitive_json, ToolDispatcher};
use crate::turn_machine::TurnStateMachine;

/// Configuration options for orchestrator behavior.
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Maximum number of past episodic/semantic memory excerpts to inject into context.
    pub max_memory_excerpts: usize,
    /// Default system instructions prompt.
    pub system_prompt: String,
    /// Hyperparameters for LLM generation.
    pub sampling_params: SamplingParams,
    /// Whether to synthesize speech chunks using TTS during turn generation.
    pub synthesize_speech: bool,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            max_memory_excerpts: 3,
            system_prompt: "You are an intelligent, helpful, 100% offline personal AI assistant."
                .to_string(),
            sampling_params: SamplingParams::default(),
            synthesize_speech: false,
        }
    }
}

/// Structured summary returned at the completion of an interactive turn.
#[derive(Debug, Clone, PartialEq)]
pub struct TurnSummary {
    /// Unique identifier of the generated assistant turn.
    pub assistant_turn_id: TurnId,
    /// Parent conversation session identifier.
    pub conversation_id: ConversationId,
    /// User input text for the turn.
    pub user_prompt: String,
    /// Generated assistant response text.
    pub assistant_response: String,
    /// Number of memory excerpts injected from episodic/semantic search.
    pub memory_hits_count: usize,
    /// Generated token count.
    pub token_count: usize,
    /// Total turn elapsed latency in milliseconds.
    pub latency_ms: u64,
    /// Time to first token in milliseconds from inference start.
    pub ttft_ms: Option<u64>,
    /// Whether the turn was interrupted by barge-in or user cancellation.
    pub interrupted: bool,
    /// Optional tool output result if a tool was executed.
    pub tool_result: Option<serde_json::Value>,
}

/// Execution outcome of an interactive turn.
#[derive(Debug, Clone, PartialEq)]
pub enum TurnExecution {
    /// Turn completed (either conversational response or completed tool).
    Completed(TurnSummary),
    /// Turn paused requiring explicit user confirmation before executing a privileged tool.
    AwaitingConfirmation(PendingConfirmation),
}

type EventListener = Arc<dyn Fn(&OrchestratorEvent) + Send + Sync>;

/// Central orchestrator coordinating turn state transitions, memory recall,
/// policy-gated tool execution, and user confirmation.
#[derive(Clone)]
pub struct Orchestrator {
    config: OrchestratorConfig,
    state_machine: TurnStateMachine,
    conversation_store: ConversationStore,
    vector_store: VectorMemoryStore,
    hybrid_search: HybridSearchEngine,
    embedding_engine: Arc<dyn EmbeddingEngine>,
    llm_engine: Arc<dyn LlmEngine>,
    tts_engine: Option<Arc<Mutex<dyn TtsEngine>>>,
    tool_dispatcher: Arc<ToolDispatcher>,
    policy_engine: Arc<RwLock<PolicyEngine>>,
    audit_store: Option<Arc<AuditStore>>,
    confirmation_manager: Arc<ConfirmationManager>,
    taint_tracker: Arc<RwLock<TaintTracker>>,
    persona_store: Option<Arc<PersonaStore>>,
    event_listeners: Arc<Mutex<Vec<EventListener>>>,
}

impl Orchestrator {
    /// Creates a new `Orchestrator` instance with the supplied components and configuration.
    #[must_use]
    pub fn new(
        config: OrchestratorConfig,
        conversation_store: ConversationStore,
        vector_store: VectorMemoryStore,
        hybrid_search: HybridSearchEngine,
        embedding_engine: Arc<dyn EmbeddingEngine>,
        llm_engine: Arc<dyn LlmEngine>,
        tts_engine: Option<Arc<Mutex<dyn TtsEngine>>>,
    ) -> Self {
        Self {
            config,
            state_machine: TurnStateMachine::new(),
            conversation_store,
            vector_store,
            hybrid_search,
            embedding_engine,
            llm_engine,
            tts_engine,
            tool_dispatcher: Arc::new(ToolDispatcher::new()),
            policy_engine: Arc::new(RwLock::new(PolicyEngine::new())),
            audit_store: None,
            confirmation_manager: Arc::new(ConfirmationManager::new()),
            taint_tracker: Arc::new(RwLock::new(TaintTracker::new())),
            persona_store: None,
            event_listeners: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Sets the tool dispatcher.
    #[must_use]
    pub fn with_tool_dispatcher(mut self, dispatcher: Arc<ToolDispatcher>) -> Self {
        self.tool_dispatcher = dispatcher;
        self
    }

    /// Sets the policy engine.
    #[must_use]
    pub fn with_policy_engine(mut self, engine: Arc<RwLock<PolicyEngine>>) -> Self {
        self.policy_engine = engine;
        self
    }

    /// Sets the persistent audit log store.
    #[must_use]
    pub fn with_audit_store(mut self, store: Arc<AuditStore>) -> Self {
        self.audit_store = Some(store);
        self
    }

    /// Sets the confirmation manager.
    #[must_use]
    pub fn with_confirmation_manager(mut self, manager: Arc<ConfirmationManager>) -> Self {
        self.confirmation_manager = manager;
        self
    }

    /// Sets the taint tracker.
    #[must_use]
    pub fn with_taint_tracker(mut self, tracker: Arc<RwLock<TaintTracker>>) -> Self {
        self.taint_tracker = tracker;
        self
    }

    /// Sets the persona and preference store.
    #[must_use]
    pub fn with_persona_store(mut self, store: Arc<PersonaStore>) -> Self {
        self.persona_store = Some(store);
        self
    }

    /// Returns a reference to the persona store if configured.
    #[must_use]
    pub fn persona_store(&self) -> Option<&Arc<PersonaStore>> {
        self.persona_store.as_ref()
    }

    /// Returns a reference to the tool dispatcher.
    #[must_use]
    pub fn tool_dispatcher(&self) -> &Arc<ToolDispatcher> {
        &self.tool_dispatcher
    }

    /// Returns a reference to the confirmation manager.
    #[must_use]
    pub fn confirmation_manager(&self) -> &Arc<ConfirmationManager> {
        &self.confirmation_manager
    }

    /// Returns a reference to the policy engine.
    #[must_use]
    pub fn policy_engine(&self) -> &Arc<RwLock<PolicyEngine>> {
        &self.policy_engine
    }

    /// Returns a reference to the audit store if configured.
    #[must_use]
    pub fn audit_store(&self) -> Option<&Arc<AuditStore>> {
        self.audit_store.as_ref()
    }

    /// Returns a reference to the taint tracker.
    #[must_use]
    pub fn taint_tracker(&self) -> &Arc<RwLock<TaintTracker>> {
        &self.taint_tracker
    }

    /// Registers an event listener callback for real-time turn state machine telemetry.
    pub fn on_event<F>(&self, listener: F)
    where
        F: Fn(&OrchestratorEvent) + Send + Sync + 'static,
    {
        if let Ok(mut listeners) = self.event_listeners.lock() {
            listeners.push(Arc::new(listener));
        }
    }

    /// Broadcasts an event to registered listeners without holding the lock during invocation.
    fn emit(&self, event: OrchestratorEvent) {
        let listeners_snapshot: Vec<EventListener> = {
            if let Ok(listeners) = self.event_listeners.lock() {
                listeners.clone()
            } else {
                Vec::new()
            }
        };

        for listener in listeners_snapshot {
            listener(&event);
        }
    }

    /// Returns the current state of the turn machine.
    #[must_use]
    pub fn current_state(&self) -> TurnState {
        self.state_machine.state()
    }

    /// Handles immediate barge-in interruption, canceling active generation and flushing buffers.
    pub fn handle_barge_in(&self) {
        let now_ms = current_time_ms();
        self.state_machine.trigger_cancellation();
        self.emit(OrchestratorEvent::BargeInTriggered {
            timestamp_ms: now_ms,
        });
    }

    /// Appends an entry to the hash-chained audit log if an audit store is configured.
    fn record_audit_entry(
        &self,
        actor: Actor,
        skill_id: &SkillId,
        arguments: &serde_json::Value,
        decision: PolicyDecision,
        status: AuditExecutionStatus,
    ) -> Result<()> {
        if let Some(ref audit_store) = self.audit_store {
            let prev_hash = audit_store
                .latest_hash()
                .map_err(OrchestratorError::Storage)?;
            let redacted = redact_sensitive_json(arguments).to_string();
            let entry = AuditEntry::create(
                AuditEntryId::generate(),
                current_time_ms() as u64,
                actor,
                skill_id.clone(),
                redacted,
                decision,
                status,
                prev_hash,
            );
            audit_store
                .append(&entry)
                .map_err(OrchestratorError::Storage)?;
        }
        Ok(())
    }

    /// Internal helper transitioning the state machine and broadcasting the state change event.
    fn transition(&self, target: TurnState) -> Result<()> {
        let from = self.state_machine.state();
        self.state_machine.transition_to(target)?;
        self.emit(OrchestratorEvent::StateChanged {
            from,
            to: target,
            timestamp_ms: current_time_ms(),
        });
        Ok(())
    }

    /// Processes an interactive text turn and returns the full execution outcome,
    /// pausing if explicit user confirmation is required.
    pub fn execute_turn(
        &self,
        conversation_id: &ConversationId,
        user_text: &str,
    ) -> Result<TurnExecution> {
        let trimmed_text = user_text.trim();
        if trimmed_text.is_empty() {
            return Err(OrchestratorError::Internal(
                "Cannot process empty user prompt".to_string(),
            ));
        }

        // Scan input for prompt injection and taint escalation
        let scanner = AdversarialScanner::new();
        let injection_matches = scanner.scan(trimmed_text);
        if !injection_matches.is_empty() {
            if let Ok(mut tracker) = self.taint_tracker.write() {
                tracker.mark_tainted(
                    aissistant_security::TaintSource::ExternalData {
                        label: format!("prompt_injection:{:?}", injection_matches[0].category),
                    },
                    current_time_ms() as u64,
                    format!(
                        "Adversarial prompt injection pattern detected: {:?}",
                        injection_matches[0].category
                    ),
                );
            }
        }

        // Reset state machine for new turn
        self.state_machine.reset_for_new_turn()?;

        // Transition 1: Idle -> UtteranceReceived
        self.transition(TurnState::UtteranceReceived)?;

        // Verify conversation exists
        let conversation = self
            .conversation_store
            .get_conversation(conversation_id)?
            .ok_or_else(|| OrchestratorError::ConversationNotFound(conversation_id.to_string()))?;

        // Determine next sequential turn index
        let existing_turns = self.conversation_store.list_turns(conversation_id, None)?;
        let user_turn_index = existing_turns.len() as u32;

        let now_ms = current_time_ms();
        let user_turn = ConversationTurn {
            id: TurnId::generate(),
            conversation_id: conversation_id.clone(),
            turn_index: user_turn_index,
            role: TurnRole::User,
            content: trimmed_text.to_string(),
            timestamp_ms: now_ms,
            token_count: (trimmed_text.len() / 4).max(1),
            tool_calls: None,
            metadata: TurnMetadata::default(),
        };

        // Persist user turn
        self.conversation_store.add_turn(&user_turn)?;

        // Transition 2: UtteranceReceived -> RetrievingMemory
        self.transition(TurnState::RetrievingMemory)?;
        let memory_start = Instant::now();

        // Dense embedding query
        let query_embedding = self
            .embedding_engine
            .embed(trimmed_text)
            .map_err(OrchestratorError::Llm)?;

        // Hybrid RRF search across past episodic turns
        let memory_hits = self.hybrid_search.search(
            trimmed_text,
            &query_embedding,
            self.config.max_memory_excerpts,
            None,
            Some(conversation_id),
        )?;

        let memory_duration = memory_start.elapsed().as_millis() as u64;
        self.state_machine.record_memory_hits(memory_hits.len());
        self.emit(OrchestratorEvent::MemoryRetrieved {
            count: memory_hits.len(),
            duration_ms: memory_duration,
        });

        // Transition 3: RetrievingMemory -> PromptAssembly
        self.transition(TurnState::PromptAssembly)?;

        let mut session = LlmSession::new(conversation_id.as_str(), 4096);

        // Inject retrieved episodic memory context if available
        let mut assembled_system = conversation
            .metadata
            .system_prompt
            .unwrap_or_else(|| self.config.system_prompt.clone());

        if let Some(store) = &self.persona_store {
            let active_persona = store.get_active_persona().unwrap_or(None);
            let prefs = store.list_preferences(None).unwrap_or_default();
            assembled_system = PersonaPromptInjector::inject_persona(
                &assembled_system,
                active_persona.as_ref(),
                &prefs,
            );
        }

        if !memory_hits.is_empty() {
            assembled_system.push_str("\n\n--- Relevant Prior Memories ---\n");
            for hit in &memory_hits {
                assembled_system.push_str(&format!("- {}\n", hit.snippet));
            }
        }

        // Add registered tool definitions to system prompt if any exist
        let tool_definitions = self.tool_dispatcher.tool_definitions()?;
        if !tool_definitions.is_empty() {
            assembled_system.push_str("\n\n--- Available Tools ---\n");
            for tool in &tool_definitions {
                assembled_system.push_str(&format!(
                    "- {}: {} (parameters: {})\n",
                    tool.name, tool.description, tool.parameters
                ));
            }
        }

        // Add recent conversation history within active context window (older context is recalled via episodic hybrid search)
        let context_turns = if existing_turns.len() > 20 {
            &existing_turns[existing_turns.len() - 20..]
        } else {
            &existing_turns[..]
        };
        for turn in context_turns {
            session.add_turn(turn.role.as_str(), &turn.content);
        }
        // Add current user prompt
        session.add_turn("user", trimmed_text);

        let prompt = session.format_chatml_prompt(Some(&assembled_system));

        // Transition 4: PromptAssembly -> Inferring
        self.transition(TurnState::Inferring)?;
        let inference_start = Instant::now();
        let cancel_handle = self.state_machine.cancellation_handle();

        let assistant_turn_id = TurnId::generate();
        let mut assistant_text = String::new();
        let mut token_index = 0;
        let mut interrupted = false;

        {
            let cancel_ref = Some(cancel_handle.as_ref());
            let stream_turn_id = assistant_turn_id.clone();
            let mut on_token = |chunk: TokenChunk| -> aissistant_llm::Result<()> {
                if self.state_machine.state() == TurnState::Interrupted {
                    interrupted = true;
                    return Ok(());
                }

                if token_index == 0 {
                    let ttft = inference_start.elapsed().as_millis() as u64;
                    self.state_machine.record_ttft(ttft);
                }

                assistant_text.push_str(&chunk.token);
                self.state_machine.record_token();
                self.emit(OrchestratorEvent::TokenStreamed {
                    turn_id: stream_turn_id.clone(),
                    token: chunk.token,
                    index: token_index,
                });

                token_index += 1;

                if chunk.finish_reason == Some(FinishReason::Cancelled) {
                    interrupted = true;
                }
                Ok(())
            };

            self.llm_engine
                .generate_stream(
                    &prompt,
                    &self.config.sampling_params,
                    cancel_ref,
                    &mut on_token,
                )
                .map_err(OrchestratorError::Llm)?;
        }

        // Check if interrupted mid-stream
        if interrupted || self.state_machine.cancellation_handle().is_cancelled() {
            let _ = self.state_machine.transition_to(TurnState::Interrupted);
            let metrics = self.state_machine.finalize_metrics();

            let assistant_turn = ConversationTurn {
                id: assistant_turn_id.clone(),
                conversation_id: conversation_id.clone(),
                turn_index: user_turn_index + 1,
                role: TurnRole::Assistant,
                content: assistant_text.clone(),
                timestamp_ms: current_time_ms(),
                token_count: metrics.token_count,
                tool_calls: None,
                metadata: TurnMetadata {
                    model_id: None,
                    latency_ms: metrics.latency_ms,
                    finish_reason: Some("interrupted".to_string()),
                    audio_duration_ms: None,
                },
            };

            self.conversation_store.add_turn(&assistant_turn)?;

            self.emit(OrchestratorEvent::TurnFinished {
                turn_id: assistant_turn_id.clone(),
                duration_ms: metrics.latency_ms.unwrap_or(0),
                token_count: metrics.token_count,
                interrupted: true,
            });

            let _ = self.state_machine.transition_to(TurnState::Idle);

            return Ok(TurnExecution::Completed(TurnSummary {
                assistant_turn_id,
                conversation_id: conversation_id.clone(),
                user_prompt: trimmed_text.to_string(),
                assistant_response: assistant_text,
                memory_hits_count: memory_hits.len(),
                token_count: metrics.token_count,
                latency_ms: metrics.latency_ms.unwrap_or(0),
                ttft_ms: metrics.ttft_ms,
                interrupted: true,
                tool_result: None,
            }));
        }

        let metrics = self.state_machine.finalize_metrics();

        // Check if the generated output represents a structured tool call
        let tool_candidate = extract_json_candidate(&assistant_text);
        let tool_call_opt = if !tool_definitions.is_empty() {
            ToolCallValidator::parse_and_validate(tool_candidate, &tool_definitions).ok()
        } else {
            None
        };

        if let Some(tool_call) = tool_call_opt {
            self.transition(TurnState::ToolInvocationDetected)?;
            self.emit(OrchestratorEvent::ToolCallDetected {
                tool_name: tool_call.name.clone(),
            });

            let tool_handler = self.tool_dispatcher.validate_call(&tool_call)?;
            let skill_id = SkillId::new(&tool_call.name)?;

            let taint_state = self
                .taint_tracker
                .read()
                .map(|t| t.state().clone())
                .unwrap_or(aissistant_security::TaintState::Clean);

            let policy_context = PolicyContext::llm(taint_state.clone());

            let decision = {
                let policy = self.policy_engine.read().map_err(|_| {
                    OrchestratorError::Internal("PolicyEngine lock poisoned".to_string())
                })?;
                policy.evaluate(&skill_id, tool_handler.risk_tier(), &policy_context)
            };

            match decision {
                PolicyDecision::RequireConfirmation { reason, risk_tier } => {
                    self.record_audit_entry(
                        Actor::LlmOrchestrator,
                        &skill_id,
                        &tool_call.arguments,
                        PolicyDecision::RequireConfirmation {
                            reason: reason.clone(),
                            risk_tier,
                        },
                        AuditExecutionStatus::Cancelled,
                    )?;

                    let pending = PendingConfirmation {
                        id: ConfirmationId::generate(),
                        conversation_id: conversation_id.clone(),
                        turn_id: user_turn.id,
                        tool_name: tool_call.name,
                        arguments: tool_call.arguments,
                        risk_tier,
                        reason,
                        created_at_ms: current_time_ms(),
                        untrusted_input_tainted: taint_state.is_tainted(),
                    };

                    self.confirmation_manager.insert(pending.clone())?;
                    self.transition(TurnState::AwaitingToolExecution)?;
                    self.emit(OrchestratorEvent::ConfirmationRequired {
                        confirmation: pending.clone(),
                    });

                    return Ok(TurnExecution::AwaitingConfirmation(pending));
                }

                PolicyDecision::Allow => {
                    self.transition(TurnState::AwaitingToolExecution)?;
                    self.emit(OrchestratorEvent::ToolExecutionStarted {
                        tool_name: tool_call.name.clone(),
                    });

                    let exec_start = Instant::now();
                    let tool_exec_res = tool_handler.execute(tool_call.arguments.clone());
                    let exec_duration_ms = exec_start.elapsed().as_millis() as u64;

                    let (status, success) = match &tool_exec_res {
                        Ok(_) => (AuditExecutionStatus::Success, true),
                        Err(e) => (
                            AuditExecutionStatus::Failed {
                                reason: e.to_string(),
                            },
                            false,
                        ),
                    };

                    self.record_audit_entry(
                        Actor::LlmOrchestrator,
                        &skill_id,
                        &tool_call.arguments,
                        PolicyDecision::Allow,
                        status,
                    )?;

                    self.emit(OrchestratorEvent::ToolExecutionCompleted {
                        tool_name: tool_call.name.clone(),
                        success,
                        duration_ms: exec_duration_ms,
                    });

                    let tool_val = tool_exec_res?;
                    let assistant_response =
                        format!("Executed '{}' successfully: {}", tool_call.name, tool_val);

                    let assistant_turn_id = TurnId::generate();
                    let assistant_turn = ConversationTurn {
                        id: assistant_turn_id.clone(),
                        conversation_id: conversation_id.clone(),
                        turn_index: user_turn_index + 1,
                        role: TurnRole::Assistant,
                        content: assistant_response.clone(),
                        timestamp_ms: current_time_ms(),
                        token_count: metrics.token_count,
                        tool_calls: Some(serde_json::to_value(&tool_call)?),
                        metadata: TurnMetadata {
                            model_id: None,
                            latency_ms: metrics.latency_ms,
                            finish_reason: Some("tool_call".to_string()),
                            audio_duration_ms: None,
                        },
                    };

                    self.conversation_store.add_turn(&assistant_turn)?;

                    if let Ok(embedding) = self.embedding_engine.embed(&assistant_response) {
                        let vec_rec = VectorRecord {
                            id: format!("vec_{}", assistant_turn_id.as_str()),
                            entity_type: "turn".to_string(),
                            entity_id: assistant_turn_id.as_str().to_string(),
                            conversation_id: Some(conversation_id.as_str().to_string()),
                            dimension: embedding.len(),
                            embedding,
                            content_snippet: assistant_response.chars().take(200).collect(),
                            created_at_ms: current_time_ms(),
                        };
                        let _ = self.vector_store.store_embedding(&vec_rec);
                    }

                    self.emit(OrchestratorEvent::TurnFinished {
                        turn_id: assistant_turn_id.clone(),
                        duration_ms: metrics.latency_ms.unwrap_or(0),
                        token_count: metrics.token_count,
                        interrupted: false,
                    });

                    self.transition(TurnState::TurnCompleted)?;
                    let _ = self.state_machine.transition_to(TurnState::Idle);

                    return Ok(TurnExecution::Completed(TurnSummary {
                        assistant_turn_id,
                        conversation_id: conversation_id.clone(),
                        user_prompt: trimmed_text.to_string(),
                        assistant_response,
                        memory_hits_count: memory_hits.len(),
                        token_count: metrics.token_count,
                        latency_ms: metrics.latency_ms.unwrap_or(0),
                        ttft_ms: metrics.ttft_ms,
                        interrupted: false,
                        tool_result: Some(tool_val),
                    }));
                }

                PolicyDecision::Deny { reason } => {
                    self.record_audit_entry(
                        Actor::LlmOrchestrator,
                        &skill_id,
                        &tool_call.arguments,
                        PolicyDecision::Deny {
                            reason: reason.clone(),
                        },
                        AuditExecutionStatus::Cancelled,
                    )?;

                    let assistant_response = format!(
                        "Action denied: execution of tool '{}' was blocked by security policy ({}).",
                        tool_call.name, reason
                    );

                    let assistant_turn_id = TurnId::generate();
                    let assistant_turn = ConversationTurn {
                        id: assistant_turn_id.clone(),
                        conversation_id: conversation_id.clone(),
                        turn_index: user_turn_index + 1,
                        role: TurnRole::Assistant,
                        content: assistant_response.clone(),
                        timestamp_ms: current_time_ms(),
                        token_count: metrics.token_count,
                        tool_calls: Some(serde_json::to_value(&tool_call)?),
                        metadata: TurnMetadata {
                            model_id: None,
                            latency_ms: metrics.latency_ms,
                            finish_reason: Some("policy_denied".to_string()),
                            audio_duration_ms: None,
                        },
                    };

                    self.conversation_store.add_turn(&assistant_turn)?;

                    self.emit(OrchestratorEvent::TurnFinished {
                        turn_id: assistant_turn_id.clone(),
                        duration_ms: metrics.latency_ms.unwrap_or(0),
                        token_count: metrics.token_count,
                        interrupted: false,
                    });

                    self.transition(TurnState::TurnCompleted)?;
                    let _ = self.state_machine.transition_to(TurnState::Idle);

                    return Ok(TurnExecution::Completed(TurnSummary {
                        assistant_turn_id,
                        conversation_id: conversation_id.clone(),
                        user_prompt: trimmed_text.to_string(),
                        assistant_response,
                        memory_hits_count: memory_hits.len(),
                        token_count: metrics.token_count,
                        latency_ms: metrics.latency_ms.unwrap_or(0),
                        ttft_ms: metrics.ttft_ms,
                        interrupted: false,
                        tool_result: None,
                    }));
                }
            }
        }

        // Standard Conversational Flow:
        // Transition 5: Synthesizing Speech (if enabled)
        if self.config.synthesize_speech {
            if let Some(ref tts_lock) = self.tts_engine {
                self.transition(TurnState::SynthesizingSpeech)?;
                let mut chunker = SentenceChunker::new();
                chunker.push_text(&assistant_text);

                while let Some(sentence) = chunker.next_sentence() {
                    if self.state_machine.cancellation_handle().is_cancelled() {
                        break;
                    }
                    if let Ok(mut tts) = tts_lock.lock() {
                        if let Ok(audio_chunk) = tts.synthesize_sentence(&sentence) {
                            self.emit(OrchestratorEvent::SentenceSynthesized {
                                sentence,
                                audio_samples: audio_chunk.samples.len(),
                            });
                        }
                    }
                }

                if let Some(final_sentence) = chunker.flush() {
                    if !self.state_machine.cancellation_handle().is_cancelled() {
                        if let Ok(mut tts) = tts_lock.lock() {
                            if let Ok(audio_chunk) = tts.synthesize_sentence(&final_sentence) {
                                self.emit(OrchestratorEvent::SentenceSynthesized {
                                    sentence: final_sentence,
                                    audio_samples: audio_chunk.samples.len(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Transition 6: TurnCompleted
        self.transition(TurnState::TurnCompleted)?;

        let assistant_turn = ConversationTurn {
            id: assistant_turn_id.clone(),
            conversation_id: conversation_id.clone(),
            turn_index: user_turn_index + 1,
            role: TurnRole::Assistant,
            content: assistant_text.clone(),
            timestamp_ms: current_time_ms(),
            token_count: metrics.token_count,
            tool_calls: None,
            metadata: TurnMetadata {
                model_id: None,
                latency_ms: metrics.latency_ms,
                finish_reason: Some("stop".to_string()),
                audio_duration_ms: None,
            },
        };

        // Persist assistant turn to SQLite
        self.conversation_store.add_turn(&assistant_turn)?;

        // Embed and index assistant turn into dense vector memory store
        if let Ok(embedding) = self.embedding_engine.embed(&assistant_text) {
            let vec_rec = VectorRecord {
                id: format!("vec_{}", assistant_turn_id.as_str()),
                entity_type: "turn".to_string(),
                entity_id: assistant_turn_id.as_str().to_string(),
                conversation_id: Some(conversation_id.as_str().to_string()),
                dimension: embedding.len(),
                embedding,
                content_snippet: assistant_text.chars().take(200).collect(),
                created_at_ms: current_time_ms(),
            };
            let _ = self.vector_store.store_embedding(&vec_rec);
        }

        self.emit(OrchestratorEvent::TurnFinished {
            turn_id: assistant_turn_id.clone(),
            duration_ms: metrics.latency_ms.unwrap_or(0),
            token_count: metrics.token_count,
            interrupted: false,
        });

        // Reset to Idle
        self.transition(TurnState::Idle)?;

        Ok(TurnExecution::Completed(TurnSummary {
            assistant_turn_id,
            conversation_id: conversation_id.clone(),
            user_prompt: trimmed_text.to_string(),
            assistant_response: assistant_text,
            memory_hits_count: memory_hits.len(),
            token_count: metrics.token_count,
            latency_ms: metrics.latency_ms.unwrap_or(0),
            ttft_ms: metrics.ttft_ms,
            interrupted: false,
            tool_result: None,
        }))
    }

    /// Convenience wrapper executing a text turn that expects immediate completion.
    pub fn process_text_turn(
        &self,
        conversation_id: &ConversationId,
        user_text: &str,
    ) -> Result<TurnSummary> {
        match self.execute_turn(conversation_id, user_text)? {
            TurnExecution::Completed(summary) => Ok(summary),
            TurnExecution::AwaitingConfirmation(pending) => {
                Err(OrchestratorError::Internal(format!(
                    "Turn awaiting user confirmation for tool '{}' (id: {}): resolve using resolve_confirmation",
                    pending.tool_name, pending.id
                )))
            }
        }
    }

    /// Resolves an outstanding confirmation request, executing the requested tool
    /// if approved, or aborting execution if rejected.
    pub fn resolve_confirmation(
        &self,
        id: &ConfirmationId,
        approved: bool,
    ) -> Result<TurnExecution> {
        let pending = self.confirmation_manager.take_by_id_or_turn(id.as_str())?;
        let skill_id = SkillId::new(&pending.tool_name)?;

        let existing_turns = self
            .conversation_store
            .list_turns(&pending.conversation_id, None)?;
        let next_turn_index = existing_turns.len() as u32;

        if !approved {
            self.emit(OrchestratorEvent::ConfirmationResolved {
                id: pending.id.clone(),
                approved: false,
            });

            self.record_audit_entry(
                Actor::LocalUser,
                &skill_id,
                &pending.arguments,
                PolicyDecision::RequireConfirmation {
                    reason: pending.reason.clone(),
                    risk_tier: pending.risk_tier,
                },
                AuditExecutionStatus::Cancelled,
            )?;

            let assistant_turn_id = TurnId::generate();
            let content = format!(
                "Action cancelled: user declined execution of tool '{}'.",
                pending.tool_name
            );
            let assistant_turn = ConversationTurn {
                id: assistant_turn_id.clone(),
                conversation_id: pending.conversation_id.clone(),
                turn_index: next_turn_index,
                role: TurnRole::Assistant,
                content: content.clone(),
                timestamp_ms: current_time_ms(),
                token_count: (content.len() / 4).max(1),
                tool_calls: None,
                metadata: TurnMetadata {
                    model_id: None,
                    latency_ms: None,
                    finish_reason: Some("confirmation_rejected".to_string()),
                    audio_duration_ms: None,
                },
            };

            self.conversation_store.add_turn(&assistant_turn)?;

            self.transition(TurnState::TurnCompleted)?;
            let _ = self.state_machine.transition_to(TurnState::Idle);

            return Ok(TurnExecution::Completed(TurnSummary {
                assistant_turn_id,
                conversation_id: pending.conversation_id,
                user_prompt: String::new(),
                assistant_response: content,
                memory_hits_count: 0,
                token_count: 0,
                latency_ms: 0,
                ttft_ms: None,
                interrupted: false,
                tool_result: None,
            }));
        }

        // User Approved Path
        self.emit(OrchestratorEvent::ConfirmationResolved {
            id: pending.id.clone(),
            approved: true,
        });

        self.emit(OrchestratorEvent::ToolExecutionStarted {
            tool_name: pending.tool_name.clone(),
        });

        let exec_start = Instant::now();
        let exec_res = self
            .tool_dispatcher
            .execute(&pending.tool_name, pending.arguments.clone());
        let duration_ms = exec_start.elapsed().as_millis() as u64;

        let (status, success) = match &exec_res {
            Ok(_) => (AuditExecutionStatus::Success, true),
            Err(e) => (
                AuditExecutionStatus::Failed {
                    reason: e.to_string(),
                },
                false,
            ),
        };

        self.record_audit_entry(
            Actor::LocalUser,
            &skill_id,
            &pending.arguments,
            PolicyDecision::RequireConfirmation {
                reason: pending.reason.clone(),
                risk_tier: pending.risk_tier,
            },
            status,
        )?;

        self.emit(OrchestratorEvent::ToolExecutionCompleted {
            tool_name: pending.tool_name.clone(),
            success,
            duration_ms,
        });

        let tool_val = exec_res?;
        let content = format!(
            "Confirmed and executed '{}' successfully: {}",
            pending.tool_name, tool_val
        );

        let assistant_turn_id = TurnId::generate();
        let assistant_turn = ConversationTurn {
            id: assistant_turn_id.clone(),
            conversation_id: pending.conversation_id.clone(),
            turn_index: next_turn_index,
            role: TurnRole::Assistant,
            content: content.clone(),
            timestamp_ms: current_time_ms(),
            token_count: (content.len() / 4).max(1),
            tool_calls: None,
            metadata: TurnMetadata {
                model_id: None,
                latency_ms: Some(duration_ms),
                finish_reason: Some("confirmed_tool_executed".to_string()),
                audio_duration_ms: None,
            },
        };

        self.conversation_store.add_turn(&assistant_turn)?;

        if let Ok(embedding) = self.embedding_engine.embed(&content) {
            let vec_rec = VectorRecord {
                id: format!("vec_{}", assistant_turn_id.as_str()),
                entity_type: "turn".to_string(),
                entity_id: assistant_turn_id.as_str().to_string(),
                conversation_id: Some(pending.conversation_id.as_str().to_string()),
                dimension: embedding.len(),
                embedding,
                content_snippet: content.chars().take(200).collect(),
                created_at_ms: current_time_ms(),
            };
            let _ = self.vector_store.store_embedding(&vec_rec);
        }

        self.transition(TurnState::TurnCompleted)?;
        let _ = self.state_machine.transition_to(TurnState::Idle);

        Ok(TurnExecution::Completed(TurnSummary {
            assistant_turn_id,
            conversation_id: pending.conversation_id,
            user_prompt: String::new(),
            assistant_response: content,
            memory_hits_count: 0,
            token_count: 0,
            latency_ms: duration_ms,
            ttft_ms: None,
            interrupted: false,
            tool_result: Some(tool_val),
        }))
    }
}

impl std::fmt::Debug for Orchestrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Orchestrator")
            .field("config", &self.config)
            .field("state", &self.current_state())
            .finish()
    }
}

fn current_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn extract_json_candidate(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        if let Some(inner) = stripped.strip_suffix("```") {
            return inner.trim();
        }
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        if let Some(inner) = stripped.strip_suffix("```") {
            return inner.trim();
        }
    }
    trimmed
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tool_dispatcher::ToolHandler;
    use aissistant_llm::{
        InferenceMetrics, LlmCancellationHandle, LlmEngineConfig, LocalEmbeddingEngine,
        LocalLlmEngine,
    };
    use aissistant_security::{AuditChainVerifier, RiskTier, TaintSource, GENESIS_PREV_HASH};
    use aissistant_storage::SqliteDb;

    fn setup_test_orchestrator() -> (Orchestrator, ConversationId) {
        let db = SqliteDb::open_in_memory().unwrap();
        let conv_store = ConversationStore::new(db.clone());
        let vec_store = VectorMemoryStore::new(db.clone());
        let audit_store = Arc::new(AuditStore::new(db));
        let hybrid = HybridSearchEngine::new(conv_store.clone(), vec_store.clone());
        let embed = Arc::new(LocalEmbeddingEngine::new(64));

        let mut local_llm = LocalLlmEngine::new();
        local_llm
            .load_model(LlmEngineConfig {
                model_id: "test-qwen".to_string(),
                model_path: std::path::PathBuf::from("test-qwen.gguf"),
                n_gpu_layers: 0,
                context_length: 2048,
                threads: 2,
                memory_budget_bytes: 1024 * 1024 * 1024,
            })
            .unwrap();

        let llm: Arc<dyn LlmEngine> = Arc::new(local_llm);

        let config = OrchestratorConfig::default();
        let orchestrator = Orchestrator::new(
            config,
            conv_store.clone(),
            vec_store,
            hybrid,
            embed,
            llm,
            None,
        )
        .with_audit_store(audit_store);

        let conv = conv_store
            .create_conversation("Orchestrator Test", None)
            .unwrap();
        (orchestrator, conv.id)
    }

    struct ScriptedLlmEngine {
        canned_response: String,
    }

    impl ScriptedLlmEngine {
        fn new(response: impl Into<String>) -> Self {
            Self {
                canned_response: response.into(),
            }
        }
    }

    impl LlmEngine for ScriptedLlmEngine {
        fn load_model(&mut self, _config: LlmEngineConfig) -> aissistant_llm::Result<()> {
            Ok(())
        }
        fn unload_model(&mut self) {}
        fn is_loaded(&self) -> bool {
            true
        }
        fn active_config(&self) -> Option<&LlmEngineConfig> {
            None
        }
        fn active_model_id(&self) -> Option<&str> {
            Some("scripted-model")
        }
        fn generate_stream(
            &self,
            _prompt: &str,
            _params: &SamplingParams,
            _cancel_handle: Option<&LlmCancellationHandle>,
            on_token: &mut dyn FnMut(TokenChunk) -> aissistant_llm::Result<()>,
        ) -> aissistant_llm::Result<InferenceMetrics> {
            let chunk = TokenChunk::final_chunk(&self.canned_response, 0, FinishReason::Stop);
            on_token(chunk)?;
            Ok(InferenceMetrics::new(10, 1, 5))
        }
        fn on_memory_pressure(&mut self, _critical: bool) {}
    }

    struct BatteryTool;
    impl ToolHandler for BatteryTool {
        fn name(&self) -> &str {
            "system.battery_level"
        }
        fn description(&self) -> &str {
            "Returns battery level percentage"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::ReadOnly
        }
        fn execute(&self, _arguments: serde_json::Value) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "battery_percentage": 88 }))
        }
    }

    struct FileWriteTool;
    impl ToolHandler for FileWriteTool {
        fn name(&self) -> &str {
            "file.write"
        }
        fn description(&self) -> &str {
            "Writes content to a file path"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            })
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::FileAndApp
        }
        fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value> {
            Ok(
                serde_json::json!({ "bytes_written": arguments["content"].as_str().unwrap_or("").len() }),
            )
        }
    }

    struct VolumeTool;
    impl ToolHandler for VolumeTool {
        fn name(&self) -> &str {
            "media.volume"
        }
        fn description(&self) -> &str {
            "Sets media playback volume"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "level": { "type": "integer" }
                },
                "required": ["level"]
            })
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::StateModifying
        }
        fn execute(&self, arguments: serde_json::Value) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "new_volume": arguments["level"] }))
        }
    }

    struct FactoryResetTool;
    impl ToolHandler for FactoryResetTool {
        fn name(&self) -> &str {
            "system.factory_reset"
        }
        fn description(&self) -> &str {
            "Factory reset device"
        }
        fn parameters_schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            })
        }
        fn risk_tier(&self) -> RiskTier {
            RiskTier::Destructive
        }
        fn execute(&self, _arguments: serde_json::Value) -> Result<serde_json::Value> {
            Ok(serde_json::json!({ "reset": true }))
        }
    }

    #[test]
    fn test_process_text_turn_end_to_end() {
        let (orch, conv_id) = setup_test_orchestrator();
        assert_eq!(orch.current_state(), TurnState::Idle);

        let summary = orch
            .process_text_turn(&conv_id, "What can you do?")
            .unwrap();

        assert_eq!(summary.user_prompt, "What can you do?");
        assert!(!summary.assistant_response.is_empty());
        assert!(summary.token_count > 0);
        assert!(!summary.interrupted);
        assert_eq!(orch.current_state(), TurnState::Idle);

        // Verify conversation history stored both turns
        let turns = orch.conversation_store.list_turns(&conv_id, None).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].role, TurnRole::User);
        assert_eq!(turns[0].content, "What can you do?");
        assert_eq!(turns[1].role, TurnRole::Assistant);
        assert_eq!(turns[1].content, summary.assistant_response);

        // Verify embedding was stored for the assistant turn
        let embeddings = orch
            .vector_store
            .list_embeddings_for_conversation(conv_id.as_str())
            .unwrap();
        assert_eq!(embeddings.len(), 1);
        assert_eq!(embeddings[0].dimension, 64);
    }

    #[test]
    fn test_orchestrator_event_telemetry() {
        let (orch, conv_id) = setup_test_orchestrator();
        let received_events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&received_events);

        orch.on_event(move |event| {
            if let Ok(mut lock) = events_clone.lock() {
                lock.push(event.clone());
            }
        });

        orch.process_text_turn(&conv_id, "Testing event stream")
            .unwrap();

        let events = received_events.lock().unwrap();
        assert!(!events.is_empty());

        let has_state_changed = events
            .iter()
            .any(|e| matches!(e, OrchestratorEvent::StateChanged { .. }));
        let has_tokens = events
            .iter()
            .any(|e| matches!(e, OrchestratorEvent::TokenStreamed { .. }));
        let has_finished = events
            .iter()
            .any(|e| matches!(e, OrchestratorEvent::TurnFinished { .. }));

        assert!(has_state_changed);
        assert!(has_tokens);
        assert!(has_finished);
    }

    #[test]
    fn test_barge_in_cooperative_cancellation() {
        let (orch, conv_id) = setup_test_orchestrator();
        let orch_clone = orch.clone();

        orch.on_event(move |event| {
            if let OrchestratorEvent::TokenStreamed { index, .. } = event {
                if *index == 1 {
                    orch_clone.handle_barge_in();
                }
            }
        });

        let summary = orch
            .process_text_turn(&conv_id, "Interrupted request")
            .unwrap();

        assert!(summary.interrupted);
        assert_eq!(orch.current_state(), TurnState::Idle);

        let turns = orch.conversation_store.list_turns(&conv_id, None).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(
            turns[1].metadata.finish_reason.as_deref(),
            Some("interrupted")
        );
    }

    #[test]
    fn test_orchestrator_error_handling() {
        let (orch, _conv_id) = setup_test_orchestrator();
        let bogus_conv_id = ConversationId::new("conv-nonexistent").unwrap();

        let err = orch.process_text_turn(&bogus_conv_id, "Hello").unwrap_err();
        assert!(matches!(err, OrchestratorError::ConversationNotFound(_)));

        let err_empty = orch.process_text_turn(&bogus_conv_id, "   ").unwrap_err();
        assert!(matches!(err_empty, OrchestratorError::Internal(_)));
    }

    #[test]
    fn test_orchestrator_tts_synthesis_flow() {
        use aissistant_speech::{AcousticTtsEngine, TtsConfig};

        let db = SqliteDb::open_in_memory().unwrap();
        let conv_store = ConversationStore::new(db.clone());
        let vec_store = VectorMemoryStore::new(db);
        let hybrid = HybridSearchEngine::new(conv_store.clone(), vec_store.clone());
        let embed = Arc::new(LocalEmbeddingEngine::new(64));

        let mut local_llm = LocalLlmEngine::new();
        local_llm
            .load_model(LlmEngineConfig {
                model_id: "test-qwen".to_string(),
                model_path: std::path::PathBuf::from("test-qwen.gguf"),
                n_gpu_layers: 0,
                context_length: 2048,
                threads: 2,
                memory_budget_bytes: 1024 * 1024 * 1024,
            })
            .unwrap();

        let config = OrchestratorConfig {
            synthesize_speech: true,
            ..Default::default()
        };

        let tts = Arc::new(Mutex::new(
            AcousticTtsEngine::new(TtsConfig::default()).unwrap(),
        ));
        let orchestrator = Orchestrator::new(
            config,
            conv_store.clone(),
            vec_store,
            hybrid,
            embed,
            Arc::new(local_llm),
            Some(tts),
        );

        let conv = conv_store
            .create_conversation("TTS Orchestrator Test", None)
            .unwrap();

        let synthesized_sentences = Arc::new(Mutex::new(Vec::new()));
        let sent_clone = Arc::clone(&synthesized_sentences);

        orchestrator.on_event(move |event| {
            if let OrchestratorEvent::SentenceSynthesized { sentence, .. } = event {
                if let Ok(mut list) = sent_clone.lock() {
                    list.push(sentence.clone());
                }
            }
        });

        let summary = orchestrator
            .process_text_turn(&conv.id, "Please speak to me")
            .unwrap();

        assert!(!summary.assistant_response.is_empty());
        let sents = synthesized_sentences.lock().unwrap();
        assert!(!sents.is_empty());
    }

    #[test]
    fn test_read_only_tool_auto_execution() {
        let (mut orch, conv_id) = setup_test_orchestrator();
        orch.llm_engine = Arc::new(ScriptedLlmEngine::new(
            r#"{"name": "system.battery_level", "arguments": {}}"#,
        ));

        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(BatteryTool)).unwrap();
        orch = orch.with_tool_dispatcher(dispatcher);

        let execution = orch.execute_turn(&conv_id, "Check battery").unwrap();
        assert!(matches!(execution, TurnExecution::Completed(_)));
        if let TurnExecution::Completed(summary) = execution {
            assert_eq!(
                summary.tool_result,
                Some(serde_json::json!({ "battery_percentage": 88 }))
            );
            assert!(summary.assistant_response.contains("system.battery_level"));
        }

        // Verify audit log
        let audit = orch.audit_store().unwrap();
        let entries = audit.get_entries(10, 0).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].skill_id.as_str(), "system.battery_level");
        assert_eq!(entries[0].policy_decision, PolicyDecision::Allow);
        assert_eq!(entries[0].execution_status, AuditExecutionStatus::Success);
    }

    #[test]
    fn test_file_tool_requires_confirmation_flow_approved() {
        let (mut orch, conv_id) = setup_test_orchestrator();
        orch.llm_engine = Arc::new(ScriptedLlmEngine::new(
            r#"{"name": "file.write", "arguments": {"path": "hello.txt", "content": "world"}}"#,
        ));

        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(FileWriteTool)).unwrap();
        orch = orch.with_tool_dispatcher(dispatcher);

        let execution = orch.execute_turn(&conv_id, "Write to file").unwrap();
        assert!(matches!(execution, TurnExecution::AwaitingConfirmation(_)));
        let pending = match execution {
            TurnExecution::AwaitingConfirmation(p) => p,
            _ => unreachable!(),
        };

        assert_eq!(pending.tool_name, "file.write");
        assert_eq!(pending.risk_tier, RiskTier::FileAndApp);
        assert_eq!(orch.current_state(), TurnState::AwaitingToolExecution);

        // Resolve confirmation with approval
        let resolved = orch.resolve_confirmation(&pending.id, true).unwrap();
        assert!(matches!(resolved, TurnExecution::Completed(_)));
        if let TurnExecution::Completed(summary) = resolved {
            assert_eq!(
                summary.tool_result,
                Some(serde_json::json!({ "bytes_written": 5 }))
            );
            assert!(summary
                .assistant_response
                .contains("Confirmed and executed"));
            assert_eq!(orch.current_state(), TurnState::Idle);
        }

        // Verify audit entries: 1st evaluation RequireConfirmation, 2nd execution Success
        let audit = orch.audit_store().unwrap();
        let entries = audit.get_entries(10, 0).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].actor, Actor::LlmOrchestrator);
        assert_eq!(entries[1].actor, Actor::LocalUser);
        assert_eq!(entries[1].execution_status, AuditExecutionStatus::Success);
    }

    #[test]
    fn test_file_tool_confirmation_rejected() {
        let (mut orch, conv_id) = setup_test_orchestrator();
        orch.llm_engine = Arc::new(ScriptedLlmEngine::new(
            r#"{"name": "file.write", "arguments": {"path": "hello.txt", "content": "world"}}"#,
        ));

        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(FileWriteTool)).unwrap();
        orch = orch.with_tool_dispatcher(dispatcher);

        let execution = orch.execute_turn(&conv_id, "Write to file").unwrap();
        assert!(matches!(execution, TurnExecution::AwaitingConfirmation(_)));
        let pending = match execution {
            TurnExecution::AwaitingConfirmation(p) => p,
            _ => unreachable!(),
        };

        // Reject confirmation
        let resolved = orch.resolve_confirmation(&pending.id, false).unwrap();
        assert!(matches!(resolved, TurnExecution::Completed(_)));
        if let TurnExecution::Completed(summary) = resolved {
            assert!(summary
                .assistant_response
                .contains("Action cancelled: user declined"));
            assert_eq!(summary.tool_result, None);
            assert_eq!(orch.current_state(), TurnState::Idle);
        }

        // Verify audit log has cancellation
        let audit = orch.audit_store().unwrap();
        let entries = audit.get_entries(10, 0).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].execution_status, AuditExecutionStatus::Cancelled);
    }

    #[test]
    fn test_taint_escalation_enforces_confirmation() {
        let (mut orch, conv_id) = setup_test_orchestrator();
        orch.llm_engine = Arc::new(ScriptedLlmEngine::new(
            r#"{"name": "media.volume", "arguments": {"level": 75}}"#,
        ));

        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(VolumeTool)).unwrap();
        orch = orch.with_tool_dispatcher(dispatcher);

        // 1. Clean taint state: StateModifying tool executes without confirmation
        let clean_exec = orch.execute_turn(&conv_id, "Set volume").unwrap();
        assert!(matches!(clean_exec, TurnExecution::Completed(_)));

        // 2. Mark context as tainted from external untrusted source
        {
            let mut tracker = orch.taint_tracker().write().unwrap();
            tracker.mark_tainted(TaintSource::Clipboard, 500, "Untrusted clipboard input");
        }

        // 3. Same tool call now requires confirmation due to taint escalation rule
        let tainted_exec = orch.execute_turn(&conv_id, "Set volume again").unwrap();
        assert!(matches!(
            tainted_exec,
            TurnExecution::AwaitingConfirmation(_)
        ));
    }

    #[test]
    fn test_policy_denial_handling() {
        let (mut orch, conv_id) = setup_test_orchestrator();
        orch.llm_engine = Arc::new(ScriptedLlmEngine::new(
            r#"{"name": "system.factory_reset", "arguments": {}}"#,
        ));

        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(FactoryResetTool)).unwrap();
        orch = orch.with_tool_dispatcher(dispatcher);

        // Explicitly deny the factory reset skill via policy engine override
        {
            let mut policy = orch.policy_engine().write().unwrap();
            policy.set_skill_override(
                SkillId::new("system.factory_reset").unwrap(),
                PolicyDecision::Deny {
                    reason: "Factory reset disabled by enterprise device administrator".to_string(),
                },
            );
        }

        let execution = orch.execute_turn(&conv_id, "Reset device").unwrap();
        assert!(matches!(execution, TurnExecution::Completed(_)));
        if let TurnExecution::Completed(summary) = execution {
            assert!(summary.assistant_response.contains("Action denied"));
            assert!(summary
                .assistant_response
                .contains("enterprise device administrator"));
            assert_eq!(summary.tool_result, None);
        }

        let audit = orch.audit_store().unwrap();
        let entries = audit.get_entries(10, 0).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(matches!(
            entries[0].policy_decision,
            PolicyDecision::Deny { .. }
        ));
        assert_eq!(entries[0].execution_status, AuditExecutionStatus::Cancelled);
    }

    #[test]
    fn test_audit_chain_cryptographic_integrity_after_tool_loop() {
        let (mut orch, conv_id) = setup_test_orchestrator();
        let dispatcher = Arc::new(ToolDispatcher::new());
        dispatcher.register(Arc::new(BatteryTool)).unwrap();
        dispatcher.register(Arc::new(FileWriteTool)).unwrap();
        orch = orch.with_tool_dispatcher(dispatcher);

        // 1. Auto-allowed tool
        orch.llm_engine = Arc::new(ScriptedLlmEngine::new(
            r#"{"name": "system.battery_level", "arguments": {}}"#,
        ));
        let _ = orch.execute_turn(&conv_id, "battery").unwrap();

        // 2. Confirmed tool (approved)
        orch.llm_engine = Arc::new(ScriptedLlmEngine::new(
            r#"{"name": "file.write", "arguments": {"path": "a.txt", "content": "123"}}"#,
        ));
        let exec = orch.execute_turn(&conv_id, "write file").unwrap();
        if let TurnExecution::AwaitingConfirmation(p) = exec {
            let _ = orch.resolve_confirmation(&p.id, true).unwrap();
        }

        // 3. Confirmed tool (rejected)
        orch.llm_engine = Arc::new(ScriptedLlmEngine::new(
            r#"{"name": "file.write", "arguments": {"path": "b.txt", "content": "456"}}"#,
        ));
        let exec2 = orch.execute_turn(&conv_id, "write file 2").unwrap();
        if let TurnExecution::AwaitingConfirmation(p) = exec2 {
            let _ = orch.resolve_confirmation(&p.id, false).unwrap();
        }

        // Query all entries in chronological order and verify the BLAKE3 chain
        let audit = orch.audit_store().unwrap();
        let entries = audit.get_entries(10, 0).unwrap();
        assert_eq!(entries.len(), 5);

        let verification_res = AuditChainVerifier::verify_chain(&entries, &GENESIS_PREV_HASH);
        assert!(verification_res.is_ok());
        assert_eq!(
            verification_res.unwrap(),
            entries.last().unwrap().entry_hash
        );
    }

    #[test]
    fn test_orchestrator_persona_and_preferences_prompt_injection() {
        let (mut orch, conv_id) = setup_test_orchestrator();
        let db = SqliteDb::open_in_memory().unwrap();
        let persona_store = Arc::new(PersonaStore::new(db));

        let custom_persona = aissistant_storage::UserPersona {
            id: "persona_tech".to_string(),
            name: "Senior Architect".to_string(),
            tone: aissistant_storage::PersonaTone::Technical,
            verbosity: aissistant_storage::ResponseVerbosity::Detailed,
            voice_id: "vits_voice".to_string(),
            speaking_rate: 1.0,
            pitch: 1.0,
            system_prompt_override: None,
            is_active: true,
            created_at_ms: 0,
            updated_at_ms: 0,
        };
        persona_store.save_persona(&custom_persona).unwrap();
        persona_store
            .set_preference("code.style", "\"functional\"", "developer")
            .unwrap();

        orch = orch.with_persona_store(persona_store);

        let execution = orch
            .execute_turn(&conv_id, "Explain the architecture")
            .unwrap();
        assert!(matches!(
            execution,
            TurnExecution::Completed(ref summary) if summary.conversation_id == conv_id
        ));
    }
}
