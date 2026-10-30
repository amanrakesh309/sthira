use regex::Regex;
use rusqlite::params;
use std::sync::Arc;
use std::time::Duration;
use sthira_core::epistemic::EpistemicOrigin;
use sthira_core::memory::{
    compute_basis_fingerprint, ungrounded_tokens, visible_words, PaneHintReason, ResumePaneHint,
};
use sthira_core::{Actor, ArtifactType, Authorship, Engine, EventType, Materiality};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (Engine, Arc<FakeClock>, Arc<DeterministicFakeModel>, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");
    let clock = Arc::new(FakeClock::new(1_000_000));
    let model = Arc::new(DeterministicFakeModel::new("mock-memory-model"));
    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), model.clone())
        .expect("Failed to initialize engine");
    (engine, clock, model, dir)
}

fn assert_no_guilt_language(text: &str) {
    let guilt_re = Regex::new(r"(?i)\b(you haven't|haven't you|overdue|behind schedule|you didn't|slacking|streak)\b").unwrap();
    assert!(
        !guilt_re.is_match(text),
        "Guilt or pressure language detected in user-facing copy: '{}'",
        text
    );
}

#[tokio::test]
async fn test_4_1_scenario_a_gold_standard_cold_return() {
    let (engine, clock, _model, dir) = setup_test_engine();

    let proj = engine
        .create_project("Local Coding Model Research", None)
        .expect("create proj failed");

    // Add note and decision
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Evaluation criteria"),
            "We decided to drop 30B+ models due to memory limits. Focused on compact checkpoints.",
            None,
        )
        .expect("create note failed");
    engine
        .attach_artifact(&proj.id, &note.id, "user_explicit", None)
        .expect("attach failed");

    // Extract node
    let writer = engine.db().writer();
    writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('node_30b', ?1, 'decision', 'user_recorded', 'Dropped 30B+ models', 'We decided to drop 30B+ models due to memory limits.', 0, 52, 'qh_30b', 'bh1', 'run1', 'm1', 1000, 1000);",
        params![note.id],
    ).expect("insert node failed");
    drop(writer);

    // Advance clock by 72 hours (Cold Return)
    clock.advance(Duration::from_secs(72 * 3600));

    let hint = engine.get_resume_pane_hint(&proj.id).expect("get hint failed");
    assert_eq!(
        hint,
        ResumePaneHint::Show {
            reason: PaneHintReason::ColdReturn
        }
    );

    let brief = engine.get_resume_brief(&proj.id, 1).await.expect("get brief failed");

    // Assert word cap <= 75 words
    let words = visible_words(&brief);
    assert!(
        words <= 75,
        "Level 1 brief must be capped at <= 75 words, got {}",
        words
    );

    // Cites valid artifacts
    assert!(!brief.recent_progress.is_empty());
    for claim in &brief.recent_progress {
        assert_no_guilt_language(&claim.text);
        assert!(!claim.artifact_ids.is_empty());
    }

    // Verify markdown mirror was written
    let mirror_file = dir
        .path()
        .join(".sthira_meta")
        .join("memory_mirrors")
        .join("projects")
        .join(format!("{}.md", proj.id));
    assert!(mirror_file.exists(), "Memory mirror markdown file must exist");
    let mirror_content = std::fs::read_to_string(&mirror_file).expect("read mirror failed");
    assert!(mirror_content.contains("<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST"));
    assert_no_guilt_language(&mirror_content);
}

