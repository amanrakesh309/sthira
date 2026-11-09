use std::sync::Arc;
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_desktop::workflow_settings::{
    get_workflow_settings, save_workflow_setting, workflow_model, workflow_prompt,
    workflow_template, WorkflowSetting,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};

fn open(root: &std::path::Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("test")),
    )
    .unwrap()
}

#[test]
fn preferences_are_opt_in_durable_and_do_not_touch_documents() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    assert!(get_workflow_settings(&engine)
        .unwrap()
        .values()
        .all(|value| !value.enabled));
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Keep this"),
            "An original thought.",
            None,
        )
        .unwrap();
    save_workflow_setting(
        &engine,
        "research",
        WorkflowSetting {
            enabled: true,
            templates_enabled: true,
            model_id: None,
        },
    )
    .unwrap();
    assert!(save_workflow_setting(&engine, "unknown", WorkflowSetting::default()).is_err());
    assert!(save_workflow_setting(
        &engine,
        "resume",
        WorkflowSetting {
            enabled: true,
            ..Default::default()
        }
    )
    .is_ok());
    assert!(save_workflow_setting(
        &engine,
        "draft",
        WorkflowSetting {
            templates_enabled: true,
            ..Default::default()
        }
    )
    .is_err());
    drop(engine);
    let reopened = open(tmp.path());
    assert!(get_workflow_settings(&reopened).unwrap()["research"].templates_enabled);
    assert!(!get_workflow_settings(&reopened).unwrap()["chat"].enabled);
    assert_eq!(
        reopened.get_artifact(&note.id).unwrap().content_md,
        "An original thought."
    );
}

#[tokio::test]
async fn disabled_generation_rejects_before_dispatch_and_preserves_the_draft() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Saved draft"),
            "Keep my draft.",
            None,
        )
        .unwrap();
    let error = sthira_desktop::handle_run_research_into_page(
        &engine,
        note.id.clone(),
        "Question".into(),
        Some("local_vault_only".into()),
        None,
        None,
        tauri::ipc::Channel::new(|_| panic!("disabled workflow emitted progress")),
    )
    .await
    .unwrap_err();
    assert!(error.contains("Enable research"));
    assert_eq!(
        engine.get_artifact(&note.id).unwrap().content_md,
        "Keep my draft."
    );
    assert!(workflow_model(&engine, "chat", None)
        .unwrap_err()
        .contains("Enable chat"));
}

#[test]
fn model_precedence_and_template_gates_are_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    {
        let writer = engine.db().writer();
        writer.execute_batch("DELETE FROM provider_models; DELETE FROM providers;
            INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('qa','QA','ollama','http://127.0.0.1:1',1,0);
            INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('workspace','qa','one','One',1,1,0),('workflow','qa','two','Two',1,0,0),('explicit','qa','three','Three',1,0,0);").unwrap();
    }
    save_workflow_setting(
        &engine,
        "plan",
        WorkflowSetting {
            enabled: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        workflow_model(&engine, "plan", None).unwrap().as_deref(),
        Some("workspace")
    );
    save_workflow_setting(
        &engine,
        "plan",
        WorkflowSetting {
            enabled: true,
            model_id: Some("workflow".into()),
            templates_enabled: false,
        },
    )
    .unwrap();
    assert_eq!(
        workflow_model(&engine, "plan", None).unwrap().as_deref(),
        Some("workflow")
    );
    assert_eq!(
        workflow_model(&engine, "plan", Some("explicit".into()))
            .unwrap()
            .as_deref(),
        Some("explicit")
    );
    assert!(workflow_model(&engine, "plan", Some("missing".into())).is_err());
    assert_eq!(
        workflow_template(&engine, "plan", Some("plan-milestone-roadmap".into())).unwrap(),
        None
    );
    save_workflow_setting(
        &engine,
        "plan",
        WorkflowSetting {
            enabled: true,
            model_id: Some("workflow".into()),
            templates_enabled: true,
        },
    )
    .unwrap();
    assert_eq!(
        workflow_template(&engine, "plan", Some("plan-milestone-roadmap".into()))
            .unwrap()
            .as_deref(),
        Some("plan-milestone-roadmap")
    );
    assert!(workflow_template(&engine, "plan", Some("research-deep-dive".into())).is_err());
    let prompt = workflow_prompt(
        &engine,
        "plan",
        "Plan a garden".into(),
        Some("plan-milestone-roadmap".into()),
    )
    .unwrap();
    assert!(prompt.starts_with("Plan a garden"));
    assert!(prompt.contains("Scope & Objectives"));
    save_workflow_setting(
        &engine,
        "plan",
        WorkflowSetting {
            enabled: true,
            model_id: Some("workflow".into()),
            templates_enabled: false,
        },
    )
    .unwrap();
    assert_eq!(
        workflow_prompt(
            &engine,
            "plan",
            "Plan a garden".into(),
            Some("plan-milestone-roadmap".into())
        )
        .unwrap(),
        "Plan a garden"
    );
    engine
        .db()
        .writer()
        .execute(
            "UPDATE provider_models SET is_enabled=0 WHERE id='workflow'",
            [],
        )
        .unwrap();
    assert!(workflow_model(&engine, "plan", None).is_err());
}
