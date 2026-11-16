#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

//! 1,000-turn continuous soak and stress test for conversational orchestrator and persistence.

use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;

use aissistant_llm::{
    FinishReason, InferenceMetrics, LlmCancellationHandle, LlmEngine, LlmEngineConfig,
    LocalEmbeddingEngine, SamplingParams, TokenChunk,
};
use aissistant_orchestrator::{Orchestrator, OrchestratorConfig, TurnState};
use aissistant_storage::{
    AuditStore, ConversationStore, HybridSearchEngine, SqliteDb, TurnRole, VectorMemoryStore,
};

/// Fast deterministic scripted LLM engine for stress and soak testing.
struct SoakScriptedEngine;

impl LlmEngine for SoakScriptedEngine {
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
        Some("soak-mock-qwen")
    }
    fn generate_stream(
        &self,
        prompt: &str,
        _params: &SamplingParams,
        _cancel_handle: Option<&LlmCancellationHandle>,
        on_token: &mut dyn FnMut(TokenChunk) -> aissistant_llm::Result<()>,
    ) -> aissistant_llm::Result<InferenceMetrics> {
        let first_line = prompt
            .lines()
            .find(|line| line.contains("Turn #"))
            .unwrap_or("Turn response");
        let response = format!("Acknowledged: {first_line}");
        let chunk = TokenChunk::final_chunk(&response, 0, FinishReason::Stop);
        on_token(chunk)?;
        Ok(InferenceMetrics::new(8, 1, 3))
    }
    fn on_memory_pressure(&mut self, _critical: bool) {}
}

#[test]
fn test_1000_turn_continuous_soak_stress() {
    let tmp = tempdir().expect("Create tempdir");
    let db_path = tmp.path().join("soak_test.db");

    // Initialize real file-backed SQLite database
    let db = SqliteDb::open(&db_path).expect("Open file-backed db");
    let conv_store = ConversationStore::new(db.clone());
    let vec_store = VectorMemoryStore::new(db.clone());
    let audit_store = Arc::new(AuditStore::new(db.clone()));
    let hybrid = HybridSearchEngine::new(conv_store.clone(), vec_store.clone());
    let embed = Arc::new(LocalEmbeddingEngine::new(64));
    let llm: Arc<dyn LlmEngine> = Arc::new(SoakScriptedEngine);

    let config = OrchestratorConfig {
        max_memory_excerpts: 2,
        system_prompt: "You are a soak stress test assistant.".to_string(),
        sampling_params: SamplingParams::default(),
        synthesize_speech: false,
    };

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

    let conversation = conv_store
        .create_conversation("1000-Turn Soak Session", None)
        .expect("Create soak conversation");

    let total_turns = 1000;
    let soak_start = Instant::now();
    let mut latencies_micros = Vec::with_capacity(total_turns);

    println!("Starting 1,000-turn continuous soak test...");

    for turn_idx in 1..=total_turns {
        let prompt = format!("Turn #{turn_idx}: check system status and logs.");
        let turn_start = Instant::now();

        let summary = orchestrator
            .process_text_turn(&conversation.id, &prompt)
            .unwrap_or_else(|e| panic!("Turn #{turn_idx} failed: {e}"));

        let turn_duration = turn_start.elapsed();
        latencies_micros.push(turn_duration.as_micros());

        assert_eq!(summary.user_prompt, prompt);
        assert!(!summary.assistant_response.is_empty());
        assert!(summary.token_count > 0);
        assert!(!summary.interrupted);

        // Every 200 turns, trim database memory and assert state machine idle
        if turn_idx % 200 == 0 {
            assert_eq!(
                orchestrator.current_state(),
                TurnState::Idle,
                "Orchestrator must remain idle between turns at checkpoint {turn_idx}"
            );
            db.trim_memory().expect("Trim SQLite memory");
            println!(
                "Completed {turn_idx}/{total_turns} turns. Latest latency: {} µs",
                turn_duration.as_micros()
            );
        }
    }

    let total_elapsed = soak_start.elapsed();
    let avg_latency_micros: u128 =
        latencies_micros.iter().copied().sum::<u128>() / total_turns as u128;

    println!(
        "1,000-turn soak completed in {:.2?} (avg {:.2} µs/turn)",
        total_elapsed, avg_latency_micros
    );

    // Verify state machine state is cleanly Idle
    assert_eq!(orchestrator.current_state(), TurnState::Idle);

    // Verify 2,000 turns stored in DB (1,000 user + 1,000 assistant)
    let turns = conv_store
        .list_turns(&conversation.id, None)
        .expect("List stored turns");
    assert_eq!(
        turns.len(),
        2000,
        "Exactly 2,000 turns must be persisted in SQLite"
    );

    // Verify first and last turn integrity
    assert_eq!(turns[0].role, TurnRole::User);
    assert_eq!(turns[1].role, TurnRole::Assistant);
    assert_eq!(turns[1998].role, TurnRole::User);
    assert_eq!(turns[1999].role, TurnRole::Assistant);

    // Run full SQLite integrity check
    assert!(
        db.check_integrity().expect("Integrity check"),
        "Database must pass quick integrity check after 1,000 turns"
    );
    assert!(
        db.check_full_integrity().expect("Full integrity check"),
        "Database must pass full integrity check after 1,000 turns"
    );
}
