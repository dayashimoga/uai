//! Main conversational AI orchestrator coordinating memory retrieval, LLM streaming, and turn lifecycle.

use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use aissistant_core::{ConversationId, TurnId};
use aissistant_llm::{
    EmbeddingEngine, FinishReason, LlmEngine, LlmSession, SamplingParams, TokenChunk,
};
use aissistant_speech::{SentenceChunker, TtsEngine};
use aissistant_storage::{
    ConversationStore, ConversationTurn, HybridSearchEngine, TurnMetadata, TurnRole,
    VectorMemoryStore, VectorRecord,
};

use crate::error::{OrchestratorError, Result};
use crate::state::{OrchestratorEvent, TurnState};
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
}

type EventListener = Arc<dyn Fn(&OrchestratorEvent) + Send + Sync>;

/// Central orchestrator coordinating turn state transitions, memory recall, and model inference.
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
            event_listeners: Arc::new(Mutex::new(Vec::new())),
        }
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

    /// Processes a full text turn from user prompt to assistant response and persistence.
    pub fn process_text_turn(
        &self,
        conversation_id: &ConversationId,
        user_text: &str,
    ) -> Result<TurnSummary> {
        let trimmed_text = user_text.trim();
        if trimmed_text.is_empty() {
            return Err(OrchestratorError::Internal(
                "Cannot process empty user prompt".to_string(),
            ));
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

        if !memory_hits.is_empty() {
            assembled_system.push_str("\n\n--- Relevant Prior Memories ---\n");
            for hit in &memory_hits {
                assembled_system.push_str(&format!("- {}\n", hit.snippet));
            }
        }

        // Add conversation history
        for turn in &existing_turns {
            session.add_turn(turn.role.as_str(), &turn.content);
        }
        // Add current user prompt
        session.add_turn("user", trimmed_text);

        let prompt = session.format_chatml_prompt(Some(&assembled_system));

        // Transition 4: PromptAssembly -> Inferring
        self.transition(TurnState::Inferring)?;
        let inference_start = Instant::now();
        let cancel_handle = self.state_machine.cancellation_handle();

        let mut assistant_text = String::new();
        let mut token_index = 0;
        let mut interrupted = false;

        {
            let cancel_ref = Some(cancel_handle.as_ref());
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
                    token: chunk.token,
                    index: token_index,
                });

                token_index += 1;

                if chunk.finish_reason == Some(FinishReason::Cancelled) {
                    interrupted = true;
                }
                Ok(())
            };

            let _ = self.llm_engine.generate_stream(
                &prompt,
                &self.config.sampling_params,
                cancel_ref,
                &mut on_token,
            );
        }

        // Check if interrupted mid-stream
        if interrupted || self.state_machine.cancellation_handle().is_cancelled() {
            let _ = self.state_machine.transition_to(TurnState::Interrupted);
            let metrics = self.state_machine.finalize_metrics();

            let assistant_turn_id = TurnId::generate();
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

            return Ok(TurnSummary {
                assistant_turn_id,
                conversation_id: conversation_id.clone(),
                user_prompt: trimmed_text.to_string(),
                assistant_response: assistant_text,
                memory_hits_count: memory_hits.len(),
                token_count: metrics.token_count,
                latency_ms: metrics.latency_ms.unwrap_or(0),
                ttft_ms: metrics.ttft_ms,
                interrupted: true,
            });
        }

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

        // Transition 6: Inferring/Synthesizing -> TurnCompleted
        self.transition(TurnState::TurnCompleted)?;
        let metrics = self.state_machine.finalize_metrics();

        let assistant_turn_id = TurnId::generate();
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

        Ok(TurnSummary {
            assistant_turn_id,
            conversation_id: conversation_id.clone(),
            user_prompt: trimmed_text.to_string(),
            assistant_response: assistant_text,
            memory_hits_count: memory_hits.len(),
            token_count: metrics.token_count,
            latency_ms: metrics.latency_ms.unwrap_or(0),
            ttft_ms: metrics.ttft_ms,
            interrupted: false,
        })
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use aissistant_llm::{LlmEngineConfig, LocalEmbeddingEngine, LocalLlmEngine};
    use aissistant_storage::SqliteDb;

    fn setup_test_orchestrator() -> (Orchestrator, ConversationId) {
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
        );

        let conv = conv_store
            .create_conversation("Orchestrator Test", None)
            .unwrap();
        (orchestrator, conv.id)
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
            if let Ok(mut g) = events_clone.lock() {
                g.push(event.clone());
            }
        });

        orch.process_text_turn(&conv_id, "Testing event stream")
            .unwrap();

        let events = received_events.lock().unwrap().clone();
        assert!(!events.is_empty());

        // Must have StateChanged events and TokenStreamed events
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

        // Wire event listener to simulate barge-in interruption as soon as the first token arrives
        let orch_clone = orch.clone();
        orch.on_event(move |event| {
            if matches!(event, OrchestratorEvent::TokenStreamed { index: 0, .. }) {
                orch_clone.handle_barge_in();
            }
        });

        // Process turn
        let summary = orch
            .process_text_turn(&conv_id, "Interrupted request")
            .unwrap();

        assert!(summary.interrupted);
        assert_eq!(orch.current_state(), TurnState::Idle);

        // Verify partial assistant turn is recorded as interrupted in SQLite
        let turns = orch.conversation_store.list_turns(&conv_id, None).unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[1].role, TurnRole::Assistant);
        assert_eq!(
            turns[1].metadata.finish_reason.as_deref(),
            Some("interrupted")
        );
    }

    #[test]
    fn test_orchestrator_error_handling() {
        let (orch, _conv_id) = setup_test_orchestrator();

        // 1. Non-existent conversation fails
        let bogus_conv_id = ConversationId::new("non_existent").unwrap();
        let err = orch.process_text_turn(&bogus_conv_id, "Hello").unwrap_err();
        assert!(matches!(
            err,
            OrchestratorError::ConversationNotFound(ref id) if id == bogus_conv_id.as_str()
        ));

        // 2. Empty prompt fails
        let err_empty = orch.process_text_turn(&bogus_conv_id, "   ").unwrap_err();
        assert!(matches!(err_empty, OrchestratorError::Internal(_)));
    }

    #[test]
    fn test_orchestrator_tts_synthesis_flow() {
        use aissistant_speech::AcousticTtsEngine;

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

        let llm: Arc<dyn LlmEngine> = Arc::new(local_llm);
        let tts_engine = AcousticTtsEngine::new(aissistant_speech::TtsConfig::default()).unwrap();
        let tts = Arc::new(Mutex::new(tts_engine));

        let config = OrchestratorConfig {
            synthesize_speech: true,
            ..Default::default()
        };

        let orchestrator = Orchestrator::new(
            config,
            conv_store.clone(),
            vec_store,
            hybrid,
            embed,
            llm,
            Some(tts),
        );

        let conv = conv_store.create_conversation("TTS Test", None).unwrap();
        let summary = orchestrator
            .process_text_turn(&conv.id, "Please speak to me")
            .unwrap();

        assert!(!summary.assistant_response.is_empty());
        assert!(!summary.interrupted);
    }
}
