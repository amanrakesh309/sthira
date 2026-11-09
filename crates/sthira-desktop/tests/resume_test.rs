use std::{
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    sync::Arc,
    thread,
};
use sthira_core::{
    resume::{validate_output, ResumeModelIdentity, ResumePreferences},
    ArtifactType, Authorship, Engine,
};
use sthira_desktop::{
    handle_save_provider,
    resume::{generate_brief, get_settings, read_brief, save_settings, ResumeSettings},
    resume_pack,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};

fn open(root: &Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(10000)),
        Arc::new(DeterministicFakeModel::new("must-not-be-used").with_panic_on_call()),
    )
    .unwrap()
}
fn project(engine: &Engine) -> (String, String) {
    let project = engine.create_project("Restore review", None).unwrap();
    let note=engine.create_artifact(ArtifactType::Note,Authorship::User,Some("Release checklist"),"- [ ] Mira must test restore from backup before release. This test has not been completed.",None).unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();
    (project.id, note.id)
}
fn configured(engine: &Engine, url: &str) {
    let provider = sthira_core::ProviderEntry {
        id: "workspace-provider".into(),
        display_name: "Workspace HTTP".into(),
        provider_type: "openai_compatible".into(),
        base_url: format!("{url}/v1"),
        has_api_key: false,
        is_enabled: true,
        models: vec![sthira_core::ProviderModelEntry {
            id: "workspace-model".into(),
            provider_id: "workspace-provider".into(),
            model_identifier: "chosen-identifier".into(),
            display_name: "Workspace choice".into(),
            context_window: 8192,
            supports_structured_output: true,
            is_enabled: true,
            is_default: true,
            web_search_capability: Default::default(),
        }],
    };
    handle_save_provider(engine, provider, None).unwrap();
    save_settings(
        engine,
        ResumeSettings {
            enabled: true,
            preferences: ResumePreferences::default(),
        },
    )
    .unwrap();
}
fn response_server(valid: bool) -> (String, thread::JoinHandle<serde_json::Value>) {
    response_server_with_hook(valid, None)
}
fn response_server_with_hook(
    valid: bool,
    hook: Option<Box<dyn FnOnce() + Send>>,
) -> (String, thread::JoinHandle<serde_json::Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut data = Vec::new();
        let mut end = 0;
        let mut length = usize::MAX;
        loop {
            let mut bytes = [0; 8192];
            let size = stream.read(&mut bytes).unwrap();
            data.extend_from_slice(&bytes[..size]);
            if end == 0 {
                if let Some(index) = data.windows(4).position(|value| value == b"\r\n\r\n") {
                    end = index + 4;
                    let header = String::from_utf8_lossy(&data[..index]);
                    length = header
                        .lines()
                        .find_map(|line| {
                            line.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|value| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                }
            }
            if size == 0 || end > 0 && data.len() >= end + length {
                break;
            }
        }
        let request: serde_json::Value = serde_json::from_slice(&data[end..]).unwrap();
        if let Some(hook) = hook {
            hook();
        }
        let packet: serde_json::Value =
            serde_json::from_str(request["messages"][1]["content"].as_str().unwrap()).unwrap();
        let source = &packet["sources"][0];
        let quote = source["text"].as_str().unwrap();
        let answer=serde_json::json!({"items":[{"text":quote,"evidence":[{"source_id":source["source_id"],"quote":if valid{quote}else{"fabricated quote"}}]}]}).to_string();
        let body = serde_json::json!({"choices":[{"message":{"content":answer}}]}).to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        request
    });
    (url, server)
}

