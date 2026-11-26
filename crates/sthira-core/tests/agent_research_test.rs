//! Agent Research/Plan gating, scope, snapshot, and review barrier against loopback fixtures.
//! These are deterministic local transports, not live-provider evidence.
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use sthira_core::{
    db::search_repo, llm::lifecycle::CancellationToken, search::ScrapedWebDocument,
    workspace_agent::*, ArtifactType, Authorship, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn open(path: &std::path::Path) -> Engine {
    Engine::open(
        path.join("sthira.db"),
        path.join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("unused-fallback")),
    )
    .unwrap()
}

fn configure(engine: &Engine, url: &str, workflows: &[&str], templates: bool) {
    let writer = engine.db().writer();
    writer.execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('fixture','Fixture provider','openai_compatible',?1,1,0)",[url]).unwrap();
    writer.execute_batch("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('fixture-model','fixture','agent-fixture','Fixture model',1,1,0);").unwrap();
    for workflow in workflows {
        writer
            .execute(
                "INSERT INTO workflow_settings(workflow,enabled,model_id,templates_enabled) VALUES(?1,1,'fixture-model',?2)",
                rusqlite::params![workflow, templates],
            )
            .unwrap();
    }
}

async fn request(socket: &mut tokio::net::TcpStream) -> Value {
    let mut bytes = vec![];
    let mut buffer = [0; 8192];
    loop {
        let count = socket.read(&mut buffer).await.unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..end]);
            let length: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|value| value.trim().parse().unwrap_or(0))
                })
                .unwrap_or(0);
            if bytes.len() >= end + 4 + length {
                return serde_json::from_slice(&bytes[end + 4..end + 4 + length]).unwrap();
            }
        }
    }
}
async fn fixture(actions: Vec<Value>) -> (String, tokio::task::JoinHandle<Vec<Value>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let mut requests = vec![];
        for action in actions {
            let (mut socket, _) = listener.accept().await.unwrap();
            let req = request(&mut socket).await;
            requests.push(req);
            let body = json!({"choices":[{"message":{"content":action.to_string()}}]}).to_string();
            let _ = socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
        }
        requests
    });
    (url, task)
}
fn count(engine: &Engine, table: &str) -> i64 {
    engine
        .db()
        .reader()
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}
fn goal(project: Option<&str>, research_template: Option<&str>, allow_web: bool) -> AgentGoal {
    AgentGoal {
        goal: "Research how this practice is understood today.".into(),
        project_id: project.map(str::to_owned),
        workspace_memory: false,
        model_id: "fixture-model".into(),
        allow_web,
        research_template_mode: Some(
            if research_template.is_some() {
                "explicit"
            } else {
                "inherit"
            }
            .into(),
        ),
        research_template_id: research_template.map(str::to_owned),
        plan_template_mode: Some("none".into()),
        plan_template_id: None,
    }
}

#[tokio::test]
async fn disabled_research_and_disabled_web_grant_fail_before_any_page_exists() {
    // Research workflow disabled: the tool is rejected and no generation runs.
    let (url, server) = fixture(vec![
        json!({"type":"propose_research","query":"A tradition","use_web":false}),
    ])
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    configure(&engine, &url, &["agent"], true);
    let run = engine.create_agent_run(goal(None, None, false)).unwrap();
    let failed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed
        .error
        .unwrap_or_default()
        .contains("Research is disabled"));
    assert_eq!(count(&engine, "artifacts"), 0);
    server.await.unwrap();

    // Research enabled but web permission off: use_web must fail without a network search.
    let (url, server) = fixture(vec![
        json!({"type":"propose_research","query":"A tradition","use_web":true}),
    ])
    .await;
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    configure(&engine, &url, &["agent", "research"], true);
    let run = engine.create_agent_run(goal(None, None, false)).unwrap();
    let failed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed
        .error
        .unwrap_or_default()
        .contains("Web research was not allowed"));
    assert_eq!(count(&engine, "artifacts"), 0);
    server.await.unwrap();
}

#[tokio::test]
async fn research_is_scoped_snapshotted_and_only_review_creates_the_page() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let private_project = engine.create_project("Private", None).unwrap();
    let allowed = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Allowed tradition note"),
            "A tradition is practised differently across regions.",
            None,
        )
        .unwrap();
    let private = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Private tradition note"),
            "PRIVATE-CONTEXT-SHOULD-NOT-LEAK about tradition.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &allowed.id, "user_explicit", None)
        .unwrap();
    engine
        .attach_artifact(&private_project.id, &private.id, "user_explicit", None)
        .unwrap();
    let template = engine
        .create_synthesis_template(&sthira_core::SynthesisTemplateDraft {
            id: None,
            name: "Four Lenses (agent)".into(),
            target_mode: "research".into(),
            description: "Evidence-led guide.".into(),
            template_skeleton_md: "# {{title}}\n\n## 1. What is it?\n\n## Sources\n".into(),
            guidance: "Preserve uncertainty.".into(),
            expected_version: None,
        })
        .unwrap();

    let (url, server) = fixture(vec![
        json!({"type":"propose_research","query":"tradition","use_web":false,"template_id":null}),
        json!({"synthesis":"Reported practice varies; the guide should preserve uncertainty.","page_blocks":[],"contradictions_and_evolution":["Sources disagree on origins."],"open_questions":["What remains unverified?"]}),
        json!({"type":"finish","summary":"A research proposal awaits review.","handoff":"Accept it before deriving a plan."}),
    ])
    .await;
    configure(&engine, &url, &["agent", "research"], true);
    // A configured search provider exists, but the run grant is off: expect zero calls.
    let search_hits = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let search_origin = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::clone(&search_hits);
    let search_task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            hits.fetch_add(1, Ordering::SeqCst);
            let body = "{\"results\":[]}";
            let _ = socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: {}