#[tokio::test]
async fn test_4_2_scenario_b_view_and_touch_edit_trap() {
    let (engine, clock, _model, _dir) = setup_test_engine();

    let proj = engine
        .create_project("Resumption Trap Test", None)
        .expect("create proj failed");

    // Session 1: Deep substantive work on art_07
    let art_07 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Deep Work Note 07"),
            "Detailed research findings on concurrency models.",
            None,
        )
        .expect("create art 07 failed");
    engine
        .attach_artifact(&proj.id, &art_07.id, "user_explicit", None)
        .expect("attach failed");

    engine
        .log_event(
            Some(&proj.id),
            Some(&art_07.id),
            Actor::User,
            EventType::ArtifactUpdated,
            Materiality::Substantive,
            serde_json::json!({}),
        )
        .expect("log event failed");

    // Advance clock past idle gap into Session 2
    clock.advance(Duration::from_secs(40 * 60));

    // Session 2: View art_01, minor typo edit on art_02
    let art_01 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Note 01"),
            "Initial background notes.",
            None,
        )
        .expect("create art 01 failed");
    engine
        .attach_artifact(&proj.id, &art_01.id, "user_explicit", None)
        .expect("attach failed");

    let art_02 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Note 02"),
            "Teh initial note.",
            None,
        )
        .expect("create art 02 failed");
    engine
        .attach_artifact(&proj.id, &art_02.id, "user_explicit", None)
        .expect("attach failed");

    engine
        .log_event(
            Some(&proj.id),
            Some(&art_01.id),
            Actor::User,
            EventType::ArtifactViewed,
            Materiality::None,
            serde_json::json!({}),
        )
        .expect("view event failed");

    engine
        .log_event(
            Some(&proj.id),
            Some(&art_02.id),
            Actor::User,
            EventType::ArtifactUpdated,
            Materiality::Minor,
            serde_json::json!({}),
        )
        .expect("minor update event failed");

    let brief = engine.get_resume_brief(&proj.id, 1).await.expect("brief failed");

    // Primary continuation must remain art_07 (where deep substantive work occurred)
    assert_eq!(
        brief.primary_continuation_artifact_id,
        Some(art_07.id),
        "Continuation point must not be fooled by views or minor typo edits"
    );
}

#[tokio::test]
async fn test_4_3_scenario_c_sparse_evidence_no_guilt() {
    let (engine, _clock, _model, _dir) = setup_test_engine();

    // Project (a): 2 saved sources and 0 notes
    let proj_a = engine
        .create_project("Distributed KV Research", None)
        .expect("create proj failed");

    let s1 = engine
        .create_artifact(
            ArtifactType::Source,
            Authorship::External,
            Some("Raft paper"),
            "In search of an understandable consensus algorithm.",
            Some("https://raft.github.io"),
        )
        .expect("create s1 failed");
    let s2 = engine
        .create_artifact(
            ArtifactType::Source,
            Authorship::External,
            Some("Paxos Made Simple"),
            "The Paxos algorithm for consensus.",
            Some("https://paxos.org"),
        )
        .expect("create s2 failed");

    engine
        .attach_artifact(&proj_a.id, &s1.id, "user_explicit", None)
        .expect("attach failed");
    engine
        .attach_artifact(&proj_a.id, &s2.id, "user_explicit", None)
        .expect("attach failed");

    let brief_a = engine.get_resume_brief(&proj_a.id, 1).await.expect("brief failed");
    assert_eq!(brief_a.recent_progress.len(), 1);
    let copy = &brief_a.recent_progress[0].text;

    assert!(
        copy.contains("2 saved sources about Distributed KV Research"),
        "Must match neutral sparse copy template, got: '{}'",
        copy
    );
    assert!(
        copy.contains("Reviewing the sources is a good place to start"),
        "Must provide gentle next action"
    );
    assert_no_guilt_language(copy);

    // Project (b): 0 sources and purpose only
    let proj_b = engine
        .create_project("Empty Purpose Project", None)
        .expect("create proj failed");
    let brief_b = engine.get_resume_brief(&proj_b.id, 1).await.expect("brief failed");
    for claim in &brief_b.recent_progress {
        assert_no_guilt_language(&claim.text);
    }
}

