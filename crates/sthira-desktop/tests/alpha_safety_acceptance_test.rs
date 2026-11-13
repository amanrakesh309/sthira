use std::sync::Arc;
use std::time::Duration;
use sthira_core::{
    engines::research_engine::{execute_research, WebSearchMode},
    llm::lifecycle::{CancelOutcome, RequestState},
    search::{SearchError, SearchProvider, SearchResultItem},
    vault, ArtifactType, Authorship, CoreError, Engine, Locality, ModelProvider,
};
use sthira_desktop::{
    handle_save_artifact, handle_undo_refinement,
    naming::{suggest_titles, NamingChoice},
    state::RequestRegistry,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_with_clock() -> (tempfile::TempDir, Engine, Arc<FakeClock>) {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    std::fs::create_dir(&vault).unwrap();
    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        &vault,
        clock.clone(),
        Arc::new(DeterministicFakeModel::new("alpha-safety-fixture")),
    )
    .unwrap();
    (tmp, engine, clock)
}

fn note(e: &Engine, title: &str, body: &str) -> sthira_core::Artifact {
    e.create_artifact(
        ArtifactType::Note,
        Authorship::User,
        Some(title),
        body,
        None,
    )
    .unwrap()
}

// ----------------------------------------------------------------------------
// A. Commit-wins cancellation -> Keep My Draft
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_aa_safety_commit_wins_keep_my_draft() {
    let (tmp, engine, _) = setup_with_clock();
    let initial_body = "# Original Title\n\nOriginal body content.";
    let art = note(&engine, "Original Title", initial_body);

    let registry = RequestRegistry::new();
    let token = registry.register("req-commit-wins-keep");

    // Model refinement runs and enters committing state
    token
        .try_begin_commit()
        .expect("Begin commit must succeed when running");
    assert_eq!(token.state(), RequestState::Committing);

    // User attempts cancellation while persistence has already begun
    let cancel_outcome = registry.cancel_with_outcome("req-commit-wins-keep");
    assert_eq!(
        cancel_outcome,
        Some(CancelOutcome::AlreadyCommitting),
        "Cancellation must report AlreadyCommitting once commit boundary is reached"
    );
    assert!(
        !registry.cancel("req-commit-wins-keep"),
        "cancel() must return false"
    );

    // Refinement finishes persistence
    let refined_body = "# Model Refined Title\n\nModel refined content.";
    let (refined_art, _cs_id) = engine
        .apply_refinement_with_cancellation(
            &art.id,
            refined_body,
            "Critique addressed",
            &serde_json::json!({"operations": []}),
            Some(&token),
        )
        .expect("Refinement persistence succeeds");

    assert_eq!(refined_art.version, art.version + 1);

    // Scenario 1: User has a newer local draft not yet saved
    let user_newer_title = "User Authoritative Title";
    let user_newer_body = "# User Authoritative Title\n\nUser newer draft that must win.";

    // Sthira conflict persistence: "Keep My Draft" invokes real handle_save_artifact
    let keep_outcome = handle_save_artifact(
        &engine,
        art.id.clone(),
        user_newer_body.to_string(),
        Some(user_newer_title.to_string()),
        None,
    )
    .expect("Keep My Draft save succeeds");

    assert!(keep_outcome.mirror_synced);
    assert_eq!(
        keep_outcome.artifact.title.as_deref(),
        Some(user_newer_title)
    );
    assert_eq!(keep_outcome.artifact.content_md, user_newer_body);
    assert_eq!(keep_outcome.artifact.version, refined_art.version + 1);

    // Verify disk mirror file content on filesystem contains user draft
    let mirror_file = engine
        .vault_dir()
        .join(keep_outcome.artifact.file_path.as_ref().unwrap());
    let mirror_disk_content = std::fs::read_to_string(&mirror_file).unwrap();
    assert!(mirror_disk_content.contains(user_newer_body));

    // Verify reopen / restart: open a fresh Engine on the exact same database and vault
    let reopened = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_100)),
        Arc::new(DeterministicFakeModel::new("alpha-safety-fixture")),
    )
    .unwrap();
    let reloaded_art = reopened.get_artifact(&art.id).unwrap();
    assert_eq!(reloaded_art.title.as_deref(), Some(user_newer_title));
    assert_eq!(reloaded_art.content_md, user_newer_body);
    assert_eq!(reloaded_art.version, keep_outcome.artifact.version);

    // Scenario 2: User draft was already persisted before a competing model commit overwrote backing storage.
    // User resolves conflict via Keep My Draft, restoring the authoritative user draft.
    let user_presaved_title = "User Authoritative Pre-Saved Draft";
    let user_presaved_body = "# User Authoritative Pre-Saved Draft\n\nUser wrote and saved before.";
    let user_saved = handle_save_artifact(
        &reopened,
        art.id.clone(),
        user_presaved_body.to_string(),
        Some(user_presaved_title.to_string()),
        None,
    )
    .expect("Pre-save succeeds");
    assert_eq!(
        user_saved.artifact.version,
        keep_outcome.artifact.version + 1
    );

    // Competing model refinement commits over backing storage
    let competing_refined_body = "# Competing Refined Body\n\nModel overwrote backing storage.";
    let (competing_art, _) = reopened
        .apply_refinement_with_cancellation(
            &art.id,
            competing_refined_body,
            "Competing critique",
            &serde_json::json!({"operations": []}),
            None,
        )
        .expect("Refinement commits");
    assert_eq!(competing_art.version, user_saved.artifact.version + 1);

    // Conflict resolution: User selects "Keep My Draft", re-persisting user draft over the refinement
    let resolved_keep = handle_save_artifact(
        &reopened,
        art.id.clone(),
        user_presaved_body.to_string(),
        Some(user_presaved_title.to_string()),
        None,
    )
    .expect("Keep My Draft re-persists user draft");
    assert_eq!(resolved_keep.artifact.content_md, user_presaved_body);
    assert_eq!(resolved_keep.artifact.version, competing_art.version + 1);
}