Connection: close

{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await;
        }
    });
    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO search_providers(id,display_name,provider_type,endpoint_url,is_enabled,is_default,created_at) VALUES('local','Local fixture','searxng',?1,1,1,0)",
            [&search_origin],
        )
        .unwrap();
    let run = engine
        .create_agent_run(goal(Some(&project.id), Some(&template.id), false))
        .unwrap();
    let reviewed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        reviewed.status, "review",
        "generated work must await review: {:?}",
        reviewed.error
    );
    assert_eq!(
        count(&engine, "artifacts"),
        2,
        "no knowledge page may exist before acceptance"
    );
    assert_eq!(count(&engine, "synthesis_template_runs"), 0);

    let step = reviewed
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposeResearch { .. }))
        .unwrap();
    let packet = &step.result["packet"];
    assert_eq!(packet["destination"]["project_id"], project.id);
    assert_eq!(packet["template"]["id"], template.id);
    assert_eq!(packet["template"]["version"], 1);
    assert_eq!(packet["web_used"], false);
    assert_eq!(packet["model_name"], "Fixture model");
    assert_eq!(
        packet["sources"][0]["url"],
        format!("sthira://artifact/{}", allowed.id)
    );

    // The private page must not reach model context or citations.
    let requests = server.await.unwrap();
    let finish_context: Value =
        serde_json::from_str(requests[2]["messages"][1]["content"].as_str().unwrap()).unwrap();
    let activity = &finish_context["activity"][0];
    assert_eq!(activity["action"]["query"], "tradition");
    assert_eq!(activity["result"]["title"], packet["title"]);
    assert_eq!(activity["result"]["grounding"], "local_vault_only");
    assert_eq!(activity["result"]["template"]["id"], template.id);
    assert!(activity["result"].get("content_md").is_none());
    assert!(requests.iter().all(|request| !request
        .to_string()
        .contains("PRIVATE-CONTEXT-SHOULD-NOT-LEAK")));
    assert_eq!(
        search_hits.load(Ordering::SeqCst),
        0,
        "web permission off must make zero search calls"
    );
    search_task.abort();

    // Editing the live template must not change this pending proposal.
    let mut edit = sthira_core::SynthesisTemplateDraft {
        id: Some(template.id.clone()),
        name: "Four Lenses (agent)".into(),
        target_mode: "research".into(),
        description: "Edited after dispatch.".into(),
        template_skeleton_md: "# {{title}}\n\n## Rewritten after dispatch\n".into(),
        guidance: String::new(),
        expected_version: Some(template.version),
    };
    edit.expected_version = Some(template.version);
    engine.update_synthesis_template(&edit).unwrap();

    let applied = engine
        .review_agent_step(&reviewed.id, &step.id, true)
        .unwrap();
    assert_eq!(count(&engine, "artifacts"), 3);
    assert_eq!(count(&engine, "synthesis_template_runs"), 1);
    let created = applied
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposeResearch { .. }))
        .and_then(|step| step.artifact_id.clone())
        .unwrap();
    let artifact = engine.get_artifact(&created).unwrap();
    assert_eq!(artifact.authorship, Authorship::AiGenerated);
    let applied_step = applied
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposeResearch { .. }))
        .unwrap();
    assert_eq!(
        applied_step.result["packet"]["sources"][0]["url"],
        format!("sthira://artifact/{}", allowed.id),
        "accepted work keeps its external/source attribution"
    );
    assert!(artifact.content_md.contains("Reported practice varies"));
    let stored: (String, i64, String, String, String) = engine.db().reader().unwrap().query_row(
        "SELECT template_id, template_version, template_mode, model_name, provider_name FROM synthesis_template_runs WHERE artifact_id=?1",
        [&created],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ).unwrap();
    assert_eq!(stored.0, template.id);
    assert_eq!(stored.1, 1);
    assert_eq!(stored.2, "explicit");
    assert_eq!(stored.3, "Fixture model");
    assert_eq!(stored.4, "Fixture provider");

    let research_member: bool = engine.db().reader().unwrap().query_row(
        "SELECT EXISTS(SELECT 1 FROM project_artifacts WHERE project_id=?1 AND artifact_id=?2 AND detached_at IS NULL)",
        rusqlite::params![project.id, created],
        |r| r.get(0),
    ).unwrap();
    assert!(
        research_member,
        "accepted Research page must belong to the intended project"
    );
    let created_for_reopen = created.clone();

    // A wrapped Inkling-style search response still resolves the accepted page and
    // lets the ordinary-language follow-up create a linked Plan proposal.
    let (follow_url, follow_server) = fixture(vec![
        json!({"name":"search","args":{"query":"Roots and Reason","type":"search"}}),
        json!({"name":"read_page","args":{"artifact_id":created,"type":"read_page"}}),
        json!({"type":"propose_plan","goal":"Plan a practical editorial launch","source_artifact_id":created,"template_id":null}),
        json!({"summary":"Editorial launch plan","phases":["Publish","Measure"],"open_questions":["Who approves?"],"project_name":"Roots and Reason","project_purpose":"Publish the guide","page_title":"Editorial launch plan","page_content":"# Editorial launch plan

## Guardrails

- Keep sources and review."}),
        json!({"type":"finish","summary":"A linked plan awaits review.","handoff":"Accept it to save the plan."}),
    ])
    .await;
    engine
        .db()
        .writer()
        .execute(
            "UPDATE providers SET base_url=?1 WHERE id='fixture'",
            [&follow_url],
        )
        .unwrap();
    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO workflow_settings(workflow,enabled,model_id,templates_enabled) VALUES('plan',1,'fixture-model',1)",
            [],
        )
        .unwrap();
    let follow = engine
        .follow_up_agent_run(
            &applied.id,
            "Turn that saved research into a practical launch plan.",
            None,
        )
        .unwrap();
    let follow_review = engine
        .execute_agent_run(&follow.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(follow_review.status, "review", "{:?}", follow_review.error);
    let plan_step = follow_review
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposePlan { .. }))
        .expect("wrapped follow-up must reach a plan proposal");
    assert_eq!(plan_step.result["packet"]["source"]["artifact_id"], created);
    assert_eq!(
        plan_step.result["packet"]["derivation"]["type"],
        "plan_from_research"
    );
    let follow_applied = engine
        .review_agent_step(&follow_review.id, &plan_step.id, true)
        .unwrap();
    let plan_id = follow_applied
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposePlan { .. }))
        .and_then(|step| step.artifact_id.clone())
        .unwrap();
    let plan = engine.get_artifact(&plan_id).unwrap();
    assert_eq!(plan.derived_from_artifact_id, Some(created.clone()));
    assert_eq!(plan.derivation_type, Some("plan_from_research".into()));
    let reopened = Engine::open(
        tmp.path().join("sthira.db"),
        tmp.path().join("vault"),
        Arc::new(FakeClock::new(1_700_000_000_000)),
        Arc::new(DeterministicFakeModel::new("reopen")),
    )
    .unwrap();
    let reopened_plan = reopened.get_artifact(&plan_id).unwrap();
    assert_eq!(
        reopened_plan.derived_from_artifact_id,
        Some(created_for_reopen.clone())
    );
    assert_eq!(
        reopened_plan.derivation_type,
        Some("plan_from_research".into())
    );
    let reopened_research_member: bool = reopened.db().reader().unwrap().query_row(
        "SELECT EXISTS(SELECT 1 FROM project_artifacts WHERE project_id=?1 AND artifact_id=?2 AND detached_at IS NULL)",
        rusqlite::params![project.id, created_for_reopen],
        |r| r.get(0),
    ).unwrap();
    assert!(reopened_research_member);
    let reopened_template_id: String = reopened
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT template_id FROM synthesis_template_runs WHERE artifact_id=?1",
            [&created_for_reopen],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(reopened_template_id, template.id);
    let reopened_plan_member: bool = reopened.db().reader().unwrap().query_row(
        "SELECT EXISTS(SELECT 1 FROM project_artifacts WHERE project_id=?1 AND artifact_id=?2 AND detached_at IS NULL)",
        rusqlite::params![project.id, plan_id],
        |r| r.get(0),
    ).unwrap();
    assert!(reopened_plan_member);
    follow_server.await.unwrap();

    // Reviewing twice cannot create a duplicate page.
    assert!(engine
        .review_agent_step(&reviewed.id, &step.id, true)
        .is_err());
    assert_eq!(count(&engine, "artifacts"), 4);
}

