//! Real stdio MCP subprocess against a loopback mock of the session service. The MCP transport and
//! the shared tool surface are exercised here; the service authorization itself is covered by
//! `sthira-session-service/tests/agent_access_test.rs`.
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read as _, Write},
    net::TcpListener,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

struct Mock {
    base: String,
    seen: Arc<Mutex<Vec<(String, String)>>>,
}

fn start_mock() -> Mock {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&seen);
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let _ = stream.set_read_timeout(Some(Duration::from_millis(400)));
            let mut raw = Vec::new();
            let mut buffer = [0u8; 16384];
            loop {
                match stream.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        raw.extend_from_slice(&buffer[..n]);
                        if let Some(index) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                            let header_end = index + 4;
                            let headers = String::from_utf8_lossy(&raw[..header_end]).to_string();
                            let length = headers
                                .lines()
                                .find_map(|line| {
                                    let lower = line.to_ascii_lowercase();
                                    lower
                                        .strip_prefix("content-length:")
                                        .and_then(|value| value.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            if raw.len() >= header_end + length {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            let request = String::from_utf8_lossy(&raw).to_string();
            let mut lines = request.lines();
            let request_line = lines.next().unwrap_or_default();
            let mut authorization = String::new();
            for line in lines {
                let lower = line.to_ascii_lowercase();
                if let Some(value) = lower.strip_prefix("authorization:") {
                    authorization = value.trim().to_string();
                }
            }
            let body_start = request.find("\r\n\r\n").map(|i| i + 4).unwrap_or(raw.len());
            let body = if raw.len() > body_start {
                String::from_utf8_lossy(&raw[body_start..]).to_string()
            } else {
                String::new()
            };
            let path = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_string();
            recorded.lock().unwrap().push((path.clone(), authorization));
            let (status, payload) = if path == "/v1/agent-access/status" {
                (
                    "200 OK",
                    json!({
                        "connectionId":"agc_1","workspaceId":"ws1","workspaceName":"Alpha Team",
                        "projectId":"proj1","projectName":"Alpha","memberDisplayName":"B",
                        "deviceLabel":"dev-b","permission":"read","generation":1,"createdAtMs":1,
                        "pages":[{"pageId":"shp_ok","title":"One","currentRevisionNumber":2,"available":true}],
                        "limits":{"maxPagesPerList":50,"maxContentBytes":49152,"canSuggest":false}
                    }),
                )
            } else if path.starts_with("/v1/agent-access/pages/shp_ok") {
                (
                    "200 OK",
                    json!({
                        "pageId":"shp_ok","workspaceId":"ws1","projectId":"proj1","title":"One",
                        "contentMd":"complete body","revisionId":"rev_2","revisionNumber":2,
                        "contentSha256":"deadbeef","pageUpdatedAtMs":5,"complete":true,
                        "provenance":{"originKind":"userAuthored"},
                        "contributorNames":[]
                    }),
                )
            } else if path == "/v1/agent-access/pages"
                || path.starts_with("/v1/agent-access/pages?")
            {
                (
                    "200 OK",
                    json!({"items":[{"pageId":"shp_ok","title":"One","revisionNumber":2,
                        "updatedAtMs":5,"snippet":"complete body"}],"nextCursor":null}),
                )
            } else if path == "/v1/agent-access/proposals/edit" {
                let id = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| v["requestId"].as_str().map(str::to_owned))
                    .unwrap_or_default();
                (
                    "200 OK",
                    json!({"proposalId":format!("prp_{id}"),"kind":"edit","status":"pending",
                        "modelTrust":"declared_unverified","projectId":"proj1",
                        "targetPageId":"shp_ok","appliedPageId":null}),
                )
            } else {
                (
                    "404 Not Found",
                    json!({"code":"page_not_found","error":"This page is not available to this agent connection."}),
                )
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.to_string().len()
            );
            let _ = stream.write_all(response.as_bytes());
            let _ = stream.flush();
        }
    });
    Mock { base, seen }
}