#[tokio::test]
async fn test_4_4_scenario_d_metamorphic_inference_flip() {
    let (engine, _clock, _model, _dir) = setup_test_engine();

    let proj = engine
        .create_project("Model Selection Project", None)
        .expect("create proj failed");

    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Comparative Thoughts"),
            "Preliminary observation: Architecture A shows promising throughput.",
            None,
        )
        .expect("create note failed");
    engine
        .attach_artifact(&proj.id, &note.id, "user_explicit", None)
        .expect("attach failed");

    // Add unreviewed AI hypothesis for Option A
    let writer = engine.db().writer();
    writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('hyp_a', ?1, 'hypothesis', 'ai_inferred', 'Leaning toward Architecture A', 'Architecture A shows promising throughput.', 0, 43, 'qh_a', 'bh1', 'run1', 'm1', 100, 100);",
        params![note.id],
    ).expect("insert node failed");
    drop(writer);

    let brief_1 = engine.get_resume_brief(&proj.id, 1).await.expect("brief failed");
    assert_eq!(brief_1.recent_progress.len(), 1);
    assert_eq!(brief_1.recent_progress[0].origin, EpistemicOrigin::AiInferred);
    assert!(brief_1.recent_progress[0].text.contains("Architecture A"));

    // Mutate to Architecture B
    let writer = engine.db().writer();
    writer.execute(
        "UPDATE epistemic_nodes SET statement = 'Leaning toward Architecture B', quote = 'Architecture B shows promising throughput.', quote_hash = 'qh_b', updated_at = 200 WHERE id = 'hyp_a';",
        [],
    ).expect("update node failed");
    drop(writer);

    let brief_2 = engine.get_resume_brief(&proj.id, 1).await.expect("brief failed");
    assert!(brief_2.recent_progress[0].text.contains("Architecture B"));
}

#[tokio::test]
async fn test_4_5_scenario_e_conflicting_decisions_and_supersession() {
    let (engine, clock, _model, _dir) = setup_test_engine();

    let proj = engine
        .create_project("Architecture Decisions", None)
        .expect("create proj failed");

    let note_1 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Day 2 Decision"),
            "We decided to use single-node SQLite for storage.",
            None,
        )
        .expect("create art failed");
    engine
        .attach_artifact(&proj.id, &note_1.id, "user_explicit", None)
        .expect("attach failed");

    let writer = engine.db().writer();
    writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('dec_1', ?1, 'decision', 'user_recorded', 'Decision: SQLite single node', 'We decided to use single-node SQLite for storage.', 0, 48, 'qh1', 'bh1', 'r1', 'm1', 2 * 86400 * 1000, 2 * 86400 * 1000);",
        params![note_1.id],
    ).expect("insert dec 1 failed");
    drop(writer);

    clock.advance(Duration::from_secs(2 * 86400)); // Day 4

    let note_2 = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Day 4 Decision"),
            "We decided to switch to distributed Raft storage.",
            None,
        )
        .expect("create art failed");
    engine
        .attach_artifact(&proj.id, &note_2.id, "user_explicit", None)
        .expect("attach failed");

    let writer = engine.db().writer();
    writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('dec_2', ?1, 'decision', 'user_recorded', 'Decision: distributed Raft', 'We decided to switch to distributed Raft storage.', 0, 48, 'qh2', 'bh2', 'r2', 'm1', 4 * 86400 * 1000, 4 * 86400 * 1000);",
        params![note_2.id],
    ).expect("insert dec 2 failed");
    drop(writer);

    // Initial brief: both conflicting decisions appear
    let brief = engine.get_resume_brief(&proj.id, 1).await.expect("brief failed");
    assert_eq!(brief.conflicting_decisions.len(), 2, "Both decisions must appear while conflicting");

    // Verify ai_proposal was created
    let reader = engine.db().reader().expect("reader failed");
    let prop_id: String = reader
        .query_row(
            "SELECT id FROM ai_proposals WHERE kind = 'supersede_decision' AND state = 'pending';",
            [],
            |r| r.get(0),
        )
        .expect("query proposal failed");

    // Accept proposal to supersede decision 1 with decision 2
    engine.decide_proposal(&prop_id, "accepted").expect("accept proposal failed");

    let brief_after = engine.get_resume_brief(&proj.id, 1).await.expect("brief after failed");
    assert_eq!(
        brief_after.conflicting_decisions.len(),
        0,
        "Conflicting decisions cleared after resolution"
    );
}

