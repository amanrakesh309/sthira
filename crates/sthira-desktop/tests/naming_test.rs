use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::Arc,
    thread,
};
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_desktop::{
    handle_save_provider,
    naming::{suggest_titles, NamingChoice},
    resume::{get_settings, model_view, save_settings},
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
fn open_engine(root: &std::path::Path) -> Engine {
    Engine::open(
        root.join("sthira.db"),
        root.join("vault"),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("must-not-run").with_panic_on_call()),
    )
    .unwrap()
}
fn provider(engine: &Engine, url: &str) {
    handle_save_provider(
        engine,
        sthira_core::ProviderEntry {
            id: "provider".into(),
            display_name: "My LM Studio".into(),
            provider_type: "openai_compatible".into(),
            base_url: format!("{url}/v1"),
            has_api_key: false,
            is_enabled: true,
            models: vec![sthira_core::ProviderModelEntry {
                id: "model".into(),
                provider_id: "provider".into(),
                model_identifier: "user-chosen-model".into(),
                display_name: "My selected model".into(),
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
}
fn server(
    raw: serde_json::Value,
    hook: Option<Box<dyn FnOnce() + Send>>,
) -> (String, thread::JoinHandle<serde_json::Value>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let (end, length) = loop {
            let mut buf = [0; 8192];
            let n = stream.read(&mut buf).unwrap();
            assert_ne!(n, 0);
            bytes.extend_from_slice(&buf[..n]);
            if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&bytes[..i]);
                let len = header
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse::<usize>().unwrap())
                    })
                    .unwrap();
                break (i + 4, len);
            }
        };
        while bytes.len() < end + length {
            let mut buf = [0; 8192];
            let n = stream.read(&mut buf).unwrap();
            assert_ne!(n, 0);
            bytes.extend_from_slice(&buf[..n]);
        }
        let request = serde_json::from_slice(&bytes[end..]).unwrap();
        if let Some(hook) = hook {
            hook();
        }
        let body =
            serde_json::json!({"choices":[{"message":{"content":raw.to_string()}}]}).to_string();
        write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        request
    });
    (url, handle)
}
#[tokio::test]
async fn naming_uses_resume_choice_without_changing_manual_title_or_body() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open_engine(temp.path());
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("My manual title"),
            "The launch might slip; the vendor has not confirmed delivery.",
            None,
        )
        .unwrap();
    let (url, server) = server(
        serde_json::json!({"titles":["Possible launch delay","Pending vendor confirmation"]}),
        None,
    );
    provider(&engine, &url);
    // Explicit naming works independently of the switch for Resume summaries.
    assert!(!get_settings(&engine).unwrap().enabled);
    let before = engine.get_artifact(&note.id).unwrap();
    let suggested = suggest_titles(&engine, &note.id, NamingChoice::default())
        .await
        .unwrap();
    assert_eq!(suggested.model_name, "My selected model");
    assert_eq!(suggested.provider_name, "My LM Studio");
    assert_eq!(suggested.basis_title.as_deref(), Some("My manual title"));
    let request = server.join().unwrap();
    assert_eq!(request["model"], "user-chosen-model");
    assert!(request["messages"][1]["content"]
        .as_str()
        .unwrap()
        .contains("launch"));
    let after = engine.get_artifact(&note.id).unwrap();
    assert_eq!(after.title, before.title);
    assert_eq!(after.content_md, before.content_md);
    assert_eq!(after.version, before.version);
    let view = model_view(&engine, true).unwrap();
    assert!(view.available);
    assert_eq!(view.provider_name.as_deref(), Some("My LM Studio"));
    assert!(!model_view(&engine, false).unwrap().available);
}
#[tokio::test]
async fn missing_model_and_invalid_output_leave_saved_titles_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open_engine(temp.path());
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Manual title"),
            "Review storage checkpoints.",
            None,
        )
        .unwrap();
    assert!(suggest_titles(&engine, &note.id, NamingChoice::default())
        .await
        .is_err());
    let (url, server) = server(serde_json::json!({"titles":["# A fake heading"]}), None);
    provider(&engine, &url);
    assert!(suggest_titles(&engine, &note.id, NamingChoice::default())
        .await
        .is_err());
    server.join().unwrap();
    assert_eq!(engine.get_artifact(&note.id).unwrap().title, note.title);
}
#[tokio::test]
async fn source_changes_during_naming_reject_late_suggestions() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open_engine(temp.path());
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Manual title"),
            "Review storage checkpoints.",
            None,
        )
        .unwrap();
    let changed = engine.clone();
    let id = note.id.clone();
    let (url, server) = server(
        serde_json::json!({"titles":["Storage checkpoint review"]}),
        Some(Box::new(move || {
            changed
                .update_artifact(&id, "A different subject.", sthira_core::Actor::User, None)
                .unwrap();
        })),
    );
    provider(&engine, &url);
    assert!(suggest_titles(&engine, &note.id, NamingChoice::default())
        .await
        .unwrap_err()
        .contains("document changed"));
    server.join().unwrap();
    assert_eq!(engine.get_artifact(&note.id).unwrap().title, note.title);
}
#[tokio::test]
async fn changed_resume_selection_rejects_late_suggestions() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open_engine(temp.path());
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Manual title"),
            "Review storage checkpoints.",
            None,
        )
        .unwrap();
    let changed = engine.clone();
    let (url, server) = server(
        serde_json::json!({"titles":["Storage checkpoint review"]}),
        Some(Box::new(move || {
            let mut settings = get_settings(&changed).unwrap();
            settings.preferences.model_mode = "factual".into();
            save_settings(&changed, settings).unwrap();
        })),
    );
    provider(&engine, &url);
    assert!(suggest_titles(&engine, &note.id, NamingChoice::default())
        .await
        .is_err());
    server.join().unwrap();
    assert_eq!(engine.get_artifact(&note.id).unwrap().title, note.title);
}
#[test]
fn naming_format_validation_rejects_control_text_and_bounds_output() {
    for raw in [
        "{}",
        "{\"titles\":[]}",
        "{\"titles\":[\"Title\\nInjected\"]}",
        "{\"titles\":[\"single\"]}",
    ] {
        assert!(sthira_core::naming::validate(raw).is_err());
    }
    assert_eq!(
        sthira_core::naming::validate("{\"titles\":[\"Storage review\",\"storage review\"]}")
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn explicit_title_acceptance_persists_identity_body_and_mirror_and_rejects_stale_basis() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open_engine(temp.path());
    let original = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Manual heading"),
            "# Manual heading\n\nThe launch might slip. Nothing is approved.",
            None,
        )
        .unwrap();
    let updated = engine
        .apply_suggested_title(
            &original.id,
            "Possible launch delay",
            &original.content_hash,
            original.title.as_deref(),
        )
        .unwrap();
    assert_eq!(updated.id, original.id);
    assert_eq!(updated.authorship, original.authorship);
    assert_eq!(
        updated.content_md,
        original
            .content_md
            .replacen("# Manual heading", "# Possible launch delay", 1)
    );
    assert_eq!(updated.version, original.version + 1);
    let mirror =
        std::fs::read_to_string(engine.vault_dir().join(updated.file_path.as_ref().unwrap()))
            .unwrap();
    assert!(mirror.contains("Possible launch delay"));
    assert!(mirror.contains("Nothing is approved."));
    assert!(engine
        .apply_suggested_title(
            &original.id,
            "Another suggested title",
            &original.content_hash,
            original.title.as_deref()
        )
        .is_err());
    drop(engine);
    let reopened = open_engine(temp.path());
    assert_eq!(
        reopened
            .get_artifact(&original.id)
            .unwrap()
            .title
            .as_deref(),
        Some("Possible launch delay")
    );
}

