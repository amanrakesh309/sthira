use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sthira-cli"))
}
fn cli(tmp: &tempfile::TempDir, args: &[&str], token: Option<&str>) -> std::process::Output {
    let mut command = bin();
    command
        .arg("--workspace")
        .arg(tmp.path())
        .args(args)
        .env_remove("STHIRA_ACCESS_TOKEN");
    if let Some(t) = token {
        command.env("STHIRA_ACCESS_TOKEN", t);
    }
    command.output().unwrap()
}
fn json_output(output: std::process::Output) -> Value {
    assert!(
        output.status.success(),
        "stderr: {} stdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn init() -> (tempfile::TempDir, String, String) {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(json_output(cli(&tmp, &["init"], None))["initialized"], true);
    let grant=json!({"name":"Transport client","whole_workspace":true,"workspace_memory":false,"can_propose":true,"model":"Declared model","provider":"Declared provider"}).to_string();
    let value = json_output(cli(&tmp, &["access", "grant", &grant], None));
    let token = value["token"].as_str().unwrap().into();
    let id = value["connection"]["id"].as_str().unwrap().into();
    (tmp, token, id)
}
struct Server {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<String>,
}
impl Server {
    fn start(tmp: &tempfile::TempDir, token: &str) -> Self {
        let mut child = bin()
            .arg("--workspace")
            .arg(tmp.path())
            .arg("mcp")
            .env("STHIRA_ACCESS_TOKEN", token)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, output) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        Self {
            child,
            input,
            output,
        }
    }
    fn raw(&mut self, text: &str) -> Value {
        writeln!(self.input, "{text}").unwrap();
        self.input.flush().unwrap();
        serde_json::from_str(
            &self
                .output
                .recv_timeout(Duration::from_secs(15))
                .expect("MCP response timed out"),
        )
        .unwrap()
    }
    fn send(&mut self, value: Value) -> Value {
        self.raw(&value.to_string())
    }
    fn notify(&mut self, value: Value) {
        writeln!(self.input, "{value}").unwrap();
        self.input.flush().unwrap();
    }
    fn initialize(&mut self) {
        let result=self.send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"Test","version":"1"},"capabilities":{}}}));
        assert_eq!(result["result"]["protocolVersion"], "2025-11-25");
        self.notify(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    }
    fn call(&mut self, name: &str, args: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":"tool","method":"tools/call","params":{"name":name,"arguments":args,"_meta":{"progressToken":"optional"}}}))
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
#[test]
fn actual_cli_can_initialize_discover_propose_review_and_revoke() {
    let (tmp, token, id) = init();
    let schema = json_output(cli(&tmp, &["api", "schema"], None));
    assert_eq!(schema.as_array().unwrap().len(), 4);
    assert!(!schema.to_string().contains("review_agent_step"));
    for tool in schema.as_array().unwrap() {
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
    let memory = &schema[1]["inputSchema"]["oneOf"];
    assert_eq!(memory.as_array().unwrap().len(), 4);
    assert!(!memory.to_string().contains("\"archive\""));
    assert!(
        !schema[2]["inputSchema"]["definitions"]["AgentAction"]["oneOf"]
            .to_string()
            .contains("\"finish\"")
    );
    let request=json!({"request_id":"first","goal":"Prepare a new project for review.","actions":[{"type":"propose_project_create","name":"CLI project","purpose":"Explore uncertainty.","reason":"Keep work together."}]}).to_string();
    let first = json_output(cli(
        &tmp,
        &["api", "call", "sthira_workspace", &request],
        Some(&token),
    ));
    let run = &first["structuredContent"]["run"];
    assert_eq!(run["steps"][0]["state"], "pending");
    let run_id = run["id"].as_str().unwrap();
    let step_id = run["steps"][0]["id"].as_str().unwrap();
    let accepted = json_output(cli(&tmp, &["review", "apply", run_id, step_id], None));
    assert_eq!(accepted["steps"][0]["state"], "applied");
    assert!(accepted["steps"][0]["result"]["project_id"].is_string());
    let undo = json_output(cli(&tmp, &["review", "undo", run_id, step_id], None));
    assert_eq!(undo["steps"][0]["state"], "reverted");
    let retry = json_output(cli(
        &tmp,
        &["api", "call", "sthira_workspace", &request],
        Some(&token),
    ));
    assert_eq!(retry["structuredContent"]["replayed"], true);
    assert!(retry["structuredContent"].get("run").is_none());
    json_output(cli(&tmp, &["access", "revoke", &id], None));
    assert!(
        !cli(&tmp, &["api", "call", "sthira_status", "{}"], Some(&token))
            .status
            .success()
    );
}
#[test]
fn stdio_handshake_discovery_errors_and_revocation_use_real_transport() {
    let (tmp, token, id) = init();
    let mut server = Server::start(&tmp, &token);
    assert_eq!(
        server.send(json!({"jsonrpc":"2.0","id":0,"method":"tools/list"}))["error"]["code"],
        -32000
    );
    server.initialize();
    let listed =
        server.send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":{}}}));
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 4);
    assert_eq!(server.raw("{broken json")["error"]["code"], -32700);
    assert_eq!(
        server.call("owner_review", json!({}))["error"]["code"],
        -32602
    );
    assert_eq!(
        server.call("sthira_status", json!({"owner":true}))["error"]["code"],
        -32602
    );
    assert_eq!(
        server.call("sthira_status", json!({}))["result"]["isError"],
        false
    );
    json_output(cli(&tmp, &["access", "revoke", &id], None));
    let denied = server.call("sthira_status", json!({}));
    assert_eq!(denied["error"]["code"], -32001);
    assert!(!denied.to_string().contains(&token));
}
#[test]
fn stdio_proposals_and_memory_are_scoped_and_never_self_approved() {
    let (tmp, token, _) = init();
    let mut server = Server::start(&tmp, &token);
    server.initialize();
    let submitted=server.call("sthira_workspace",json!({"request_id":"mcp-create","goal":"Propose an AI page.","actions":[{"type":"propose_create","title":"MCP proposal","content_md":"Unverified external reasoning."}]}));
    assert_eq!(submitted["result"]["isError"], false);
    let run = &submitted["result"]["structuredContent"]["run"];
    assert_eq!(run["steps"][0]["state"], "pending");
    assert!(run["steps"][0]["artifact_id"].is_null());
    let denied=server.call("sthira_memory",json!({"type":"create","draft":{"scope":{"kind":"workspace"},"title":"Do not save","body":"Unapproved","source_ids":[],"pinned":false}}));
    assert_eq!(denied["result"]["isError"], true);
    let read = server.call(
        "sthira_memory",
        json!({"type":"context","query":{},"max_entries":10,"max_chars":8000}),
    );
    assert_eq!(read["result"]["isError"], false);
    assert!(!cli(&tmp, &["api", "call", "sthira_status", "{}"], None)
        .status
        .success());
}
#[test]
fn oversized_frames_and_invalid_ids_are_rejected_without_nonprotocol_stdout() {
    let (tmp, token, _) = init();
    let mut server = Server::start(&tmp, &token);
    server.initialize();
    assert_eq!(
        server.send(json!({"jsonrpc":"2.0","id":false,"method":"ping"}))["error"]["code"],
        -32600
    );
    assert_eq!(server.raw(&"x".repeat(262145))["error"]["code"], -32700);
}
#[test]
fn missing_vaults_and_repeat_init_do_not_create_or_recover_data() {
    let tmp = tempfile::tempdir().unwrap();
    let out = cli(
        &tmp,
        &["api", "call", "sthira_status", "{}"],
        Some(&"x".repeat(64)),
    );
    assert!(!out.status.success());
    assert!(!tmp.path().join("sthira.db").exists());
    json_output(cli(&tmp, &["init"], None));
    assert!(!cli(&tmp, &["init"], None).status.success());
}
#[test]
fn scoped_stdio_reads_do_not_leak_other_project_pages_or_allow_write_escalation() {
    use std::sync::Arc;
    use sthira_core::{ArtifactType, Authorship, Engine};
    use sthira_testkit::{DeterministicFakeModel, FakeClock};
    let (tmp, _, _) = init();
    let e = Engine::attach(
        tmp.path().join("sthira.db"),
        tmp.path(),
        Arc::new(FakeClock::new(100)),
        Arc::new(DeterministicFakeModel::new("unused").with_panic_on_call()),
    )
    .unwrap();
    let p = e.create_project("Allowed", None).unwrap();
    let a = e
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Allowed page"),
            "Visible context",
            None,
        )
        .unwrap();
    e.attach_artifact(&p.id, &a.id, "user_explicit", None)
        .unwrap();
    let b = e
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Unrelated page"),
            "Private unrelated body",
            None,
        )
        .unwrap();
    let config=json!({"name":"Scoped reader","project_id":p.id,"can_propose":false,"model":"Declared model","provider":"Declared provider"}).to_string();
    let credentials = json_output(cli(&tmp, &["access", "grant", &config], None));
    let token = credentials["token"].as_str().unwrap();
    let mut server = Server::start(&tmp, token);
    server.initialize();
    let read=server.call("sthira_workspace",json!({"request_id":"scoped-read","goal":"Inspect only granted context.","actions":[{"type":"read_page","artifact_id":a.id},{"type":"read_page","artifact_id":b.id}]}));
    assert_eq!(read["result"]["isError"], true);
    assert!(read.to_string().contains("Visible context"));
    assert!(!read.to_string().contains("Private unrelated body"));
    let write=server.call("sthira_workspace",json!({"request_id":"denied-write","goal":"Cannot elevate access.","actions":[{"type":"propose_create","title":"Unapproved","content_md":"Must not save"}]}));
    assert_eq!(write["result"]["isError"], true);
    assert!(write.to_string().contains("cannot propose"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_handoff_crosses_real_mcp_then_returns_to_configured_native_model() {
    use std::sync::Arc;
    use sthira_core::{
        llm::lifecycle::CancellationToken, workspace_agent::AgentGoal, ArtifactType, Authorship,
        Engine,
    };
    use sthira_testkit::{DeterministicFakeModel, FakeClock};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (tmp, token, connection) = init();
    let e = Engine::attach(
        tmp.path().join("sthira.db"),
        tmp.path(),
        Arc::new(FakeClock::new(1000)),
        Arc::new(DeterministicFakeModel::new("unused").with_panic_on_call()),
    )
    .unwrap();
    let p = e.create_project("Scoped release", None).unwrap();
    let page = e
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Release uncertainty"),
            "The date remains unknown.",
            None,
        )
        .unwrap();
    e.attach_artifact(&p.id, &page.id, "user_explicit", None)
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/v1", listener.local_addr().unwrap());
    e.db().writer().execute("INSERT INTO providers(id,display_name,provider_type,base_url,is_enabled,created_at) VALUES('fixture','Loopback fixture','openai_compatible',?1,1,0)",[base]).unwrap();
    e.db().writer().execute_batch("INSERT INTO provider_models(id,provider_id,model_identifier,display_name,is_enabled,is_default,created_at) VALUES('native-model','fixture','native-fixture','Native fixture model',1,1,0); INSERT INTO workflow_settings(workflow,enabled) VALUES('agent',1);").unwrap();
    let provider = tokio::spawn(async move {
        let mut prompts = vec![];
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut data = vec![];
            let mut buf = [0; 4096];
            loop {
                let n = socket.read(&mut buf).await.unwrap();
                assert!(n > 0);
                data.extend_from_slice(&buf[..n]);
                if let Some(end) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                    let header = String::from_utf8_lossy(&data[..end]);
                    let length: usize = header
                        .lines()
                        .find_map(|l| {
                            l.to_lowercase()
                                .strip_prefix("content-length:")
                                .map(|v| v.trim().parse().unwrap())
                        })
                        .unwrap();
                    if data.len() >= end + 4 + length {
                        prompts.push(
                            serde_json::from_slice::<Value>(&data[end + 4..end + 4 + length])
                                .unwrap(),
                        );
                        break;
                    }
                }
            }
            let action = json!({"type":"finish","summary":"The date remains uncertain; contribution inspected.","handoff":"Check unresolved evidence."});
            let body = json!({"choices":[{"message":{"content":action.to_string()}}]}).to_string();
            socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
        }
        prompts
    });
    let native = e
        .create_agent_run(AgentGoal {
            goal: "Inspect release uncertainty.".into(),
            project_id: Some(p.id),
            workspace_memory: false,
            model_id: "native-model".into(),
        })
        .unwrap();
    let native = e
        .execute_agent_run(&native.id, &CancellationToken::new())
        .await
        .unwrap();
    let d=json!({"request_id":"cli-handoff","run_id":native.id,"expected_run_revision":native.revision,"recipient_connection_id":connection,"title":"Release continuity","next_steps":"Review the unresolved wording.","open_questions":"Which evidence is missing?","artifact_ids":[page.id],"memory_ids":[]}).to_string();
    let saved = json_output(cli(&tmp, &["handoff", "save", &d], None));
    let id = saved["handoff"]["id"].as_str().unwrap();
    let mut server = Server::start(&tmp, &token);
    server.initialize();
    let read = server.call("sthira_handoffs", json!({"type":"read","id":id}));
    assert_eq!(read["result"]["isError"], false);
    let packet = &read["result"]["structuredContent"]["handoff"];
    let contributed=server.call("sthira_handoffs",json!({"type":"contribute","contribution":{"handoff_id":id,"expected_revision":packet["handoff"]["revision"],"request_id":"external-continuation","summary":"Proposed wording while preserving uncertainty.","next_steps":"Review the wording.","open_questions":"The date is not confirmed.","actions":[{"type":"read_page","artifact_id":page.id},{"type":"propose_refine","artifact_id":page.id,"content_md":"The date remains unknown; review the evidence."}]}}));
    assert_eq!(contributed["result"]["isError"], false);
    let external = &contributed["result"]["structuredContent"]["run"];
    assert_eq!(external["project_id"], json!(native.project_id));
    let external_id = external["id"].as_str().unwrap();
    let step = external["steps"][1]["id"].as_str().unwrap();
    assert_eq!(e.get_artifact(&page.id).unwrap().version, 1);
    json_output(cli(&tmp, &["review", "apply", external_id, step], None));
    let current = json_output(cli(&tmp, &["handoff", "read", id], None));
    assert_eq!(current["sources_changed"], true);
    let request=json!({"id":id,"request_id":"native-return","expected_context_version":current["context_version"],"text":"Continue after the reviewed external contribution.","model_id":"native-model"}).to_string();
    let continued = json_output(cli(&tmp, &["handoff", "continue", &request], None));
    assert_eq!(continued["status"], "completed");
    let requests = provider.await.unwrap();
    let context: Value =
        serde_json::from_str(requests[1]["messages"][1]["content"].as_str().unwrap()).unwrap();
    assert_eq!(context["saved_handoff"]["handoff_id"], id);
    assert_eq!(
        context["saved_handoff"]["contributions"][0]["current_review"]["proposals"][0]["state"],
        "applied"
    );
    assert_eq!(
        e.get_artifact(&page.id).unwrap().authorship,
        Authorship::User
    );
}
#[test]
fn owner_cli_migration_preview_apply_retry_and_rollback_preserve_export_text() {
    let (tmp, _, _) = init();
    let manifest=json!({"request_id":"import-cli","source":"notion","collection":"export-one","project_id":null,"files":[{"path":"Question.md","content":"# Question\nThe date is unconfirmed."},{"path":"database.csv","content":"Name,Status"}]}).to_string();
    let preview = json_output(cli(&tmp, &["migration", "preview", &manifest], None));
    assert_eq!(preview["status"], "preview");
    assert_eq!(preview["plan"]["items"][1]["status"], "unsupported");
    let id = preview["id"].as_str().unwrap();
    let applied = json_output(cli(&tmp, &["migration", "apply", id, "1"], None));
    assert_eq!(applied["receipt"]["pages"].as_array().unwrap().len(), 1);
    let retry = json_output(cli(&tmp, &["migration", "apply", id, "1"], None));
    assert_eq!(retry["receipt"]["pages"], applied["receipt"]["pages"]);
    let rolled = json_output(cli(&tmp, &["migration", "rollback", id, "2"], None));
    assert_eq!(rolled["status"], "rolled_back");
    assert_eq!(
        rolled["plan"]["items"][0]["original"],
        "# Question\nThe date is unconfirmed."
    );
}
