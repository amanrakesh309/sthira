//! Owner-only setup for project-scoped local MCP connections.
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::ffi::OsStr;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
    time::Duration,
};
use sthira_cli::{external, CliModel, SystemClock};
use sthira_core::{workspace_agent::external::ExternalGrant, Engine};
use tauri::{AppHandle, Manager, Runtime, State};

const PREFIX: &str = "sthira-";
const SERVE: &str = "--sthira-agent-mcp";
const CHECK: &str = "--sthira-agent-mcp-check";
static CHANGE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Client {
    Codex,
    Antigravity,
    ClaudeCode,
    Cline,
    Kilo,
    #[serde(rename = "opencode")]
    OpenCode,
    Custom,
}

impl Client {
    const ALL: [Self; 7] = [
        Self::Codex,
        Self::Antigravity,
        Self::ClaudeCode,
        Self::Cline,
        Self::Kilo,
        Self::OpenCode,
        Self::Custom,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Antigravity => "Antigravity",
            Self::ClaudeCode => "Claude Code",
            Self::Cline => "Cline",
            Self::Kilo => "Kilo Code",
            Self::OpenCode => "OpenCode",
            Self::Custom => "Other MCP client",
        }
    }

    fn supports_config(self) -> bool {
        !matches!(self, Self::Codex | Self::Antigravity)
    }

    fn executable(self) -> Option<PathBuf> {
        if !cfg!(target_os = "linux") {
            return None;
        }
        let name = match self {
            Self::Codex => "codex",
            Self::Antigravity => "agy",
            Self::ClaudeCode => "claude",
            Self::Cline => "cline",
            _ => return None,
        };
        let extra = env::var_os("HOME").into_iter().flat_map(move |home| {
            let home = PathBuf::from(home);
            [
                home.join(".local/bin").join(name),
                home.join(".cargo/bin").join(name),
            ]
        });
        let codex_desktop =
            matches!(self, Self::Codex).then(|| PathBuf::from("/usr/lib/chatgpt/resources/codex"));
        env::split_paths(&env::var_os("PATH").unwrap_or_default())
            .filter(|path| path.is_absolute())
            .map(|path| path.join(name))
            .chain(extra)
            .chain(codex_desktop)
            .find(|path| executable_file(path))
    }
}