#[tokio::test]
async fn out_of_scope_page_cannot_become_a_plan_source() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let private_project = engine.create_project("Private", None).unwrap();
    let private = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Private page"),
            "Private scope.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&private_project.id, &private.id, "user_explicit", None)
        .unwrap();
    let (url, server) = fixture(vec![
        json!({"type":"read_page","artifact_id":private.id}),
        json!({"type":"finish","summary":"Should not run.","handoff":""}),
    ])
    .await;
    configure(&engine, &url, &["agent", "plan"], true);
    let run = engine
        .create_agent_run(goal(Some(&project.id), None, false))
        .unwrap();
    let failed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert!(failed
        .steps
        .iter()
        .all(|step| !matches!(step.action, AgentAction::ProposePlan { .. })));
    assert_eq!(count(&engine, "artifacts"), 1);
    server.await.unwrap();
}

#[tokio::test]
async fn dismissing_a_local_research_proposal_creates_no_artifact_or_receipt() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Local tradition note"),
            "A tradition is practised differently across regions.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();
    let (url, server) = fixture(vec![
        json!({"type":"propose_research","query":"tradition","use_web":false,"template_id":null}),
        json!({"synthesis":"Local evidence varies.","page_blocks":[],"contradictions_and_evolution":[],"open_questions":[]}),
        json!({"type":"finish","summary":"Review the local proposal.","handoff":"Dismiss or accept it."}),
    ])
    .await;
    configure(&engine, &url, &["agent", "research"], true);
    let run = engine
        .create_agent_run(goal(Some(&project.id), None, false))
        .unwrap();
    let reviewed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(reviewed.status, "review", "{:?}", reviewed.error);
    let step = reviewed
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposeResearch { .. }))
        .unwrap();
    engine
        .review_agent_step(&reviewed.id, &step.id, false)
        .unwrap();
    assert_eq!(count(&engine, "artifacts"), 1);
    assert_eq!(count(&engine, "synthesis_template_runs"), 0);
    let state: String = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT state FROM agent_steps WHERE id=?1",
            [&step.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "dismissed");
    server.await.unwrap();
}