#[test]
fn failed_title_mirror_write_keeps_the_saved_manual_title() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open_engine(temp.path());
    let original = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Manual title"),
            "Review the checkpoint schedule.",
            None,
        )
        .unwrap();
    let path = engine
        .vault_dir()
        .join(original.file_path.as_ref().unwrap());
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(engine
        .apply_suggested_title(
            &original.id,
            "Checkpoint schedule review",
            &original.content_hash,
            original.title.as_deref()
        )
        .is_err());
    let current = engine.get_artifact(&original.id).unwrap();
    assert_eq!(current.title, original.title);
    assert_eq!(current.version, original.version);
    assert_eq!(current.content_md, original.content_md);
}

#[test]
fn accepted_title_preserves_an_unrelated_heading_and_identical_title_is_a_noop() {
    let temp = tempfile::tempdir().unwrap();
    let engine = open_engine(temp.path());
    let original = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Manual metadata title"),
            "# Findings\n\nThe launch might slip.",
            None,
        )
        .unwrap();
    let unchanged = engine
        .apply_suggested_title(
            &original.id,
            "Manual metadata title",
            &original.content_hash,
            original.title.as_deref(),
        )
        .unwrap();
    assert_eq!(unchanged.version, original.version);
    let updated = engine
        .apply_suggested_title(
            &original.id,
            "Possible launch delay",
            &original.content_hash,
            original.title.as_deref(),
        )
        .unwrap();
    assert_eq!(updated.content_md, original.content_md);
    assert_eq!(updated.title.as_deref(), Some("Possible launch delay"));
}
