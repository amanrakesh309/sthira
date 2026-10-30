use regex::Regex;
use rusqlite::params;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sthira_core::epistemic::EpistemicOrigin;
use sthira_core::memory::{visible_words, PaneHintReason, ResumePaneHint};
use sthira_core::{
    Actor, ArtifactType, Authorship, Clock, Engine, EventType, IntentType, Materiality,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

fn setup_test_engine() -> (Engine, Arc<FakeClock>, Arc<DeterministicFakeModel>, tempfile::TempDir) {
    let dir = tempdir().expect("Failed to create tempdir");
    let db_path = dir.path().join("sthira.db");
    let vault_dir = dir.path().join(".sthira_vault").join("artifacts");
    let clock = Arc::new(FakeClock::new(100_000_000));
    let model = Arc::new(DeterministicFakeModel::new("mock-lifecycle-model"));
    let engine = Engine::open(&db_path, &vault_dir, clock.clone(), model.clone())
        .expect("Failed to initialize engine");
    (engine, clock, model, dir)
}

fn assert_no_guilt_language(text: &str) {
    let guilt_re = Regex::new(
        r"(?i)\b(you haven't|haven't you|overdue|behind schedule|you didn't|slacking|streak|catch up)\b",
    )
    .unwrap();
    assert!(
        !guilt_re.is_match(text),
        "Guilt or pressure language detected in user-facing copy: '{}'",
        text
    );
}

#[tokio::test]
async fn test_7_1_fourteen_day_end_to_end_lifecycle() {
    let (engine, clock, model, dir) = setup_test_engine();

    // =========================================================================
    // Day 1 (Path A — Spontaneous Capture):
    // User captures a loose thought on small fine-tuned models for test generation.
    // Saved globally (project_ids: [], authorship = 'user').
    // =========================================================================
    let day1_note_content =
        "Small fine-tuned models might be fast enough for local unit test generation.";
    let day1_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Small Fine-Tuned Models"),
            day1_note_content,
            None,
        )
        .expect("Day 1: failed to create loose note");

    // Verify it is not attached to any project
    let reader = engine.db().reader().expect("reader failed");
    let attached_count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM project_artifacts WHERE artifact_id = ?1;",
            params![day1_note.id],
            |r| r.get(0),
        )
        .expect("count failed");
    assert_eq!(attached_count, 0, "Day 1 note must be unattached");

    // Verify it surfaces in HomeState as a loose capture
    let home_state_day1 = engine.get_home_state().expect("get home state failed");
    assert_eq!(home_state_day1.loose_captures.len(), 1);
    assert_eq!(home_state_day1.loose_captures[0].id, day1_note.id);

    // =========================================================================
    // Day 2 (Organic Promotion + Path B):
    // User promotes Day 1 note into proj_coding ("Local Coding Model Research").
    // Discovery links relevant sources.
    // =========================================================================
    clock.advance(Duration::from_secs(24 * 3600)); // Advance to Day 2

    let proj_coding = engine
        .create_project("Local Coding Model Research", None)
        .expect("Day 2: failed to create project");
    engine
        .set_project_purpose(
            &proj_coding.id,
            "Evaluate local coding models under 16GB memory limit for unit test generation.",
        )
        .expect("Day 2: set purpose failed");

    // Promote Day 1 note into proj_coding
    engine
        .attach_artifact(&proj_coding.id, &day1_note.id, "user_explicit", None)
        .expect("Day 2: attach note failed");

    // Discovery links relevant external source
    let source_day2 = engine
        .create_artifact(
            ArtifactType::Source,
            Authorship::External,
            Some("Test Generation Benchmarks"),
            "Paper on small model test generation latency and pass rates.",
            Some("https://arxiv.org/abs/2401.00001"),
        )
        .expect("Day 2: create source failed");
    engine
        .attach_artifact(&proj_coding.id, &source_day2.id, "user_explicit", None)
        .expect("Day 2: attach source failed");

    // Verify Day 1 note is no longer in loose captures
    let home_state_day2 = engine.get_home_state().expect("get home state failed");
    assert_eq!(
        home_state_day2.loose_captures.len(),
        0,
        "Day 1 note should no longer be loose"
    );

    // =========================================================================
    // Day 3 (Deep Work, Source Capture & Decision):
    // User captures benchmark URLs (authorship = 'external'), records a decision
    // to drop 30B+ models due to a 16GB VRAM limit, and leaves a second note
    // mid-sentence (is_unfinished_draft = 1).
    // =========================================================================
    clock.advance(Duration::from_secs(24 * 3600)); // Advance to Day 3

    // 1. Benchmark URL source
    let bench_source_content =
        "Benchmark findings: 7B models have 65% pass rate. 30B models exceed memory limits on developer hardware.";
    let bench_source = engine
        .create_artifact(
            ArtifactType::Source,
            Authorship::External,
            Some("Coding Benchmark Results"),
            bench_source_content,
            Some("https://benchmarks.example.org/models"),
        )
        .expect("Day 3: create bench source failed");
    engine
        .attach_artifact(&proj_coding.id, &bench_source.id, "user_explicit", None)
        .expect("Day 3: attach bench source failed");

    model.register_response(
        format!(
            "Extract knowledge nodes from this document:\n\n{}",
            bench_source.content_md
        ),
        serde_json::json!({
            "nodes": [
                {
                    "node_type": "finding",
                    "statement": "30B models exceed memory limits on developer hardware",
                    "quote": "30B models exceed memory limits on developer hardware."
                }
            ]
        })
        .to_string(),
    );
    engine
        .extract_epistemic_nodes(&bench_source.id)
        .await
        .expect("Day 3: extract bench source failed");

    // 2. Decision note
    let decision_content =
        "We decided to drop 30B+ models due to a 16GB VRAM limit. Can we evaluate 14B models on realistic tasks?";
    let decision_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Model Size Decision"),
            decision_content,
            None,
        )
        .expect("Day 3: create decision note failed");
    engine
        .attach_artifact(&proj_coding.id, &decision_note.id, "user_explicit", None)
        .expect("Day 3: attach decision note failed");

    engine
        .log_event(
            Some(&proj_coding.id),
            Some(&decision_note.id),
            Actor::User,
            EventType::ArtifactCreated,
            Materiality::Substantive,
            serde_json::json!({}),
        )
        .expect("Day 3: log event failed");

    model.register_response(
        format!(
            "Extract knowledge nodes from this document:\n\n{}",
            decision_note.content_md
        ),
        serde_json::json!({
            "nodes": [
                {
                    "node_type": "decision",
                    "statement": "We decided to drop 30B+ models due to a 16GB VRAM limit.",
                    "quote": "We decided to drop 30B+ models due to a 16GB VRAM limit."
                },
                {
                    "node_type": "question",
                    "statement": "Can we evaluate 14B models on realistic tasks?",
                    "quote": "Can we evaluate 14B models on realistic tasks?"
                }
            ]
        })
        .to_string(),
    );
    engine
        .extract_epistemic_nodes(&decision_note.id)
        .await
        .expect("Day 3: extract decision note failed");

    // 3. Second note mid-sentence (is_unfinished_draft = 1)
    let draft_content =
        "Currently testing the harness with local test runner but the token budget";
    let draft_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Evaluation Harness Setup Draft"),
            draft_content,
            None,
        )
        .expect("Day 3: create draft note failed");
    engine
        .attach_artifact(&proj_coding.id, &draft_note.id, "user_explicit", None)
        .expect("Day 3: attach draft note failed");

    // Mid-sentence detection marks is_unfinished_draft = true
    assert!(
        draft_note.is_unfinished_draft,
        "Mid-sentence draft must have is_unfinished_draft = true"
    );

    // Precompute Level 1 resume brief at end of Day 3 deep work session
    let precomputed_brief = engine
        .get_resume_brief(&proj_coding.id, 1)
        .await
        .expect("Day 3: precompute brief failed");
    assert!(!precomputed_brief.recent_progress.is_empty());

    // =========================================================================
    // Day 4 (Context Switch & Transient QA):
    // User switches to proj_webhooks, asks "What is speculative decoding?"
    // (TransientQa), and saves the AI answer (authorship = 'ai_generated').
    // =========================================================================
    clock.advance(Duration::from_secs(24 * 3600)); // Advance to Day 4

    let proj_webhooks = engine
        .create_project("Webhooks Integration Service", None)
        .expect("Day 4: create webhooks proj failed");
    engine
        .set_project_purpose(
            &proj_webhooks.id,
            "Reliable outbound webhook dispatch system",
        )
        .expect("Day 4: set webhooks purpose failed");

    // User asks question to router
    let router_plan = engine
        .route_input("What is speculative decoding?")
        .await
        .expect("Day 4: route_input failed");
    assert_eq!(
        router_plan.intent_type,
        IntentType::TransientQa,
        "Question must route to TransientQa"
    );
    assert!(
        !router_plan.mutates_immediately(),
        "TransientQa must never mutate database"
    );

    // User saves AI answer
    let ai_answer_content =
        "Speculative decoding uses a smaller draft model to generate candidate tokens which are verified in parallel by the target model.";
    let ai_note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::AiGenerated,
            Some("Speculative Decoding Explanation"),
            ai_answer_content,
            None,
        )
        .expect("Day 4: create ai answer note failed");
    engine
        .attach_artifact(&proj_webhooks.id, &ai_note.id, "user_explicit", None)
        .expect("Day 4: attach ai note failed");
    engine
        .log_event(
            Some(&proj_webhooks.id),
            Some(&ai_note.id),
            Actor::User,
            EventType::ArtifactCreated,
            Materiality::Substantive,
            serde_json::json!({}),
        )
        .expect("Day 4: log event failed");

    // Even if LLM extractor erroneously claims this is a 'decision', Rust derive_provenance must forbid it
    model.register_response(
        format!(
            "Extract knowledge nodes from this document:\n\n{}",
            ai_note.content_md
        ),
        serde_json::json!({
            "nodes": [
                {
                    "node_type": "decision",
                    "statement": "Decided to use speculative decoding for acceleration",
                    "quote": "Speculative decoding uses a smaller draft model to generate candidate tokens which are verified in parallel by the target model."
                }
            ]
        })
        .to_string(),
    );
    engine
        .extract_epistemic_nodes(&ai_note.id)
        .await
        .expect("Day 4: extract ai note failed");

    // =========================================================================
    // Days 5–11: Complete Absence (clock.advance(Duration::from_days(7)))
    // =========================================================================
    clock.advance(Duration::from_secs(7 * 24 * 3600));

    // =========================================================================
    // Day 12 (The Cold Return):
    // =========================================================================

    // 1. Engine::get_home_state() surfaces proj_coding and proj_webhooks ordered by last_substantive_at
    let home_state_day12 = engine
        .get_home_state()
        .expect("Day 12: get home state failed");
    assert_eq!(
        home_state_day12.threads.len(),
        2,
        "Both projects must be surfaced"
    );
    // proj_webhooks was touched on Day 4, proj_coding on Day 3
    assert_eq!(
        home_state_day12.threads[0].project_id, proj_webhooks.id,
        "proj_webhooks was updated more recently and must be first"
    );
    assert_eq!(
        home_state_day12.threads[1].project_id, proj_coding.id,
        "proj_coding must be second"
    );
    assert!(
        home_state_day12.threads[0].last_substantive_at
            > home_state_day12.threads[1].last_substantive_at,
        "Threads must be strictly ordered by last_substantive_at DESC"
    );

    // 2. User opens proj_coding. resume_pane_hint returns Show { reason: ColdReturn }
    let hint = engine
        .get_resume_pane_hint(&proj_coding.id)
        .expect("Day 12: get hint failed");
    assert_eq!(
        hint,
        ResumePaneHint::Show {
            reason: PaneHintReason::ColdReturn
        },
        "Must return ColdReturn after > 48h idle"
    );

    // 3. Engine::get_resume_brief(proj_coding, Level1) returns in < 5ms from derived_briefs
    let mut min_elapsed = Duration::from_secs(1);
    let mut brief_opt = None;
    for _ in 0..5 {
        let brief_start = Instant::now();
        let b = engine
            .get_resume_brief(&proj_coding.id, 1)
            .await
            .expect("Day 12: get resume brief failed");
        let elapsed = brief_start.elapsed();
        if elapsed < min_elapsed {
            min_elapsed = elapsed;
        }
        brief_opt = Some(b);
    }
    let brief = brief_opt.unwrap();
    assert!(
        min_elapsed.as_millis() < 5,
        "Level 1 brief must return from derived_briefs in < 5ms, took {:?}",
        min_elapsed
    );

    // Word cap check
    let words = visible_words(&brief);
    assert!(
        words <= 75,
        "Level 1 brief must be capped at <= 75 visible words, got {}",
        words
    );

    // Surfacing the 16GB VRAM decision (user_recorded)
    let decision_claim = brief
        .recent_progress
        .iter()
        .find(|c| c.text.contains("16GB"));
    assert!(
        decision_claim.is_some(),
        "Brief must surface the 16GB VRAM decision"
    );
    assert_eq!(
        decision_claim.unwrap().origin,
        EpistemicOrigin::UserRecorded,
        "16GB VRAM decision must have UserRecorded origin"
    );

    // Surfacing benchmark findings (source_backed)
    let finding_claim = brief
        .recent_progress
        .iter()
        .find(|c| c.origin == EpistemicOrigin::SourceBacked);
    assert!(
        finding_claim.is_some(),
        "Brief must surface benchmark findings with SourceBacked origin"
    );

    // Surfacing the open realistic-task evaluation question
    let question_claim = brief
        .recent_progress
        .iter()
        .find(|c| c.text.contains("realistic tasks") || c.text.ends_with('?'));
    assert!(
        question_claim.is_some(),
        "Brief must surface open question regarding realistic tasks"
    );

    // Surfacing the unfinished draft continuation point
    assert!(
        brief.continuation_point.is_some(),
        "Brief must have continuation point"
    );
    assert!(
        brief
            .continuation_point
            .as_ref()
            .unwrap()
            .contains("Unfinished draft"),
        "Continuation point must point to unfinished draft"
    );
    assert_eq!(
        brief.primary_continuation_artifact_id,
        Some(draft_note.id.clone()),
        "Primary continuation artifact must be draft_note"
    );

    // 4. Verify that the saved AI answer from Day 4 was NEVER laundered into a user_recorded decision
    let reader = engine.db().reader().expect("reader failed");
    let ai_nodes: Vec<(String, String, String)> = reader
        .prepare(
            "SELECT id, origin, node_type FROM epistemic_nodes WHERE artifact_id = ?1;",
        )
        .expect("prepare failed")
        .query_map(params![ai_note.id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .expect("query failed")
        .filter_map(|r| r.ok())
        .collect();

    assert!(!ai_nodes.is_empty(), "AI note must have extracted nodes");
    for (_id, origin, node_type) in &ai_nodes {
        assert_ne!(
            origin, "user_recorded",
            "AI generated note must NEVER have user_recorded origin!"
        );
        assert_ne!(
            node_type, "decision",
            "AI generated note must NEVER produce a decision!"
        );
        assert_eq!(
            origin, "ai_inferred",
            "AI generated note must strictly have ai_inferred origin"
        );
    }

    // 5. Verify memory mirrors exist and were never re-ingested by reconcile_vault()
    let proj_mirror_path = dir
        .path()
        .join(".sthira_meta")
        .join("memory_mirrors")
        .join("projects")
        .join(format!("{}.md", proj_coding.id));
    assert!(
        proj_mirror_path.exists(),
        "Project memory mirror must exist at: {}",
        proj_mirror_path.display()
    );
    let proj_mirror_content =
        std::fs::read_to_string(&proj_mirror_path).expect("read proj mirror failed");
    assert!(
        proj_mirror_content.contains("<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST"),
        "Project mirror must have DO_NOT_INGEST header"
    );

    let global_mirror_path = dir
        .path()
        .join(".sthira_meta")
        .join("memory_mirrors")
        .join("global_working_memory.md");
    assert!(
        global_mirror_path.exists(),
        "Global working memory mirror must exist at: {}",
        global_mirror_path.display()
    );
    let global_mirror_content =
        std::fs::read_to_string(&global_mirror_path).expect("read global mirror failed");
    assert!(
        global_mirror_content.contains("<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST"),
        "Global mirror must have DO_NOT_INGEST header"
    );

    // Reconcile vault against workspace
    let reconcile_res = sthira_core::vault::reconcile_vault(
        engine.db(),
        engine.vault_dir(),
        clock.now_ms(),
        |_conn, _pid, _aid, _actor, _etype, _mat, _delta, _ts| Ok(()),
    )
    .expect("reconcile_vault failed");
    assert_eq!(
        reconcile_res.conflicts_detected, 0,
        "Zero conflicts expected"
    );

    // Assert that .sthira_meta files were never ingested into artifacts table
    let meta_artifact_count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts WHERE file_path LIKE '%.sthira_meta%' OR content_md LIKE '%DO_NOT_INGEST%';",
            [],
            |r| r.get(0),
        )
        .expect("count failed");
    assert_eq!(
        meta_artifact_count, 0,
        "reconcile_vault must strictly ignore .sthira_meta mirrors"
    );

    // 6. Run the No-Pressure Regex Lint across every user-visible string generated during 14-day simulation
    let mut user_facing_strings = Vec::new();
    user_facing_strings.push(brief.purpose.clone());
    user_facing_strings.push(brief.active_working_breadcrumb.clone());
    if let Some(ref cp) = brief.continuation_point {
        user_facing_strings.push(cp.clone());
    }
    for claim in &brief.recent_progress {
        user_facing_strings.push(claim.text.clone());
    }
    for claim in &brief.conflicting_decisions {
        user_facing_strings.push(claim.text.clone());
    }
    for thread in &home_state_day12.threads {
        user_facing_strings.push(thread.name.clone());
        user_facing_strings.push(thread.purpose.clone());
    }
    user_facing_strings.push(proj_mirror_content);
    user_facing_strings.push(global_mirror_content);

    for text in &user_facing_strings {
        assert_no_guilt_language(text);
    }

    // =========================================================================
    // Day 13 (Resuming Work & Resolving the Continuation Point):
    // User resumes the unfinished draft, finishes the sentence, saves.
    // =========================================================================
    clock.advance(Duration::from_secs(24 * 3600)); // Advance to Day 13

    let finished_draft_content =
        "Currently testing the harness with local test runner but the token budget was optimized to 4096 tokens.";
    let updated_draft = engine
        .update_artifact(
            &draft_note.id,
            finished_draft_content,
            Actor::User,
            Some(&proj_coding.id),
        )
        .expect("Day 13: update draft failed");
    assert!(
        !updated_draft.is_unfinished_draft,
        "Finished draft must transition is_unfinished_draft to false"
    );

    // =========================================================================
    // Day 14 (Project Evolution & Final Audit):
    // Audit project history milestones and final integrity check.
    // =========================================================================
    clock.advance(Duration::from_secs(24 * 3600)); // Advance to Day 14

    let history = engine
        .get_project_history(&proj_coding.id)
        .expect("Day 14: get history failed");
    assert!(!history.milestones.is_empty(), "Milestones must exist");
    for seg in &history.segments {
        assert_no_guilt_language(&seg.text);
    }
    for m in &history.milestones {
        assert_no_guilt_language(&m.title);
        assert_no_guilt_language(&m.description);
    }

    println!("=== 14-Day Simulation Complete & Verified ===");
}