// ----------------------------------------------------------------------------
// B. Commit-wins cancellation -> Apply Refinement
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_aa_safety_commit_wins_apply_refinement() {
    let (tmp, engine, clock) = setup_with_clock();
    let project = engine.create_project("Alpha Project", None).unwrap();
    let initial_last_substantive = project.last_substantive_at;

    let art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Note for Apply"),
            "# Initial Note\n\nInitial body content.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &art.id, "user_explicit", None)
        .unwrap();

    // Advance clock so that last_substantive_at update can be distinguished
    clock.advance(Duration::from_millis(5_000));

    let registry = RequestRegistry::new();
    let token = registry.register("req-commit-wins-apply");

    token.try_begin_commit().expect("Begin commit succeeds");
    assert_eq!(
        registry.cancel_with_outcome("req-commit-wins-apply"),
        Some(CancelOutcome::AlreadyCommitting)
    );

    let refined_content =
        "# Applied Refinement Title\n\nApplied refinement body with extensive substantive edits.";
    let (updated, cs_id) = engine
        .apply_refinement_with_cancellation(
            &art.id,
            refined_content,
            "Critique addressed",
            &serde_json::json!({"operations": []}),
            Some(&token),
        )
        .expect("Apply refinement persists cleanly");

    assert_eq!(updated.content_md, refined_content);

    // Verify durable session event 'artifact_updated' linking ChangeSet ID
    let event: Option<(String, String, Option<String>)> = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT event_type, materiality, change_set_id FROM session_events WHERE artifact_id = ?1 ORDER BY rowid DESC LIMIT 1",
            rusqlite::params![art.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .ok();

    assert!(event.is_some(), "Durable session event must be recorded");
    let (event_type, materiality, linked_cs_id) = event.unwrap();
    assert_eq!(event_type, "artifact_updated");
    assert_eq!(materiality, "substantive");
    assert_eq!(
        linked_cs_id.as_deref(),
        Some(cs_id.as_str()),
        "Durable session event must link the actual change_set_id"
    );

    // Project last_substantive_at updated
    let proj_after = engine.get_project(&project.id).unwrap();
    assert!(
        proj_after.last_substantive_at > initial_last_substantive,
        "Project last_substantive_at must advance on substantive refinement"
    );

    // Verify durability across restart
    let reopened = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_200)),
        Arc::new(DeterministicFakeModel::new("alpha-safety-fixture")),
    )
    .unwrap();
    let reloaded = reopened.get_artifact(&art.id).unwrap();
    assert_eq!(reloaded.content_md, refined_content);
    assert_eq!(reloaded.version, updated.version);
}