struct Mcp {
    child: Child,
    input: ChildStdin,
    output: mpsc::Receiver<String>,
}

impl Mcp {
    fn start(connection_file: &std::path::Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_sthira-cli"))
            .arg("shared-mcp")
            .arg("--connection-file")
            .arg(connection_file)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, output) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let _ = tx.send(line);
            }
        });
        Self {
            child,
            input,
            output,
        }
    }

    fn send(&mut self, value: Value) {
        writeln!(self.input, "{value}").unwrap();
        self.input.flush().unwrap();
    }

    fn recv(&self) -> Value {
        let line = self
            .output
            .recv_timeout(Duration::from_secs(10))
            .expect("MCP response timed out");
        serde_json::from_str(&line).unwrap()
    }

    fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        self.recv()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn shared_mcp_subprocess_scopes_tools_and_never_exposes_the_credential() {
    let mock = start_mock();
    let dir = tempfile::tempdir().unwrap();
    let token = "stha_test_secret_credential_value";
    let file = dir.path().join("connection.json");
    std::fs::write(
        &file,
        serde_json::to_vec(&json!({
            "serviceUrl": mock.base,
            "connectionId": "agc_1",
            "token": token,
        }))
        .unwrap(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    let mut mcp = Mcp::start(&file);
    let init = mcp.request(
        1,
        "initialize",
        json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"test"},"capabilities":{}}),
    );
    assert_eq!(init["result"]["serverInfo"]["name"], "sthira-shared");
    mcp.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));

    let tools = mcp.request(2, "tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "sthira_shared_status",
            "sthira_shared_pages",
            "sthira_shared_propose",
            "sthira_shared_request",
        ]
    );
    assert!(!names
        .iter()
        .any(|name| name.starts_with("sthira_workspace")));

    let status = mcp.request(
        3,
        "tools/call",
        json!({"name":"sthira_shared_status","arguments":{}}),
    );
    assert_eq!(
        status["result"]["structuredContent"]["projectName"],
        "Alpha"
    );
    assert_eq!(
        status["result"]["structuredContent"]["limits"]["canSuggest"],
        false
    );

    let read = mcp.request(
        4,
        "tools/call",
        json!({"name":"sthira_shared_pages","arguments":{"action":"read","pageId":"shp_ok"}}),
    );
    assert_eq!(read["result"]["structuredContent"]["complete"], true);
    assert_eq!(read["result"]["structuredContent"]["revisionNumber"], 2);
    assert_eq!(
        read["result"]["structuredContent"]["contentSha256"],
        "deadbeef"
    );

    let propose = mcp.request(
        5,
        "tools/call",
        json!({"name":"sthira_shared_propose","arguments":{
            "action":"edit","requestId":"r-1","pageId":"shp_ok","baseRevisionNumber":2,
            "baseContentSha256":"deadbeef","proposedTitle":"One","proposedContentMd":"x",
            "summary":"s","agentLabel":"Antigravity","modelLabel":"declared-model",
            "providerLabel":"declared-provider"
        }}),
    );
    assert_eq!(propose["result"]["structuredContent"]["status"], "pending");
    assert_eq!(
        propose["result"]["structuredContent"]["modelTrust"],
        "declared_unverified"
    );

    let denied = mcp.request(
        6,
        "tools/call",
        json!({"name":"sthira_shared_pages","arguments":{"action":"read","pageId":"shp_other"}}),
    );
    assert_eq!(denied["result"]["isError"], true);
    assert!(denied["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("page_not_found"));

    let invalid = mcp.request(
        7,
        "tools/call",
        json!({"name":"sthira_shared_pages","arguments":{"action":"read"}}),
    );
    assert_eq!(invalid["error"]["code"], -32602);

    // The credential is never in the process arguments nor in any MCP output.
    if let Ok(cmdline) = std::fs::read(format!("/proc/{}/cmdline", mcp.child.id())) {
        assert!(!String::from_utf8_lossy(&cmdline).contains(token));
    }
    for (_, authorization) in mock.seen.lock().unwrap().iter() {
        assert_eq!(authorization, &format!("bearer {token}"));
    }
    assert!(mock
        .seen
        .lock()
        .unwrap()
        .iter()
        .all(|(path, _)| !path.contains(token)));
}

#[test]
fn shared_connection_file_must_be_private() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("connection.json");
    std::fs::write(
        &file,
        serde_json::to_vec(&json!({
            "serviceUrl": "http://127.0.0.1:1",
            "connectionId": "agc_1",
            "token": "stha_example",
        }))
        .unwrap(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(sthira_cli::shared_access::load_connection(&file).is_err());
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(sthira_cli::shared_access::load_connection(&file).is_ok());
    }
}

#[test]
fn shared_connection_requires_tls_outside_loopback() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("connection.json");
    for (url, allowed) in [
        ("http://127.0.0.1:1234", true),
        ("http://[::1]:1234", true),
        ("https://service.example.invalid", true),
        ("http://192.168.1.2:1234", false),
        ("https://user:password@service.example.invalid", false),
        ("https://service.example.invalid/?token=secret", false),
        ("https://service.example.invalid/#secret", false),
        ("https://service.example.invalid/path", false),
    ] {
        std::fs::write(
            &file,
            json!({"serviceUrl":url,"connectionId":"agc_test","token":"stha_fixture"}).to_string(),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        assert_eq!(
            sthira_cli::shared_access::load_connection(&file).is_ok(),
            allowed,
            "{url}"
        );
    }
}

#[test]
fn shared_mcp_refuses_redirects_and_oversized_responses() {
    for oversized in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let (sent, received) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            if oversized {
                let body = format!("\"{}\"", "x".repeat(1_048_577));
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            } else {
                write!(stream, "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{address}/stolen\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").unwrap();
            }
            drop(stream);
            listener.set_nonblocking(true).unwrap();
            thread::sleep(Duration::from_millis(200));
            sent.send(listener.accept().is_ok()).unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("connection.json");
        std::fs::write(
            &file,
            json!({"serviceUrl":format!("http://{address}"),
            "connectionId":"agc_test","token":"stha_fixture"})
            .to_string(),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        let mut client = Mcp::start(&file);
        client.request(
            1,
            "initialize",
            json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"test"},"capabilities":{}}),
        );
        client.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        let result = client.request(
            2,
            "tools/call",
            json!({"name":"sthira_shared_status","arguments":{}}),
        );
        assert_eq!(result["result"]["isError"], true, "{result}");
        assert!(!result.to_string().contains("stha_fixture"));
        assert!(
            !received.recv_timeout(Duration::from_secs(3)).unwrap(),
            "redirect was followed"
        );
        server.join().unwrap();
    }
}

