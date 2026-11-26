//! Directly streamed Plan history: dispatch-time snapshots, no re-resolution, dedupe,
//! explicit no-template/disabled decisions, deletion/edits during generation and reopen.
use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, Engine, SynthesisTemplateDraft};
use sthira_desktop::workflow_settings::{
    resolve_workflow_template, save_workflow_setting, WorkflowSetting,
};
use sthira_desktop::{handle_save_generated_artifact_with_generation, GenerationReceipt};
use sthira_testkit::{DeterministicFakeModel, FakeClock};

fn open(root: &std::path::Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("unused")),
    )
    .unwrap()
}

fn template(engine: &Engine, name: &str, skeleton: &str) -> sthira_core::SynthesisTemplate {
    engine
        .create_synthesis_template(&SynthesisTemplateDraft {
            id: None,
            name: name.into(),
            target_mode: "plan".into(),
            description: String::new(),
            template_skeleton_md: skeleton.into(),
            guidance: format!("Guidance for {name}"),
            expected_version: None,
        })
        .unwrap()
}

fn enable_plan(engine: &Engine, templates: bool) {
    save_workflow_setting(
        engine,
        "plan",
        WorkflowSetting {
            enabled: true,
            model_id: None,
            templates_enabled: templates,
        },
    )
    .unwrap();
}

fn draft_page(engine: &Engine) -> sthira_core::Artifact {
    engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Plan draft"),
            "# Plan draft\n\nDraft body.",
            None,
        )
        .unwrap()
}

fn receipt(
    choice: &sthira_desktop::workflow_settings::WorkflowTemplateChoice,
    key: &str,
) -> GenerationReceipt {
    GenerationReceipt {
        generation_key: key.into(),
        workflow: "plan".into(),
        template_mode: choice.mode.clone(),
        template: choice.template.as_ref().map(|value| value.snapshot()),
        model_id: "fixture-model".into(),
        model_name: "Fixture model".into(),
        provider_name: "Fixture provider".into(),
    }
}

fn rows(engine: &Engine, artifact: &str) -> Vec<(String, i64, String, String, String)> {
    let reader = engine.db().reader().unwrap();
    let mut stmt = reader
        .prepare("SELECT template_id, template_version, template_name, template_mode, status FROM synthesis_template_runs WHERE artifact_id=?1 ORDER BY created_at")
        .unwrap();
    stmt.query_map([artifact], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })
    .unwrap()
    .collect::<Result<Vec<_>, _>>()
    .unwrap()
}

#[test]
fn explicit_template_is_snapshotted_and_survives_later_edits_delete_defaults_and_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    enable_plan(&engine, true);
    let first = template(&engine, "Launch A", "# {{title}}\n\n## Original structure");
    let choice = resolve_workflow_template(&engine, "plan", Some(first.id.clone())).unwrap();
    assert_eq!(choice.mode, "explicit");
    assert_eq!(choice.template.as_ref().unwrap().version, 1);

    // Template is edited and then deleted between dispatch and save.
    let mut edit = SynthesisTemplateDraft {
        id: Some(first.id.clone()),
        name: "Launch A".into(),
        target_mode: "plan".into(),
        description: String::new(),
        template_skeleton_md: "# {{title}}\n\n## Rewritten later".into(),
        guidance: "Changed guidance".into(),
        expected_version: Some(1),
    };
    engine.update_synthesis_template(&edit).unwrap();
    engine.delete_synthesis_template(&first.id).unwrap();
    edit.expected_version = Some(2);

    let page = draft_page(&engine);
    let receipt = receipt(&choice, "plan-explicit-1");
    handle_save_generated_artifact_with_generation(
        &engine,
        page.id.clone(),
        "# Plan\n\n## Original structure\n\nCompleted.".into(),
        "Plan".into(),
        None,
        Some(&receipt),
    )
    .unwrap();
    let stored = rows(&engine, &page.id);
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].0, first.id);
    assert_eq!(stored[0].1, 1);
    assert_eq!(stored[0].2, "Launch A");
    assert_eq!(stored[0].3, "explicit");
    assert_eq!(stored[0].4, "completed");
    let skeleton: String = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT skeleton_md FROM synthesis_template_runs WHERE artifact_id=?1",
            [&page.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(skeleton.contains("## Original structure"));
    assert!(!skeleton.contains("## Rewritten later"));

    // Repeated save callbacks with the same receipt remain idempotent.
    handle_save_generated_artifact_with_generation(
        &engine,
        page.id.clone(),
        "# Plan\n\n## Original structure\n\nCompleted.".into(),
        "Plan".into(),
        None,
        Some(&receipt),
    )
    .unwrap();
    assert_eq!(rows(&engine, &page.id).len(), 1);

    // Ordinary document edits do not invent generation records.
    engine
        .save_artifact(
            &page.id,
            sthira_core::SaveArtifactRequest {
                content_md: "# Plan\n\nOwner edit.",
                title: Some("Plan"),
                actor: sthira_core::Actor::User,
                context_project_id: None,
                authorship: None,
                is_unfinished_draft: None,
            },
        )
        .unwrap();
    assert_eq!(rows(&engine, &page.id).len(), 1);

    drop(engine);
    let engine = open(tmp.path());
    assert_eq!(rows(&engine, &page.id), stored);
}

