use crate::account_sign_in::{
    apply_terminal_notice, open_session_store, sign_in_client, SavedDeviceSession, SessionStatus,
    SessionStore, SignInAttempt, SignInConfig, SignOutReason,
};
use reqwest::{Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, Manager, Runtime};

const ACTIVE_WORKSPACE_FILE: &str = "active-workspace.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum WorkspaceRole {
    Owner,
    Admin,
    Editor,
    Viewer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum InvitationStatus {
    Pending,
    Accepted,
    Expired,
    Revoked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TransferStatus {
    Pending,
    Accepted,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceSummary {
    pub(crate) workspace_id: String,
    pub(crate) name: String,
    pub(crate) my_membership_id: String,
    pub(crate) my_role: WorkspaceRole,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateWorkspaceOutcome {
    pub(crate) created: bool,
    pub(crate) workspace: WorkspaceSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceMemberView {
    pub(crate) membership_id: String,
    pub(crate) uid: String,
    pub(crate) display_name: String,
    pub(crate) role: WorkspaceRole,
    pub(crate) joined_at_ms: i64,
    pub(crate) is_current_user: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceInvitationView {
    pub(crate) invitation_id: String,
    pub(crate) workspace_id: String,
    pub(crate) recipient_email: String,
    pub(crate) offered_role: WorkspaceRole,
    pub(crate) status: InvitationStatus,
    pub(crate) invited_by_membership_id: String,
    pub(crate) invited_by_display_name: String,
    pub(crate) created_at_ms: i64,
    pub(crate) expires_at_ms: i64,
    pub(crate) accepted_at_ms: Option<i64>,
    pub(crate) revoked_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreatedInvitation {
    pub(crate) invitation: WorkspaceInvitationView,
    pub(crate) invite_code: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InvitationPreview {
    pub(crate) invitation_id: String,
    pub(crate) workspace_id: String,
    pub(crate) workspace_name: String,
    pub(crate) offered_role: WorkspaceRole,
    pub(crate) expires_at_ms: i64,
    pub(crate) already_member: bool,
    pub(crate) existing_role: Option<WorkspaceRole>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcceptInvitationOutcome {
    pub(crate) already_member: bool,
    pub(crate) workspace: WorkspaceSummary,
    pub(crate) membership: WorkspaceMemberView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OwnershipTransferView {
    pub(crate) transfer_id: String,
    pub(crate) workspace_id: String,
    pub(crate) workspace_name: String,
    pub(crate) from_membership_id: String,
    pub(crate) from_uid: String,
    pub(crate) from_display_name: String,
    pub(crate) to_membership_id: String,
    pub(crate) to_uid: String,
    pub(crate) to_display_name: String,
    pub(crate) status: TransferStatus,
    pub(crate) created_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceDetails {
    pub(crate) workspace: WorkspaceSummary,
    pub(crate) members: Vec<WorkspaceMemberView>,
    pub(crate) invitations: Vec<WorkspaceInvitationView>,
    pub(crate) pending_transfer: Option<OwnershipTransferView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WorkspaceListResponse {
    pub(crate) workspaces: Vec<WorkspaceSummary>,
    pub(crate) incoming_transfers: Vec<OwnershipTransferView>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub(crate) enum WorkspaceResponse<T: Serialize> {
    Ok {
        data: T,
    },
    SessionUpdated {
        session: SessionStatus,
    },
    AccessLost {
        message: String,
        workspaces: Option<WorkspaceListResponse>,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActiveWorkspaceRecord {
    workspace_id: Option<String>,
}

fn read_active_workspace_id(data_dir: &Path) -> Option<String> {
    let path = data_dir.join(ACTIVE_WORKSPACE_FILE);
    let bytes = fs::read(path).ok()?;
    let rec: ActiveWorkspaceRecord = serde_json::from_slice(&bytes).ok()?;
    rec.workspace_id
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn write_active_workspace_id(data_dir: &Path, workspace_id: Option<&str>) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|_| "Could not open account workspace storage.")?;
    let path = data_dir.join(ACTIVE_WORKSPACE_FILE);
    let normalized = workspace_id
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let rec = ActiveWorkspaceRecord {
        workspace_id: normalized,
    };
    let bytes =
        serde_json::to_vec(&rec).map_err(|_| "Could not prepare active workspace setting.")?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| "Could not save active workspace selection.")?;
        f.write_all(&bytes)
            .map_err(|_| "Could not save active workspace selection.")?;
        let _ = f.sync_all();
    }
    fs::rename(&tmp, path).map_err(|_| "Could not save active workspace selection.")?;
    Ok(())
}

async fn load_verified_caller(
    store: &SessionStore,
    attempt: Option<&SignInAttempt>,
) -> Result<SavedDeviceSession, SessionStatus> {
    match store.load_session().await {
        Ok(Some(saved)) => Ok(saved),
        Ok(None) => {
            let reason = if store.has_legacy_pilot_session().await.unwrap_or(false) {
                Some(SignOutReason::MigrationRequired)
            } else {
                None
            };
            let raw = SessionStatus::SignedOut { reason };
            let final_status = attempt
                .and_then(|a| apply_terminal_notice(a, raw.clone()).ok())
                .unwrap_or(raw);
            Err(final_status)
        }
        Err(msg) => Err(SessionStatus::Unverified { message: msg }),
    }
}

async fn request_json<T: DeserializeOwned + Serialize>(
    store: &SessionStore,
    attempt: Option<&SignInAttempt>,
    config: &SignInConfig,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    scoped_workspace_id: Option<&str>,
) -> Result<WorkspaceResponse<T>, String> {
    let saved = match load_verified_caller(store, attempt).await {
        Ok(s) => s,
        Err(session) => {
            if matches!(session, SessionStatus::SignedOut { .. }) {
                let _ = write_active_workspace_id(&store.data_dir, None);
            }
            return Ok(WorkspaceResponse::SessionUpdated { session });
        }
    };

    let http = sign_in_client()?;
    let url = format!("{}{path}", config.session_service_url);
    let mut builder = http.request(method, &url).bearer_auth(&saved.credential);
    if let Some(payload) = body {
        builder = builder.json(&payload);
    }

    let response = match builder.send().await {
        Ok(resp) => resp,
        Err(_) => {
            return Ok(WorkspaceResponse::Error {
                code: "service_unavailable".into(),
                message: "Could not reach the Sthira session service. Your Personal workspace remains usable offline.".into(),
            });
        }
    };

    if response.status() == StatusCode::UNAUTHORIZED {
        let status = store.handle_unauthorized(response).await?;
        let final_status = attempt
            .and_then(|a| apply_terminal_notice(a, status.clone()).ok())
            .unwrap_or(status);
        if matches!(final_status, SessionStatus::SignedOut { .. }) {
            let _ = write_active_workspace_id(&store.data_dir, None);
        }
        return Ok(WorkspaceResponse::SessionUpdated {
            session: final_status,
        });
    }

    let status = response.status();
    if !status.is_success() {
        let parsed = response.json::<serde_json::Value>().await.ok();
        let code = parsed
            .as_ref()
            .and_then(|v| v["code"].as_str())
            .unwrap_or("request_failed")
            .to_owned();
        let message = parsed
            .as_ref()
            .and_then(|v| v["error"].as_str())
            .unwrap_or("The workspace request could not be completed.")
            .to_owned();

        if let Some(ws_id) = scoped_workspace_id {
            if matches!(status, StatusCode::FORBIDDEN | StatusCode::NOT_FOUND)
                && matches!(code.as_str(), "forbidden" | "workspace_not_found")
            {
                if let Ok(Some(list)) = try_list_workspaces(config, &saved).await {
                    let still_member = list.workspaces.iter().any(|w| w.workspace_id == ws_id);
                    if !still_member {
                        let same_active_session = store
                            .load_session()
                            .await
                            .ok()
                            .flatten()
                            .is_some_and(|current| current.session_id == saved.session_id);
                        if !same_active_session {
                            return Ok(WorkspaceResponse::Error { code, message });
                        }
                        if read_active_workspace_id(&store.data_dir).as_deref() == Some(ws_id) {
                            let _ = write_active_workspace_id(&store.data_dir, None);
                        }
                        if let Ok(uid) = crate::shared_drafts::read_verified_session_account_uid(
                            &store.data_dir,
                            &saved.session_id,
                            None,
                        ) {
                            crate::shared_drafts::purge_workspace_drafts(
                                &store.data_dir,
                                &uid,
                                ws_id,
                            );
                        }
                        return Ok(WorkspaceResponse::AccessLost {
                            message:
                                "You no longer have access to this workspace. Returned to Personal."
                                    .into(),
                            workspaces: Some(list),
                        });
                    }
                }
            }
        }

        return Ok(WorkspaceResponse::Error { code, message });
    }

    let data: T = response
        .json()
        .await
        .map_err(|_| "Unexpected workspace response from Sthira session service.".to_owned())?;
    Ok(WorkspaceResponse::Ok { data })
}

async fn try_list_workspaces(
    config: &SignInConfig,
    saved: &SavedDeviceSession,
) -> Result<Option<WorkspaceListResponse>, String> {
    let http = sign_in_client()?;
    let resp = match http
        .get(format!("{}/v1/workspaces", config.session_service_url))
        .bearer_auth(&saved.credential)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    if !resp.status().is_success() {
        return Ok(None);
    }
    Ok(resp.json::<WorkspaceListResponse>().await.ok())
}

async fn invoke_workspace_api<T: DeserializeOwned + Serialize>(
    app: &AppHandle<impl Runtime>,
    attempt: &SignInAttempt,
    method: Method,
    path: &str,
    body: Option<serde_json::Value>,
    scoped_workspace_id: Option<&str>,
) -> Result<(WorkspaceResponse<T>, PathBuf), String> {
    let Some(config) = SignInConfig::from_env() else {
        return Ok((
            WorkspaceResponse::SessionUpdated {
                session: SessionStatus::Unavailable,
            },
            PathBuf::new(),
        ));
    };
    let store = open_session_store(app, &config).await?;
    let data_dir = store.data_dir.clone();
    let resp = request_json(
        &store,
        Some(attempt),
        &config,
        method,
        path,
        body,
        scoped_workspace_id,
    )
    .await?;
    if matches!(
        &resp,
        WorkspaceResponse::SessionUpdated {
            session: SessionStatus::SignedOut { .. }
        } | WorkspaceResponse::AccessLost { .. }
    ) {
        if let Some(mgr) = app.try_state::<crate::live_collab::LiveCollabManager>() {
            mgr.disconnect_all().await;
        }
    }
    Ok((resp, data_dir))
}

#[tauri::command]
pub(crate) fn cmd_get_active_workspace<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Could not find account workspace storage.")?;
    Ok(read_active_workspace_id(&dir))
}

#[tauri::command]
pub(crate) fn cmd_set_active_workspace<R: Runtime>(
    app: AppHandle<R>,
    workspace_id: Option<String>,
) -> Result<(), String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "Could not find account workspace storage.")?;
    write_active_workspace_id(&dir, workspace_id.as_deref())
}

#[tauri::command]
pub(crate) async fn cmd_list_workspaces<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
) -> Result<WorkspaceResponse<WorkspaceListResponse>, String> {
    let (resp, data_dir): (WorkspaceResponse<WorkspaceListResponse>, PathBuf) =
        invoke_workspace_api(&app, &attempt, Method::GET, "/v1/workspaces", None, None).await?;
    if let WorkspaceResponse::Ok { ref data } = resp {
        if let Some(active) = read_active_workspace_id(&data_dir) {
            if !data.workspaces.iter().any(|w| w.workspace_id == active) {
                let _ = write_active_workspace_id(&data_dir, None);
            }
        }
    }
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_create_workspace<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    name: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<CreateWorkspaceOutcome>, String> {
    let _operation = attempt
        .operations
        .try_lock()
        .map_err(|_| "An account operation is already in progress.")?;
    let (resp, _): (WorkspaceResponse<CreateWorkspaceOutcome>, PathBuf) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        "/v1/workspaces",
        Some(serde_json::json!({ "name": name, "idempotency_key": idempotency_key })),
        None,
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_get_workspace<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
) -> Result<WorkspaceResponse<WorkspaceDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_rename_workspace<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    name: String,
) -> Result<WorkspaceResponse<WorkspaceSummary>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/rename"),
        Some(serde_json::json!({ "name": name })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_create_invitation<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    email: String,
    role: WorkspaceRole,
) -> Result<WorkspaceResponse<CreatedInvitation>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/invitations"),
        Some(serde_json::json!({ "email": email, "role": role })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_revoke_invitation<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    invitation_id: String,
) -> Result<WorkspaceResponse<WorkspaceInvitationView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/invitations/{invitation_id}/revoke"),
        Some(serde_json::json!({})),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_replace_invitation<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    invitation_id: String,
) -> Result<WorkspaceResponse<CreatedInvitation>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/invitations/{invitation_id}/replace"),
        Some(serde_json::json!({})),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_preview_invitation<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    invite_code: String,
) -> Result<WorkspaceResponse<InvitationPreview>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        "/v1/invitations/preview",
        Some(serde_json::json!({ "invite_code": invite_code })),
        None,
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_accept_invitation<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    invite_code: String,
) -> Result<WorkspaceResponse<AcceptInvitationOutcome>, String> {
    let _operation = attempt
        .operations
        .try_lock()
        .map_err(|_| "An account operation is already in progress.")?;
    let (resp, _): (WorkspaceResponse<AcceptInvitationOutcome>, PathBuf) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        "/v1/invitations/accept",
        Some(serde_json::json!({ "invite_code": invite_code })),
        None,
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_change_member_role<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    membership_id: String,
    role: WorkspaceRole,
) -> Result<WorkspaceResponse<WorkspaceDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/members/{membership_id}/role"),
        Some(serde_json::json!({ "role": role })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_remove_member<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    membership_id: String,
) -> Result<WorkspaceResponse<WorkspaceDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/members/{membership_id}/remove"),
        Some(serde_json::json!({})),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_leave_workspace<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
) -> Result<WorkspaceResponse<serde_json::Value>, String> {
    let (resp, data_dir): (WorkspaceResponse<serde_json::Value>, PathBuf) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/leave"),
        Some(serde_json::json!({})),
        Some(&workspace_id),
    )
    .await?;
    if matches!(resp, WorkspaceResponse::Ok { .. })
        && read_active_workspace_id(&data_dir).as_deref() == Some(workspace_id.as_str())
    {
        let _ = write_active_workspace_id(&data_dir, None);
    }
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_initiate_ownership_transfer<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    to_membership_id: String,
) -> Result<WorkspaceResponse<WorkspaceDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/transfer"),
        Some(serde_json::json!({ "to_membership_id": to_membership_id })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_accept_ownership_transfer<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    transfer_id: String,
) -> Result<WorkspaceResponse<WorkspaceDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/transfer/accept"),
        Some(serde_json::json!({ "transfer_id": transfer_id })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_cancel_ownership_transfer<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    transfer_id: String,
) -> Result<WorkspaceResponse<WorkspaceDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/transfer/cancel"),
        Some(serde_json::json!({ "transfer_id": transfer_id })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PageEntryMode {
    New,
    CopyPersonal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PageOriginKind {
    UserAuthored,
    AiAssisted,
    ExternalSource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ShareableProvenance {
    pub(crate) origin_kind: PageOriginKind,
    #[serde(default)]
    pub(crate) original_author_label: Option<String>,
    #[serde(default)]
    pub(crate) ai_model_label: Option<String>,
    #[serde(default)]
    pub(crate) source_note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateSharedPageInput {
    pub(crate) title: String,
    pub(crate) content_md: String,
    pub(crate) entry_mode: PageEntryMode,
    pub(crate) provenance: ShareableProvenance,
    #[serde(default)]
    pub(crate) omitted_summary: Vec<String>,
    pub(crate) idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedPageSummary {
    pub(crate) page_id: String,
    pub(crate) workspace_id: String,
    pub(crate) title: String,
    pub(crate) current_revision_number: i64,
    pub(crate) entry_mode: PageEntryMode,
    pub(crate) provenance: ShareableProvenance,
    pub(crate) omitted_summary: Vec<String>,
    pub(crate) publisher_uid: String,
    pub(crate) publisher_display_name: String,
    pub(crate) last_editor_uid: String,
    pub(crate) last_editor_display_name: String,
    pub(crate) last_reviewer_uid: Option<String>,
    pub(crate) last_reviewer_display_name: Option<String>,
    pub(crate) archived_at_ms: Option<i64>,
    pub(crate) archived_by_display_name: Option<String>,
    pub(crate) unresolved_thread_count: i64,
    pub(crate) pending_proposal_count: i64,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedPageRevisionView {
    pub(crate) revision_id: String,
    pub(crate) workspace_id: String,
    pub(crate) page_id: String,
    pub(crate) revision_number: i64,
    pub(crate) title: String,
    pub(crate) content_md: String,
    pub(crate) content_sha256: String,
    pub(crate) change_summary: String,
    pub(crate) author_uid: String,
    pub(crate) author_display_name: String,
    pub(crate) reviewer_uid: Option<String>,
    pub(crate) reviewer_display_name: Option<String>,
    pub(crate) proposal_id: Option<String>,
    #[serde(default)]
    pub(crate) contributor_names: Vec<String>,
    pub(crate) created_at_ms: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ThreadStatus {
    Open,
    Resolved,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedCommentMessageView {
    pub(crate) message_id: String,
    pub(crate) thread_id: String,
    pub(crate) author_uid: String,
    pub(crate) author_display_name: String,
    pub(crate) body: String,
    pub(crate) created_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedCommentThreadView {
    pub(crate) thread_id: String,
    pub(crate) workspace_id: String,
    pub(crate) page_id: String,
    pub(crate) base_revision_number: i64,
    pub(crate) author_uid: String,
    pub(crate) author_display_name: String,
    pub(crate) status: ThreadStatus,
    pub(crate) resolved_at_ms: Option<i64>,
    pub(crate) resolved_by_display_name: Option<String>,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
    pub(crate) messages: Vec<SharedCommentMessageView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProposalStatus {
    Pending,
    ChangesRequested,
    Rejected,
    Applied,
    Withdrawn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProposalReviewAction {
    Apply,
    RequestChanges,
    Reject,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProposalDecisionView {
    pub(crate) decision_id: String,
    pub(crate) submission_number: i64,
    pub(crate) action: String,
    pub(crate) actor_uid: String,
    pub(crate) actor_display_name: String,
    pub(crate) note: Option<String>,
    pub(crate) base_revision_number: i64,
    pub(crate) submitted_title: String,
    pub(crate) submitted_content_md: String,
    pub(crate) resulting_revision_number: Option<i64>,
    pub(crate) created_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedProposalSummary {
    pub(crate) proposal_id: String,
    pub(crate) workspace_id: String,
    pub(crate) page_id: String,
    pub(crate) page_title: String,
    pub(crate) base_revision_number: i64,
    pub(crate) summary: String,
    pub(crate) author_uid: String,
    pub(crate) author_display_name: String,
    pub(crate) status: ProposalStatus,
    pub(crate) submission_count: i64,
    pub(crate) is_stale: bool,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
    pub(crate) reviewed_at_ms: Option<i64>,
    pub(crate) reviewer_display_name: Option<String>,
    pub(crate) applied_revision_number: Option<i64>,
    #[serde(default)]
    pub(crate) live_draft_id: Option<String>,
    #[serde(default)]
    pub(crate) draft_version: Option<i64>,
    #[serde(default)]
    pub(crate) contributor_uids: Vec<String>,
    #[serde(default)]
    pub(crate) contributor_names: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedProposalDetails {
    pub(crate) proposal: SharedProposalSummary,
    pub(crate) base_title: String,
    pub(crate) base_content_md: String,
    pub(crate) proposed_title: String,
    pub(crate) proposed_content_md: String,
    pub(crate) current_page_revision_number: i64,
    pub(crate) current_page_title: String,
    pub(crate) current_page_content_md: String,
    pub(crate) page_archived: bool,
    pub(crate) can_review: bool,
    pub(crate) can_author_edit: bool,
    #[serde(default)]
    pub(crate) has_eligible_reviewer: bool,
    #[serde(default)]
    pub(crate) ineligible_reason: Option<String>,
    pub(crate) decisions: Vec<ProposalDecisionView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedPageDetails {
    pub(crate) page: SharedPageSummary,
    pub(crate) current_revision: SharedPageRevisionView,
    pub(crate) revisions: Vec<SharedPageRevisionView>,
    pub(crate) revisions_next_cursor: Option<String>,
    pub(crate) threads: Vec<SharedCommentThreadView>,
    pub(crate) threads_next_cursor: Option<String>,
    pub(crate) proposals: Vec<SharedProposalSummary>,
    pub(crate) proposals_next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateSharedPageOutcome {
    pub(crate) created: bool,
    pub(crate) page: SharedPageDetails,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedActivityItem {
    pub(crate) activity_id: String,
    pub(crate) workspace_id: String,
    pub(crate) actor_uid: String,
    pub(crate) actor_display_name: String,
    pub(crate) action: String,
    pub(crate) target_type: String,
    #[serde(default)]
    pub(crate) project_id: Option<String>,
    #[serde(default)]
    pub(crate) project_name: Option<String>,
    pub(crate) page_id: Option<String>,
    pub(crate) page_title: Option<String>,
    pub(crate) proposal_id: Option<String>,
    pub(crate) thread_id: Option<String>,
    pub(crate) detail_text: String,
    pub(crate) created_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedProjectSummary {
    pub(crate) project_id: String,
    pub(crate) workspace_id: String,
    pub(crate) name: String,
    pub(crate) purpose: String,
    pub(crate) version: i64,
    pub(crate) active_page_count: i64,
    pub(crate) created_by_uid: String,
    pub(crate) created_by_display_name: String,
    pub(crate) updated_by_uid: String,
    pub(crate) updated_by_display_name: String,
    pub(crate) archived_at_ms: Option<i64>,
    pub(crate) archived_by_display_name: Option<String>,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedProjectPageItem {
    pub(crate) membership_id: String,
    pub(crate) generation: i64,
    pub(crate) attached_by_uid: String,
    pub(crate) attached_by_display_name: String,
    pub(crate) attached_at_ms: i64,
    pub(crate) page: SharedPageSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedProjectDetails {
    pub(crate) project: SharedProjectSummary,
    pub(crate) pages: Vec<SharedProjectPageItem>,
    pub(crate) pages_next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateSharedProjectInput {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) purpose: String,
    pub(crate) idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateSharedProjectOutcome {
    pub(crate) created: bool,
    pub(crate) project: SharedProjectDetails,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectPageMembershipOutcome {
    pub(crate) attached: bool,
    pub(crate) membership_id: String,
    pub(crate) generation: i64,
    pub(crate) project: SharedProjectDetails,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PagedList<T> {
    pub(crate) items: Vec<T>,
    pub(crate) next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum BatchInviteItemOutcome {
    Created,
    AlreadyMember,
    AlreadyPending,
    InvalidEmail,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BatchInvitationItemResult {
    pub(crate) email: String,
    pub(crate) outcome: BatchInviteItemOutcome,
    pub(crate) message: String,
    pub(crate) invitation: Option<WorkspaceInvitationView>,
    pub(crate) invite_code: Option<String>,
    pub(crate) replayed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BatchInvitationOutcome {
    pub(crate) offered_role: WorkspaceRole,
    pub(crate) replayed: bool,
    pub(crate) results: Vec<BatchInvitationItemResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LiveDraftStatus {
    Editing,
    PendingReview,
    Applied,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDraftContributorView {
    pub(crate) uid: String,
    pub(crate) display_name: String,
    pub(crate) contribution_count: i64,
    pub(crate) last_edited_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedLiveDraftView {
    pub(crate) draft_id: String,
    pub(crate) workspace_id: String,
    pub(crate) page_id: String,
    pub(crate) base_revision_number: i64,
    pub(crate) draft_version: i64,
    pub(crate) checkpoint_seq: i64,
    pub(crate) checkpoint_id: String,
    pub(crate) status: LiveDraftStatus,
    pub(crate) snapshot_state_b64: String,
    pub(crate) snapshot_vector_b64: String,
    pub(crate) base_title: String,
    pub(crate) base_content_md: String,
    pub(crate) current_title: String,
    pub(crate) current_content_md: String,
    pub(crate) contributors: Vec<LiveDraftContributorView>,
    pub(crate) pending_proposal_id: Option<String>,
    pub(crate) created_by_uid: String,
    pub(crate) created_by_display_name: String,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenLiveDraftOutcome {
    pub(crate) created: bool,
    pub(crate) draft: SharedLiveDraftView,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDraftDiffResponse {
    pub(crate) draft: SharedLiveDraftView,
    pub(crate) missing_update_b64: String,
    pub(crate) server_vector_b64: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LiveDraftUpdateOutcome {
    pub(crate) accepted: bool,
    pub(crate) duplicate: bool,
    pub(crate) draft_version: i64,
    pub(crate) checkpoint_seq: i64,
    pub(crate) checkpoint_id: String,
    pub(crate) server_seq: i64,
    pub(crate) accepted_update_b64: String,
    pub(crate) contributors: Vec<LiveDraftContributorView>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LiveDraftRebaseMode {
    CarryDraft,
    RestartFromCurrent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedProjectMessageView {
    pub(crate) message_id: String,
    pub(crate) thread_id: String,
    pub(crate) author_uid: String,
    pub(crate) author_display_name: String,
    pub(crate) body: String,
    pub(crate) created_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedProjectThreadView {
    pub(crate) thread_id: String,
    pub(crate) workspace_id: String,
    pub(crate) project_id: String,
    pub(crate) author_uid: String,
    pub(crate) author_display_name: String,
    pub(crate) status: ThreadStatus,
    pub(crate) resolved_at_ms: Option<i64>,
    pub(crate) resolved_by_display_name: Option<String>,
    pub(crate) created_at_ms: i64,
    pub(crate) updated_at_ms: i64,
    pub(crate) messages: Vec<SharedProjectMessageView>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RealtimeCatchupEvent {
    pub(crate) event_seq: i64,
    pub(crate) workspace_id: String,
    pub(crate) event_type: String,
    pub(crate) project_id: Option<String>,
    pub(crate) page_id: Option<String>,
    pub(crate) thread_id: Option<String>,
    pub(crate) proposal_id: Option<String>,
    pub(crate) draft_id: Option<String>,
    pub(crate) payload_json: serde_json::Value,
    pub(crate) created_at_ms: i64,
}

#[tauri::command]
pub(crate) async fn cmd_create_invitation_batch<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    emails: Vec<String>,
    role: WorkspaceRole,
    idempotency_key: String,
) -> Result<WorkspaceResponse<BatchInvitationOutcome>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/invitations/batch"),
        Some(serde_json::json!({
            "emails": emails,
            "role": role,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_open_live_draft<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    initial_update_b64: Option<String>,
    expected_base_revision_number: Option<i64>,
) -> Result<WorkspaceResponse<OpenLiveDraftOutcome>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/live-draft"),
        Some(serde_json::json!({
            "initialUpdateB64": initial_update_b64,
            "expectedBaseRevisionNumber": expected_base_revision_number,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_get_live_draft<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
) -> Result<WorkspaceResponse<Option<SharedLiveDraftView>>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/live-draft"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_get_live_draft_diff<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    client_state_vector_b64: Option<String>,
) -> Result<WorkspaceResponse<Option<LiveDraftDiffResponse>>, String> {
    let mut path = format!("/v1/workspaces/{workspace_id}/pages/{page_id}/live-draft/diff");
    if let Some(sv) = client_state_vector_b64
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
    {
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("sv", &sv)
            .finish();
        path.push('?');
        path.push_str(&encoded);
    }
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &path,
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_apply_live_draft_update<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    draft_id: String,
    draft_version: i64,
    client_conn_id: String,
    client_op_seq: i64,
    update_b64: String,
) -> Result<WorkspaceResponse<LiveDraftUpdateOutcome>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/live-draft/update"),
        Some(serde_json::json!({
            "draftId": draft_id,
            "draftVersion": draft_version,
            "clientConnId": client_conn_id,
            "clientOpSeq": client_op_seq,
            "updateB64": update_b64,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_submit_live_draft<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    expected_draft_id: String,
    expected_draft_version: i64,
    expected_checkpoint_id: String,
    summary: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProposalDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/live-draft/submit"),
        Some(serde_json::json!({
            "expectedDraftId": expected_draft_id,
            "expectedDraftVersion": expected_draft_version,
            "expectedCheckpointId": expected_checkpoint_id,
            "summary": summary,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_rebase_live_draft<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    expected_draft_id: String,
    expected_draft_version: i64,
    expected_checkpoint_id: String,
    expected_base_revision_number: i64,
    mode: LiveDraftRebaseMode,
    rebased_title: Option<String>,
    rebased_content_md: Option<String>,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedLiveDraftView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/live-draft/rebase"),
        Some(serde_json::json!({
            "expectedDraftId": expected_draft_id,
            "expectedDraftVersion": expected_draft_version,
            "expectedCheckpointId": expected_checkpoint_id,
            "expectedBaseRevisionNumber": expected_base_revision_number,
            "mode": mode,
            "rebasedTitle": rebased_title,
            "rebasedContentMd": rebased_content_md,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_list_project_threads<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<PagedList<SharedProjectThreadView>>, String> {
    let qs = build_query_suffix(None, cursor.as_deref(), limit);
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/threads{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_create_project_thread<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    body: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProjectThreadView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/threads"),
        Some(serde_json::json!({
            "body": body,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_reply_project_thread<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    thread_id: String,
    body: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProjectThreadView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/threads/{thread_id}/replies"),
        Some(serde_json::json!({
            "body": body,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_resolve_project_thread<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    thread_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProjectThreadView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/threads/{thread_id}/resolve"),
        Some(serde_json::json!({ "idempotencyKey": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_reopen_project_thread<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    thread_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProjectThreadView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/threads/{thread_id}/reopen"),
        Some(serde_json::json!({ "idempotencyKey": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_list_realtime_events<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    after_seq: i64,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<Vec<RealtimeCatchupEvent>>, String> {
    let mut path = format!(
        "/v1/workspaces/{workspace_id}/realtime-events?after_seq={}",
        after_seq.max(0)
    );
    if let Some(lim) = limit {
        path.push_str(&format!("&limit={lim}"));
    }
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &path,
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

fn build_query_suffix_with_search(
    include_archived: Option<bool>,
    q: Option<&str>,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> String {
    let mut pairs: Vec<(String, String)> = Vec::new();
    if let Some(flag) = include_archived {
        pairs.push((
            "include_archived".into(),
            if flag { "true" } else { "false" }.into(),
        ));
    }
    if let Some(search) = q.map(str::trim).filter(|s| !s.is_empty()) {
        pairs.push(("q".into(), search.to_owned()));
    }
    if let Some(c) = cursor.map(str::trim).filter(|s| !s.is_empty()) {
        pairs.push(("cursor".into(), c.to_owned()));
    }
    if let Some(l) = limit {
        pairs.push(("limit".into(), l.to_string()));
    }
    if pairs.is_empty() {
        return String::new();
    }
    let encoded = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish();
    format!("?{encoded}")
}

fn build_query_suffix(
    include_archived: Option<bool>,
    cursor: Option<&str>,
    limit: Option<usize>,
) -> String {
    build_query_suffix_with_search(include_archived, None, cursor, limit)
}

#[tauri::command]
pub(crate) async fn cmd_list_shared_projects<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    include_archived: Option<bool>,
    q: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<PagedList<SharedProjectSummary>>, String> {
    let qs = build_query_suffix_with_search(
        Some(include_archived.unwrap_or(false)),
        q.as_deref(),
        cursor.as_deref(),
        limit,
    );
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/projects{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_create_shared_project<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    input: CreateSharedProjectInput,
) -> Result<WorkspaceResponse<CreateSharedProjectOutcome>, String> {
    let body = serde_json::to_value(input).map_err(|_| "Invalid shared project input.")?;
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects"),
        Some(body),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_get_shared_project<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    q: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<SharedProjectDetails>, String> {
    let qs = build_query_suffix_with_search(None, q.as_deref(), cursor.as_deref(), limit);
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_update_shared_project<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    name: String,
    purpose: String,
    expected_version: i64,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProjectDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::PATCH,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}"),
        Some(serde_json::json!({
            "name": name,
            "purpose": purpose,
            "expectedVersion": expected_version,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_archive_shared_project<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProjectDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/archive"),
        Some(serde_json::json!({ "idempotency_key": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_restore_shared_project<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProjectDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/restore"),
        Some(serde_json::json!({ "idempotency_key": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_list_project_picker_pages<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    q: Option<String>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<PagedList<SharedPageSummary>>, String> {
    let qs = build_query_suffix_with_search(None, q.as_deref(), cursor.as_deref(), limit);
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/page-picker{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_attach_page_to_project<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    page_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<ProjectPageMembershipOutcome>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/pages"),
        Some(serde_json::json!({
            "pageId": page_id,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_detach_page_from_project<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    project_id: String,
    page_id: String,
    membership_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProjectDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/projects/{project_id}/pages/{page_id}/detach"),
        Some(serde_json::json!({
            "membershipId": membership_id,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_list_shared_pages<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    include_archived: Option<bool>,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<PagedList<SharedPageSummary>>, String> {
    let qs = build_query_suffix(
        Some(include_archived.unwrap_or(false)),
        cursor.as_deref(),
        limit,
    );
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/pages{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_create_shared_page<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    input: CreateSharedPageInput,
) -> Result<WorkspaceResponse<CreateSharedPageOutcome>, String> {
    let body = serde_json::to_value(input).map_err(|_| "Invalid shared page input.")?;
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages"),
        Some(body),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_get_shared_page<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
) -> Result<WorkspaceResponse<SharedPageDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_list_shared_revisions<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<PagedList<SharedPageRevisionView>>, String> {
    let qs = build_query_suffix(None, cursor.as_deref(), limit);
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/revisions{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_archive_shared_page<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedPageDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/archive"),
        Some(serde_json::json!({ "idempotency_key": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_restore_shared_page<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedPageDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/restore"),
        Some(serde_json::json!({ "idempotency_key": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_list_page_threads<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<PagedList<SharedCommentThreadView>>, String> {
    let qs = build_query_suffix(None, cursor.as_deref(), limit);
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/threads{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_create_page_thread<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    base_revision_number: i64,
    body: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedCommentThreadView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/threads"),
        Some(serde_json::json!({
            "baseRevisionNumber": base_revision_number,
            "body": body,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_reply_page_thread<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    thread_id: String,
    body: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedCommentThreadView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/threads/{thread_id}/replies"),
        Some(serde_json::json!({
            "body": body,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_resolve_page_thread<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    thread_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedCommentThreadView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/threads/{thread_id}/resolve"),
        Some(serde_json::json!({ "idempotency_key": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_reopen_page_thread<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    thread_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedCommentThreadView>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/threads/{thread_id}/reopen"),
        Some(serde_json::json!({ "idempotency_key": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_list_page_proposals<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<PagedList<SharedProposalSummary>>, String> {
    let qs = build_query_suffix(None, cursor.as_deref(), limit);
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/proposals{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_get_proposal<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    proposal_id: String,
) -> Result<WorkspaceResponse<SharedProposalDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/proposals/{proposal_id}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_create_proposal<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    page_id: String,
    base_revision_number: i64,
    proposed_title: String,
    proposed_content_md: String,
    summary: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProposalDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/pages/{page_id}/proposals"),
        Some(serde_json::json!({
            "baseRevisionNumber": base_revision_number,
            "proposedTitle": proposed_title,
            "proposedContentMd": proposed_content_md,
            "summary": summary,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_resubmit_proposal<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    proposal_id: String,
    base_revision_number: i64,
    proposed_title: String,
    proposed_content_md: String,
    summary: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProposalDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/proposals/{proposal_id}/resubmit"),
        Some(serde_json::json!({
            "baseRevisionNumber": base_revision_number,
            "proposedTitle": proposed_title,
            "proposedContentMd": proposed_content_md,
            "summary": summary,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_withdraw_proposal<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    proposal_id: String,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProposalDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/proposals/{proposal_id}/withdraw"),
        Some(serde_json::json!({ "idempotency_key": idempotency_key })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_review_proposal<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    proposal_id: String,
    action: ProposalReviewAction,
    expected_current_revision: i64,
    expected_submission_number: i64,
    note: Option<String>,
    idempotency_key: String,
) -> Result<WorkspaceResponse<SharedProposalDetails>, String> {
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::POST,
        &format!("/v1/workspaces/{workspace_id}/proposals/{proposal_id}/review"),
        Some(serde_json::json!({
            "action": action,
            "expectedCurrentRevision": expected_current_revision,
            "expectedSubmissionNumber": expected_submission_number,
            "note": note,
            "idempotencyKey": idempotency_key,
        })),
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

#[tauri::command]
pub(crate) async fn cmd_list_workspace_activity<R: Runtime>(
    app: AppHandle<R>,
    attempt: tauri::State<'_, SignInAttempt>,
    workspace_id: String,
    cursor: Option<String>,
    limit: Option<usize>,
) -> Result<WorkspaceResponse<PagedList<SharedActivityItem>>, String> {
    let qs = build_query_suffix(None, cursor.as_deref(), limit);
    let (resp, _) = invoke_workspace_api(
        &app,
        &attempt,
        Method::GET,
        &format!("/v1/workspaces/{workspace_id}/activity{qs}"),
        None,
        Some(&workspace_id),
    )
    .await?;
    Ok(resp)
}

pub(crate) async fn resolve_verified_draft_uid(
    store: &SessionStore,
    config: &SignInConfig,
    saved: &SavedDeviceSession,
    expected_uid: Option<&str>,
) -> Result<String, String> {
    match crate::shared_drafts::read_verified_session_account_uid(
        &store.data_dir,
        &saved.session_id,
        expected_uid,
    ) {
        Ok(uid) => return Ok(uid),
        Err(crate::shared_drafts::DraftBindingError::AccountMismatch(msg)) => {
            return Err(msg);
        }
        Err(crate::shared_drafts::DraftBindingError::NeedsVerification) => {}
    }

    match store.fetch_devices(config, saved).await? {
        SessionStatus::SignedIn { uid, .. } => {
            let current = store.load_session().await?;
            if current.as_ref().map(|s| s.session_id.as_str()) != Some(saved.session_id.as_str()) {
                return Err(
                    "Active account changed; reload before accessing local shared drafts.".into(),
                );
            }
            let verified_uid = uid.trim();
            if verified_uid.is_empty() {
                return Err(
                    "Verified account identity is unavailable for local shared drafts.".into(),
                );
            }
            if let Some(expected) = expected_uid.map(str::trim).filter(|s| !s.is_empty()) {
                if expected != verified_uid {
                    return Err(
                        "Active account changed; reload before accessing local shared drafts."
                            .into(),
                    );
                }
            }
            Ok(verified_uid.to_owned())
        }
        SessionStatus::Unverified { message } => Err(format!(
            "Verify your Google session online before using local shared drafts. {message}"
        )),
        SessionStatus::SignedOut { .. } | SessionStatus::Unavailable => {
            Err("Sign in with Google to use local shared drafts.".into())
        }
    }
}

#[tauri::command]
pub(crate) async fn cmd_get_shared_draft<R: Runtime>(
    app: AppHandle<R>,
    workspace_id: String,
    draft_key: String,
    account_uid: Option<String>,
) -> Result<Option<crate::shared_drafts::SharedDraftRecord>, String> {
    let Some(config) = SignInConfig::from_env() else {
        return Ok(None);
    };
    let store = open_session_store(&app, &config).await?;
    let Some(saved) = store.load_session().await? else {
        return Ok(None);
    };
    let uid = resolve_verified_draft_uid(&store, &config, &saved, account_uid.as_deref()).await?;
    if store
        .load_session()
        .await?
        .as_ref()
        .map(|s| s.session_id.as_str())
        != Some(saved.session_id.as_str())
    {
        return Err("Active account changed; reload before accessing local shared drafts.".into());
    }
    crate::shared_drafts::load_draft(&store.data_dir, &uid, &workspace_id, &draft_key)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub(crate) async fn cmd_save_shared_draft<R: Runtime>(
    app: AppHandle<R>,
    workspace_id: String,
    draft_key: String,
    page_id: Option<String>,
    proposal_id: Option<String>,
    base_revision_number: Option<i64>,
    title: String,
    content_md: String,
    summary: String,
    account_uid: Option<String>,
) -> Result<crate::shared_drafts::SharedDraftRecord, String> {
    let config = SignInConfig::from_env().ok_or("Google sign-in is unavailable.")?;
    let store = open_session_store(&app, &config).await?;
    let saved = store
        .load_session()
        .await?
        .ok_or("Sign in with Google to save shared drafts.")?;
    let uid = resolve_verified_draft_uid(&store, &config, &saved, account_uid.as_deref()).await?;
    if store
        .load_session()
        .await?
        .as_ref()
        .map(|s| s.session_id.as_str())
        != Some(saved.session_id.as_str())
    {
        return Err("Active account changed; reload before saving local shared drafts.".into());
    }
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let record = crate::shared_drafts::SharedDraftRecord {
        account_uid: uid,
        workspace_id,
        draft_key,
        page_id,
        proposal_id,
        base_revision_number,
        title,
        content_md,
        summary,
        updated_at_ms: now_ms,
    };
    crate::shared_drafts::save_draft(&store.data_dir, &record)?;
    Ok(record)
}

#[tauri::command]
pub(crate) async fn cmd_delete_shared_draft<R: Runtime>(
    app: AppHandle<R>,
    workspace_id: String,
    draft_key: String,
    account_uid: Option<String>,
) -> Result<(), String> {
    let Some(config) = SignInConfig::from_env() else {
        return Ok(());
    };
    let store = open_session_store(&app, &config).await?;
    let Some(saved) = store.load_session().await? else {
        return Ok(());
    };
    let uid = resolve_verified_draft_uid(&store, &config, &saved, account_uid.as_deref()).await?;
    if store
        .load_session()
        .await?
        .as_ref()
        .map(|s| s.session_id.as_str())
        != Some(saved.session_id.as_str())
    {
        return Err("Active account changed; reload before discarding local shared drafts.".into());
    }
    crate::shared_drafts::delete_draft(&store.data_dir, &uid, &workspace_id, &draft_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_sign_in::{session::SecretBackend, tests::exchange_request};
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    #[derive(Default)]
    struct TestSecretBackend {
        entries: Mutex<HashMap<(String, String), Vec<u8>>>,
    }

    impl SecretBackend for TestSecretBackend {
        fn get_secret(&self, service: &str, slot: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self
                .entries
                .lock()
                .unwrap()
                .get(&(service.to_owned(), slot.to_owned()))
                .cloned())
        }

        fn set_secret(&self, service: &str, slot: &str, bytes: &[u8]) -> Result<(), String> {
            self.entries
                .lock()
                .unwrap()
                .insert((service.to_owned(), slot.to_owned()), bytes.to_vec());
            Ok(())
        }

        fn delete_secret(&self, service: &str, slot: &str) -> Result<(), String> {
            self.entries
                .lock()
                .unwrap()
                .remove(&(service.to_owned(), slot.to_owned()));
            Ok(())
        }
    }

    fn test_config(base: String) -> SignInConfig {
        SignInConfig {
            client_id: "qa.apps.googleusercontent.com".into(),
            client_secret: None,
            firebase_api_key: "qa-key".into(),
            session_service_url: base,
            device_label_override: None,
            google_authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            google_token_url: "https://oauth2.googleapis.com/token".into(),
        }
    }

    #[test]
    fn active_workspace_selection_persists_independently_and_clears_to_personal() {
        let dir = TempDir::new().unwrap();
        assert_eq!(read_active_workspace_id(dir.path()), None);
        write_active_workspace_id(dir.path(), Some("ws-team-1")).unwrap();
        assert_eq!(
            read_active_workspace_id(dir.path()).as_deref(),
            Some("ws-team-1")
        );
        write_active_workspace_id(dir.path(), None).unwrap();
        assert_eq!(read_active_workspace_id(dir.path()), None);
    }

    #[tokio::test]
    async fn workspace_removal_clears_active_workspace_and_preserves_unverified_on_ambiguous_401() {
        let dir = TempDir::new().unwrap();
        let backend = Arc::new(TestSecretBackend::default());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cfg = test_config(format!("http://{}", listener.local_addr().unwrap()));
        let store =
            SessionStore::open_with_backend(dir.path().to_path_buf(), &cfg, backend.clone())
                .await
                .unwrap();
        let saved = SavedDeviceSession {
            session_id: "sid-b".into(),
            credential: "sths_b_secret".into(),
            device_id: "dev-b".into(),
            device_label: "Desktop B".into(),
        };
        store.persist_and_migrate_legacy(&saved).await.unwrap();
        write_active_workspace_id(dir.path(), Some("ws-1")).unwrap();

        // 1. Ambiguous 401 keeps keyring credential and active workspace, returning SessionUpdated(Unverified).
        let (res, _) = tokio::join!(
            request_json::<WorkspaceDetails>(
                &store,
                None,
                &cfg,
                Method::GET,
                "/v1/workspaces/ws-1",
                None,
                Some("ws-1"),
            ),
            exchange_request(
                &listener,
                "GET /v1/workspaces/ws-1 HTTP/1.1",
                "401 Unauthorized",
                "<html>proxy auth</html>",
            )
        );
        assert!(matches!(
            res.unwrap(),
            WorkspaceResponse::SessionUpdated {
                session: SessionStatus::Unverified { .. }
            }
        ));
        assert!(backend
            .get_secret(
                "app.sthira.desktop.session",
                store.open_with_backend_sync_slot()
            )
            .unwrap()
            .is_some());
        assert_eq!(
            read_active_workspace_id(dir.path()).as_deref(),
            Some("ws-1")
        );

        // 2. Removed from active workspace -> 403 forbidden + refreshed /v1/workspaces omits ws-1 -> AccessLost clears to Personal and purges ws-1 drafts!
        crate::shared_drafts::record_session_account_uid(dir.path(), "sid-b", "uid-b").unwrap();
        crate::shared_drafts::save_draft(
            dir.path(),
            &crate::shared_drafts::SharedDraftRecord {
                account_uid: "uid-b".into(),
                workspace_id: "ws-1".into(),
                draft_key: "page_p1".into(),
                page_id: Some("p1".into()),
                proposal_id: None,
                base_revision_number: Some(1),
                title: "Unsent".into(),
                content_md: "Body".into(),
                summary: "S".into(),
                updated_at_ms: 100,
            },
        )
        .unwrap();
        let (access_res, _) = tokio::join!(
            request_json::<WorkspaceDetails>(
                &store,
                None,
                &cfg,
                Method::GET,
                "/v1/workspaces/ws-1",
                None,
                Some("ws-1"),
            ),
            async {
                exchange_request(
                    &listener,
                    "GET /v1/workspaces/ws-1 HTTP/1.1",
                    "403 Forbidden",
                    r#"{"code":"forbidden","error":"Removed"}"#,
                )
                .await;
                exchange_request(
                    &listener,
                    "GET /v1/workspaces HTTP/1.1",
                    "200 OK",
                    r#"{"workspaces":[],"incomingTransfers":[]}"#,
                )
                .await;
            }
        );
        assert!(matches!(
            access_res.unwrap(),
            WorkspaceResponse::AccessLost { .. }
        ));
        assert_eq!(read_active_workspace_id(dir.path()), None);
        assert_eq!(
            crate::shared_drafts::load_draft(dir.path(), "uid-b", "ws-1", "page_p1").unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn resolve_verified_draft_uid_requires_service_verification_when_binding_missing_or_stale_and_preserves_credential(
    ) {
        let dir = TempDir::new().unwrap();
        let backend = Arc::new(TestSecretBackend::default());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cfg = test_config(format!("http://{}", listener.local_addr().unwrap()));
        let store =
            SessionStore::open_with_backend(dir.path().to_path_buf(), &cfg, backend.clone())
                .await
                .unwrap();
        let saved = SavedDeviceSession {
            session_id: "sid-real".into(),
            credential: "sths_real".into(),
            device_id: "dev-real".into(),
            device_label: "Desktop Real".into(),
        };
        store.persist_and_migrate_legacy(&saved).await.unwrap();

        // Pre-seed Account B's draft under uid-b.
        crate::shared_drafts::save_draft(
            dir.path(),
            &crate::shared_drafts::SharedDraftRecord {
                account_uid: "uid-b".into(),
                workspace_id: "ws-1".into(),
                draft_key: "page_p1".into(),
                page_id: Some("p1".into()),
                proposal_id: None,
                base_revision_number: Some(1),
                title: "Secret B Draft".into(),
                content_md: "Private B".into(),
                summary: "S".into(),
                updated_at_ms: 100,
            },
        )
        .unwrap();

        // 1. Missing binding + unreachable/503 verification -> refuses draft access & preserves credential.
        let closed_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed_cfg = test_config(format!("http://{}", closed_listener.local_addr().unwrap()));
        drop(closed_listener);
        let unverified_res =
            resolve_verified_draft_uid(&store, &closed_cfg, &saved, Some("uid-b")).await;
        assert!(unverified_res.is_err());
        assert!(store.load_session().await.unwrap().is_some());

        // 2. Caller supplies uid-b, but verified /v1/devices returns uid-a -> mismatched UID is rejected & repairs to uid-a!
        let devices_body = serde_json::json!({
            "uid": "uid-a",
            "email": "a@example.invalid",
            "session_id": "sid-real",
            "devices": []
        })
        .to_string();
        let (mismatch_res, _) = tokio::join!(
            resolve_verified_draft_uid(&store, &cfg, &saved, Some("uid-b")),
            exchange_request(
                &listener,
                "GET /v1/devices HTTP/1.1",
                "200 OK",
                &devices_body,
            )
        );
        assert!(mismatch_res.is_err());
        assert_eq!(
            crate::shared_drafts::read_verified_session_account_uid(dir.path(), "sid-real", None),
            Ok("uid-a".into())
        );

        // 3. Previously verified offline recovery succeeds for uid-a, never for uid-b!
        let offline_ok =
            resolve_verified_draft_uid(&store, &closed_cfg, &saved, Some("uid-a")).await;
        assert_eq!(offline_ok, Ok("uid-a".into()));
        assert!(
            resolve_verified_draft_uid(&store, &closed_cfg, &saved, Some("uid-b"))
                .await
                .is_err()
        );
        assert!(
            crate::shared_drafts::load_draft(dir.path(), "uid-a", "ws-1", "page_p1")
                .unwrap()
                .is_none()
        );

        // 4. Stale binding belonging to another session requires fresh verification cleanly:
        crate::shared_drafts::record_session_account_uid(dir.path(), "sid-old", "uid-b").unwrap();
        let (repaired_res, _) = tokio::join!(
            resolve_verified_draft_uid(&store, &cfg, &saved, None),
            exchange_request(
                &listener,
                "GET /v1/devices HTTP/1.1",
                "200 OK",
                &devices_body,
            )
        );
        assert_eq!(repaired_res, Ok("uid-a".into()));
    }
}
