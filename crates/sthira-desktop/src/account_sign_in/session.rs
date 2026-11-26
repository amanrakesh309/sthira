use super::{sign_in_client, SignInConfig};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

const SERVICE: &str = "app.sthira.desktop.session";
const LEGACY_FIREBASE_SERVICE: &str = "app.sthira.desktop.firebase";
const KEYRING_ERROR: &str = "Could not access the desktop keyring. Unlock it and try again.";
const VERIFY_UNAVAILABLE_MESSAGE: &str =
    "Saved session can’t be verified right now because the session service is unreachable. Your personal vault still works offline.";
const AMBIGUOUS_UNAUTHORIZED_MESSAGE: &str =
    "Saved session could not be verified because the session service returned an unexpected authentication response. Your saved session was kept; try again.";

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum SignOutReason {
    Revoked,
    Expired,
    Invalid,
    MigrationRequired,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub(crate) enum RevokeDeviceResult {
    #[serde(rename_all = "camelCase")]
    Revoked {
        already_revoked: bool,
        session: SessionStatus,
    },
    #[serde(rename_all = "camelCase")]
    StaleReplaced {
        message: String,
        replacement_session_id: Option<String>,
        session: SessionStatus,
    },
    Missing {
        message: String,
        session: SessionStatus,
    },
    CallerInvalid {
        session: SessionStatus,
    },
    Unverified {
        session: SessionStatus,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(crate) enum SessionStatus {
    Unavailable,
    SignedOut {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<SignOutReason>,
    },
    Unverified {
        message: String,
    },
    #[serde(rename_all = "camelCase")]
    SignedIn {
        #[serde(default)]
        uid: String,
        email: String,
        current_session_id: String,
        current_device_label: String,
        devices: Vec<DeviceInfo>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceInfo {
    pub(crate) session_id: String,
    pub(crate) device_id: String,
    pub(crate) device_label: String,
    pub(crate) created_at_ms: i64,
    pub(crate) last_active_at_ms: i64,
    pub(crate) expires_at_ms: i64,
    pub(crate) is_current: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct DeviceIdentity {
    pub(crate) device_id: String,
    pub(crate) device_label: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct SavedDeviceSession {
    pub(crate) session_id: String,
    pub(crate) credential: String,
    pub(crate) device_id: String,
    pub(crate) device_label: String,
}

impl fmt::Debug for SavedDeviceSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SavedDeviceSession")
            .field("session_id", &self.session_id)
            .field("credential", &"[redacted]")
            .field("device_id", &self.device_id)
            .field("device_label", &self.device_label)
            .finish()
    }
}

#[derive(Deserialize)]
pub(crate) struct EnrolledSessionResponse {
    pub(crate) session_id: String,
    pub(crate) credential: String,
    #[serde(default)]
    pub(crate) uid: String,
    pub(crate) email: String,
    pub(crate) device_id: String,
    pub(crate) device_label: String,
}

#[derive(Deserialize)]
struct DevicesApiResponse {
    session_id: String,
    #[serde(default)]
    uid: String,
    email: String,
    devices: Vec<WireDeviceSummary>,
}

#[derive(Deserialize)]
struct WireDeviceSummary {
    session_id: String,
    device_id: String,
    device_label: String,
    created_at_ms: i64,
    last_active_at_ms: i64,
    expires_at_ms: i64,
    is_current: bool,
}

pub(crate) trait SecretBackend: Send + Sync {
    fn get_secret(&self, service: &str, slot: &str) -> Result<Option<Vec<u8>>, String>;
    fn set_secret(&self, service: &str, slot: &str, bytes: &[u8]) -> Result<(), String>;
    fn delete_secret(&self, service: &str, slot: &str) -> Result<(), String>;
}

struct KeyringSecretBackend;

impl SecretBackend for KeyringSecretBackend {
    fn get_secret(&self, service: &str, slot: &str) -> Result<Option<Vec<u8>>, String> {
        let entry = keyring::Entry::new(service, slot).map_err(|_| KEYRING_ERROR.to_owned())?;
        match entry.get_secret() {
            Ok(bytes) => Ok(Some(bytes)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err(KEYRING_ERROR.to_owned()),
        }
    }

    fn set_secret(&self, service: &str, slot: &str, bytes: &[u8]) -> Result<(), String> {
        let entry = keyring::Entry::new(service, slot).map_err(|_| KEYRING_ERROR.to_owned())?;
        entry
            .set_secret(bytes)
            .map_err(|_| KEYRING_ERROR.to_owned())
    }

    fn delete_secret(&self, service: &str, slot: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(service, slot).map_err(|_| KEYRING_ERROR.to_owned())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(KEYRING_ERROR.to_owned()),
        }
    }
}

pub(crate) struct SessionStore {
    pub(crate) data_dir: PathBuf,
    slot: String,
    backend: Arc<dyn SecretBackend>,
    // Serialize credential replacement, device identity, and deletion across desktop processes.
    _lock: File,
}

impl SessionStore {
    pub(crate) async fn open(path: PathBuf, config: &SignInConfig) -> Result<Self, String> {
        Self::open_with_backend(path, config, Arc::new(KeyringSecretBackend)).await
    }

    #[cfg(test)]
    pub(crate) fn open_with_backend_sync_slot(&self) -> &str {
        &self.slot
    }

    pub(crate) async fn open_with_backend(
        path: PathBuf,
        config: &SignInConfig,
        backend: Arc<dyn SecretBackend>,
    ) -> Result<Self, String> {
        let mut scope = Sha256::new();
        scope.update(config.client_id.as_bytes());
        scope.update(b"\0");
        scope.update(config.firebase_api_key.as_bytes());
        let slot = format!("session-{:x}", scope.finalize());
        let lock_dir = path.clone();
        let lock_slot = slot.clone();
        let lock = keyring_task(move || {
            fs::create_dir_all(&lock_dir).map_err(|_| "Could not open account session storage.")?;
            let lock = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(lock_dir.join(format!("{lock_slot}.lock")))
                .map_err(|_| "Could not open account session storage.")?;
            lock.lock()
                .map_err(|_| "Could not lock account session storage.")?;
            Ok(lock)
        })
        .await?;
        Ok(Self {
            data_dir: path,
            slot,
            backend,
            _lock: lock,
        })
    }

    pub(super) async fn load_or_create_device_identity(
        &self,
        config: &SignInConfig,
    ) -> Result<DeviceIdentity, String> {
        let path = self.data_dir.join("device-identity.json");
        let label_override = config.device_label_override.clone();
        keyring_task(move || read_or_create_device_identity(&path, label_override)).await
    }

    pub(crate) async fn load_session(&self) -> Result<Option<SavedDeviceSession>, String> {
        let backend = Arc::clone(&self.backend);
        let slot = self.slot.clone();
        keyring_task(move || {
            let Some(bytes) = backend.get_secret(SERVICE, &slot)? else {
                return Ok(None);
            };
            serde_json::from_slice(&bytes).map(Some).map_err(|_| {
                "The saved session could not be read. Sign out and sign in again.".into()
            })
        })
        .await
    }

    pub(crate) async fn has_legacy_pilot_session(&self) -> Result<bool, String> {
        let backend = Arc::clone(&self.backend);
        let slot = self.slot.clone();
        keyring_task(move || {
            Ok(backend
                .get_secret(LEGACY_FIREBASE_SERVICE, &slot)?
                .is_some())
        })
        .await
    }

    pub(crate) async fn persist_and_migrate_legacy(
        &self,
        session: &SavedDeviceSession,
    ) -> Result<(), String> {
        let backend = Arc::clone(&self.backend);
        let slot = self.slot.clone();
        let bytes = serde_json::to_vec(session)
            .map_err(|_| "Could not prepare the session credential for storage.")?;
        keyring_task(move || {
            // Save the new Sthira credential securely before removing any legacy pilot keyring entry.
            backend.set_secret(SERVICE, &slot, &bytes)?;
            let _ = backend.delete_secret(LEGACY_FIREBASE_SERVICE, &slot);
            Ok(())
        })
        .await
    }

    async fn clear_all_credentials(
        &self,
        cleanup: crate::shared_drafts::DraftCleanup,
    ) -> Result<(), String> {
        let backend = Arc::clone(&self.backend);
        let slot = self.slot.clone();
        let data_dir = self.data_dir.clone();
        keyring_task(move || {
            let new_res = backend.delete_secret(SERVICE, &slot);
            let legacy_res = backend.delete_secret(LEGACY_FIREBASE_SERVICE, &slot);
            crate::shared_drafts::clear_session_account_and_drafts(&data_dir, cleanup);
            new_res?;
            legacy_res?;
            Ok(())
        })
        .await
    }

    pub(super) async fn restore(&self, config: &SignInConfig) -> Result<SessionStatus, String> {
        let Some(saved) = self.load_session().await? else {
            if self.has_legacy_pilot_session().await? {
                // Legacy Firebase refresh tokens never auto-restore or enroll a replacement device.
                return Ok(SessionStatus::SignedOut {
                    reason: Some(SignOutReason::MigrationRequired),
                });
            }
            return Ok(SessionStatus::SignedOut { reason: None });
        };

        self.fetch_devices(config, &saved).await
    }

    pub(crate) async fn fetch_devices(
        &self,
        config: &SignInConfig,
        saved: &SavedDeviceSession,
    ) -> Result<SessionStatus, String> {
        let http = sign_in_client()?;
        let url = format!("{}/v1/devices", config.session_service_url);
        let response = match http.get(&url).bearer_auth(&saved.credential).send().await {
            Ok(resp) => resp,
            Err(_) => {
                return Ok(SessionStatus::Unverified {
                    message: VERIFY_UNAVAILABLE_MESSAGE.into(),
                });
            }
        };

        if response.status() == StatusCode::UNAUTHORIZED {
            return self.handle_unauthorized(response).await;
        }
        if !response.status().is_success() {
            return Ok(SessionStatus::Unverified {
                message: VERIFY_UNAVAILABLE_MESSAGE.into(),
            });
        }

        let payload: DevicesApiResponse = match response.json().await {
            Ok(p) => p,
            Err(_) => {
                return Ok(SessionStatus::Unverified {
                    message: VERIFY_UNAVAILABLE_MESSAGE.into(),
                });
            }
        };

        let devices: Vec<DeviceInfo> = payload
            .devices
            .into_iter()
            .map(|d| DeviceInfo {
                session_id: d.session_id,
                device_id: d.device_id,
                device_label: d.device_label,
                created_at_ms: d.created_at_ms,
                last_active_at_ms: d.last_active_at_ms,
                expires_at_ms: d.expires_at_ms,
                is_current: d.is_current,
            })
            .collect();

        if !payload.uid.trim().is_empty() {
            let _ = crate::shared_drafts::record_session_account_uid(
                &self.data_dir,
                &saved.session_id,
                &payload.uid,
            );
        }

        Ok(SessionStatus::SignedIn {
            uid: payload.uid,
            email: payload.email,
            current_session_id: payload.session_id,
            current_device_label: saved.device_label.clone(),
            devices,
        })
    }

    pub(super) async fn revoke_other_device(
        &self,
        config: &SignInConfig,
        target_session_id: &str,
    ) -> Result<RevokeDeviceResult, String> {
        let Some(saved) = self.load_session().await? else {
            return Ok(RevokeDeviceResult::CallerInvalid {
                session: SessionStatus::SignedOut { reason: None },
            });
        };

        let http = sign_in_client()?;
        let url = format!("{}/v1/devices/revoke", config.session_service_url);
        let response = http
            .post(&url)
            .bearer_auth(&saved.credential)
            .json(&serde_json::json!({ "session_id": target_session_id }))
            .send()
            .await
            .map_err(|_| {
                "Could not reach the Sthira session service to sign out that device. Try again when connected."
                    .to_owned()
            })?;

        if response.status() == StatusCode::UNAUTHORIZED {
            return match self.handle_unauthorized(response).await? {
                unverified @ SessionStatus::Unverified { .. } => {
                    Ok(RevokeDeviceResult::Unverified {
                        session: unverified,
                    })
                }
                terminal => Ok(RevokeDeviceResult::CallerInvalid { session: terminal }),
            };
        }
        if response.status() == StatusCode::FORBIDDEN {
            return Err("You do not have permission to sign out that device.".into());
        }
        if response.status() == StatusCode::NOT_FOUND {
            let session = self.fetch_devices(config, &saved).await?;
            return Ok(match session {
                SessionStatus::SignedOut { .. } => RevokeDeviceResult::CallerInvalid { session },
                SessionStatus::Unverified { .. } => RevokeDeviceResult::Unverified { session },
                other => RevokeDeviceResult::Missing {
                    message:
                        "That device session was no longer active. The Devices list has been refreshed."
                            .into(),
                    session: other,
                },
            });
        }
        if response.status() == StatusCode::CONFLICT {
            let body = response.json::<serde_json::Value>().await.ok();
            let code = body
                .as_ref()
                .and_then(|v| v["code"].as_str())
                .unwrap_or_default();
            if code == "stale_device_session" {
                let replacement_session_id = body
                    .as_ref()
                    .and_then(|v| v["replacement_session_id"].as_str())
                    .map(str::to_owned);
                let session = self.fetch_devices(config, &saved).await?;
                return Ok(match session {
                    SessionStatus::SignedOut { .. } => {
                        RevokeDeviceResult::CallerInvalid { session }
                    }
                    SessionStatus::Unverified { .. } => RevokeDeviceResult::Unverified { session },
                    other => RevokeDeviceResult::StaleReplaced {
                        message: "This device signed in again with a newer session. Review the refreshed Devices list and confirm if you still want to sign it out.".into(),
                        replacement_session_id,
                        session: other,
                    },
                });
            }
            return Err(
                "Could not sign out that device right now. Refresh Devices and try again.".into(),
            );
        }
        if !response.status().is_success() {
            return Err("Could not sign out that device right now. Try again.".into());
        }

        #[derive(Deserialize)]
        struct RevokeApiResponse {
            revoked: bool,
            #[serde(default)]
            already_revoked: bool,
            session_id: String,
        }
        let body: RevokeApiResponse = response.json().await.map_err(|_| {
            "Unexpected revocation response from Sthira session service.".to_owned()
        })?;
        if !body.revoked || body.session_id != target_session_id {
            return Err(
                "Sthira session service did not confirm revocation for that device.".into(),
            );
        }

        let session = self.fetch_devices(config, &saved).await?;
        Ok(match session {
            SessionStatus::SignedOut { .. } => RevokeDeviceResult::CallerInvalid { session },
            other => RevokeDeviceResult::Revoked {
                already_revoked: body.already_revoked,
                session: other,
            },
        })
    }

    pub(super) async fn local_sign_out(&self, config: &SignInConfig) -> Result<(), String> {
        if let Some(saved) = self.load_session().await.ok().flatten() {
            if let Ok(http) = sign_in_client() {
                let url = format!("{}/v1/sessions/sign-out", config.session_service_url);
                let _ = http
                    .post(&url)
                    .timeout(std::time::Duration::from_secs(3))
                    .bearer_auth(&saved.credential)
                    .send()
                    .await;
            }
        }
        self.clear_all_credentials(crate::shared_drafts::DraftCleanup::KeepLiveRecovery)
            .await
    }

    pub(crate) async fn handle_unauthorized(
        &self,
        response: reqwest::Response,
    ) -> Result<SessionStatus, String> {
        let Ok(raw_text) = response.text().await else {
            return Ok(SessionStatus::Unverified {
                message: AMBIGUOUS_UNAUTHORIZED_MESSAGE.into(),
            });
        };
        let Ok(body) = serde_json::from_str::<serde_json::Value>(&raw_text) else {
            return Ok(SessionStatus::Unverified {
                message: AMBIGUOUS_UNAUTHORIZED_MESSAGE.into(),
            });
        };
        let Some(code) = body.get("code").and_then(|v| v.as_str()) else {
            return Ok(SessionStatus::Unverified {
                message: AMBIGUOUS_UNAUTHORIZED_MESSAGE.into(),
            });
        };
        let reason = match code {
            "session_revoked" => SignOutReason::Revoked,
            "session_expired" => SignOutReason::Expired,
            "invalid_session" => SignOutReason::Invalid,
            _ => {
                // Unknown 401 codes must preserve saved credentials so ambiguous responses never wipe a session.
                return Ok(SessionStatus::Unverified {
                    message: AMBIGUOUS_UNAUTHORIZED_MESSAGE.into(),
                });
            }
        };
        self.clear_all_credentials(crate::shared_drafts::DraftCleanup::All)
            .await?;
        Ok(SessionStatus::SignedOut {
            reason: Some(reason),
        })
    }
}

fn read_or_create_device_identity(
    path: &Path,
    label_override: Option<String>,
) -> Result<DeviceIdentity, String> {
    if path.exists() {
        let bytes = fs::read(path).map_err(|_| "Could not read device installation identity.")?;
        let mut identity: DeviceIdentity = serde_json::from_slice(&bytes)
            .map_err(|_| "Could not read device installation identity.")?;
        if let Some(custom) = label_override {
            if !custom.is_empty() && identity.device_label != custom {
                identity.device_label = custom;
                write_device_identity_atomic(path, &identity)?;
            }
        }
        return Ok(identity);
    }

    let device_id = Uuid::new_v4().to_string();
    let device_label = label_override.unwrap_or_else(|| default_device_label(&device_id));
    let identity = DeviceIdentity {
        device_id,
        device_label,
    };
    write_device_identity_atomic(path, &identity)?;
    Ok(identity)
}

fn write_device_identity_atomic(path: &Path, identity: &DeviceIdentity) -> Result<(), String> {
    let bytes =
        serde_json::to_vec(identity).map_err(|_| "Could not save device installation identity.")?;
    let tmp = path.with_extension("json.tmp");
    {
        use std::io::Write;
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| "Could not save device installation identity.")?;
        f.write_all(&bytes)
            .map_err(|_| "Could not save device installation identity.")?;
        let _ = f.sync_all();
    }
    fs::rename(&tmp, path).map_err(|_| "Could not save device installation identity.")?;
    Ok(())
}

fn default_device_label(device_id: &str) -> String {
    let host = fs::read_to_string("/etc/hostname")
        .ok()
        .or_else(|| std::env::var("HOSTNAME").ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Linux device".to_owned());
    let short_id = device_id.get(..4).unwrap_or("dev");
    format!("{host} (Linux · {short_id})")
}

async fn keyring_task<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| KEYRING_ERROR.to_owned())?
}

#[cfg(test)]
mod tests {
    use super::super::tests::exchange_request;
    use super::*;
    use std::{collections::HashMap, sync::Mutex};
    use tempfile::TempDir;
    use tokio::net::TcpListener;

    #[derive(Default)]
    struct MemorySecretBackend {
        entries: Mutex<HashMap<(String, String), Vec<u8>>>,
        fail_next_set: Mutex<bool>,
    }

    impl SecretBackend for MemorySecretBackend {
        fn get_secret(&self, service: &str, slot: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self
                .entries
                .lock()
                .unwrap()
                .get(&(service.to_owned(), slot.to_owned()))
                .cloned())
        }

        fn set_secret(&self, service: &str, slot: &str, bytes: &[u8]) -> Result<(), String> {
            let mut fail = self.fail_next_set.lock().unwrap();
            if *fail {
                *fail = false;
                return Err("Simulated keyring failure".into());
            }
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

    fn test_config(service_url: String) -> SignInConfig {
        SignInConfig {
            client_id: "qa.apps.googleusercontent.com".into(),
            client_secret: None,
            firebase_api_key: "qa-key".into(),
            session_service_url: service_url,
            device_label_override: None,
            google_authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            google_token_url: "https://oauth2.googleapis.com/token".into(),
        }
    }

    #[tokio::test]
    async fn legacy_pilot_session_requires_explicit_migration_and_preserves_legacy_until_saved() {
        let dir = TempDir::new().unwrap();
        let backend = Arc::new(MemorySecretBackend::default());
        let config = test_config("http://127.0.0.1:9".into());
        let store =
            SessionStore::open_with_backend(dir.path().to_path_buf(), &config, backend.clone())
                .await
                .unwrap();

        backend
            .set_secret(
                LEGACY_FIREBASE_SERVICE,
                &store.slot,
                br#"{"uid":"old-uid","refresh_token":"old-fb-refresh"}"#,
            )
            .unwrap();

        // Must NEVER restore or exchange old Firebase refresh token
        let restored = store.restore(&config).await.unwrap();
        assert!(matches!(
            restored,
            SessionStatus::SignedOut {
                reason: Some(SignOutReason::MigrationRequired)
            }
        ));

        // If persisting new credential fails, legacy must NOT be deleted
        *backend.fail_next_set.lock().unwrap() = true;
        let new_session = SavedDeviceSession {
            session_id: "sid-new".into(),
            credential: "sths_new_secret".into(),
            device_id: "dev-1".into(),
            device_label: "Workstation".into(),
        };
        assert!(store
            .persist_and_migrate_legacy(&new_session)
            .await
            .is_err());
        assert!(backend
            .get_secret(LEGACY_FIREBASE_SERVICE, &store.slot)
            .unwrap()
            .is_some());

        // Succeeding write deletes legacy entry cleanly
        store
            .persist_and_migrate_legacy(&new_session)
            .await
            .unwrap();
        assert!(backend
            .get_secret(LEGACY_FIREBASE_SERVICE, &store.slot)
            .unwrap()
            .is_none());
        assert!(backend.get_secret(SERVICE, &store.slot).unwrap().is_some());
    }

    #[tokio::test]
    async fn distinguishes_confirmed_revocation_from_service_outage() {
        let dir = TempDir::new().unwrap();
        let backend = Arc::new(MemorySecretBackend::default());

        // 1. Service unreachable -> Unverified, keyring preserved!
        let unreachable_cfg = test_config("http://127.0.0.1:9".into());
        let store = SessionStore::open_with_backend(
            dir.path().to_path_buf(),
            &unreachable_cfg,
            backend.clone(),
        )
        .await
        .unwrap();
        let saved = SavedDeviceSession {
            session_id: "sid-b".into(),
            credential: "sths_b_secret".into(),
            device_id: "dev-b".into(),
            device_label: "Desktop B".into(),
        };
        store.persist_and_migrate_legacy(&saved).await.unwrap();

        let outage_status = store.restore(&unreachable_cfg).await.unwrap();
        assert!(matches!(outage_status, SessionStatus::Unverified { .. }));
        assert!(backend.get_secret(SERVICE, &store.slot).unwrap().is_some());

        // 2. Service returns 401 session_revoked -> keyring cleared, Revoked status!
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let live_cfg = test_config(format!("http://{}", listener.local_addr().unwrap()));
        let (status, _) = tokio::join!(
            store.restore(&live_cfg),
            exchange_request(
                &listener,
                "GET /v1/devices HTTP/1.1",
                "401 Unauthorized",
                r#"{"code":"session_revoked","error":"This device was signed out."}"#,
            )
        );
        assert!(matches!(
            status.unwrap(),
            SessionStatus::SignedOut {
                reason: Some(SignOutReason::Revoked)
            }
        ));
        assert!(backend.get_secret(SERVICE, &store.slot).unwrap().is_none());
    }

    #[tokio::test]
    async fn device_identity_persists_with_distinct_default_label_across_installations() {
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        let cfg = test_config("http://127.0.0.1:8787".into());
        let backend = Arc::new(MemorySecretBackend::default());

        let store_a =
            SessionStore::open_with_backend(dir_a.path().to_path_buf(), &cfg, backend.clone())
                .await
                .unwrap();
        let store_b = SessionStore::open_with_backend(dir_b.path().to_path_buf(), &cfg, backend)
            .await
            .unwrap();

        let id_a1 = store_a.load_or_create_device_identity(&cfg).await.unwrap();
        let id_a2 = store_a.load_or_create_device_identity(&cfg).await.unwrap();
        let id_b = store_b.load_or_create_device_identity(&cfg).await.unwrap();

        assert_eq!(id_a1.device_id, id_a2.device_id);
        assert_eq!(id_a1.device_label, id_a2.device_label);
        assert_ne!(id_a1.device_id, id_b.device_id);
        assert_ne!(id_a1.device_label, id_b.device_label);
    }

    #[tokio::test]
    async fn ambiguous_401_preserves_credential_across_restore_and_revoke_while_terminal_codes_clear(
    ) {
        let dir = TempDir::new().unwrap();
        let backend = Arc::new(MemorySecretBackend::default());
        let saved = SavedDeviceSession {
            session_id: "sid-a".into(),
            credential: "sths_a_secret".into(),
            device_id: "dev-a".into(),
            device_label: "Laptop A".into(),
        };

        // Ambiguous 401 payloads during restore must NEVER wipe saved credential, allowing later retry to succeed.
        for (body, status_line) in [
            ("", "401 Unauthorized"),
            (
                "<html><body>Proxy auth required</body></html>",
                "401 Unauthorized",
            ),
            ("{}", "401 Unauthorized"),
            (
                r#"{"code":"unknown_proxy_code","error":"Proxy blocked"}"#,
                "401 Unauthorized",
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let cfg = test_config(format!("http://{}", listener.local_addr().unwrap()));
            let store =
                SessionStore::open_with_backend(dir.path().to_path_buf(), &cfg, backend.clone())
                    .await
                    .unwrap();
            store.persist_and_migrate_legacy(&saved).await.unwrap();

            let (status, _) = tokio::join!(
                store.restore(&cfg),
                exchange_request(&listener, "GET /v1/devices HTTP/1.1", status_line, body)
            );
            assert!(matches!(status.unwrap(), SessionStatus::Unverified { .. }));
            assert!(
                backend.get_secret(SERVICE, &store.slot).unwrap().is_some(),
                "ambiguous 401 must keep credential for body={body}"
            );
        }

        // Subsequent retry with 200 OK restores SignedIn cleanly!
        let listener_ok = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cfg_ok = test_config(format!("http://{}", listener_ok.local_addr().unwrap()));
        let store =
            SessionStore::open_with_backend(dir.path().to_path_buf(), &cfg_ok, backend.clone())
                .await
                .unwrap();
        let (retry_status, _) = tokio::join!(
            store.restore(&cfg_ok),
            exchange_request(
                &listener_ok,
                "GET /v1/devices HTTP/1.1",
                "200 OK",
                r#"{"session_id":"sid-a","email":"a@example.invalid","devices":[]}"#,
            )
        );
        assert!(matches!(
            retry_status.unwrap(),
            SessionStatus::SignedIn { .. }
        ));

        // Ambiguous 401 during remote revoke must return Unverified, keep A's credential intact,
        // and allow subsequent Retry verification (restore) to recover Devices cleanly.
        let listener_rev = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cfg_rev = test_config(format!("http://{}", listener_rev.local_addr().unwrap()));
        let (rev_res, _) = tokio::join!(
            store.revoke_other_device(&cfg_rev, "sid-b"),
            exchange_request(
                &listener_rev,
                "POST /v1/devices/revoke HTTP/1.1",
                "401 Unauthorized",
                "<html>Gateway Auth</html>",
            )
        );
        assert!(matches!(
            rev_res.unwrap(),
            RevokeDeviceResult::Unverified {
                session: SessionStatus::Unverified { .. }
            }
        ));
        let kept_bytes = backend
            .get_secret(SERVICE, &store.slot)
            .unwrap()
            .expect("ambiguous 401 during revoke must preserve caller credential");
        let kept_session: SavedDeviceSession = serde_json::from_slice(&kept_bytes).unwrap();
        assert_eq!(kept_session.credential, saved.credential);

        let listener_recover = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cfg_recover = test_config(format!("http://{}", listener_recover.local_addr().unwrap()));
        let (recover_status, _) = tokio::join!(
            store.restore(&cfg_recover),
            exchange_request(
                &listener_recover,
                "GET /v1/devices HTTP/1.1",
                "200 OK",
                r#"{"session_id":"sid-a","email":"a@example.invalid","devices":[{"session_id":"sid-a","device_id":"dev-a","device_label":"Laptop A","created_at_ms":1,"last_active_at_ms":2,"expires_at_ms":99,"is_current":true},{"session_id":"sid-b","device_id":"dev-b","device_label":"Desktop B","created_at_ms":3,"last_active_at_ms":4,"expires_at_ms":99,"is_current":false}]}"#,
            )
        );
        match recover_status.unwrap() {
            SessionStatus::SignedIn { devices, .. } => assert_eq!(devices.len(), 2),
            other => panic!("Expected SignedIn recovery after ambiguous revoke 401, got {other:?}"),
        }

        // Confirmed terminal codes clear credentials with exact reason on both restore and revoke
        for (code, expected_reason) in [
            ("session_expired", SignOutReason::Expired),
            ("invalid_session", SignOutReason::Invalid),
            ("session_revoked", SignOutReason::Revoked),
        ] {
            store.persist_and_migrate_legacy(&saved).await.unwrap();
            let listener_term = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let cfg_term = test_config(format!("http://{}", listener_term.local_addr().unwrap()));
            let payload = serde_json::json!({ "code": code, "error": "terminal" }).to_string();
            let (res, _) = tokio::join!(
                store.restore(&cfg_term),
                exchange_request(
                    &listener_term,
                    "GET /v1/devices HTTP/1.1",
                    "401 Unauthorized",
                    &payload,
                )
            );
            match res.unwrap() {
                SessionStatus::SignedOut { reason: Some(r) } => assert_eq!(r, expected_reason),
                other => panic!("Expected terminal SignedOut({expected_reason:?}), got {other:?}"),
            }
            assert!(backend.get_secret(SERVICE, &store.slot).unwrap().is_none());

            store.persist_and_migrate_legacy(&saved).await.unwrap();
            let listener_rev_term = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let cfg_rev_term = test_config(format!(
                "http://{}",
                listener_rev_term.local_addr().unwrap()
            ));
            let (rev_term_res, _) = tokio::join!(
                store.revoke_other_device(&cfg_rev_term, "sid-b"),
                exchange_request(
                    &listener_rev_term,
                    "POST /v1/devices/revoke HTTP/1.1",
                    "401 Unauthorized",
                    &payload,
                )
            );
            match rev_term_res.unwrap() {
                RevokeDeviceResult::CallerInvalid {
                    session: SessionStatus::SignedOut { reason: Some(r) },
                } => assert_eq!(r, expected_reason),
                other => {
                    panic!("Expected terminal CallerInvalid({expected_reason:?}), got {other:?}")
                }
            }
            assert!(backend.get_secret(SERVICE, &store.slot).unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn revoke_other_device_returns_typed_stale_and_missing_outcomes() {
        let dir = TempDir::new().unwrap();
        let backend = Arc::new(MemorySecretBackend::default());
        let saved = SavedDeviceSession {
            session_id: "sid-a".into(),
            credential: "sths_a_secret".into(),
            device_id: "dev-a".into(),
            device_label: "Laptop A".into(),
        };

        // 1. Stale session replaced -> StaleReplaced with refreshed Devices including sid-b2
        let listener_stale = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cfg_stale = test_config(format!("http://{}", listener_stale.local_addr().unwrap()));
        let store =
            SessionStore::open_with_backend(dir.path().to_path_buf(), &cfg_stale, backend.clone())
                .await
                .unwrap();
        store.persist_and_migrate_legacy(&saved).await.unwrap();

        let (stale_res, _) = tokio::join!(store.revoke_other_device(&cfg_stale, "sid-b1"), async {
            exchange_request(
                    &listener_stale,
                    "POST /v1/devices/revoke HTTP/1.1",
                    "409 Conflict",
                    r#"{"code":"stale_device_session","error":"replaced","replacement_session_id":"sid-b2"}"#,
                )
                .await;
            exchange_request(
                    &listener_stale,
                    "GET /v1/devices HTTP/1.1",
                    "200 OK",
                    r#"{"session_id":"sid-a","email":"a@example.invalid","devices":[{"session_id":"sid-a","device_id":"dev-a","device_label":"Laptop A","created_at_ms":1,"last_active_at_ms":2,"expires_at_ms":99,"is_current":true},{"session_id":"sid-b2","device_id":"dev-b","device_label":"Desktop B","created_at_ms":3,"last_active_at_ms":4,"expires_at_ms":99,"is_current":false}]}"#,
                )
                .await;
        });
        match stale_res.unwrap() {
            RevokeDeviceResult::StaleReplaced {
                replacement_session_id,
                session: SessionStatus::SignedIn { devices, .. },
                ..
            } => {
                assert_eq!(replacement_session_id.as_deref(), Some("sid-b2"));
                assert_eq!(devices.len(), 2);
                assert_eq!(devices[1].session_id, "sid-b2");
            }
            other => panic!("Expected StaleReplaced, got {other:?}"),
        }

        // 2. Missing target -> Missing
        let listener_miss = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let cfg_miss = test_config(format!("http://{}", listener_miss.local_addr().unwrap()));
        let (miss_res, _) = tokio::join!(store.revoke_other_device(&cfg_miss, "sid-gone"), async {
            exchange_request(
                &listener_miss,
                "POST /v1/devices/revoke HTTP/1.1",
                "404 Not Found",
                r#"{"code":"device_not_found","error":"not found"}"#,
            )
            .await;
            exchange_request(
                    &listener_miss,
                    "GET /v1/devices HTTP/1.1",
                    "200 OK",
                    r#"{"session_id":"sid-a","email":"a@example.invalid","devices":[{"session_id":"sid-a","device_id":"dev-a","device_label":"Laptop A","created_at_ms":1,"last_active_at_ms":2,"expires_at_ms":99,"is_current":true}]}"#,
                )
                .await;
        });
        assert!(matches!(
            miss_res.unwrap(),
            RevokeDeviceResult::Missing { .. }
        ));
    }
}