#[tokio::test]
async fn accepted_research_can_derive_a_reviewed_plan_with_real_lineage() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let research = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::AiGenerated,
            Some("Roots and Reason: Four Lenses"),
            "# Four Lenses\n\nReported practice varies. What remains uncertain is listed below.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &research.id, "user_explicit", None)
        .unwrap();

    let (url, server) = fixture(vec![
        json!({"type":"read_page","artifact_id":research.id}),
        json!({"type":"propose_plan","goal":"Plan a website implementation","source_artifact_id":research.id,"template_id":null}),
        json!({"summary":"Implementation plan","phases":["Research","Build"],"open_questions":["Which sources are primary?"],"project_name":"Roots and Reason","project_purpose":"Publish the guide","page_title":"Website implementation plan","page_content":"# Website implementation plan\n\n## Guardrails\n\n- Keep sources and review."}),
        json!({"type":"finish","summary":"A plan proposal awaits review.","handoff":"Accept it to save the page."}),
    ]).await;
    configure(&engine, &url, &["agent", "plan"], true);
    let run = engine
        .create_agent_run(goal(Some(&project.id), None, false))
        .unwrap();
    let reviewed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(reviewed.status, "review", "{:?}", reviewed.error);
    assert_eq!(
        count(&engine, "artifacts"),
        1,
        "plan generation must not publish a page"
    );

    let step = reviewed
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposePlan { .. }))
        .unwrap();
    let packet = &step.result["packet"];
    assert_eq!(packet["source"]["artifact_id"], research.id);
    assert_eq!(packet["source"]["version"], research.version);
    assert_eq!(packet["derivation"]["type"], "plan_from_research");
    assert_eq!(packet["web_used"], false);
    server.await.unwrap();

    let applied = engine
        .review_agent_step(&reviewed.id, &step.id, true)
        .unwrap();
    let plan_id = applied
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposePlan { .. }))
        .and_then(|step| step.artifact_id.clone())
        .unwrap();
    let plan = engine.get_artifact(&plan_id).unwrap();
    assert_eq!(plan.derived_from_artifact_id, Some(research.id.clone()));
    assert_eq!(plan.derivation_type, Some("plan_from_research".to_string()));
    assert_eq!(plan.authorship, Authorship::AiGenerated);
    let plan_history: (String, String) = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT template_id, template_mode FROM synthesis_template_runs WHERE artifact_id=?1",
            [&plan_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(plan_history, ("__none__".to_string(), "none".to_string()));
    assert_eq!(count(&engine, "artifacts"), 2);
    assert!(engine
        .review_agent_step(&reviewed.id, &step.id, true)
        .is_err());
}