#[tokio::test]
async fn test_7_2_metamorphic_cold_return_across_arbitrary_time_gaps() {
    let (engine, clock, _model, _dir) = setup_test_engine();

    let proj = engine
        .create_project("Long Idle Project", None)
        .expect("create proj failed");
    engine
        .set_project_purpose(&proj.id, "Testing cold return invariance")
        .expect("set purpose failed");

    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Note 1"),
            "Initial observation and baseline architecture.",
            None,
        )
        .expect("create note failed");
    engine
        .attach_artifact(&proj.id, &note.id, "user_explicit", None)
        .expect("attach failed");
    engine
        .log_event(
            Some(&proj.id),
            Some(&note.id),
            Actor::User,
            EventType::ArtifactCreated,
            Materiality::Substantive,
            serde_json::json!({}),
        )
        .expect("log event failed");

    // Metamorphic testing across 3, 14, 30, and 180 day absences
    let gaps_days = [3, 14, 30, 180];
    for days in gaps_days {
        clock.advance(Duration::from_secs(days * 24 * 3600));

        let hint = engine.get_resume_pane_hint(&proj.id).expect("get hint failed");
        assert_eq!(
            hint,
            ResumePaneHint::Show {
                reason: PaneHintReason::ColdReturn
            },
            "Cold return must trigger consistently after {} days",
            days
        );

        let brief = engine.get_resume_brief(&proj.id, 1).await.expect("get brief failed");
        assert_no_guilt_language(&brief.purpose);
        for claim in &brief.recent_progress {
            assert_no_guilt_language(&claim.text);
        }
    }
}

