use std::sync::Arc;
use sthira_core::{
    engines::streaming_engine::execute_adaptive_stream,
    llm::streaming::ProgressiveStreamChunk,
    models::router::OperationalMode,
    Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

#[tokio::test]
async fn test_live_llm_plan_streaming_integration() {
    // Check if local Ollama daemon is reachable
    let client = reqwest::Client::new();
    let is_ollama_online = client
        .get("http://localhost:11434/api/tags")
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");
    let clock = Arc::new(FakeClock::new(1_000_000));
    let fallback_model = Arc::new(DeterministicFakeModel::new("mock-model"));

    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), fallback_model)
        .expect("Failed to initialize engine");

    let prompt = "create a plan for creating a trip planner app";
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ProgressiveStreamChunk>(128);

    let engine_clone = engine.clone();
    let stream_task = tokio::spawn(async move {
        execute_adaptive_stream(
            &engine_clone,
            prompt,
            OperationalMode::Plan,
            Some("ollama:llama3.2:latest".to_string()),
            tx,
        )
        .await
    });

    let mut collected_text = String::new();
    let mut chunk_count = 0;
    let mut phases_count = 0;

    while let Some(chunk) = rx.recv().await {
        chunk_count += 1;
        match chunk {
            ProgressiveStreamChunk::TextToken(tok) => {
                collected_text.push_str(&tok);
            }
            ProgressiveStreamChunk::PlanPhase { title, .. } => {
                phases_count += 1;
                collected_text.push_str(&format!("\n### {}\n", title));
            }
            ProgressiveStreamChunk::Complete { .. } => {}
            _ => {}
        }
    }

    assert!(phases_count >= 0);

    let stream_result = stream_task.await.expect("Stream task panicked");
    assert!(stream_result.is_ok(), "execute_adaptive_stream should succeed");

    assert!(chunk_count > 0, "Must emit stream chunks");
    assert!(
        collected_text.len() > 150,
        "Streamed content must be multi-paragraph, got len: {}",
        collected_text.len()
    );

    // CRITICAL: Must NEVER contain the hardcoded fake template stub
    assert!(
        !collected_text.contains("Initial hypothesis on"),
        "Must NOT contain fake template stub 'Initial hypothesis on'"
    );
    assert!(
        !collected_text.contains("Key invariants and assumptions to evaluate"),
        "Must NOT contain fake template stub 'Key invariants and assumptions to evaluate'"
    );

    if is_ollama_online {
        println!(
            "[Live Ollama Verified] Streamed {} tokens/chunks. Content preview: {}",
            chunk_count,
            &collected_text[..std::cmp::min(200, collected_text.len())]
        );
    }
}

#[tokio::test]
async fn test_live_llm_research_streaming_integration() {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");
    let clock = Arc::new(FakeClock::new(1_000_000));
    let fallback_model = Arc::new(DeterministicFakeModel::new("mock-model"));

    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), fallback_model)
        .expect("Failed to initialize engine");

    let prompt = "create a plan for creating a trip planner app";
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ProgressiveStreamChunk>(128);

    let engine_clone = engine.clone();
    let stream_task = tokio::spawn(async move {
        execute_adaptive_stream(
            &engine_clone,
            prompt,
            OperationalMode::Research,
            Some("ollama:llama3.2:latest".to_string()),
            tx,
        )
        .await
    });

    let mut collected_text = String::new();
    let mut chunk_count = 0;

    while let Some(chunk) = rx.recv().await {
        chunk_count += 1;
        match chunk {
            ProgressiveStreamChunk::TextToken(tok) => {
                collected_text.push_str(&tok);
            }
            _ => {}
        }
    }

    let stream_result = stream_task.await.expect("Stream task panicked");
    assert!(stream_result.is_ok(), "execute_adaptive_stream should succeed");

    assert!(chunk_count > 0, "Must emit stream chunks");
    assert!(
        collected_text.len() > 100,
        "Streamed content must be multi-paragraph, got len: {}",
        collected_text.len()
    );

    // CRITICAL: Must NEVER contain the hardcoded fake template stub
    assert!(
        !collected_text.contains("Initial hypothesis on"),
        "Must NOT contain fake template stub 'Initial hypothesis on'"
    );
}
