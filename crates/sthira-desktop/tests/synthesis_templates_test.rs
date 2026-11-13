use std::sync::{Arc, Mutex};
use sthira_core::{ArtifactType, Authorship, Engine};
use sthira_desktop::{
    handle_derive_artifact, handle_list_synthesis_templates, handle_refine_artifact,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct TemplateFixture {
    base: String,
    prompts: Arc<Mutex<Vec<String>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TemplateFixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TemplateFixture {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let prompts_clone = prompts.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let prompts_inner = prompts_clone.clone();
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let (end, len) = loop {
                        let mut buf = [0; 8192];
                        let n = socket.read(&mut buf).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let head = String::from_utf8_lossy(&bytes[..end]);
                            let len = head
                                .lines()
                                .find_map(|line| {
                                    line.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                                })
                                .unwrap_or(0);
                            break (end + 4, len);
                        }
                    };
                    while bytes.len() < end + len {
                        let mut buf = [0; 8192];
                        let n = socket.read(&mut buf).await.unwrap_or(0);
                        if n == 0 {
                            break;
                        }
                        bytes.extend_from_slice(&buf[..n]);
                    }

                    let payload: serde_json::Value =
                        serde_json::from_slice(&bytes[end..]).unwrap_or_default();
                    let prompt = payload["prompt"].as_str().unwrap_or_default().to_string();
                    prompts_inner.lock().unwrap().push(prompt.clone());

                    let body = if prompt.contains("Scope & Objectives") {
                        serde_json::json!({
                            "response": serde_json::json!({
                                "title": "Execution Plan: Database Architecture",
                                "content_md": "# Execution Plan: Database Architecture\n\n## Scope & Objectives\nObjectives.\n\n## Phase 1: Foundation & Core Setup\nCore setup.\n\n## Phase 2: Feature Implementation\nFeature work.\n\n## Open Decisions & Trade-offs\nTrade-offs.",
                                "page_blocks": []
                            }).to_string(),
                            "done": true
                        }).to_string()
                    } else if prompt.contains("milestone execution plan") {
                        serde_json::json!({
                            "response": serde_json::json!({
                                "title": "Execution Plan: Database Architecture",
                                "content_md": "# Execution Plan: Database Architecture\n\n## Overview\nOverview without injected template skeleton.\n\n## Action Items\nAction items.",
                                "page_blocks": []
                            }).to_string(),
                            "done": true
                        }).to_string()
                    } else {
                        serde_json::json!({
                            "response": serde_json::json!({
                                "refined_content_md": "# Vector Indexing Options (Refined)\n\nIncorporate IVFFlat recall vs latency tradeoffs and memory boundaries.",
                                "page_blocks": []
                            }).to_string(),
                            "done": true
                        }).to_string()
                    };

                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                });
            }
        });
        Self {
            base,
            prompts,
            task,
        }
    }

    fn last_prompt(&self) -> String {
        self.prompts
            .lock()
            .unwrap()
            .last()
            .cloned()
            .unwrap_or_default()
    }
}

fn setup_test_engine(fixture_base: &str) -> (tempfile::TempDir, Arc<Engine>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("synthesis_templates_test.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-unused").with_panic_on_call());

    let engine = Arc::new(
        Engine::open(
            &db_path,
            &vault_dir,
            Arc::clone(&clock) as _,
            Arc::clone(&model) as _,
        )
        .unwrap(),
    );

    engine.db().writer().execute_batch(&format!(
        "INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('fixture','Local test fixture','ollama','{fixture_base}',1,0);
        INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('fixture-model','fixture','fixture-model','Fixture model',1,1,0);"
    )).unwrap();
    for workflow in ["plan", "draft"] {
        sthira_desktop::workflow_settings::save_workflow_setting(
            &engine,
            workflow,
            sthira_desktop::workflow_settings::WorkflowSetting {
                enabled: true,
                model_id: Some("fixture-model".into()),
                templates_enabled: workflow == "plan",
            },
        )
        .unwrap();
    }
    (tmp, engine)
}

#[tokio::test]
async fn test_synthesis_templates_seeding_and_filtering() {
    let fixture = TemplateFixture::start().await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    // 1. List all synthesis templates
    let all_templates = handle_list_synthesis_templates(&engine, None)
        .expect("Listing all synthesis templates must succeed");
    assert!(
        all_templates.len() >= 4,
        "Engine::open must pre-seed default templates (found {})",
        all_templates.len()
    );

    // 2. Filter by target_mode = "research"
    let research_templates = handle_list_synthesis_templates(&engine, Some("research".to_string()))
        .expect("Listing research templates must succeed");
    assert!(
        !research_templates.is_empty(),
        "Must have research templates"
    );
    for t in &research_templates {
        assert_eq!(t.target_mode, "research");
    }
    let default_research = research_templates.iter().find(|t| t.is_default);
    assert!(
        default_research.is_some(),
        "Default research template must exist"
    );
    let def_res = default_research.unwrap();
    assert_eq!(def_res.name, "Comparative Tech Evaluation");
    assert!(def_res
        .template_skeleton_md
        .contains("## Comparison Matrix"));

    // 3. Filter by target_mode = "plan"
    let plan_templates = handle_list_synthesis_templates(&engine, Some("plan".to_string()))
        .expect("Listing plan templates must succeed");
    assert!(!plan_templates.is_empty(), "Must have plan templates");
    for t in &plan_templates {
        assert_eq!(t.target_mode, "plan");
    }
    let default_plan = plan_templates.iter().find(|t| t.is_default);
    assert!(default_plan.is_some(), "Default plan template must exist");
    let def_plan = default_plan.unwrap();
    assert_eq!(def_plan.name, "Milestone Roadmap");
    assert!(def_plan.template_skeleton_md.contains("## Phase 1"));
    assert!(def_plan.template_skeleton_md.contains("## Phase 2"));
}

