//! User-managed synthesis template lifecycle against real SQLite. No model/network calls.
use std::sync::Arc;
use sthira_core::{
    db::templates_repo::TemplatePage, ArtifactType, Authorship, Engine, SynthesisTemplateDraft,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};

fn open(path: &std::path::Path) -> Engine {
    Engine::open(
        path.join("sthira.db"),
        path.join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("unused-fixture")),
    )
    .unwrap()
}

fn draft(name: &str, mode: &str, skeleton: &str) -> SynthesisTemplateDraft {
    SynthesisTemplateDraft {
        id: None,
        name: name.into(),
        target_mode: mode.into(),
        description: "A user-authored structure.".into(),
        template_skeleton_md: skeleton.into(),
        guidance: "Stay within supplied evidence.".into(),
        expected_version: None,
    }
}

#[test]
fn seeding_is_additive_and_never_invents_a_default() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let initial = engine.list_synthesis_templates(None).unwrap();
    assert!(initial
        .iter()
        .any(|t| t.name == "Research: Deep Dive" && t.is_builtin));
    assert!(initial
        .iter()
        .any(|t| t.name == "Research: Cultural Tradition — Four Lenses"));
    assert!(initial
        .iter()
        .any(|t| t.name == "Plan: MVP / Product Roadmap" && t.is_builtin));
    assert!(initial.iter().any(|t| t.target_mode == "capture"));
    assert!(
        initial.iter().all(|t| !t.is_default),
        "seeding must not choose a default"
    );
    // Reopening must not duplicate or overwrite built-ins.
    let count = initial.len();
    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(engine.list_synthesis_templates(None).unwrap().len(), count);
}

#[test]
fn create_edit_duplicate_filter_and_stale_version_rejection() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let first = engine
        .create_synthesis_template(&draft(
            "Roots and Reason",
            "research",
            "# {{title}}\n\n## Scope\n\n## Sources\n",
        ))
        .unwrap();
    let second = engine
        .create_synthesis_template(&draft(
            "Launch Sequence",
            "plan",
            "# {{title}}\n\n## Milestones\n",
        ))
        .unwrap();
    assert!(!first.is_builtin);
    assert_eq!(first.version, 1);

    let research = engine.list_synthesis_templates(Some("research")).unwrap();
    assert!(research.iter().all(|t| t.target_mode == "research"));
    assert!(research.iter().any(|t| t.id == first.id));
    assert!(!research.iter().any(|t| t.id == second.id));

    // Mode mismatch cannot be read as a different workflow's template.
    assert!(engine
        .require_synthesis_template(&first.id, "plan")
        .is_err());

    let mut edit = draft(
        "Roots and Reason",
        "research",
        "# {{title}}\n\n## Scope\n\n## Revised\n",
    );
    edit.id = Some(first.id.clone());
    edit.expected_version = Some(1);
    let updated = engine.update_synthesis_template(&edit).unwrap();
    assert_eq!(updated.version, 2);
    assert!(updated.template_skeleton_md.contains("## Revised"));

    // A stale editor cannot overwrite newer content.
    let mut stale = edit.clone();
    stale.expected_version = Some(1);
    stale.template_skeleton_md = "# stale\n".into();
    assert!(engine.update_synthesis_template(&stale).is_err());

    // Built-ins are read-only; duplication produces an editable custom copy.
    let four = engine
        .list_synthesis_templates(Some("research"))
        .unwrap()
        .into_iter()
        .find(|t| t.id == "research-cultural-four-lenses")
        .unwrap();
    let mut edit_builtin = draft("Changed builtin", "research", "# {{title}}\n");
    edit_builtin.id = Some(four.id.clone());
    edit_builtin.expected_version = Some(four.version);
    assert!(engine.update_synthesis_template(&edit_builtin).is_err());
    let copy = engine.duplicate_synthesis_template(&four.id).unwrap();
    assert!(!copy.is_builtin);
    assert!(copy.name.ends_with("(copy)"));
}

