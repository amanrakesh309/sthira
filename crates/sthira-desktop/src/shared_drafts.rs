use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

const SESSION_ACCOUNT_FILE: &str = "session-account.json";
const DRAFTS_DIR: &str = "shared-drafts";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SharedDraftRecord {
    pub(crate) account_uid: String,
    pub(crate) workspace_id: String,
    pub(crate) draft_key: String,
    pub(crate) page_id: Option<String>,
    pub(crate) proposal_id: Option<String>,
    pub(crate) base_revision_number: Option<i64>,
    pub(crate) title: String,
    pub(crate) content_md: String,
    pub(crate) summary: String,
    pub(crate) updated_at_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionAccountBinding {
    session_id: String,
    uid: String,
}

fn account_dir_name(uid: &str) -> String {
    let digest = Sha256::digest(uid.as_bytes());
    format!("u-{:x}", digest)
}

fn safe_segment(raw: &str, label: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty()
        || trimmed.len() > 128
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err(format!("Invalid {label} identifier."));
    }
    Ok(trimmed.to_owned())
}

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| "Could not prepare local draft folder.")?;
        let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
    }
    let bytes = serde_json::to_vec(value).map_err(|_| "Could not serialize local draft.")?;
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| "Could not write local draft.")?;
        f.write_all(&bytes)
            .map_err(|_| "Could not write local draft.")?;
        let _ = f.sync_all();
    }
    fs::rename(&tmp, path).map_err(|_| "Could not finalize local draft.")?;
    Ok(())
}

pub(crate) fn record_session_account_uid(
    data_dir: &Path,
    session_id: &str,
    uid: &str,
) -> Result<(), String> {
    let uid = uid.trim();
    let session_id = session_id.trim();
    if uid.is_empty() || session_id.is_empty() {
        return Ok(());
    }
    write_private_json(
        &data_dir.join(SESSION_ACCOUNT_FILE),
        &SessionAccountBinding {
            session_id: session_id.to_owned(),
            uid: uid.to_owned(),
        },
    )
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DraftBindingError {
    NeedsVerification,
    AccountMismatch(String),
}

pub(crate) fn read_verified_session_account_uid(
    data_dir: &Path,
    session_id: &str,
    expected_uid: Option<&str>,
) -> Result<String, DraftBindingError> {
    let Ok(bytes) = fs::read(data_dir.join(SESSION_ACCOUNT_FILE)) else {
        return Err(DraftBindingError::NeedsVerification);
    };
    let Ok(binding) = serde_json::from_slice::<SessionAccountBinding>(&bytes) else {
        return Err(DraftBindingError::NeedsVerification);
    };
    let uid = binding.uid.trim();
    let bound_session = binding.session_id.trim();
    if uid.is_empty() || bound_session.is_empty() || bound_session != session_id {
        return Err(DraftBindingError::NeedsVerification);
    }
    if let Some(expected) = expected_uid.map(str::trim).filter(|s| !s.is_empty()) {
        if expected != uid {
            return Err(DraftBindingError::AccountMismatch(
                "Active account changed; reload before accessing local shared drafts.".into(),
            ));
        }
    }
    Ok(uid.to_owned())
}

fn draft_file_path(
    data_dir: &Path,
    account_uid: &str,
    workspace_id: &str,
    draft_key: &str,
) -> Result<PathBuf, String> {
    let ws_seg = safe_segment(workspace_id, "workspace")?;
    let key_seg = safe_segment(draft_key, "draft")?;
    Ok(data_dir
        .join(DRAFTS_DIR)
        .join(account_dir_name(account_uid))
        .join(ws_seg)
        .join(format!("{key_seg}.json")))
}

pub(crate) fn save_draft(data_dir: &Path, draft: &SharedDraftRecord) -> Result<(), String> {
    if draft.account_uid.trim().is_empty() {
        return Err("Account UID is required to save a shared draft.".into());
    }
    let path = draft_file_path(
        data_dir,
        &draft.account_uid,
        &draft.workspace_id,
        &draft.draft_key,
    )?;
    write_private_json(&path, draft)
}

pub(crate) fn load_draft(
    data_dir: &Path,
    account_uid: &str,
    workspace_id: &str,
    draft_key: &str,
) -> Result<Option<SharedDraftRecord>, String> {
    let path = draft_file_path(data_dir, account_uid, workspace_id, draft_key)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Could not open the saved local draft.".into()),
    };
    let parsed: SharedDraftRecord = serde_json::from_slice(&bytes)
        .map_err(|_| "Saved local draft could not be read.".to_owned())?;
    if parsed.account_uid != account_uid
        || parsed.workspace_id != workspace_id
        || parsed.draft_key != draft_key
    {
        return Err("Saved local draft could not be verified for this page.".into());
    }
    Ok(Some(parsed))
}