#[test]
fn inherit_default_is_frozen_before_a_later_default_change() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    enable_plan(&engine, true);
    let a = template(&engine, "Default A", "# {{title}}\n\n## A");
    let b = template(&engine, "Default B", "# {{title}}\n\n## B");
    engine
        .set_synthesis_template_default("plan", &a.id)
        .unwrap();
    let choice = resolve_workflow_template(&engine, "plan", None).unwrap();
    assert_eq!(choice.mode, "inherit");
    assert_eq!(choice.template.as_ref().unwrap().id, a.id);
    engine
        .set_synthesis_template_default("plan", &b.id)
        .unwrap();

    let page = draft_page(&engine);
    let receipt = receipt(&choice, "plan-inherit-1");
    handle_save_generated_artifact_with_generation(
        &engine,
        page.id.clone(),
        "# Plan\n\n## A\n\nDone.".into(),
        "Plan".into(),
        None,
        Some(&receipt),
    )
    .unwrap();
    let stored = rows(&engine, &page.id);
    assert_eq!(
        stored[0].0, a.id,
        "the old default remains the recorded template"
    );
    assert_eq!(stored[0].3, "inherit");
}

#[test]
fn no_template_and_templates_disabled_are_recorded_as_explicit_decisions() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    enable_plan(&engine, true);
    let none_choice = resolve_workflow_template(&engine, "plan", Some("__none__".into())).unwrap();
    assert_eq!(none_choice.mode, "none");
    assert!(none_choice.template.is_none());
    let page = draft_page(&engine);
    handle_save_generated_artifact_with_generation(
        &engine,
        page.id.clone(),
        "# Plan\n\nNo template content.".into(),
        "Plan".into(),
        None,
        Some(&receipt(&none_choice, "plan-none-1")),
    )
    .unwrap();
    let stored = rows(&engine, &page.id);
    assert_eq!(stored[0].0, "__none__");
    assert_eq!(stored[0].3, "none");

    enable_plan(&engine, false);
    let disabled = resolve_workflow_template(&engine, "plan", None).unwrap();
    assert_eq!(disabled.mode, "disabled");
    let page2 = draft_page(&engine);
    handle_save_generated_artifact_with_generation(
        &engine,
        page2.id.clone(),
        "# Plan\n\nDisabled templates.".into(),
        "Plan".into(),
        None,
        Some(&receipt(&disabled, "plan-disabled-1")),
    )
    .unwrap();
    assert_eq!(rows(&engine, &page2.id)[0].3, "disabled");
}

#[test]
fn failed_generation_does_not_record_a_completed_run() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    enable_plan(&engine, true);
    let t = template(&engine, "T", "# {{title}}\n");
    let choice = resolve_workflow_template(&engine, "plan", Some(t.id)).unwrap();
    let page = draft_page(&engine);
    let error = handle_save_generated_artifact_with_generation(
        &engine,
        page.id.clone(),
        "   ".into(),
        "Plan".into(),
        None,
        Some(&receipt(&choice, "plan-failed-1")),
    )
    .unwrap_err();
    assert!(error.contains("empty"));
    assert!(rows(&engine, &page.id).is_empty());
}

#[test]
fn stream_planning_preparation_captures_the_snapshot_before_later_template_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    engine
        .db()
        .writer()
        .execute_batch(
            "INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('p','Fixture provider','openai_compatible','http://127.0.0.1:1/v1',1,0);\n             INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('fixture-model','p','fixture','Fixture model',1,1,0);",
        )
        .unwrap();
    save_workflow_setting(
        &engine,
        "plan",
        WorkflowSetting {
            enabled: true,
            model_id: Some("fixture-model".into()),
            templates_enabled: true,
        },
    )
    .unwrap();
    let template = template(
        &engine,
        "Stream template",
        "# {{title}}\n\n## Streamed structure",
    );
    let (prompt, receipt) = sthira_desktop::prepare_stream_generation(
        &engine,
        "plan",
        None,
        "Plan the launch".into(),
        Some(template.id.clone()),
    )
    .unwrap();
    assert!(prompt.contains("Plan the launch"));
    assert!(prompt.contains("## Streamed structure"));
    assert_eq!(receipt.template_mode, "explicit");
    assert_eq!(receipt.template.as_ref().unwrap().version, 1);
    assert_eq!(receipt.model_name, "Fixture model");
    assert_eq!(receipt.provider_name, "Fixture provider");

    // Change and delete the live template after dispatch preparation.
    let mut edit = SynthesisTemplateDraft {
        id: Some(template.id.clone()),
        name: "Stream template".into(),
        target_mode: "plan".into(),
        description: String::new(),
        template_skeleton_md: "# {{title}}\n\n## New structure".into(),
        guidance: "new".into(),
        expected_version: Some(1),
    };
    engine.update_synthesis_template(&edit).unwrap();
    edit.expected_version = Some(2);
    engine.delete_synthesis_template(&template.id).unwrap();

    let page = draft_page(&engine);
    handle_save_generated_artifact_with_generation(
        &engine,
        page.id.clone(),
        "# Plan\n\n## Streamed structure\n\nSaved.".into(),
        "Plan".into(),
        None,
        Some(&receipt),
    )
    .unwrap();
    let stored = rows(&engine, &page.id);
    assert_eq!(stored[0].0, template.id);
    assert_eq!(stored[0].1, 1);
    let skeleton: String = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT skeleton_md FROM synthesis_template_runs WHERE artifact_id=?1",
            [&page.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(skeleton.contains("## Streamed structure"));
    assert!(!skeleton.contains("## New structure"));
}