#[tokio::test]
async fn owner_template_choice_cannot_be_replaced_or_added_by_the_model() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let owner = engine
        .create_synthesis_template(&sthira_core::SynthesisTemplateDraft {
            id: None,
            name: "Owner choice".into(),
            target_mode: "research".into(),
            description: String::new(),
            template_skeleton_md: "# {{title}}\n".into(),
            guidance: String::new(),
            expected_version: None,
        })
        .unwrap();

    // Explicit owner template: the model may not substitute another template.
    let (url, server) = fixture(vec![json!({"type":"propose_research","query":"tradition","use_web":false,"template_id":"research-deep-dive"})]).await;
    configure(&engine, &url, &["agent", "research"], true);
    let run = engine
        .create_agent_run(goal(None, Some(&owner.id), false))
        .unwrap();
    let failed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert!(failed.error.unwrap_or_default().contains("cannot replace"));
    assert_eq!(count(&engine, "artifacts"), 0);
    server.await.unwrap();

    // Owner "no template": the model cannot add one later.
    let (url, server) = fixture(vec![json!({"type":"propose_research","query":"tradition","use_web":false,"template_id":"research-deep-dive"})]).await;
    engine
        .db()
        .writer()
        .execute(
            "UPDATE providers SET base_url=?1 WHERE id='fixture'",
            [&url],
        )
        .unwrap();
    let mut none = goal(None, None, false);
    none.research_template_mode = Some("none".into());
    let run = engine.create_agent_run(none).unwrap();
    let failed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(failed.status, "failed");
    let error = failed.error.unwrap_or_default();
    assert!(error.contains("Do not substitute a template"), "{error}");
    assert_eq!(count(&engine, "artifacts"), 0);
    server.await.unwrap();
}

#[tokio::test]
async fn a_changed_plan_source_rejects_application_without_creating_a_page() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let research = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Accepted research"),
            "# Accepted research\n\nFindings to base a plan on.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &research.id, "user_explicit", None)
        .unwrap();
    let (url, server) = fixture(vec![
        json!({"type":"read_page","artifact_id":research.id}),
        json!({"type":"propose_plan","goal":"Plan next steps","source_artifact_id":research.id}),
        json!({"summary":"Plan","phases":["A","B"],"open_questions":[],"project_name":"Roots and Reason","project_purpose":"Guide","page_title":"Plan next steps","page_content":"# Plan next steps\n\n- [ ] Review sources"}),
        json!({"type":"finish","summary":"Plan ready for review.","handoff":"Review and accept the plan."}),
    ])
    .await;
    configure(&engine, &url, &["agent", "plan"], true);
    let run = engine
        .create_agent_run(goal(Some(&project.id), None, false))
        .unwrap();
    let reviewed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(reviewed.status, "review", "{:?}", reviewed.error);
    server.await.unwrap();
    engine
        .save_artifact(
            &research.id,
            sthira_core::SaveArtifactRequest {
                content_md: "# Accepted research\n\nThe owner added new findings.",
                title: Some("Accepted research"),
                actor: sthira_core::Actor::User,
                context_project_id: Some(&project.id),
                authorship: None,
                is_unfinished_draft: None,
            },
        )
        .unwrap();
    let step = reviewed
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposePlan { .. }))
        .unwrap();
    let error = engine
        .review_agent_step(&reviewed.id, &step.id, true)
        .expect_err("stale plan source must not apply");
    assert!(error.to_string().contains("source changed"), "{error}");
    assert_eq!(count(&engine, "artifacts"), 1);
    assert_eq!(
        engine
            .agent_run(&reviewed.id)
            .unwrap()
            .steps
            .iter()
            .find(|value| value.id == step.id)
            .unwrap()
            .state,
        "pending"
    );
}

/// Accepts the first agent action immediately, then holds the nested generation call
/// until the test observes progress or releases it.
async fn delayed_fixture(
    first: Value,
    second: Value,
    started: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        for (index, action) in [first, second].into_iter().enumerate() {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let _ = request(&mut socket).await;
            if index == 1 {
                started.notify_waiters();
                let _ =
                    tokio::time::timeout(std::time::Duration::from_secs(10), release.notified())
                        .await;
            }
            let body = json!({"choices":[{"message":{"content":action.to_string()}}]}).to_string();
            let _ = socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await;
        }
    });
    (url, task)
}