#[tokio::test]
async fn test_7_3_vault_reconcile_isolation_strictness() {
    let (engine, clock, _model, dir) = setup_test_engine();

    // Generate nested mirrors in .sthira_meta
    let meta_dir = dir.path().join(".sthira_meta");
    let mirrors_dir = meta_dir.join("memory_mirrors").join("nested");
    std::fs::create_dir_all(&mirrors_dir).expect("create mirrors dir failed");

    for i in 1..=10 {
        let dummy_mirror = mirrors_dir.join(format!("mirror_{}.md", i));
        std::fs::write(
            &dummy_mirror,
            format!(
                "<!-- STHIRA_DERIVED_MEMORY: DO_NOT_INGEST -->\n# Mirror {}\nDerived memory content.",
                i
            ),
        )
        .expect("write dummy mirror failed");
    }

    // Run reconcile vault
    let result = sthira_core::vault::reconcile_vault(
        engine.db(),
        engine.vault_dir(),
        clock.now_ms(),
        |_conn, _pid, _aid, _actor, _etype, _mat, _delta, _ts| Ok(()),
    )
    .expect("reconcile failed");

    assert_eq!(result.files_imported, 0);

    // Verify 0 artifacts exist from .sthira_meta
    let reader = engine.db().reader().expect("reader failed");
    let count: i64 = reader
        .query_row(
            "SELECT COUNT(*) FROM artifacts WHERE content_md LIKE '%Derived memory content%';",
            [],
            |r| r.get(0),
        )
        .expect("count failed");
    assert_eq!(count, 0, "No .sthira_meta files may ever enter artifacts table");
}