#[tokio::test]
async fn changing_preferences_during_inference_does_not_overwrite_the_cache() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, _) = project(&engine);
    let changed_engine = engine.clone();
    let (url, server) = response_server_with_hook(
        true,
        Some(Box::new(move || {
            let mut settings = get_settings(&changed_engine).unwrap();
            settings.preferences.include_questions = false;
            save_settings(&changed_engine, settings).unwrap();
        })),
    );
    configured(&engine, &url);
    let result = generate_brief(&engine, &pid).await.unwrap();
    server.join().unwrap();
    assert_eq!(result.generation.unwrap().mode, "factual");
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row("SELECT count(*) FROM resume_brief_cache", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn no_model_has_read_only_factual_resume_even_when_summaries_are_off() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, aid) = project(&engine);
    let count_before: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row("SELECT count(*) FROM ai_proposals", [], |row| row.get(0))
        .unwrap();
    let brief = read_brief(&engine, &pid, 1).unwrap();
    assert_eq!(brief.generation.unwrap().mode, "factual");
    assert_eq!(
        brief.primary_continuation_artifact_id.as_deref(),
        Some(aid.as_str())
    );
    assert!(brief
        .recent_progress
        .iter()
        .any(|claim| claim.text.contains("has not been completed")));
    assert!(brief.conflicting_decisions.is_empty());
    assert!(!get_settings(&engine).unwrap().enabled);
    assert_eq!(
        generate_brief(&engine, &pid)
            .await
            .unwrap()
            .generation
            .unwrap()
            .mode,
        "factual"
    );
    let reader = engine.db().reader().unwrap();
    assert_eq!(
        reader
            .query_row("SELECT count(*) FROM ai_proposals", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        count_before
    );
    assert_eq!(
        reader
            .query_row("SELECT count(*) FROM resume_brief_cache", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn workspace_model_is_used_and_one_cached_summary_serves_both_levels_and_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, aid) = project(&engine);
    let (url, server) = response_server(true);
    configured(&engine, &url);
    let generated = generate_brief(&engine, &pid).await.unwrap();
    let request = server.join().unwrap();
    assert_eq!(request["model"], "chosen-identifier");
    let meta = generated.generation.unwrap();
    assert_eq!(meta.mode, "generated");
    assert_eq!(meta.model_id.as_deref(), Some("workspace-model"));
    assert_eq!(meta.model_name.as_deref(), Some("Workspace choice"));
    assert!(!meta.factual_progress.is_empty());
    assert_eq!(generated.recent_progress[0].artifact_ids, vec![aid]);
    for level in [1, 2] {
        let cached = read_brief(&engine, &pid, level).unwrap();
        assert_eq!(cached.generation.unwrap().mode, "cached");
        assert_eq!(
            cached.recent_progress[0].text,
            generated.recent_progress[0].text
        );
        assert_eq!(cached.level, level as i64);
    }
    drop(engine);
    let reopened = open(temp.path());
    assert_eq!(
        read_brief(&reopened, &pid, 1)
            .unwrap()
            .generation
            .unwrap()
            .mode,
        "cached"
    );
}

#[tokio::test]
async fn unverified_output_falls_back_without_replacing_the_note_or_adding_cache() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, aid) = project(&engine);
    let before = engine.get_artifact(&aid).unwrap();
    let (url, server) = response_server(false);
    configured(&engine, &url);
    let result = generate_brief(&engine, &pid).await.unwrap();
    server.join().unwrap();
    let meta = result.generation.unwrap();
    assert_eq!(meta.mode, "factual");
    assert!(meta
        .notice
        .unwrap()
        .contains("could not be completed or verified"));
    assert_eq!(
        engine.get_artifact(&aid).unwrap().content_md,
        before.content_md
    );
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row("SELECT count(*) FROM resume_brief_cache", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn unavailable_explicit_model_and_absent_local_pack_remain_factual() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, _) = project(&engine);
    save_settings(
        &engine,
        ResumeSettings {
            enabled: true,
            preferences: ResumePreferences::default(),
        },
    )
    .unwrap();
    assert_eq!(
        generate_brief(&engine, &pid)
            .await
            .unwrap()
            .generation
            .unwrap()
            .mode,
        "factual"
    );
    let mut preferences = ResumePreferences::default();
    preferences.model_mode = "local".into();
    save_settings(
        &engine,
        ResumeSettings {
            enabled: true,
            preferences,
        },
    )
    .unwrap();
    let result = generate_brief(&engine, &pid).await.unwrap();
    assert_eq!(result.generation.as_ref().unwrap().mode, "factual");
    assert!(result
        .generation
        .unwrap()
        .notice
        .unwrap()
        .contains("Download"));
    let mut invalid = ResumePreferences::default();
    invalid.model_id = Some("removed".into());
    assert!(save_settings(
        &engine,
        ResumeSettings {
            enabled: true,
            preferences: invalid
        }
    )
    .is_err());
}

#[tokio::test]
async fn source_and_model_changes_invalidate_cached_claims() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, aid) = project(&engine);
    let (url, server) = response_server(true);
    configured(&engine, &url);
    generate_brief(&engine, &pid).await.unwrap();
    server.join().unwrap();
    engine
        .db()
        .writer()
        .execute(
            "UPDATE providers SET base_url='http://127.0.0.1:1/v1' WHERE id='workspace-provider'",
            [],
        )
        .unwrap();
    assert_eq!(
        read_brief(&engine, &pid, 1)
            .unwrap()
            .generation
            .unwrap()
            .mode,
        "factual"
    );
    engine
        .update_artifact(&aid, "Changed saved work", sthira_core::Actor::User, None)
        .unwrap();
    let brief = read_brief(&engine, &pid, 1).unwrap();
    assert!(brief
        .recent_progress
        .iter()
        .all(|claim| !claim.text.contains("Mira")));
}

#[test]
fn settings_bound_sources_persist_and_never_rewrite_documents() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, aid) = project(&engine);
    let mut preferences = ResumePreferences::default();
    preferences.include_tasks = false;
    preferences.include_excerpts = false;
    preferences.max_sources = 2;
    let packet = engine.prepare_resume(&pid, &preferences).unwrap();
    assert!(packet.sources.is_empty());
    save_settings(
        &engine,
        ResumeSettings {
            enabled: true,
            preferences: preferences.clone(),
        },
    )
    .unwrap();
    drop(engine);
    let reopened = open(temp.path());
    assert_eq!(get_settings(&reopened).unwrap().preferences, preferences);
    assert!(reopened
        .get_artifact(&aid)
        .unwrap()
        .content_md
        .contains("Mira"));
    preferences.max_sources = 50;
    assert!(save_settings(
        &reopened,
        ResumeSettings {
            enabled: true,
            preferences
        }
    )
    .is_err());
}