#[tokio::test]
async fn test_4_6_scenario_f_fingerprint_cache_hit_and_disk_invalidation() {
    let (engine, _clock, _model, dir) = setup_test_engine();
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");

    let proj = engine
        .create_project("Cache Hit Project", None)
        .expect("create proj failed");

    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Note for Cache"),
            "We decided to use local embeddings for indexing.",
            None,
        )
        .expect("create note failed");
    engine
        .attach_artifact(&proj.id, &note.id, "user_explicit", None)
        .expect("attach failed");

    let writer = engine.db().writer();
    writer.execute(
        "INSERT INTO epistemic_nodes (id, artifact_id, node_type, origin, statement, quote, quote_start, quote_end, quote_hash, basis_content_hash, extraction_run_id, model_id, first_seen_at, updated_at)
         VALUES ('dec_cache', ?1, 'decision', 'user_recorded', 'Decision: local embeddings', 'We decided to use local embeddings for indexing.', 0, 48, 'qh_c', 'bh1', 'r1', 'm1', 100, 100);",
        params![note.id],
    ).expect("insert failed");
    drop(writer);

    let fp1 = compute_basis_fingerprint(engine.db(), &proj.id).expect("fp1 failed");
    let brief1 = engine.get_resume_brief(&proj.id, 1).await.expect("b1 failed");

    // Second call: fingerprint matches, cached in derived_briefs
    let fp2 = compute_basis_fingerprint(engine.db(), &proj.id).expect("fp2 failed");
    assert_eq!(fp1, fp2);
    let brief2 = engine.get_resume_brief(&proj.id, 1).await.expect("b2 failed");
    assert_eq!(brief1.purpose, brief2.purpose);

    // Edit artifact on disk: delete decision sentence
    let disk_file = vault_dir.join(note.file_path.as_ref().unwrap());
    let modified = format!(
        "---\nid: \"{}\"\ntitle: \"Note for Cache\"\ntype: \"note\"\nauthorship: \"user\"\n---\nOnly peripheral comments remain.",
        note.id
    );
    std::fs::write(&disk_file, modified).expect("write failed");

    engine.reconcile_vault().expect("reconcile failed");

    let fp3 = compute_basis_fingerprint(engine.db(), &proj.id).expect("fp3 failed");
    assert_ne!(
        fp1, fp3,
        "Basis fingerprint must automatically change upon disk edit"
    );
}

#[test]
fn test_4_7_scenario_g_poisoned_citation_guard() {
    let prose_safe = "We benchmarked on the local cluster and observed steady latency.";
    let prose_poisoned = "We deployed 32GB memory with ModelX on Version 4.";

    let cited_quotes = [
        "Testing on the local cluster showed steady latency across runs.",
    ];

    let ungrounded_safe = ungrounded_tokens(prose_safe, &cited_quotes);
    assert!(ungrounded_safe.is_empty(), "Grounded prose must pass");

    let ungrounded_bad = ungrounded_tokens(prose_poisoned, &cited_quotes);
    assert!(
        !ungrounded_bad.is_empty(),
        "Hallucinated numbers and identifiers must be rejected as ungrounded"
    );
    assert!(ungrounded_bad.iter().any(|t| t == "32GB" || t == "32"));
    assert!(ungrounded_bad.iter().any(|t| t == "ModelX"));
}

#[tokio::test]
async fn test_4_8_scenario_h_home_state_ordering() {
    let (engine, clock, _model, _dir) = setup_test_engine();

    // Create 2 projects
    let p1 = engine.create_project("Project Older", None).expect("p1 failed");
    clock.advance(Duration::from_secs(60));
    let p2 = engine.create_project("Project Newer", None).expect("p2 failed");

    // Create loose capture (not attached to any project)
    let _loose = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Spontaneous loose thought"),
            "Loose thought captured without project context.",
            None,
        )
        .expect("loose failed");

    let home = engine.get_home_state().expect("get home state failed");

    assert_eq!(home.threads.len(), 2);
    // Ordered by last_substantive_at DESC: p2 before p1
    assert_eq!(home.threads[0].project_id, p2.id);
    assert_eq!(home.threads[1].project_id, p1.id);

    // Loose captures separated
    assert_eq!(home.loose_captures.len(), 1);
    assert_eq!(
        home.loose_captures[0].title.as_deref(),
        Some("Spontaneous loose thought")
    );
}