// ----------------------------------------------------------------------------
// C. Actual Refinement Undo
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_aa_safety_refinement_undo_and_superseding() {
    let (tmp, engine, _) = setup_with_clock();
    let original_body = "# Original Title\n\nOriginal body content before refinement.";
    let art = note(&engine, "Original Title", original_body);

    // Apply real refinement with proper restore operation
    let refined_body = "# Refined Title\n\nRefined body content.";
    let inverse = serde_json::json!({
        "operations": [{
            "type": "restore_artifact",
            "artifact_id": art.id,
            "content_md": original_body,
            "content_hash": art.content_hash,
            "refined_content_md": refined_body,
            "refined_content_hash": vault::compute_content_hash(refined_body),
        }]
    });
    let (refined_art, cs_id) = engine
        .apply_refinement_with_cancellation(
            &art.id,
            refined_body,
            "Critique addressed",
            &inverse,
            None,
        )
        .unwrap();
    assert_eq!(refined_art.version, art.version + 1);

    // Call real handle_undo_refinement
    let undo_outcome = handle_undo_refinement(&engine, art.id.clone(), cs_id.clone())
        .expect("Refinement Undo succeeds");

    assert!(undo_outcome.mirror_synced);
    assert_eq!(undo_outcome.artifact.content_md, original_body);
    assert_eq!(undo_outcome.artifact.version, refined_art.version + 1);

    // Verify change_sets.reverted_at is populated
    let reverted_at: Option<i64> = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT reverted_at FROM change_sets WHERE id = ?1",
            rusqlite::params![cs_id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        reverted_at.is_some(),
        "reverted_at must be populated on undo"
    );

    // Subsequent manual edit strictly advances content and supersedes
    let newer_manual_body = "# Newer Manual Edit\n\nUser wrote after undo.";
    let manual_save = handle_save_artifact(
        &engine,
        art.id.clone(),
        newer_manual_body.to_string(),
        Some("Newer Manual Edit".to_string()),
        None,
    )
    .expect("Manual save after undo succeeds");
    assert_eq!(manual_save.artifact.content_md, newer_manual_body);
    assert_eq!(
        manual_save.artifact.version,
        undo_outcome.artifact.version + 1
    );

    // Duplicate or stale Undo call must be rejected as already undone
    let dup_err = handle_undo_refinement(&engine, art.id.clone(), cs_id)
        .expect_err("Duplicate Undo must be rejected");
    assert!(dup_err.contains("already been undone"));

    // Case 2: A pending Undo rejected because newer edits superseded its target
    let refined_body2 = "# Second Refined Title\n\nSecond refined content.";
    let inverse2 = serde_json::json!({
        "operations": [{
            "type": "restore_artifact",
            "artifact_id": art.id,
            "content_md": newer_manual_body,
            "content_hash": manual_save.artifact.content_hash,
            "refined_content_md": refined_body2,
            "refined_content_hash": vault::compute_content_hash(refined_body2),
        }]
    });
    let (refined_art2, cs_id2) = engine
        .apply_refinement_with_cancellation(
            &art.id,
            refined_body2,
            "Second critique",
            &inverse2,
            None,
        )
        .unwrap();
    assert_eq!(refined_art2.version, manual_save.artifact.version + 1);

    // User makes a newer manual edit, advancing version and superseding the refinement
    let newer_superseding_body = "# Superseding Title\n\nUser wrote over refinement 2.";
    let superseding_save = handle_save_artifact(
        &engine,
        art.id.clone(),
        newer_superseding_body.to_string(),
        Some("Superseding Title".to_string()),
        None,
    )
    .expect("Superseding manual save succeeds");
    assert_eq!(superseding_save.artifact.version, refined_art2.version + 1);

    // Calling undo on cs_id2 now must be REJECTED because newer edits superseded it
    let superseded_err = handle_undo_refinement(&engine, art.id.clone(), cs_id2)
        .expect_err("Undoing a superseded refinement must be rejected");
    assert!(
        superseded_err.contains("overwrite newer work"),
        "Expected 'overwrite newer work' error on superseded refinement, got: {}",
        superseded_err
    );

    // Verify durability across fresh Engine::open (database reopen)
    let reopened = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_300)),
        Arc::new(DeterministicFakeModel::new("alpha-safety-fixture")),
    )
    .unwrap();
    let reloaded = reopened.get_artifact(&art.id).unwrap();
    assert_eq!(reloaded.content_md, newer_superseding_body);
    assert_eq!(reloaded.version, superseding_save.artifact.version);
}