#[cfg(unix)]
fn executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable_file(_: &Path) -> bool {
    false
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AvailableApp {
    client: Client,
    installed: bool,
    supports_config: bool,
}

#[derive(Deserialize, Serialize)]
struct Credential {
    client: Client,
    token: String,
    #[serde(default)]
    manual: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Connection {
    id: String,
    client: Client,
    project_id: String,
    can_propose: bool,
    registered: bool,
    verified: bool,
    manual: bool,
}

#[derive(Serialize)]
pub(crate) struct Setup {
    supported: bool,
    apps: Vec<AvailableApp>,
    connections: Vec<Connection>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Disconnect {
    client_entry_removed: bool,
}

fn connection_dir<R: Runtime>(app: &AppHandle<R>, engine: &Engine) -> Result<PathBuf, String> {
    let instance = engine.vault_instance_id().map_err(|err| err.to_string())?;
    Ok(app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Sthira couldn't find its local app data folder.")?
        .join("agent-connections")
        .join(instance))
}

fn read_credential(directory: &Path, id: &str) -> io::Result<Credential> {
    uuid::Uuid::parse_str(id).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let text = fs::read(directory.join(id).with_extension("json"))?;
    Ok(serde_json::from_slice(&text)?)
}

fn list_connections(engine: &Engine, directory: &Path) -> Result<Vec<Connection>, String> {
    let files = match fs::read_dir(directory) {
        Ok(files) => files,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err("Sthira couldn't open its agent connections.".into()),
    };
    let mut connections = Vec::new();
    for entry in files.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Ok(credential) = read_credential(directory, id) else {
            continue;
        };
        let Some(entry) = engine
            .get_external_connection(id)
            .map_err(|err| err.to_string())?
        else {
            continue;
        };
        if let Some(project_id) = entry
            .grant
            .project_id
            .filter(|_| entry.revoked_at.is_none())
        {
            connections.push(Connection {
                id: id.into(),
                client: credential.client,
                project_id,
                can_propose: entry.grant.can_propose,
                manual: credential.manual,
                registered: directory.join(id).with_extension("registered").is_file(),
                verified: directory.join(id).with_extension("verified").is_file(),
            });
        }
    }
    connections.sort_by(|a, b| (a.client.name(), &a.id).cmp(&(b.client.name(), &b.id)));
    Ok(connections)
}

fn private_dir(directory: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
    }
    Ok(())
}

fn private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn clear_files(directory: &Path, id: &str) {
    for suffix in ["json", "registered", "verified"] {
        let _ = fs::remove_file(directory.join(id).with_extension(suffix));
    }
}

fn launcher() -> Result<PathBuf, String> {
    if let Some(appimage) = env::var_os("APPIMAGE").map(PathBuf::from) {
        if appimage.is_absolute() && executable_file(&appimage) {
            return Ok(appimage);
        }
    }
    env::current_exe().map_err(|_| "Sthira couldn't locate its connection launcher.".into())
}

struct Launcher {
    executable: PathBuf,
    vault: PathBuf,
    directory: PathBuf,
}

impl Launcher {
    fn new(engine: &Engine, directory: PathBuf) -> Result<Self, String> {
        Ok(Self {
            executable: launcher()?,
            vault: engine
                .vault_dir()
                .canonicalize()
                .map_err(|_| "Sthira couldn't find this vault.")?,
            directory: directory
                .canonicalize()
                .map_err(|_| "Sthira couldn't find its private connection folder.")?,
        })
    }

    fn args<'a>(&'a self, mode: &'a str, id: &'a str) -> [&'a OsStr; 4] {
        [
            OsStr::new(mode),
            self.vault.as_os_str(),
            self.directory.as_os_str(),
            OsStr::new(id),
        ]
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConfigSnippet {
    label: &'static str,
    instructions: &'static str,
    json: Value,
}

fn install_args(client: Client, name: &str) -> Vec<&str> {
    match client {
        Client::ClaudeCode => vec![
            "mcp",
            "add",
            "--scope",
            "user",
            "--transport",
            "stdio",
            name,
            "--",
        ],
        Client::Cline => vec!["mcp", "add", name, "--yes", "--"],
        _ => vec!["mcp", "add", name, "--"],
    }
}

fn remove_args(client: Client, name: &str) -> Vec<&str> {
    match client {
        Client::ClaudeCode => vec!["mcp", "remove", "--scope", "user", name],
        _ => vec!["mcp", "remove", name],
    }
}

fn config_snippets(
    client: Client,
    name: &str,
    executable: &str,
    args: &[&str],
) -> Vec<ConfigSnippet> {
    let stdio = json!({ "command": executable, "args": args });
    let command: Vec<_> = std::iter::once(executable)
        .chain(args.iter().copied())
        .collect();
    let local = json!({ "type": "local", "command": command });
    let (label, instructions, json) = match client {
        Client::OpenCode => {
            return vec![
                ConfigSnippet {
                    label: "OpenCode 1",
                    instructions: "Add the entry under mcp in your opencode.json or opencode.jsonc.",
                    json: json!({ "mcp": { name: local } }),
                },
                ConfigSnippet {
                    label: "OpenCode 2",
                    instructions: "Add the entry under mcp.servers in your opencode.json or opencode.jsonc.",
                    json: json!({ "mcp": { "servers": { name: local } } }),
                },
            ];
        }
        Client::Kilo => (
            "Kilo Code",
            "Add the entry under mcp in your kilo.json or kilo.jsonc.",
            json!({ "mcp": { name: local } }),
        ),
        Client::Cline => (
            "Cline",
            "In Cline, open MCP Servers, choose Configure, and add the entry under mcpServers.",
            json!({ "mcpServers": { name: stdio } }),
        ),
        Client::ClaudeCode => (
            "Claude Code",
            "Add the entry under mcpServers in a local .mcp.json file. Approve it in Claude Code when prompted.",
            json!({ "mcpServers": { name: stdio } }),
        ),
        _ => (
            "Local stdio",
            "Add the entry to your client's local MCP settings. If it asks for separate fields, use command and args.",
            json!({ "mcpServers": { name: stdio } }),
        ),
    };
    vec![ConfigSnippet {
        label,
        instructions,
        json,
    }]
}

async fn run_command(mut command: tokio::process::Command) -> bool {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .env_remove("STHIRA_ACCESS_TOKEN")
        .kill_on_drop(true);
    tokio::time::timeout(Duration::from_secs(12), command.status())
        .await
        .is_ok_and(|result| result.is_ok_and(|status| status.success()))
}

async fn remove_client_entry(client: Client, id: &str) -> bool {
    let Some(binary) = client.executable() else {
        return false;
    };
    let mut command = tokio::process::Command::new(binary);
    command.args(remove_args(client, &format!("{PREFIX}{id}")));
    run_command(command).await
}

#[tauri::command]
pub(crate) async fn cmd_get_agent_connections<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Setup, String> {
    let directory = connection_dir(&app, &state.engine)?;
    Ok(Setup {
        supported: cfg!(target_os = "linux"),
        apps: Client::ALL
            .into_iter()
            .map(|client| AvailableApp {
                client,
                installed: client.executable().is_some(),
                supports_config: client.supports_config(),
            })
            .collect(),
        connections: list_connections(&state.engine, &directory)?,
    })
}

#[tauri::command]
pub(crate) async fn cmd_connect_agent<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    client: Client,
    project_id: String,
    can_propose: bool,
    use_config: Option<bool>,
) -> Result<String, String> {
    let _change = CHANGE.lock().await;
    if !cfg!(target_os = "linux") {
        return Err("Agent connections are available on Linux.".into());
    }
    let binary = client.executable();
    let manual = client.supports_config() && (binary.is_none() || use_config.unwrap_or(false));
    if !manual && binary.is_none() {
        return Err(format!("Install {} to connect it.", client.name()));
    }
    let engine = &state.engine;
    let directory = connection_dir(&app, engine)?;
    private_dir(&directory).map_err(|_| "Sthira couldn't create its private connection folder.")?;
    let launcher = Launcher::new(engine, directory)?;
    let directory = &launcher.directory;
    if list_connections(engine, directory)?
        .iter()
        .any(|entry| entry.client == client && entry.project_id == project_id)
    {
        return Err(format!(
            "{} is already connected to this project.",
            client.name()
        ));
    }
    let (entry, token) = engine
        .grant_external_access(ExternalGrant {
            name: client.name().into(),
            whole_workspace: false,
            project_id: Some(project_id),
            workspace_memory: false,
            can_propose,
            model: "Client-selected model".into(),
            provider: client.name().into(),
        })
        .map_err(|e| e.to_string())?;
    let id = &entry.id;
    let setup = async {
        let credential = serde_json::to_vec(&Credential {
            client,
            token,
            manual,
        })
        .map_err(|_| "Sthira couldn't encode the connection credential.")?;
        private_file(&directory.join(id).with_extension("json"), &credential)
            .map_err(|_| "Sthira couldn't save the private credential.".to_string())?;
        if let Some(binary) = binary.filter(|_| !manual) {
            let name = format!("{PREFIX}{id}");
            let mut command = tokio::process::Command::new(binary);
            command
                .args(install_args(client, &name))
                .arg(&launcher.executable)
                .args(launcher.args(SERVE, id));
            if !run_command(command).await {
                return Err(format!(
                    "{} couldn't register the connection. Check its installation and try again.",
                    client.name()
                ));
            }
            private_file(&directory.join(id).with_extension("registered"), b"")
                .map_err(|_| "Sthira couldn't save the connection status.".to_string())?;
        }
        Ok(())
    };
    if let Err(error) = setup.await {
        // Revoke first so a partial CLI registration cannot retain access.
        engine
            .revoke_external_access(id)
            .map_err(|err| err.to_string())?;
        clear_files(directory, id);
        if !manual {
            let _ = remove_client_entry(client, id).await;
        }
        return Err(error);
    }
    Ok(entry.id)
}

#[tauri::command]
pub(crate) async fn cmd_get_agent_connection_config<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<ConfigSnippet>, String> {
    let engine = &state.engine;
    let directory = connection_dir(&app, engine)?;
    let credential =
        read_credential(&directory, &id).map_err(|_| "This Sthira connection is unavailable.")?;
    if !credential.manual {
        return Err("This connection is managed by its client installer.".into());
    }
    let status = engine
        .external_status(&credential.token)
        .map_err(|err| err.to_string())?;
    if status["connection"]["id"] != id {
        return Err("This Sthira connection is unavailable.".into());
    }
    let launcher = Launcher::new(engine, directory)?;
    let invalid_path = "The local MCP configuration needs UTF-8 paths.";
    let executable = launcher.executable.to_str().ok_or(invalid_path)?;
    let raw_args = launcher.args(SERVE, &id);
    let args = raw_args
        .iter()
        .map(|arg| arg.to_str().ok_or(invalid_path))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(config_snippets(
        credential.client,
        &format!("{PREFIX}{id}"),
        executable,
        &args,
    ))
}

#[tauri::command]
pub(crate) async fn cmd_test_agent_connection<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let directory = connection_dir(&app, &state.engine)?;
    if read_credential(&directory, &id).is_err() {
        return Err("This Sthira connection is unavailable.".into());
    }
    let launcher = Launcher::new(&state.engine, directory)?;
    let mut command = tokio::process::Command::new(&launcher.executable);
    command.args(launcher.args(CHECK, &id));
    if run_command(command).await {
        Ok(())
    } else {
        Err(
            "This connection couldn't access its project. Check that the project still exists."
                .into(),
        )
    }
}