#[test]
fn default_switching_is_atomic_and_clearable_per_workflow() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let a = engine
        .create_synthesis_template(&draft("Research A", "research", "# {{title}}\n"))
        .unwrap();
    let b = engine
        .create_synthesis_template(&draft("Research B", "research", "# {{title}}\n\n## B\n"))
        .unwrap();
    let plan = engine
        .create_synthesis_template(&draft("Plan A", "plan", "# {{title}}\n"))
        .unwrap();
    engine
        .set_synthesis_template_default("research", &a.id)
        .unwrap();
    engine
        .set_synthesis_template_default("research", &b.id)
        .unwrap();
    engine
        .set_synthesis_template_default("plan", &plan.id)
        .unwrap();
    let defaults = engine.list_synthesis_templates(None).unwrap();
    assert_eq!(
        defaults
            .iter()
            .filter(|t| t.is_default && t.target_mode == "research")
            .count(),
        1
    );
    assert_eq!(
        defaults
            .iter()
            .filter(|t| t.is_default && t.target_mode == "plan")
            .count(),
        1
    );
    assert_eq!(
        engine
            .get_default_synthesis_template("research")
            .unwrap()
            .unwrap()
            .id,
        b.id
    );
    engine.clear_synthesis_template_default("research").unwrap();
    assert!(engine
        .get_default_synthesis_template("research")
        .unwrap()
        .is_none());
    assert!(engine
        .get_default_synthesis_template("plan")
        .unwrap()
        .is_some());
    // Capture structures never take a workflow default.
    let capture = engine
        .list_synthesis_templates(Some("capture"))
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert!(engine
        .set_synthesis_template_default("capture", &capture.id)
        .is_err());
    // A chosen default and custom rows survive a restart without being replaced.
    let plan_default = engine
        .get_default_synthesis_template("plan")
        .unwrap()
        .unwrap()
        .id;
    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(
        engine
            .get_default_synthesis_template("plan")
            .unwrap()
            .unwrap()
            .id,
        plan_default
    );
    assert!(engine.get_synthesis_template(&a.id).unwrap().is_some());
}

#[test]
fn validation_rejects_blank_oversized_and_unsupported_input() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    assert!(engine
        .create_synthesis_template(&draft("   ", "research", "# {{title}}\n"))
        .is_err());
    assert!(engine
        .create_synthesis_template(&draft("Wrong mode", "draft", "# {{title}}\n"))
        .is_err());
    let huge = "x".repeat(20_001);
    assert!(engine
        .create_synthesis_template(&draft("Too large", "research", &huge))
        .is_err());
    let mut long_name = draft("ok", "research", "# {{title}}\n");
    long_name.name = "n".repeat(121);
    assert!(engine.create_synthesis_template(&long_name).is_err());
    let missing = draft("ok", "research", "# {{title}}\n");
    assert!(missing.template_skeleton_md.contains("{{title}}"));
}

#[test]
fn archive_and_delete_survive_restart_without_resurrection() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let custom = engine
        .create_synthesis_template(&draft("Temporary", "research", "# {{title}}\n"))
        .unwrap();
    engine
        .archive_synthesis_template(&custom.id, Some(custom.version))
        .unwrap();
    let builtin = engine
        .list_synthesis_templates(Some("research"))
        .unwrap()
        .into_iter()
        .find(|t| t.id == "research-deep-dive")
        .unwrap();
    engine
        .archive_synthesis_template(&builtin.id, Some(builtin.version))
        .unwrap();
    assert!(engine
        .list_synthesis_templates(Some("research"))
        .unwrap()
        .iter()
        .all(|t| t.id != builtin.id));
    drop(engine);

    let engine = open(tmp.path());
    assert!(
        engine
            .list_synthesis_templates(Some("research"))
            .unwrap()
            .iter()
            .all(|t| t.id != builtin.id),
        "archived built-in must not reappear"
    );
    let archived = engine
        .list_synthesis_templates_page(None, true, TemplatePage::default())
        .unwrap();
    assert!(archived
        .iter()
        .any(|t| t.id == builtin.id && t.archived_at.is_some()));
    engine.delete_synthesis_template(&custom.id).unwrap();
    drop(engine);
    let engine = open(tmp.path());
    assert!(engine.get_synthesis_template(&custom.id).unwrap().is_none());
    assert!(engine
        .list_synthesis_templates(Some("capture"))
        .unwrap()
        .iter()
        .all(|t| t.target_mode == "capture"));
}