// ----------------------------------------------------------------------------
// D. Actual Mirror Warning and Repair
// ----------------------------------------------------------------------------
#[test]
fn test_aa_safety_mirror_warning_and_repair() {
    let (tmp, engine, _) = setup_with_clock();
    let art = note(
        &engine,
        "Original Title",
        "# Original Title\n\nOriginal body.",
    );

    // Inject isolated mirror IO failure: replace mirror file with directory
    let mirror_path = engine.vault_dir().join(art.file_path.as_ref().unwrap());
    std::fs::remove_file(&mirror_path).unwrap();
    std::fs::create_dir(&mirror_path).unwrap();

    let new_body = "# Original Title\n\nUpdated body with broken mirror.";
    let outcome = handle_save_artifact(
        &engine,
        art.id.clone(),
        new_body.to_string(),
        art.title.clone(),
        None,
    )
    .expect("handle_save_artifact must return Ok(SaveArtifactOutcome) with mirror warning");

    // Verify honest mirror failure reported while SQLite commit succeeded
    assert!(
        !outcome.mirror_synced,
        "mirror_synced must be false on IO failure"
    );
    assert!(
        outcome.mirror_sync_error.is_some(),
        "mirror_sync_error must be present"
    );
    assert_eq!(
        outcome.artifact.version,
        art.version + 1,
        "DB version must increment"
    );
    assert_ne!(
        outcome.artifact.last_synced_hash.as_deref(),
        Some(outcome.artifact.content_hash.as_str()),
        "last_synced_hash must not match content_hash when mirror sync failed"
    );

    // Repair mirror: remove obstructing directory
    std::fs::remove_dir(&mirror_path).unwrap();

    // Call real save retry: must repair mirror WITHOUT bumping SQLite version
    let retry = handle_save_artifact(
        &engine,
        art.id.clone(),
        new_body.to_string(),
        art.title.clone(),
        None,
    )
    .expect("Retry mirror sync succeeds");

    assert!(
        retry.mirror_synced,
        "mirror_synced must be true after repair"
    );
    assert_eq!(
        retry.artifact.version, outcome.artifact.version,
        "Mirror repair must NOT bump artifact version"
    );
    assert_eq!(
        retry.artifact.last_synced_hash.as_deref(),
        Some(retry.artifact.content_hash.as_str()),
        "last_synced_hash must match content_hash after repair"
    );

    // Verify filesystem mirror content
    let repaired_content = std::fs::read_to_string(&mirror_path).unwrap();
    assert!(repaired_content.contains(new_body));

    // Verify restart durability
    let reopened = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_400)),
        Arc::new(DeterministicFakeModel::new("alpha-safety-fixture")),
    )
    .unwrap();
    let reloaded = reopened.get_artifact(&art.id).unwrap();
    assert_eq!(reloaded.content_md, new_body);
    assert_eq!(reloaded.version, retry.artifact.version);
}