#[tauri::command]
pub(crate) async fn cmd_disconnect_agent<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: String,
) -> Result<Disconnect, String> {
    let _change = CHANGE.lock().await;
    let directory = connection_dir(&app, &state.engine)?;
    let credential =
        read_credential(&directory, &id).map_err(|_| "This Sthira connection is unavailable.")?;
    state
        .engine
        .revoke_external_access(&id)
        .map_err(|err| err.to_string())?;
    clear_files(&directory, &id);
    Ok(Disconnect {
        client_entry_removed: !credential.manual
            && remove_client_entry(credential.client, &id).await,
    })
}

pub fn run_if_requested() -> Option<Result<(), Box<dyn std::error::Error>>> {
    let mut args = env::args_os().skip(1);
    let mode = args.next()?;
    if mode != SERVE && mode != CHECK {
        return None;
    }
    let (Some(vault), Some(directory), Some(id), None) =
        (args.next(), args.next(), args.next(), args.next())
    else {
        return Some(Err("Missing local connection arguments.".into()));
    };
    Some(run_mcp(
        Path::new(&vault),
        Path::new(&directory),
        &id.to_string_lossy(),
        mode == CHECK,
    ))
}

fn run_mcp(
    vault: &Path,
    directory: &Path,
    id: &str,
    check: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let credential =
        read_credential(directory, id).map_err(|_| "This Sthira connection is unavailable.")?;
    let engine = Engine::attach(
        vault.join("sthira.db"),
        vault,
        Arc::new(SystemClock),
        Arc::new(CliModel),
    )?;
    let status = engine.external_status(&credential.token)?;
    if status["connection"]["id"] != id {
        return Err("This Sthira connection is unavailable.".into());
    }
    if !check {
        external::serve(
            &engine,
            &credential.token,
            io::stdin().lock(),
            io::stdout().lock(),
            || {
                let _ = private_file(&directory.join(id).with_extension("verified"), b"");
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_configs_match_each_client_schema_and_reference_only_the_launcher() {
        let name = "sthira-test";
        let executable = "/opt/Sthira Notes/sthira";
        let args = [SERVE, "/tmp/My vault", "/tmp/private", "connection-id"];
        let full_command = json!([executable, args[0], args[1], args[2], args[3]]);
        for client in [Client::ClaudeCode, Client::Cline, Client::Custom] {
            let snippets = config_snippets(client, name, executable, &args);
            assert_eq!(
                snippets[0].json["mcpServers"][name],
                json!({"command": executable, "args": args})
            );
        }
        assert_eq!(
            config_snippets(Client::Kilo, name, executable, &args)[0].json["mcp"][name]["command"],
            full_command
        );
        let versions = config_snippets(Client::OpenCode, name, executable, &args);
        assert_eq!(versions[0].json["mcp"][name]["command"], full_command);
        assert_eq!(
            versions[1].json["mcp"]["servers"][name]["command"],
            full_command
        );
    }

    #[test]
    fn older_managed_credentials_still_use_the_installer() {
        let credential: Credential =
            serde_json::from_value(json!({"client":"codex","token":"example"})).unwrap();
        assert_eq!(credential.client, Client::Codex);
        assert!(!credential.manual);
        assert_eq!(
            install_args(Client::ClaudeCode, "sthira-test"),
            [
                "mcp",
                "add",
                "--scope",
                "user",
                "--transport",
                "stdio",
                "sthira-test",
                "--"
            ]
        );
        assert_eq!(
            remove_args(Client::ClaudeCode, "sthira-test"),
            ["mcp", "remove", "--scope", "user", "sthira-test"]
        );
        assert_eq!(
            install_args(Client::Cline, "sthira-test"),
            ["mcp", "add", "sthira-test", "--yes", "--"]
        );
    }
}