#[test]
fn snapshot_is_immutable_after_template_edits_and_deletion() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let template = engine
        .create_synthesis_template(&draft("Stable", "research", "# {{title}}\n\n## Original\n"))
        .unwrap();
    let snapshot = template.snapshot();
    assert!(snapshot.render("Example").contains("# Example"));

    let mut edit = draft("Stable", "research", "# {{title}}\n\n## Rewritten\n");
    edit.id = Some(template.id.clone());
    edit.expected_version = Some(template.version);
    engine.update_synthesis_template(&edit).unwrap();
    assert!(snapshot.skeleton_md.contains("## Original"));
    assert!(!snapshot.skeleton_md.contains("## Rewritten"));

    engine.delete_synthesis_template(&template.id).unwrap();
    assert!(snapshot.skeleton_md.contains("## Original"));
    assert_eq!(snapshot.version, 1);
    assert_eq!(snapshot.name, "Stable");
}

#[test]
fn template_run_snapshot_stays_with_the_artifact_and_survives_deletion() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let artifact = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::AiGenerated,
            Some("Saved research"),
            "# Saved research\n\nFindings.",
            None,
        )
        .unwrap();
    let template = engine
        .create_synthesis_template(&draft("Run record", "research", "# {{title}}\n"))
        .unwrap();
    engine
        .record_synthesis_template_run("research", Some(&artifact.id), &template.snapshot())
        .unwrap();
    engine.delete_synthesis_template(&template.id).unwrap();
    let reader = engine.db().reader().unwrap();
    let stored: (String, i64, String) = reader
        .query_row(
            "SELECT template_id, template_version, template_name FROM synthesis_template_runs WHERE artifact_id=?1",
            [&artifact.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(stored, (template.id, 1, "Run record".to_string()));
}

#[test]
fn listing_is_bounded_and_paginated() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    for index in 0..5 {
        engine
            .create_synthesis_template(&draft(&format!("Custom {index}"), "plan", "# {{title}}\n"))
            .unwrap();
    }
    let first = engine
        .list_synthesis_templates_page(
            Some("plan"),
            false,
            TemplatePage {
                limit: 2,
                offset: 0,
            },
        )
        .unwrap();
    let second = engine
        .list_synthesis_templates_page(
            Some("plan"),
            false,
            TemplatePage {
                limit: 2,
                offset: 2,
            },
        )
        .unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!(second.len(), 2);
    assert!(first.iter().all(|t| t.target_mode == "plan"));
    let clamp = engine
        .list_synthesis_templates_page(
            None,
            false,
            TemplatePage {
                limit: 10_000,
                offset: 0,
            },
        )
        .unwrap();
    assert!(clamp.len() <= 200, "listing must clamp oversized pages");
}

#[test]
fn generation_history_records_no_template_and_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let artifact = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("No-template plan"),
            "# Plan\n\nBody.",
            None,
        )
        .unwrap();
    let record = sthira_core::db::templates_repo::GenerationRecord {
        workflow: "plan",
        artifact_id: Some(&artifact.id),
        snapshot: None,
        template_mode: "none",
        generation_key: "plan:test:1",
        model_name: "Fixture model",
        provider_name: "Fixture provider",
        status: "completed",
    };
    assert!(engine.record_generation_run(&record).unwrap());
    assert!(
        !engine.record_generation_run(&record).unwrap(),
        "repeated callbacks are idempotent"
    );
    let stored: (String, String, String) = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT template_id, template_mode, status FROM synthesis_template_runs WHERE artifact_id=?1",
            [&artifact.id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        stored,
        ("__none__".into(), "none".into(), "completed".into())
    );
    let count: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM synthesis_template_runs", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
    let version: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 30);
    let table: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agent_progress_events'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(table, 1);
}

#[test]
fn v30_upgrade_is_additive_idempotent_and_keeps_existing_generation_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let artifact = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Existing history"),
            "# History\n",
            None,
        )
        .unwrap();
    let record = sthira_core::db::templates_repo::GenerationRecord {
        workflow: "plan",
        artifact_id: Some(&artifact.id),
        snapshot: None,
        template_mode: "disabled",
        generation_key: "upgrade:1",
        model_name: "M",
        provider_name: "P",
        status: "completed",
    };
    engine.record_generation_run(&record).unwrap();
    {
        let writer = engine.db().writer();
        writer
            .execute_batch("DROP TABLE agent_progress_events; PRAGMA user_version = 29;")
            .unwrap();
    }
    drop(engine);
    let engine = open(tmp.path());
    let stored: (String, String) = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT template_mode, status FROM synthesis_template_runs WHERE artifact_id=?1",
            [&artifact.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(stored, ("disabled".into(), "completed".into()));
    let version: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 30);
    // Repeated startup remains additive.
    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        30
    );
}