#[tokio::test]
async fn test_derive_artifact_with_template_skeleton_injection() {
    let fixture = TemplateFixture::start().await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    // 1. Create a source research note
    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Database Architecture Evaluation"),
            "# Database Architecture Evaluation\n\nExecutive findings comparing SQLite vs RocksDB.",
            None,
        )
        .expect("Create source artifact");

    // 2. Derive plan with explicit default plan template
    let derived_plan_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".to_string(),
        Some("plan-milestone-roadmap".to_string()),
        None,
        None,
    )
    .await
    .expect("Deriving plan with template must succeed");

    // Outbound prompt assertion: verify the actual template skeleton and directive reached the provider
    let outbound_prompt = fixture.last_prompt();
    assert!(
        outbound_prompt
            .contains("Synthesize strictly into this markdown skeleton preserving all headers:"),
        "Outbound prompt must include template synthesis directive"
    );
    assert!(
        outbound_prompt.contains("## Scope & Objectives"),
        "Outbound prompt must include skeleton header: Scope & Objectives"
    );
    assert!(
        outbound_prompt.contains("## Phase 1: Foundation & Core Setup"),
        "Outbound prompt must include skeleton header: Phase 1"
    );
    assert!(
        outbound_prompt.contains("## Phase 2: Feature Implementation"),
        "Outbound prompt must include skeleton header: Phase 2"
    );
    assert!(
        outbound_prompt.contains("## Open Decisions & Trade-offs"),
        "Outbound prompt must include skeleton header: Open Decisions & Trade-offs"
    );

    let derived_art = engine
        .get_artifact(&derived_plan_id)
        .expect("Fetch derived artifact");

    // Lineage verification
    assert_eq!(
        derived_art.derived_from_artifact_id,
        Some(src_art.id.clone()),
        "derived_from_artifact_id must match parent"
    );
    assert_eq!(
        derived_art.derivation_type,
        Some("plan_from_research".to_string())
    );

    // Skeleton preservation verification
    assert!(
        derived_art.content_md.contains("## Scope & Objectives"),
        "Derived plan must preserve template skeleton header: Scope & Objectives"
    );
    assert!(
        derived_art
            .content_md
            .contains("## Phase 1: Foundation & Core Setup"),
        "Derived plan must preserve template skeleton header: Phase 1"
    );
    assert!(
        derived_art
            .content_md
            .contains("## Phase 2: Feature Implementation"),
        "Derived plan must preserve template skeleton header: Phase 2"
    );
    assert!(
        derived_art
            .content_md
            .contains("## Open Decisions & Trade-offs"),
        "Derived plan must preserve template skeleton header: Open Decisions & Trade-offs"
    );
}

#[tokio::test]
async fn test_derive_artifact_with_disabled_template_omits_skeleton() {
    let fixture = TemplateFixture::start().await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    // Disable templates for plan workflow in settings
    sthira_desktop::workflow_settings::save_workflow_setting(
        &engine,
        "plan",
        sthira_desktop::workflow_settings::WorkflowSetting {
            enabled: true,
            model_id: Some("fixture-model".into()),
            templates_enabled: false,
        },
    )
    .unwrap();

    let src_art = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("No Template Research"),
            "# No Template Research\n\nRaw research content.",
            None,
        )
        .expect("Create source artifact");

    let derived_plan_id = handle_derive_artifact(
        &engine,
        src_art.id.clone(),
        "plan_from_research".to_string(),
        None,
        None,
        None,
    )
    .await
    .expect("Derivation without template must succeed");

    let outbound_prompt = fixture.last_prompt();
    assert!(
        !outbound_prompt
            .contains("Synthesize strictly into this markdown skeleton preserving all headers:"),
        "Outbound prompt must NOT contain template skeleton directive when templates are disabled"
    );
    assert!(
        !outbound_prompt.contains("## Scope & Objectives"),
        "Outbound prompt must NOT contain skeleton headers when templates are disabled"
    );

    let derived_art = engine
        .get_artifact(&derived_plan_id)
        .expect("Fetch derived artifact");
    assert_eq!(derived_art.derived_from_artifact_id, Some(src_art.id));
    assert!(
        !derived_art.content_md.contains("## Scope & Objectives"),
        "Derived artifact content must not contain template skeleton when disabled"
    );
    assert!(derived_art.content_md.contains("## Overview"));
}

#[tokio::test]
async fn test_refine_artifact_snapshots_and_reversibility() {
    let fixture = TemplateFixture::start().await;
    let (_tmp, engine) = setup_test_engine(&fixture.base);

    let initial = "# Vector Indexing Options\n\nInitial notes on HNSW and Flat L2.";
    let base = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Vector Indexing"),
            initial,
            None,
        )
        .expect("Create base note");

    let critique = "Incorporate IVFFlat recall vs latency tradeoffs and memory boundaries.";
    let refined =
        handle_refine_artifact(&engine, base.id.clone(), critique.to_string(), None, false)
            .await
            .expect("Refining artifact must succeed");

    assert!(refined.contains(critique));

    let updated = engine.get_artifact(&base.id).expect("Fetch updated note");
    assert_eq!(updated.content_md, refined);
}
