//! Native file boundary for logical shared content backups. No raw database import.
use crate::{
    account_sign_in::{open_session_store, SignInAttempt, SignInConfig},
    workspaces::{invoke_workspace_api, resolve_verified_draft_uid, WorkspaceResponse},
};
use reqwest::Method;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use tauri::{AppHandle, Manager, Runtime};

fn id(raw: &str) -> Result<&str, String> {
    if raw.is_empty()
        || raw.len() > 128
        || !raw
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("Invalid workspace or backup identity.".into());
    }
    Ok(raw)
}
async fn identity<R: Runtime>(app: &AppHandle<R>, expected: &str) -> Result<String, String> {
    if expected.trim().is_empty() {
        return Err("Verify your account first.".into());
    }
    let config = SignInConfig::from_env().ok_or("Configure Google sign-in first.")?;
    let store = open_session_store(app, &config).await?;
    let saved = store
        .load_session()
        .await?
        .ok_or("Sign in to use workspace recovery.")?;
    resolve_verified_draft_uid(&store, &config, &saved, Some(expected)).await
}
async fn member<R: Runtime>(
    app: &AppHandle<R>,
    ws: &str,
    method: Method,
    suffix: &str,
    body: Option<Value>,
) -> Result<WorkspaceResponse<Value>, String> {
    let attempt = app.state::<SignInAttempt>();
    Ok(invoke_workspace_api(
        app,
        &attempt,
        method,
        &format!("/v1/workspaces/{ws}/{suffix}"),
        body,
        Some(ws),
    )
    .await?
    .0)
}