#[tokio::test]
async fn progress_reports_real_generation_stages_and_cancellation_prevents_a_proposal() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let allowed = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Tradition note"),
            "A tradition is practised differently across regions.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &allowed.id, "user_explicit", None)
        .unwrap();
    let template = engine
        .create_synthesis_template(&sthira_core::SynthesisTemplateDraft {
            id: None,
            name: "Progress template".into(),
            target_mode: "research".into(),
            description: String::new(),
            template_skeleton_md: "# {{title}}\n\n## Scope\n".into(),
            guidance: String::new(),
            expected_version: None,
        })
        .unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let (url, server) = delayed_fixture(
        json!({"type":"propose_research","query":"tradition","use_web":false,"template_id":null}),
        json!({"synthesis":"Held synthesis.","page_blocks":[],"contradictions_and_evolution":[],"open_questions":[]}),
        Arc::clone(&started),
        Arc::clone(&release),
    )
    .await;
    configure(&engine, &url, &["agent", "research"], true);
    let run = engine
        .create_agent_run(goal(Some(&project.id), Some(&template.id), false))
        .unwrap();
    let token = CancellationToken::new();
    let engine_for_task = engine.clone();
    let id = run.id.clone();
    let token_for_task = token.clone();
    let handle = tokio::spawn(async move {
        engine_for_task
            .execute_agent_run(&id, &token_for_task)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), started.notified())
        .await
        .expect("nested generation request should start");

    // The durable stage history shows the genuine stages that actually ran.
    let events: Vec<String> = engine
        .db()
        .reader()
        .unwrap()
        .prepare(
            "SELECT stage FROM agent_progress_events WHERE run_id=?1 ORDER BY created_at, rowid",
        )
        .unwrap()
        .query_map([&run.id], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        events
            .iter()
            .any(|stage| stage == "context" || stage == "template"),
        "reading/template progress should be recorded: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|stage| stage == "vault" || stage == "synthesizing"),
        "research retrieval/generation progress should be recorded: {events:?}"
    );
    assert!(
        events.iter().any(|stage| stage == "synthesizing"),
        "model generation progress should be recorded: {events:?}"
    );

    // Cancel while the nested model call is still in flight.
    engine.cancel_agent_run(&run.id).unwrap();
    release.notify_waiters();
    let finished = tokio::time::timeout(std::time::Duration::from_secs(5), handle)
        .await
        .expect("run should stop")
        .unwrap()
        .unwrap();
    assert_eq!(finished.status, "cancelled");
    assert_eq!(
        count(&engine, "artifacts"),
        1,
        "no knowledge page may be created"
    );
    let pending: i64 = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM agent_steps WHERE run_id=?1 AND state='pending'",
            [&run.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        pending, 0,
        "a late nested response must not create a reviewable proposal"
    );
    let progress: (Option<String>, Option<String>) = engine
        .db()
        .reader()
        .unwrap()
        .query_row(
            "SELECT progress_stage, progress_message FROM agent_runs WHERE id=?1",
            [&run.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(progress.0.as_deref(), Some("cancelled"));
    server.abort();
}

#[tokio::test]
async fn blocked_web_search_fallback_records_zero_external_sources_not_web_used() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Local tradition note"),
            "A tradition is practised differently across regions.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();
    let (url, server) = fixture(vec![
        json!({"type":"propose_research","query":"tradition","use_web":true,"template_id":null}),
        json!({"queries":["tradition"]}),
        json!({"synthesis":"Saved local notes describe regional variation.","page_blocks":[],"contradictions_and_evolution":[],"open_questions":[]}),
        json!({"title":"Local tradition research"}),
        json!({"type":"finish","summary":"A research proposal awaits review.","handoff":"Accept it."}),
    ])
    .await;
    configure(&engine, &url, &["agent", "research"], false);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let search_origin = format!("http://{}", listener.local_addr().unwrap());
    let search_task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let body = "{\"results\":[]}";
            let _ = socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .as_bytes(),
                )
                .await;
        }
    });
    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO search_providers(id,display_name,provider_type,endpoint_url,is_enabled,is_default,created_at) VALUES('local','Local fixture','searxng',?1,1,1,0)",
            [&search_origin],
        )
        .unwrap();
    let run = engine
        .create_agent_run(goal(Some(&project.id), None, true))
        .unwrap();
    let reviewed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(reviewed.status, "review", "{:?}", reviewed.error);
    let step = reviewed
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposeResearch { .. }))
        .unwrap();
    let packet = &step.result["packet"];
    assert_eq!(packet["web_grant"], true);
    assert_eq!(packet["search_requested"], true);
    assert_eq!(packet["search_attempted"], true);
    assert_eq!(packet["web_used"], false);
    assert_eq!(packet["external_sources_count"], 0);
    assert_eq!(packet["search_status"], "web_search_failed");
    assert_eq!(
        packet["sources"][0]["url"],
        format!("sthira://artifact/{}", note.id)
    );
    server.await.unwrap();
    search_task.abort();
}