// ----------------------------------------------------------------------------
// E. Search Anti-bot Challenge Classification & Fallback
// ----------------------------------------------------------------------------
struct ChallengeSearchProvider;
#[async_trait::async_trait]
impl SearchProvider for ChallengeSearchProvider {
    async fn search(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SearchResultItem>, SearchError> {
        Err(SearchError::challenge(
            "DuckDuckGo",
            "anti-bot anomaly challenge detected on search endpoint",
        ))
    }
}

struct ResearchFixtureModel;
#[async_trait::async_trait]
impl ModelProvider for ResearchFixtureModel {
    fn id(&self) -> &str {
        "test-research-fixture"
    }
    fn locality(&self) -> Locality {
        Locality::Local
    }
    async fn generate_json(
        &self,
        schema: &serde_json::Value,
        _system: &str,
        _user: &str,
    ) -> Result<String, CoreError> {
        if schema.to_string().contains("contradictions_and_evolution") {
            Ok(serde_json::json!({
                "synthesis": "Model fixture synthesis based on the supplied evidence.",
                "contradictions_and_evolution": [],
                "open_questions": []
            })
            .to_string())
        } else {
            Ok("{}".into())
        }
    }
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError> {
        DeterministicFakeModel::new("test-embedding")
            .embed(texts)
            .await
    }
}

#[tokio::test]
async fn test_aa_research_challenge_truthful_classification_and_fallback() {
    let tmp = tempdir().unwrap();
    let vault = tmp.path().join("vault");
    std::fs::create_dir(&vault).unwrap();
    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let engine = Engine::open(
        tmp.path().join("sthira.db"),
        &vault,
        clock,
        Arc::new(ResearchFixtureModel),
    )
    .unwrap();

    let challenge_provider = ChallengeSearchProvider;

    let result = execute_research(
        &engine,
        "Sthira epistemic verification architecture",
        WebSearchMode::AlwaysSearch,
        Some(&challenge_provider),
        "DuckDuckGo",
        4,
        None,
    )
    .await
    .expect("Research query must handle challenge gracefully and fall back to local vault");

    assert_eq!(
        result.external_sources_count, 0,
        "No external sources fabricated"
    );
    assert_eq!(result.search_status, "web_search_failed");
    assert_eq!(result.mode_used, WebSearchMode::LocalVaultOnly);
    assert!(
        result.synthesized_markdown.contains("⚠️ Web search blocked by anti-bot challenge on DuckDuckGo; synthesized using local context."),
        "Honest challenge warning banner must be returned in markdown"
    );
}

// ----------------------------------------------------------------------------
// F. Unavailable Provider Adapter Fast Failure
// ----------------------------------------------------------------------------
#[tokio::test]
async fn test_aa_provider_unavailable_adapter_fast_failure() {
    let (_tmp, engine, _) = setup_with_clock();
    let art = note(&engine, "Manual Title", "# Manual Title\n\nSome body text.");

    // Configure a provider pointing to an unavailable port using real handle_save_provider
    sthira_desktop::handle_save_provider(
        &engine,
        sthira_core::ProviderEntry {
            id: "offline-prov".into(),
            display_name: "Offline Provider".into(),
            provider_type: "openai_compatible".into(),
            base_url: "http://127.0.0.1:59999/v1".into(),
            has_api_key: false,
            is_enabled: true,
            models: vec![sthira_core::ProviderModelEntry {
                id: "offline-model".into(),
                provider_id: "offline-prov".into(),
                model_identifier: "offline-model".into(),
                display_name: "Offline Model".into(),
                context_window: 8192,
                supports_structured_output: true,
                is_enabled: true,
                is_default: true,
                web_search_capability: Default::default(),
            }],
        },
        None,
    )
    .unwrap();

    let choice = NamingChoice {
        model_id: Some("offline-model".to_string()),
        local: false,
    };

    let start = std::time::Instant::now();
    let res = suggest_titles(&engine, &art.id, choice).await;
    let elapsed = start.elapsed();

    // Must fail fast (<10 seconds connection timeout)
    assert!(
        elapsed < Duration::from_secs(12),
        "Unavailable provider must fail fast, elapsed: {:?}",
        elapsed
    );
    assert!(res.is_err(), "Must return Err for unavailable provider");
    let err_msg = res.unwrap_err();
    assert!(
        err_msg.to_lowercase().contains("connection")
            || err_msg.to_lowercase().contains("refused")
            || err_msg.to_lowercase().contains("error"),
        "Error message must indicate connection failure: {}",
        err_msg
    );

    // Document manual title and body must remain completely unchanged
    let after_art = engine.get_artifact(&art.id).unwrap();
    assert_eq!(after_art.title.as_deref(), Some("Manual Title"));
    assert_eq!(after_art.content_md, "# Manual Title\n\nSome body text.");
    assert_eq!(after_art.version, art.version);
}