#[cfg(target_os = "linux")]
async fn choose<R: Runtime>(
    app: &AppHandle<R>,
    save: bool,
) -> Result<Option<std::path::PathBuf>, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        use gtk::prelude::*;
        let parent = handle
            .get_webview_window("main")
            .and_then(|w| w.gtk_window().ok());
        let dialog = gtk::FileChooserDialog::new(
            Some(if save {
                "Export workspace backup"
            } else {
                "Choose workspace backup"
            }),
            parent.as_ref(),
            if save {
                gtk::FileChooserAction::Save
            } else {
                gtk::FileChooserAction::Open
            },
        );
        dialog.add_buttons(&[
            ("Cancel", gtk::ResponseType::Cancel),
            (
                if save { "Export" } else { "Open" },
                gtk::ResponseType::Accept,
            ),
        ]);
        dialog.set_modal(true);
        if save {
            dialog.set_current_name("workspace.sthira-backup.json");
            dialog.set_do_overwrite_confirmation(true);
        }
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Sthira backup (.json)"));
        filter.add_pattern("*.json");
        dialog.add_filter(filter);
        let sender = std::cell::RefCell::new(Some(tx));
        dialog.connect_response(move |dialog, response| {
            let path = if response == gtk::ResponseType::Accept {
                dialog.filename()
            } else {
                None
            };
            if let Some(sender) = sender.borrow_mut().take() {
                let _ = sender.send(path);
            }
            dialog.close();
        });
        dialog.show_all();
    })
    .map_err(|_| "Could not open the file chooser.")?;
    rx.await
        .map_err(|_| "The file chooser closed unexpectedly.".into())
}
#[cfg(not(target_os = "linux"))]
async fn choose<R: Runtime>(
    _app: &AppHandle<R>,
    _save: bool,
) -> Result<Option<std::path::PathBuf>, String> {
    Err("Workspace backup file selection is currently available on Linux.".into())
}
fn read_bundle(path: &Path) -> Result<sthira_backup_format::Bundle, String> {
    let file = fs::File::open(path).map_err(|_| "Could not open the backup file.")?;
    if !file
        .metadata()
        .map_err(|_| "Could not inspect the backup file.")?
        .is_file()
    {
        return Err("Choose a regular backup file.".into());
    }
    let mut bytes = Vec::new();
    file.take(sthira_backup_format::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read the backup file.")?;
    sthira_backup_format::Bundle::parse(&bytes)
}
fn write_bundle(path: &Path, bundle: &sthira_backup_format::Bundle) -> Result<(), String> {
    bundle.validate()?;
    let parent = path.parent().ok_or("Invalid export destination.")?;
    let tmp = parent.join(format!(".sthira-backup-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut file = opts
            .open(&tmp)
            .map_err(|_| "Could not create the export file.")?;
        file.write_all(&serde_json::to_vec(bundle).map_err(|_| "Could not encode the backup.")?)
            .map_err(|_| "Could not write the backup.")?;
        file.sync_all()
            .map_err(|_| "Could not finish the backup file.")?;
        fs::rename(&tmp, path).map_err(|_| "Could not finish the export.")?;
        fs::File::open(parent).and_then(|f|f.sync_all()).map_err(|_|"Export was written, but the folder could not be synced. Check the file before relying on it.")?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
#[tauri::command]
pub(crate) async fn cmd_workspace_backups<R: Runtime>(
    app: AppHandle<R>,
    workspace_id: String,
    account_uid: String,
    action: String,
    input: Value,
) -> Result<WorkspaceResponse<Value>, String> {
    let ws = id(&workspace_id)?;
    let uid = identity(&app, &account_uid).await?;
    match action.as_str(){
        "list"=>member(&app,ws,Method::GET,"backups",None).await,
        "create"=>member(&app,ws,Method::POST,"backups",Some(json!({"idempotencyKey":input["idempotencyKey"]}))).await,
        "remove"=>member(&app,ws,Method::DELETE,&format!("backups/{}",id(input["backupId"].as_str().ok_or("Missing backup identity.")?)?),None).await,
        "restoration"=>member(&app,ws,Method::GET,"restoration",None).await,
        "history"=>member(&app,ws,Method::GET,"restoration/history",None).await,
        "commit"=>member(&app,ws,Method::POST,"backup-restore/commit",Some(json!({"ticketId":id(input["ticketId"].as_str().ok_or("Missing recovery preview.")?)?}))).await,
        "export"=>{
            let suffix=format!("backups/{}",id(input["backupId"].as_str().ok_or("Missing backup identity.")?)?);
            let first=member(&app,ws,Method::GET,&suffix,None).await?;
            if !matches!(first,WorkspaceResponse::Ok{..}) {return Ok(first);}
            let Some(path)=choose(&app,true).await? else {return Ok(WorkspaceResponse::Ok{data:json!({"cancelled":true})});};
            identity(&app,&uid).await?;
            // File dialogs can remain open while membership or the active account changes.
            match member(&app,ws,Method::GET,&suffix,None).await? {
                WorkspaceResponse::Ok{data}=>{
                    let bundle=sthira_backup_format::Bundle::parse(&serde_json::to_vec(&data).map_err(|_|"Invalid backup response.")?)?;
                    identity(&app,&uid).await?;
                    write_bundle(&path,&bundle)?;
                    Ok(WorkspaceResponse::Ok{data:json!({"exported":true,"checksumSha256":bundle.checksum_sha256})})
                }, other=>Ok(other)
            }
        },
        "previewFile"=>{
            // Check current permissions before displaying a native picker.
            let first=member(&app,ws,Method::GET,"backups",None).await?;
            if !matches!(first,WorkspaceResponse::Ok{..}) {return Ok(first);}
            let Some(path)=choose(&app,false).await? else{return Ok(WorkspaceResponse::Ok{data:json!({"cancelled":true})});};
            identity(&app,&uid).await?;
            let bundle=read_bundle(&path)?; // Validate the exact allowlisted format before any upload.
            identity(&app,&uid).await?;
            let legacy = if bundle.version == 1 { input["legacyVisibility"].as_str() } else { None };
            let payload = if let Some(choice) = legacy {
                serde_json::json!({"backup": bundle, "legacyVisibility": choice})
            } else {
                serde_json::to_value(bundle).map_err(|_|"Invalid backup.")?
            };
            member(&app,ws,Method::POST,"backup-restore/preview",Some(payload)).await
        },
        _=>Err("Unknown recovery action.".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_export_and_local_validation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.json");
        let bundle = sthira_backup_format::Bundle::new(sthira_backup_format::Payload {
            source_workspace_id: "qa".into(),
            source_name: "QA".into(),
            captured_at_ms: 1,
            records: sthira_backup_format::LIVE
                .iter()
                .map(|s| sthira_backup_format::Records {
                    table: s.table.into(),
                    rows: vec![],
                })
                .collect(),
            history: vec![],
            access: None,
        })
        .unwrap();
        fs::write(&path, "old destination").unwrap();
        write_bundle(&path, &bundle).unwrap();
        assert_eq!(
            read_bundle(&path).unwrap().checksum_sha256,
            bundle.checksum_sha256
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::write(&path, b"SQLite format 3\0secret").unwrap();
        assert!(read_bundle(&path).is_err());
        let mut invalid = bundle;
        invalid.payload.source_name = "tampered".into();
        assert!(write_bundle(&path, &invalid).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"SQLite format 3\0secret");
    }
}