#[tokio::test]
async fn nested_malformed_model_output_and_detached_source_fail_safely() {
    // 1. Malformed synthesis output inside ProposeResearch leaves zero artifacts or receipts.
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Local tradition note"),
            "A tradition is practised differently across regions.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/v1", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for index in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let _ = request(&mut socket).await;
            let content = if index == 0 {
                json!({"type":"propose_research","query":"tradition","use_web":false,"template_id":null}).to_string()
            } else {
                "{not-valid-json".to_string()
            };
            let body = json!({"choices":[{"message":{"content":content}}]}).to_string();
            let _ = socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await;
        }
    });
    configure(&engine, &url, &["agent", "research", "plan"], true);
    let run = engine
        .create_agent_run(goal(Some(&project.id), None, false))
        .unwrap();
    let failed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(count(&engine, "artifacts"), 1);
    assert_eq!(count(&engine, "synthesis_template_runs"), 0);
    assert_eq!(count(&engine, "agent_steps"), 0);
    server.await.unwrap();

    // 2. Detaching a Plan source page from project scope before approval rejects apply cleanly.
    let (plan_url, plan_server) = fixture(vec![
        json!({"type":"read_page","artifact_id":note.id}),
        json!({"type":"propose_plan","goal":"Plan launch from local page","source_artifact_id":note.id,"template_id":null}),
        json!({"summary":"Launch plan","phases":["Prepare"],"open_questions":[],"project_name":"Roots and Reason","project_purpose":"Guide","page_title":"Launch plan","page_content":"# Launch plan\n\n- Step 1"}),
        json!({"type":"finish","summary":"Plan awaits review.","handoff":"Review before apply."}),
    ])
    .await;
    engine
        .db()
        .writer()
        .execute(
            "UPDATE providers SET base_url=?1 WHERE id='fixture'",
            [&plan_url],
        )
        .unwrap();
    let plan_run = engine
        .create_agent_run(goal(Some(&project.id), None, false))
        .unwrap();
    let reviewed = engine
        .execute_agent_run(&plan_run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(reviewed.status, "review", "{:?}", reviewed.error);
    plan_server.await.unwrap();

    engine.detach_artifact(&project.id, &note.id).unwrap();
    let step = reviewed
        .steps
        .iter()
        .find(|step| matches!(step.action, AgentAction::ProposePlan { .. }))
        .unwrap();
    let err = engine
        .review_agent_step(&reviewed.id, &step.id, true)
        .expect_err("out-of-scope detached source must reject acceptance");
    assert!(err.to_string().contains("scope"), "{err}");
    assert_eq!(count(&engine, "artifacts"), 1);
    assert_eq!(count(&engine, "synthesis_template_runs"), 0);
    assert_eq!(
        engine
            .agent_run(&reviewed.id)
            .unwrap()
            .steps
            .iter()
            .find(|s| s.id == step.id)
            .unwrap()
            .state,
        "pending"
    );
}

#[tokio::test]
async fn agent_research_loopback_partial_retrieval_and_provenance() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();

    // Loopback HTTP server serving both SearXNG JSON search results and article HTML
    // on the same authorized origin.
    let search_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", search_listener.local_addr().unwrap());
    let cached_url = format!("{origin}/cached.html");
    let fresh_url = format!("{origin}/fresh.html");
    let broken_url = format!("{origin}/broken.html");

    search_repo::save_cached_web_document(
        &engine.db().writer(),
        &ScrapedWebDocument {
            url: cached_url.clone(),
            title: "Tradition &#039;One&#039; &amp; Cache".into(),
            content_md: "# Tradition One\n\nCached field notes record regional lantern designs."
                .into(),
            published_date: None,
            fetched_at: None,
            from_cache: false,
        },
        1_700_000_000_000,
    )
    .unwrap();

    let origin_clone = origin.clone();
    let search_task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = search_listener.accept().await else {
                break;
            };
            let origin_req = origin_clone.clone();
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                let n = socket.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let path = req
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");
                let (status, ctype, body) = if path.starts_with("/search") {
                    let body = json!({
                        "results": [
                            {"title": "Cached source", "url": format!("{origin_req}/cached.html"), "content": "Cached snippet"},
                            {"title": "Tradition &mdash; Fresh", "url": format!("{origin_req}/fresh.html"), "content": "Fresh snippet"},
                            {"title": "Broken source", "url": format!("{origin_req}/broken.html"), "content": "Broken snippet"}
                        ]
                    })
                    .to_string();
                    ("200 OK", "application/json", body)
                } else if path.starts_with("/fresh.html") {
                    let body = "<html><head><title>Tradition &mdash; Fresh</title></head><body><article><p>Freshly fetched article notes confirm evening street performances.</p></article></body></html>".to_string();
                    ("200 OK", "text/html; charset=utf-8", body)
                } else {
                    (
                        "500 Internal Server Error",
                        "text/plain",
                        "Upstream error".to_string(),
                    )
                };
                let resp = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            });
        }
    });

    // Schema-aware loopback model server.
    let model_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let model_url = format!("http://{}/v1", model_listener.local_addr().unwrap());
    let model_task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = model_listener.accept().await else {
                break;
            };
            let req = request(&mut socket).await;
            let req_text = req.to_string();
            let content = if req_text.contains("contradictions_and_evolution") {
                json!({
                    "synthesis": "Cached and fresh sources align on festival details [Source 1] [Source 2].",
                    "page_blocks": [],
                    "contradictions_and_evolution": [],
                    "open_questions": ["Are additional rural traditions documented?"]
                })
                .to_string()
            } else if req_text.contains("exact_quote") {
                if req_text.contains("evening street performances") {
                    json!({"findings": [{"statement": "Fresh article covers evening performances.", "exact_quote": "Freshly fetched article notes confirm evening street performances."}]}).to_string()
                } else {
                    json!({"findings": [{"statement": "Cached notes record lantern designs.", "exact_quote": "Cached field notes record regional lantern designs."}]}).to_string()
                }
            } else if req_text.contains("\"queries\"") {
                json!({"queries": ["tradition"]}).to_string()
            } else if req_text.contains("Summarize the user's research goal") {
                json!({"title": "Tradition Partial Retrieval Study"}).to_string()
            } else {
                let user_msg = req["messages"][1]["content"].as_str().unwrap_or("");
                let activity_empty = user_msg.contains("\"activity\":[]");
                if activity_empty {
                    json!({"type":"propose_research","query":"tradition","use_web":true,"template_id":null}).to_string()
                } else {
                    json!({"type":"finish","summary":"Research proposal with partial web retrieval is ready.","handoff":"Review source provenance."}).to_string()
                }
            };
            let body = json!({"choices":[{"message":{"content":content}}]}).to_string();
            let _ = socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                )
                .await;
        }
    });

    configure(&engine, &model_url, &["agent", "research"], false);
    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO search_providers(id,display_name,provider_type,endpoint_url,is_enabled,is_default,created_at) VALUES('loopback-searx','Loopback SearXNG','searxng',?1,1,1,0)",
            [&origin],
        )
        .unwrap();

    let run = engine
        .create_agent_run(goal(Some(&project.id), None, true))
        .unwrap();
    let reviewed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(reviewed.status, "review", "{:?}", reviewed.error);

    let step = reviewed
        .steps
        .iter()
        .find(|s| matches!(s.action, AgentAction::ProposeResearch { .. }))
        .unwrap();
    let packet = &step.result["packet"];
    assert_eq!(packet["web_used"], true);
    assert_eq!(packet["external_sources_count"], 2);
    assert_eq!(packet["search_status"], "web_search_success");
    let warnings = packet["warnings"].as_array().unwrap();
    assert!(
        warnings.iter().any(|w| w
            .as_str()
            .unwrap_or("")
            .contains("Partial article retrieval")),
        "missing partial retrieval warning: {warnings:?}"
    );
    let docs = packet["documents"].as_array().unwrap();
    assert_eq!(docs.len(), 2);
    let cached_doc = docs.iter().find(|d| d["url"] == cached_url).unwrap();
    let fresh_doc = docs.iter().find(|d| d["url"] == fresh_url).unwrap();
    assert_eq!(cached_doc["from_cache"], true);
    assert!(cached_doc["fetched_at"].as_i64().is_some());
    assert_eq!(fresh_doc["from_cache"], false);
    assert!(docs.iter().all(|d| d["url"] != broken_url));

    let sources = packet["sources"].as_array().unwrap();
    assert_eq!(sources[0]["title"], "Tradition 'One' & Cache");
    assert_eq!(sources[1]["title"], "Tradition — Fresh");

    search_task.abort();
    model_task.abort();
}