#[test]
fn bounded_document_selection_changes_the_cache_basis_without_content_edits() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, first_id) = project(&engine);
    let second = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Second checklist"),
            "- [ ] Review the recovery report before release.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&pid, &second.id, "user_explicit", None)
        .unwrap();
    let preferences = ResumePreferences {
        max_sources: 1,
        ..Default::default()
    };
    engine
        .db()
        .writer()
        .execute(
            "UPDATE artifacts SET updated_at=20000 WHERE id=?1",
            [&first_id],
        )
        .unwrap();
    let before = engine.prepare_resume(&pid, &preferences).unwrap();
    assert_eq!(before.sources[0].artifact_id, first_id);
    engine
        .db()
        .writer()
        .execute(
            "UPDATE artifacts SET updated_at=30000 WHERE id=?1",
            [&second.id],
        )
        .unwrap();
    let after = engine.prepare_resume(&pid, &preferences).unwrap();
    assert_eq!(after.sources[0].artifact_id, second.id);
    assert_ne!(before.fingerprint, after.fingerprint);
}

#[test]
fn output_validation_rejects_fabricated_links_quotes_and_paraphrases() {
    let source = sthira_core::resume::ResumeSource {
        source_id: "s1".into(),
        artifact_id: "a1".into(),
        title: "Decision".into(),
        kind: "decision".into(),
        text: "We did not approve PostgreSQL. SQLite remains current.".into(),
    };
    for (text, id, quote) in [
        (
            "Migration approved.",
            "s1",
            "We did not approve PostgreSQL.",
        ),
        (
            "SQLite remains current.",
            "unknown",
            "SQLite remains current.",
        ),
        ("We did not approve PostgreSQL.", "s1", "Made up evidence."),
    ] {
        assert!(validate_output(&serde_json::json!({"items":[{"text":text,"evidence":[{"source_id":id,"quote":quote}]}]}).to_string(),std::slice::from_ref(&source)).is_err());
    }
}

#[tokio::test]
async fn changing_sources_during_generation_does_not_install_stale_cache() {
    struct MutatingModel {
        engine: Engine,
        artifact: String,
    }
    #[async_trait::async_trait]
    impl sthira_core::ModelProvider for MutatingModel {
        fn id(&self) -> &str {
            "mutating"
        }
        fn locality(&self) -> sthira_core::Locality {
            sthira_core::Locality::Local
        }
        async fn generate_json(
            &self,
            _schema: &serde_json::Value,
            _system: &str,
            prompt: &str,
        ) -> Result<String, sthira_core::CoreError> {
            let packet: serde_json::Value = serde_json::from_str(prompt)?;
            let source = &packet["sources"][0];
            self.engine.update_artifact(
                &self.artifact,
                "Newer work",
                sthira_core::Actor::User,
                None,
            )?;
            Ok(serde_json::json!({"items":[{"text":source["text"],"evidence":[{"source_id":source["source_id"],"quote":source["text"]}]}]}).to_string())
        }
        async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, sthira_core::CoreError> {
            unreachable!()
        }
    }
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, aid) = project(&engine);
    let model = MutatingModel {
        engine: engine.clone(),
        artifact: aid,
    };
    assert!(engine
        .synthesize_resume(
            &pid,
            &ResumePreferences::default(),
            &model,
            &ResumeModelIdentity {
                id: "test".into(),
                name: "Test".into(),
                provider_name: "Test".into(),
                cache_key: "test-key".into()
            },
            &|| true
        )
        .await
        .is_err());
    assert_eq!(
        engine
            .db()
            .reader()
            .unwrap()
            .query_row("SELECT count(*) FROM resume_brief_cache", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "Downloads the optional ~410 MB pack and runs real CPU inference; run explicitly"]
async fn real_local_pack_download_and_inference_smoke() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open(temp.path());
    let (pid, _) = project(&engine);
    assert!(!resume_pack::status(&engine).unwrap().ready);
    assert!(resume_pack::install(&engine).await.unwrap().ready);
    let mut preferences = ResumePreferences::default();
    preferences.model_mode = "local".into();
    save_settings(
        &engine,
        ResumeSettings {
            enabled: true,
            preferences,
        },
    )
    .unwrap();
    let brief = generate_brief(&engine, &pid).await.unwrap();
    assert_eq!(brief.generation.unwrap().mode, "generated");
    assert!(brief.recent_progress[0].text.contains("Mira"));
    assert!(!resume_pack::remove(&engine).unwrap().ready);
    assert!(engine.get_project(&pid).is_ok());
}