struct TeamFixtureVerifier;

#[async_trait::async_trait]
impl sthira_session_service::IdentityVerifier for TeamFixtureVerifier {
    async fn verify_enrollment(
        &self,
        token: &str,
        _: Option<sthira_session_service::NonceExpectation>,
        _: i64,
    ) -> Result<sthira_session_service::VerifiedIdentity, sthira_session_service::VerifyError> {
        let value: Value = serde_json::from_str(token)
            .map_err(|_| sthira_session_service::VerifyError::InvalidProofFormat)?;
        let field = |name: &str| {
            value[name]
                .as_str()
                .map(str::to_owned)
                .ok_or(sthira_session_service::VerifyError::InvalidProofFormat)
        };
        Ok(sthira_session_service::VerifiedIdentity {
            uid: field("uid")?,
            email: field("email")?,
            display_name: field("uid")?,
            raw_nonce: field("nonce")?,
        })
    }
}

async fn team_api(
    http: &reqwest::Client,
    method: reqwest::Method,
    url: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Value {
    let mut request = http.request(method, url);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status();
    let value: Value = response.json().await.unwrap();
    assert!(status.is_success(), "{status}: {value}");
    value
}

#[tokio::test(flavor = "multi_thread", worker_threads = 3)]
async fn two_accounts_review_and_disconnect_through_real_service_and_mcp() {
    use reqwest::Method;
    use sthira_session_service::{
        membership::SqliteWorkspaceMembershipStore, shared_content::SqliteSharedContentStore,
        SessionServiceState, SqliteAuthSessionStore,
    };
    let temp = tempfile::tempdir().unwrap();
    let auth = Arc::new(SqliteAuthSessionStore::open(temp.path().join("auth.db")).unwrap());
    let members =
        Arc::new(SqliteWorkspaceMembershipStore::open(temp.path().join("team.db")).unwrap());
    let content = Arc::new(SqliteSharedContentStore::enable_on_membership_store(&members).unwrap());
    let state = SessionServiceState::new(auth, Arc::new(TeamFixtureVerifier))
        .with_membership_store(members)
        .with_shared_agent_store(content.clone())
        .with_shared_content_store(content);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let service = tokio::spawn(sthira_session_service::serve(listener, state, stopped));
    let http = reqwest::Client::new();
    let mut accounts = Vec::new();
    for uid in ["owner", "editor"] {
        let challenge = team_api(
            &http,
            Method::POST,
            &format!("{base}/v1/auth/challenges"),
            None,
            Some(json!({"device_id":uid,"device_label":uid})),
        )
        .await;
        let enrolled = team_api(
            &http,
            Method::POST,
            &format!("{base}/v1/sessions/enroll"),
            None,
            Some(
                json!({"challenge_id":challenge["challenge_id"],"device_id":uid,"device_label":uid,
                "google_id_token":json!({"uid":uid,"email":format!("{uid}@example.invalid"),
                    "nonce":challenge["nonce"]}).to_string()}),
            ),
        )
        .await;
        accounts.push(enrolled["credential"].as_str().unwrap().to_owned());
    }
    let a = &accounts[0];
    let b = &accounts[1];
    let workspace = team_api(
        &http,
        Method::POST,
        &format!("{base}/v1/workspaces"),
        Some(a),
        Some(json!({"name":"MCP Team","idempotency_key":"workspace"})),
    )
    .await;
    let ws = workspace["workspace"]["workspaceId"].as_str().unwrap();
    let invite = team_api(
        &http,
        Method::POST,
        &format!("{base}/v1/workspaces/{ws}/invitations"),
        Some(a),
        Some(json!({"email":"editor@example.invalid","role":"editor"})),
    )
    .await;
    team_api(
        &http,
        Method::POST,
        &format!("{base}/v1/invitations/accept"),
        Some(b),
        Some(json!({"invite_code":invite["inviteCode"]})),
    )
    .await;
    let page = team_api(
        &http,
        Method::POST,
        &format!("{base}/v1/workspaces/{ws}/pages"),
        Some(a),
        Some(
            json!({"title":"Shared source","contentMd":"Original text.","entryMode":"new",
            "provenance":{"originKind":"userAuthored"},"idempotencyKey":"page"}),
        ),
    )
    .await;
    let page_id = page["page"]["page"]["pageId"].as_str().unwrap();
    let project = team_api(
        &http,
        Method::POST,
        &format!("{base}/v1/workspaces/{ws}/projects"),
        Some(a),
        Some(json!({"name":"Project","purpose":"","idempotencyKey":"project"})),
    )
    .await;
    let project_id = project["project"]["project"]["projectId"].as_str().unwrap();
    team_api(
        &http,
        Method::POST,
        &format!("{base}/v1/workspaces/{ws}/projects/{project_id}/pages"),
        Some(a),
        Some(json!({"pageId":page_id,"idempotencyKey":"attach"})),
    )
    .await;
    team_api(
        &http,
        Method::PUT,
        &format!("{base}/v1/workspaces/{ws}/agent-access"),
        Some(a),
        Some(json!({"enabled":true})),
    )
    .await;
    let mut connections = Vec::new();
    for (account, permission) in [(a, "read"), (b, "suggest")] {
        connections.push(
            team_api(
                &http,
                Method::POST,
                &format!("{base}/v1/workspaces/{ws}/agent-connections"),
                Some(account),
                Some(
                    json!({"projectId":project_id,"pageIds":[page_id],"permission":permission,
                "clientName":"QA MCP","label":"QA connection"}),
                ),
            )
            .await,
        );
    }
    let delegated = &connections[1];
    let connection_id = delegated["connection"]["connectionId"].as_str().unwrap();
    let credential = delegated["credential"].as_str().unwrap();
    let file = temp.path().join("mcp-connection.json");
    std::fs::write(
        &file,
        json!({"serviceUrl":base,"connectionId":connection_id,"token":credential}).to_string(),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut mcp = Mcp::start(&file);
    mcp.request(
        1,
        "initialize",
        json!({"protocolVersion":"2025-11-25","clientInfo":{"name":"QA"},"capabilities":{}}),
    );
    mcp.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let read = mcp.request(
        2,
        "tools/call",
        json!({"name":"sthira_shared_pages","arguments":{"action":"read","pageId":page_id}}),
    );
    let basis = &read["result"]["structuredContent"];
    assert_eq!(basis["contentMd"], "Original text.");
    let proposed = mcp.request(
        3,
        "tools/call",
        json!({"name":"sthira_shared_propose","arguments":{
            "action":"edit","requestId":"edit","pageId":page_id,"baseRevisionNumber":1,
            "baseContentSha256":basis["contentSha256"],"proposedTitle":"Shared source",
            "proposedContentMd":"Original text.\n\nAgent suggestion.","summary":"Review",
            "agentLabel":"QA MCP","modelLabel":"declared fixture","providerLabel":"offline fixture"
        }}),
    );
    let proposal = &proposed["result"]["structuredContent"];
    let id = proposal["proposalId"].as_str().unwrap();
    assert_eq!(proposal["authorUid"], "editor");
    assert_eq!(proposal["modelTrust"], "declared_unverified");
    let current = team_api(
        &http,
        Method::GET,
        &format!("{base}/v1/workspaces/{ws}/pages/{page_id}"),
        Some(a),
        None,
    )
    .await;
    assert_eq!(current["currentRevision"]["revisionNumber"], 1);
    let applied = team_api(
        &http,
        Method::POST,
        &format!("{base}/v1/workspaces/{ws}/agent-proposals/{id}/review"),
        Some(a),
        Some(json!({"action":"apply","expectedSubmissionNumber":1,"idempotencyKey":"apply"})),
    )
    .await;
    assert_eq!(applied["status"], "applied");
    let current = team_api(
        &http,
        Method::GET,
        &format!("{base}/v1/workspaces/{ws}/pages/{page_id}"),
        Some(b),
        None,
    )
    .await;
    assert_eq!(current["currentRevision"]["revisionNumber"], 2);
    assert_eq!(
        current["currentRevision"]["contentMd"],
        "Original text.\n\nAgent suggestion."
    );
    team_api(
        &http,
        Method::DELETE,
        &format!("{base}/v1/workspaces/{ws}/agent-connections/{connection_id}"),
        Some(b),
        None,
    )
    .await;
    let denied = mcp.request(
        4,
        "tools/call",
        json!({"name":"sthira_shared_pages","arguments":{"action":"read","pageId":page_id}}),
    );
    assert_eq!(denied["result"]["isError"], true);
    assert!(!denied.to_string().contains(credential));
    let a_token = connections[0]["credential"].as_str().unwrap();
    let status = team_api(
        &http,
        Method::GET,
        &format!("{base}/v1/agent-access/status"),
        Some(a_token),
        None,
    )
    .await;
    assert_eq!(status["permission"], "read");
    assert!(!std::fs::read(format!("/proc/{}/cmdline", mcp.child.id()))
        .unwrap()
        .windows(credential.len())
        .any(|window| window == credential.as_bytes()));
    drop(mcp);
    stop.send(()).unwrap();
    service.await.unwrap().unwrap();
}