#[tokio::test]
async fn agent_research_loopback_blocked_search_keeps_web_used_false() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = open(tmp.path());
    let project = engine.create_project("Roots and Reason", None).unwrap();
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Local tradition note"),
            "A tradition is practised differently across regions.",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();

    // Loopback HTTP search server returning HTTP 429 rate-limited / blocked response.
    let search_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", search_listener.local_addr().unwrap());
    let search_task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = search_listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let _ = socket.read(&mut buf).await;
                let body = json!({"error": "rate limit exceeded"}).to_string();
                let resp = format!(
                    "HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(resp.as_bytes()).await;
            });
        }
    });

    let (model_url, model_server) = fixture(vec![
        json!({"type":"propose_research","query":"tradition","use_web":true,"template_id":null}),
        json!({"queries":["tradition"]}),
        json!({"synthesis":"Saved local notes describe regional variation.","page_blocks":[],"contradictions_and_evolution":[],"open_questions":[]}),
        json!({"title":"Local fallback research"}),
        json!({"type":"finish","summary":"Local fallback research awaits review.","handoff":"Search was blocked."}),
    ])
    .await;

    configure(&engine, &model_url, &["agent", "research"], false);
    engine
        .db()
        .writer()
        .execute(
            "INSERT INTO search_providers(id,display_name,provider_type,endpoint_url,is_enabled,is_default,created_at) VALUES('loopback-blocked','Loopback Blocked','searxng',?1,1,1,0)",
            [&origin],
        )
        .unwrap();

    let run = engine
        .create_agent_run(goal(Some(&project.id), None, true))
        .unwrap();
    let reviewed = engine
        .execute_agent_run(&run.id, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(reviewed.status, "review", "{:?}", reviewed.error);
    let step = reviewed
        .steps
        .iter()
        .find(|s| matches!(s.action, AgentAction::ProposeResearch { .. }))
        .unwrap();
    let packet = &step.result["packet"];
    assert_eq!(packet["web_grant"], true);
    assert_eq!(packet["search_attempted"], true);
    assert_eq!(packet["web_used"], false);
    assert_eq!(packet["external_sources_count"], 0);
    assert_eq!(packet["search_status"], "web_search_failed");
    assert_eq!(
        packet["sources"][0]["url"],
        format!("sthira://artifact/{}", note.id)
    );

    model_server.await.unwrap();
    search_task.abort();
}