pub(crate) fn delete_draft(
    data_dir: &Path,
    account_uid: &str,
    workspace_id: &str,
    draft_key: &str,
) -> Result<(), String> {
    let path = draft_file_path(data_dir, account_uid, workspace_id, draft_key)?;
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Could not clear the saved local draft.".into()),
    }
}

pub(crate) fn purge_workspace_drafts(data_dir: &Path, account_uid: &str, workspace_id: &str) {
    let Ok(ws_seg) = safe_segment(workspace_id, "workspace") else {
        return;
    };
    let dir = data_dir
        .join(DRAFTS_DIR)
        .join(account_dir_name(account_uid))
        .join(ws_seg);
    if dir.exists() {
        let _ = fs::remove_dir_all(dir);
    }
}

pub(crate) fn clear_session_account_and_drafts(data_dir: &Path) {
    let binding_path = data_dir.join(SESSION_ACCOUNT_FILE);
    if let Ok(bytes) = fs::read(&binding_path) {
        if let Ok(binding) = serde_json::from_slice::<SessionAccountBinding>(&bytes) {
            if !binding.uid.trim().is_empty() {
                let acct_dir = data_dir
                    .join(DRAFTS_DIR)
                    .join(account_dir_name(&binding.uid));
                if acct_dir.exists() {
                    let _ = fs::remove_dir_all(acct_dir);
                }
            }
        }
    }
    if binding_path.exists() {
        let _ = fs::remove_file(binding_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn drafts_are_isolated_by_account_and_cleaned_on_access_loss_or_revocation() {
        let dir = TempDir::new().unwrap();
        let draft_a = SharedDraftRecord {
            account_uid: "uid-a".into(),
            workspace_id: "ws-1".into(),
            draft_key: "page_shp1".into(),
            page_id: Some("shp1".into()),
            proposal_id: None,
            base_revision_number: Some(2),
            title: "Draft by A".into(),
            content_md: "Private unsent body".into(),
            summary: "Summary".into(),
            updated_at_ms: 1000,
        };
        record_session_account_uid(dir.path(), "sess-a", "uid-a").unwrap();
        save_draft(dir.path(), &draft_a).unwrap();

        // Account B never sees Account A's draft.
        assert_eq!(
            load_draft(dir.path(), "uid-b", "ws-1", "page_shp1").unwrap(),
            None
        );
        assert_eq!(
            load_draft(dir.path(), "uid-a", "ws-1", "page_shp1")
                .unwrap()
                .unwrap()
                .title,
            "Draft by A"
        );

        // Session binding mismatch is blocked cleanly without granting access to caller UID.
        assert_eq!(
            read_verified_session_account_uid(dir.path(), "sess-a", Some("uid-b")),
            Err(DraftBindingError::AccountMismatch(
                "Active account changed; reload before accessing local shared drafts.".into()
            ))
        );

        // Stale session binding (wrong session_id) requires verification even with caller hint.
        assert_eq!(
            read_verified_session_account_uid(dir.path(), "sess-other", Some("uid-a")),
            Err(DraftBindingError::NeedsVerification)
        );

        // Corrupt binding requires verification.
        fs::write(dir.path().join(SESSION_ACCOUNT_FILE), b"{not json").unwrap();
        assert_eq!(
            read_verified_session_account_uid(dir.path(), "sess-a", Some("uid-a")),
            Err(DraftBindingError::NeedsVerification)
        );

        // Missing binding requires verification and does NOT create session-account.json.
        fs::remove_file(dir.path().join(SESSION_ACCOUNT_FILE)).unwrap();
        assert_eq!(
            read_verified_session_account_uid(dir.path(), "sess-a", Some("uid-a")),
            Err(DraftBindingError::NeedsVerification)
        );
        assert!(!dir.path().join(SESSION_ACCOUNT_FILE).exists());
        record_session_account_uid(dir.path(), "sess-a", "uid-a").unwrap();

        // Workspace removal removes only that workspace's drafts for uid-a.
        let draft_a_ws2 = SharedDraftRecord {
            workspace_id: "ws-2".into(),
            draft_key: "new_page".into(),
            ..draft_a.clone()
        };
        save_draft(dir.path(), &draft_a_ws2).unwrap();
        purge_workspace_drafts(dir.path(), "uid-a", "ws-1");
        assert_eq!(
            load_draft(dir.path(), "uid-a", "ws-1", "page_shp1").unwrap(),
            None
        );
        assert!(load_draft(dir.path(), "uid-a", "ws-2", "new_page")
            .unwrap()
            .is_some());

        // Terminal sign-out / revocation clears remaining drafts for that account.
        clear_session_account_and_drafts(dir.path());
        assert_eq!(
            load_draft(dir.path(), "uid-a", "ws-2", "new_page").unwrap(),
            None
        );
    }
}
