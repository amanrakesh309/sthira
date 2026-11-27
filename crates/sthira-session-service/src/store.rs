use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    path::Path,
    sync::{Arc, Mutex},
};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_CHALLENGE_TTL_MS: i64 = 5 * 60 * 1000;
pub const DEFAULT_SESSION_TTL_MS: i64 = 30 * 24 * 60 * 60 * 1000;
const MAX_LABEL_LEN: usize = 96;
const MAX_DEVICE_ID_LEN: usize = 128;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Invalid request parameters: {0}")]
    InvalidInput(String),
    #[error("Enrollment challenge was not found.")]
    ChallengeNotFound,
    #[error("Enrollment challenge belongs to another device.")]
    ChallengeDeviceMismatch,
    #[error("Enrollment challenge already used.")]
    ChallengeReplayed,
    #[error("Enrollment challenge expired. Start sign-in again.")]
    ChallengeExpired,
    #[error("Enrollment challenge proof did not match.")]
    ChallengeNonceMismatch,
    #[error(
        "This device was signed out while sign-in was in progress. Sign in again to re-enroll."
    )]
    RevokedDuringEnrollment,
    #[error("This device session is invalid.")]
    SessionInvalid,
    #[error("This device was signed out.")]
    SessionRevoked,
    #[error("This session expired. Sign in again with Google.")]
    SessionExpired,
    #[error("Device session was not found.")]
    DeviceNotFound,
    #[error("This device signed in again with a newer session.")]
    StaleTargetReplaced {
        replacement_session_id: String,
        device_label: String,
    },
    #[error("You do not have permission to manage that device.")]
    Forbidden,
    #[error("Session store unavailable.")]
    Storage(#[from] rusqlite::Error),
    #[error("Random generator unavailable.")]
    Random,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ChallengeResponse {
    pub challenge_id: String,
    pub nonce: String,
    pub expires_at_ms: i64,
}

impl fmt::Debug for ChallengeResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChallengeResponse")
            .field("challenge_id", &self.challenge_id)
            .field("nonce", &"[redacted]")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

#[derive(Clone)]
pub struct ChallengeRecord {
    pub challenge_id: String,
    pub device_id: String,
    pub nonce_hash: String,
    pub issued_seq: i64,
    pub expires_at_ms: i64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EnrolledSession {
    pub session_id: String,
    pub credential: String,
    pub uid: String,
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    pub device_id: String,
    pub device_label: String,
    pub created_at_ms: i64,
    pub last_active_at_ms: i64,
    pub expires_at_ms: i64,
}

impl fmt::Debug for EnrolledSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EnrolledSession")
            .field("session_id", &self.session_id)
            .field("credential", &"[redacted]")
            .field("uid", &self.uid)
            .field("email", &"[redacted]")
            .field("display_name", &self.display_name)
            .field("device_id", &self.device_id)
            .field("device_label", &self.device_label)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VerifiedSession {
    pub session_id: String,
    pub uid: String,
    pub email: String,
    #[serde(default)]
    pub display_name: String,
    pub device_id: String,
    pub device_label: String,
    pub created_at_ms: i64,
    pub last_active_at_ms: i64,
    pub expires_at_ms: i64,
}

impl fmt::Debug for VerifiedSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedSession")
            .field("session_id", &self.session_id)
            .field("uid", &self.uid)
            .field("email", &"[redacted]")
            .field("display_name", &self.display_name)
            .field("device_id", &self.device_id)
            .field("device_label", &self.device_label)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSummary {
    pub session_id: String,
    pub device_id: String,
    pub device_label: String,
    pub created_at_ms: i64,
    pub last_active_at_ms: i64,
    pub expires_at_ms: i64,
    pub is_current: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeOutcome {
    pub revoked: bool,
    pub already_revoked: bool,
    pub session_id: String,
    pub revoked_at_ms: i64,
}

/// Server-owned session-store contract independent of workspace content or vault SQLite files.
pub trait AuthSessionStore: Send + Sync {
    fn issue_challenge(
        &self,
        device_id: &str,
        device_label: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<ChallengeResponse, StoreError>;

    fn inspect_challenge(
        &self,
        challenge_id: &str,
        device_id: &str,
        now_ms: i64,
    ) -> Result<ChallengeRecord, StoreError>;

    #[allow(clippy::too_many_arguments)]
    fn enroll_session(
        &self,
        challenge_id: &str,
        device_id: &str,
        device_label: &str,
        uid: &str,
        email: &str,
        nonce_sha256_hex: &str,
        now_ms: i64,
        session_ttl_ms: i64,
    ) -> Result<EnrolledSession, StoreError> {
        let display_name = crate::verify::resolve_display_name(None, None, email);
        self.enroll_session_with_display_name(
            challenge_id,
            device_id,
            device_label,
            uid,
            email,
            &display_name,
            nonce_sha256_hex,
            now_ms,
            session_ttl_ms,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn enroll_session_with_display_name(
        &self,
        challenge_id: &str,
        device_id: &str,
        device_label: &str,
        uid: &str,
        email: &str,
        display_name: &str,
        nonce_sha256_hex: &str,
        now_ms: i64,
        session_ttl_ms: i64,
    ) -> Result<EnrolledSession, StoreError>;

    fn verify_session(&self, credential: &str, now_ms: i64) -> Result<VerifiedSession, StoreError>;

    /// Re-verifies a specific device session by its stored identity, without accepting a
    /// credential. Delegated agent connections bind to the session that issued them; every
    /// request re-checks that binding so issuing-device revocation or replacement invalidates
    /// the connection.
    fn verify_bound_session(
        &self,
        session_id: &str,
        uid: &str,
        device_id: &str,
        now_ms: i64,
    ) -> Result<VerifiedSession, StoreError>;

    fn list_devices(
        &self,
        caller_credential: &str,
        now_ms: i64,
    ) -> Result<(VerifiedSession, Vec<DeviceSummary>), StoreError>;

    fn revoke_device(
        &self,
        caller_credential: &str,
        target_session_id: &str,
        now_ms: i64,
    ) -> Result<RevokeOutcome, StoreError>;

    fn sign_out_current(
        &self,
        caller_credential: &str,
        now_ms: i64,
    ) -> Result<RevokeOutcome, StoreError>;
}

#[derive(Clone)]
pub struct SqliteAuthSessionStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteAuthSessionStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        Self::init(conn)
    }

    fn init(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS auth_meta (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                seq INTEGER NOT NULL
            );
            INSERT OR IGNORE INTO auth_meta (id, seq) VALUES (1, 0);

            CREATE TABLE IF NOT EXISTS auth_challenges (
                challenge_id TEXT PRIMARY KEY,
                nonce_hash TEXT NOT NULL UNIQUE,
                device_id TEXT NOT NULL,
                device_label TEXT NOT NULL,
                issued_seq INTEGER NOT NULL,
                issued_at_ms INTEGER NOT NULL,
                expires_at_ms INTEGER NOT NULL,
                consumed_at_ms INTEGER
            );

            CREATE TABLE IF NOT EXISTS device_revocations (
                uid TEXT NOT NULL,
                device_id TEXT NOT NULL,
                revoked_seq INTEGER NOT NULL,
                revoked_at_ms INTEGER NOT NULL,
                PRIMARY KEY (uid, device_id)
            );

            CREATE TABLE IF NOT EXISTS auth_sessions (
                session_id TEXT PRIMARY KEY,
                credential_hash TEXT NOT NULL UNIQUE,
                uid TEXT NOT NULL,
                email TEXT NOT NULL,
                display_name TEXT NOT NULL DEFAULT '',
                device_id TEXT NOT NULL,
                device_label TEXT NOT NULL,
                created_at_ms INTEGER NOT NULL,
                last_active_at_ms INTEGER NOT NULL,
                expires_at_ms INTEGER NOT NULL,
                revoked_at_ms INTEGER,
                revoked_by_session_id TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_auth_sessions_uid_active
                ON auth_sessions (uid, revoked_at_ms, last_active_at_ms DESC);
            CREATE INDEX IF NOT EXISTS idx_auth_sessions_uid_device
                ON auth_sessions (uid, device_id);
            ",
        )?;
        let _ = conn.execute(
            "ALTER TABLE auth_sessions ADD COLUMN display_name TEXT NOT NULL DEFAULT ''",
            [],
        );
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn lock_conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().expect("auth session store lock poisoned")
    }
}

impl AuthSessionStore for SqliteAuthSessionStore {
    fn issue_challenge(
        &self,
        device_id: &str,
        device_label: &str,
        now_ms: i64,
        ttl_ms: i64,
    ) -> Result<ChallengeResponse, StoreError> {
        let device_id = sanitize_device_id(device_id)?;
        let device_label = sanitize_device_label(device_label)?;
        if ttl_ms <= 0 {
            return Err(StoreError::InvalidInput(
                "Challenge TTL must be positive.".into(),
            ));
        }
        let challenge_id = Uuid::new_v4().to_string();
        let nonce = random_secret("sthc_")?;
        let nonce_hash = sha256_hex(&nonce);
        let expires_at_ms = now_ms.saturating_add(ttl_ms);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let issued_seq = next_seq(&tx)?;
        tx.execute(
            "INSERT INTO auth_challenges
                (challenge_id, nonce_hash, device_id, device_label, issued_seq, issued_at_ms, expires_at_ms, consumed_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            params![
                challenge_id,
                nonce_hash,
                device_id,
                device_label,
                issued_seq,
                now_ms,
                expires_at_ms,
            ],
        )?;
        tx.commit()?;
        Ok(ChallengeResponse {
            challenge_id,
            nonce,
            expires_at_ms,
        })
    }

    fn inspect_challenge(
        &self,
        challenge_id: &str,
        device_id: &str,
        now_ms: i64,
    ) -> Result<ChallengeRecord, StoreError> {
        let device_id = sanitize_device_id(device_id)?;
        let conn = self.lock_conn();
        let row: Option<(String, String, i64, i64, Option<i64>)> = conn
            .query_row(
                "SELECT device_id, nonce_hash, issued_seq, expires_at_ms, consumed_at_ms
                 FROM auth_challenges WHERE challenge_id = ?1",
                params![challenge_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        let Some((row_device_id, nonce_hash, issued_seq, expires_at_ms, consumed_at_ms)) = row
        else {
            return Err(StoreError::ChallengeNotFound);
        };
        if row_device_id != device_id {
            return Err(StoreError::ChallengeDeviceMismatch);
        }
        if consumed_at_ms.is_some() {
            return Err(StoreError::ChallengeReplayed);
        }
        if now_ms > expires_at_ms {
            return Err(StoreError::ChallengeExpired);
        }
        Ok(ChallengeRecord {
            challenge_id: challenge_id.to_owned(),
            device_id: row_device_id,
            nonce_hash,
            issued_seq,
            expires_at_ms,
        })
    }

    fn enroll_session_with_display_name(
        &self,
        challenge_id: &str,
        device_id: &str,
        device_label: &str,
        uid: &str,
        email: &str,
        display_name: &str,
        nonce_sha256_hex: &str,
        now_ms: i64,
        session_ttl_ms: i64,
    ) -> Result<EnrolledSession, StoreError> {
        let device_id = sanitize_device_id(device_id)?;
        let device_label = sanitize_device_label(device_label)?;
        let uid = uid.trim();
        let email = email.trim();
        let resolved_display_name =
            crate::verify::resolve_display_name(Some(display_name), None, email);
        if uid.is_empty() || email.is_empty() || session_ttl_ms <= 0 {
            return Err(StoreError::InvalidInput(
                "Invalid verified identity.".into(),
            ));
        }

        let session_id = Uuid::new_v4().to_string();
        let credential = random_secret("sths_")?;
        let credential_hash = sha256_hex(&credential);
        let expires_at_ms = now_ms.saturating_add(session_ttl_ms);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row: Option<(String, String, i64, i64, Option<i64>)> = tx
            .query_row(
                "SELECT device_id, nonce_hash, issued_seq, expires_at_ms, consumed_at_ms
                 FROM auth_challenges WHERE challenge_id = ?1",
                params![challenge_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;
        let Some((row_device_id, expected_nonce_hash, issued_seq, challenge_expires, consumed)) =
            row
        else {
            return Err(StoreError::ChallengeNotFound);
        };
        if row_device_id != device_id {
            return Err(StoreError::ChallengeDeviceMismatch);
        }
        if consumed.is_some() {
            return Err(StoreError::ChallengeReplayed);
        }
        if now_ms > challenge_expires {
            return Err(StoreError::ChallengeExpired);
        }
        if !ct_eq_str(&expected_nonce_hash, nonce_sha256_hex) {
            return Err(StoreError::ChallengeNonceMismatch);
        }

        // Consume the single-use challenge inside the atomic enrollment transaction.
        let updated = tx.execute(
            "UPDATE auth_challenges SET consumed_at_ms = ?2
             WHERE challenge_id = ?1 AND consumed_at_ms IS NULL",
            params![challenge_id, now_ms],
        )?;
        if updated != 1 {
            return Err(StoreError::ChallengeReplayed);
        }

        // Reject if this device was revoked after the challenge was issued so an in-flight
        // enrollment cannot silently undo a concurrent or overlapping revocation.
        let revoked_seq: Option<i64> = tx
            .query_row(
                "SELECT revoked_seq FROM device_revocations WHERE uid = ?1 AND device_id = ?2",
                params![uid, device_id],
                |r| r.get(0),
            )
            .optional()?;
        if revoked_seq.is_some_and(|seq| seq > issued_seq) {
            tx.commit()?;
            return Err(StoreError::RevokedDuringEnrollment);
        }

        // Supersede any older active session belonging to the same installation on this account.
        tx.execute(
            "UPDATE auth_sessions
             SET revoked_at_ms = ?3, revoked_by_session_id = ?4
             WHERE uid = ?1 AND device_id = ?2 AND revoked_at_ms IS NULL",
            params![uid, device_id, now_ms, session_id],
        )?;

        tx.execute(
            "INSERT INTO auth_sessions
                (session_id, credential_hash, uid, email, display_name, device_id, device_label, created_at_ms, last_active_at_ms, expires_at_ms, revoked_at_ms, revoked_by_session_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, NULL, NULL)",
            params![
                session_id,
                credential_hash,
                uid,
                email,
                resolved_display_name,
                device_id,
                device_label,
                now_ms,
                expires_at_ms,
            ],
        )?;
        tx.commit()?;

        Ok(EnrolledSession {
            session_id,
            credential,
            uid: uid.to_owned(),
            email: email.to_owned(),
            display_name: resolved_display_name,
            device_id,
            device_label,
            created_at_ms: now_ms,
            last_active_at_ms: now_ms,
            expires_at_ms,
        })
    }

    fn verify_session(&self, credential: &str, now_ms: i64) -> Result<VerifiedSession, StoreError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let verified = verify_in_tx(&tx, credential, now_ms)?;
        tx.commit()?;
        Ok(verified)
    }

    fn verify_bound_session(
        &self,
        session_id: &str,
        uid: &str,
        device_id: &str,
        now_ms: i64,
    ) -> Result<VerifiedSession, StoreError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let verified = verify_bound_in_tx(&tx, session_id, uid, device_id, now_ms)?;
        tx.commit()?;
        Ok(verified)
    }

    fn list_devices(
        &self,
        caller_credential: &str,
        now_ms: i64,
    ) -> Result<(VerifiedSession, Vec<DeviceSummary>), StoreError> {
        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let caller = verify_in_tx(&tx, caller_credential, now_ms)?;

        let mut stmt = tx.prepare(
            "SELECT session_id, device_id, device_label, created_at_ms, last_active_at_ms, expires_at_ms
             FROM auth_sessions
             WHERE uid = ?1 AND revoked_at_ms IS NULL AND expires_at_ms > ?2
             ORDER BY (session_id = ?3) DESC, last_active_at_ms DESC, created_at_ms DESC",
        )?;
        let rows = stmt
            .query_map(params![caller.uid, now_ms, caller.session_id], |r| {
                let session_id: String = r.get(0)?;
                Ok(DeviceSummary {
                    is_current: session_id == caller.session_id,
                    session_id,
                    device_id: r.get(1)?,
                    device_label: r.get(2)?,
                    created_at_ms: r.get(3)?,
                    last_active_at_ms: r.get(4)?,
                    expires_at_ms: r.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);
        tx.commit()?;
        Ok((caller, rows))
    }

    fn revoke_device(
        &self,
        caller_credential: &str,
        target_session_id: &str,
        now_ms: i64,
    ) -> Result<RevokeOutcome, StoreError> {
        let target_session_id = target_session_id.trim();
        if target_session_id.is_empty() {
            return Err(StoreError::InvalidInput(
                "Missing target session ID.".into(),
            ));
        }

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let caller = verify_in_tx(&tx, caller_credential, now_ms)?;

        let target: Option<(String, String, Option<i64>)> = tx
            .query_row(
                "SELECT uid, device_id, revoked_at_ms FROM auth_sessions WHERE session_id = ?1",
                params![target_session_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((target_uid, target_device_id, existing_revoked_at)) = target else {
            return Err(StoreError::DeviceNotFound);
        };
        if target_uid != caller.uid {
            return Err(StoreError::Forbidden);
        }

        // Check inside the same transaction whether a newer active session on this
        // device replaced the stale target session before committing or reporting success.
        let replacement: Option<(String, String)> = tx
            .query_row(
                "SELECT session_id, device_label FROM auth_sessions
                 WHERE uid = ?1 AND device_id = ?2 AND session_id != ?3
                   AND revoked_at_ms IS NULL AND expires_at_ms > ?4
                 ORDER BY created_at_ms DESC, rowid DESC
                 LIMIT 1",
                params![caller.uid, target_device_id, target_session_id, now_ms],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((replacement_session_id, device_label)) = replacement {
            return Err(StoreError::StaleTargetReplaced {
                replacement_session_id,
                device_label,
            });
        }

        if let Some(revoked_at_ms) = existing_revoked_at {
            tx.commit()?;
            return Ok(RevokeOutcome {
                revoked: true,
                already_revoked: true,
                session_id: target_session_id.to_owned(),
                revoked_at_ms,
            });
        }

        let revoked_seq = next_seq(&tx)?;
        tx.execute(
            "UPDATE auth_sessions
             SET revoked_at_ms = ?2, revoked_by_session_id = ?3
             WHERE session_id = ?1 AND revoked_at_ms IS NULL",
            params![target_session_id, now_ms, caller.session_id],
        )?;
        tx.execute(
            "INSERT INTO device_revocations (uid, device_id, revoked_seq, revoked_at_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uid, device_id) DO UPDATE SET
                revoked_seq = excluded.revoked_seq,
                revoked_at_ms = excluded.revoked_at_ms",
            params![caller.uid, target_device_id, revoked_seq, now_ms],
        )?;
        tx.commit()?;
        Ok(RevokeOutcome {
            revoked: true,
            already_revoked: false,
            session_id: target_session_id.to_owned(),
            revoked_at_ms: now_ms,
        })
    }

    fn sign_out_current(
        &self,
        caller_credential: &str,
        now_ms: i64,
    ) -> Result<RevokeOutcome, StoreError> {
        let credential = caller_credential.trim();
        if credential.is_empty() {
            return Err(StoreError::SessionInvalid);
        }
        let credential_hash = sha256_hex(credential);

        let mut conn = self.lock_conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let row: Option<(String, String, String, Option<i64>)> = tx
            .query_row(
                "SELECT session_id, uid, device_id, revoked_at_ms
                 FROM auth_sessions WHERE credential_hash = ?1",
                params![credential_hash],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .optional()?;
        let Some((session_id, uid, device_id, existing_revoked)) = row else {
            return Err(StoreError::SessionInvalid);
        };
        if let Some(revoked_at_ms) = existing_revoked {
            tx.commit()?;
            return Ok(RevokeOutcome {
                revoked: true,
                already_revoked: true,
                session_id,
                revoked_at_ms,
            });
        }
        let revoked_seq = next_seq(&tx)?;
        tx.execute(
            "UPDATE auth_sessions
             SET revoked_at_ms = ?2, revoked_by_session_id = ?1
             WHERE session_id = ?1",
            params![session_id, now_ms],
        )?;
        tx.execute(
            "INSERT INTO device_revocations (uid, device_id, revoked_seq, revoked_at_ms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(uid, device_id) DO UPDATE SET
                revoked_seq = excluded.revoked_seq,
                revoked_at_ms = excluded.revoked_at_ms",
            params![uid, device_id, revoked_seq, now_ms],
        )?;
        tx.commit()?;
        Ok(RevokeOutcome {
            revoked: true,
            already_revoked: false,
            session_id,
            revoked_at_ms: now_ms,
        })
    }
}

struct LoadedSessionRow {
    session_id: String,
    stored_hash: String,
    uid: String,
    email: String,
    stored_display_name: String,
    device_id: String,
    device_label: String,
    created_at_ms: i64,
    prior_last_active_ms: i64,
    expires_at_ms: i64,
    revoked_at_ms: Option<i64>,
}

fn verify_in_tx(
    tx: &rusqlite::Transaction<'_>,
    credential: &str,
    now_ms: i64,
) -> Result<VerifiedSession, StoreError> {
    let credential = credential.trim();
    if credential.is_empty() {
        return Err(StoreError::SessionInvalid);
    }
    let credential_hash = sha256_hex(credential);
    let row = tx
        .query_row(
            "SELECT session_id, credential_hash, uid, email, display_name, device_id, device_label,
                    created_at_ms, last_active_at_ms, expires_at_ms, revoked_at_ms
             FROM auth_sessions WHERE credential_hash = ?1",
            params![credential_hash],
            |r| {
                Ok(LoadedSessionRow {
                    session_id: r.get(0)?,
                    stored_hash: r.get(1)?,
                    uid: r.get(2)?,
                    email: r.get(3)?,
                    stored_display_name: r.get(4)?,
                    device_id: r.get(5)?,
                    device_label: r.get(6)?,
                    created_at_ms: r.get(7)?,
                    prior_last_active_ms: r.get(8)?,
                    expires_at_ms: r.get(9)?,
                    revoked_at_ms: r.get(10)?,
                })
            },
        )
        .optional()?;
    let Some(LoadedSessionRow {
        session_id,
        stored_hash,
        uid,
        email,
        stored_display_name,
        device_id,
        device_label,
        created_at_ms,
        prior_last_active_ms,
        expires_at_ms,
        revoked_at_ms,
    }) = row
    else {
        return Err(StoreError::SessionInvalid);
    };
    if !ct_eq_str(&stored_hash, &credential_hash) {
        return Err(StoreError::SessionInvalid);
    }
    if revoked_at_ms.is_some() {
        return Err(StoreError::SessionRevoked);
    }
    if now_ms >= expires_at_ms {
        return Err(StoreError::SessionExpired);
    }

    let refreshed_last_active_ms = prior_last_active_ms.max(now_ms);
    if refreshed_last_active_ms > prior_last_active_ms {
        tx.execute(
            "UPDATE auth_sessions SET last_active_at_ms = ?2 WHERE session_id = ?1",
            params![session_id, refreshed_last_active_ms],
        )?;
    }

    let display_name =
        crate::verify::resolve_display_name(Some(&stored_display_name), None, &email);

    Ok(VerifiedSession {
        session_id,
        uid,
        email,
        display_name,
        device_id,
        device_label,
        created_at_ms,
        last_active_at_ms: refreshed_last_active_ms,
        expires_at_ms,
    })
}

fn verify_bound_in_tx(
    tx: &rusqlite::Transaction<'_>,
    session_id: &str,
    uid: &str,
    device_id: &str,
    now_ms: i64,
) -> Result<VerifiedSession, StoreError> {
    if session_id.trim().is_empty() || uid.trim().is_empty() || device_id.trim().is_empty() {
        return Err(StoreError::SessionInvalid);
    }
    let row = tx
        .query_row(
            "SELECT session_id, credential_hash, uid, email, display_name, device_id, device_label,
                    created_at_ms, last_active_at_ms, expires_at_ms, revoked_at_ms
             FROM auth_sessions WHERE session_id = ?1",
            params![session_id],
            |r| {
                Ok(LoadedSessionRow {
                    session_id: r.get(0)?,
                    stored_hash: r.get(1)?,
                    uid: r.get(2)?,
                    email: r.get(3)?,
                    stored_display_name: r.get(4)?,
                    device_id: r.get(5)?,
                    device_label: r.get(6)?,
                    created_at_ms: r.get(7)?,
                    prior_last_active_ms: r.get(8)?,
                    expires_at_ms: r.get(9)?,
                    revoked_at_ms: r.get(10)?,
                })
            },
        )
        .optional()?;
    let Some(LoadedSessionRow {
        session_id,
        stored_hash: _,
        uid: stored_uid,
        email,
        stored_display_name,
        device_id: stored_device_id,
        device_label,
        created_at_ms,
        prior_last_active_ms,
        expires_at_ms,
        revoked_at_ms,
    }) = row
    else {
        return Err(StoreError::SessionInvalid);
    };
    if stored_uid != uid || stored_device_id != device_id {
        return Err(StoreError::Forbidden);
    }
    if revoked_at_ms.is_some() {
        return Err(StoreError::SessionRevoked);
    }
    if now_ms >= expires_at_ms {
        return Err(StoreError::SessionExpired);
    }
    let refreshed_last_active_ms = prior_last_active_ms.max(now_ms);
    if refreshed_last_active_ms > prior_last_active_ms {
        tx.execute(
            "UPDATE auth_sessions SET last_active_at_ms = ?2 WHERE session_id = ?1",
            params![session_id, refreshed_last_active_ms],
        )?;
    }
    let display_name =
        crate::verify::resolve_display_name(Some(&stored_display_name), None, &email);
    Ok(VerifiedSession {
        session_id,
        uid: stored_uid,
        email,
        display_name,
        device_id: stored_device_id,
        device_label,
        created_at_ms,
        last_active_at_ms: refreshed_last_active_ms,
        expires_at_ms,
    })
}

fn next_seq(tx: &rusqlite::Transaction<'_>) -> Result<i64, StoreError> {
    let seq: i64 = tx.query_row(
        "UPDATE auth_meta SET seq = seq + 1 WHERE id = 1 RETURNING seq",
        [],
        |r| r.get(0),
    )?;
    Ok(seq)
}

pub fn sha256_hex(input: &str) -> String {
    format!("{:x}", Sha256::digest(input.as_bytes()))
}

pub fn ct_eq_str(left: &str, right: &str) -> bool {
    left.as_bytes().ct_eq(right.as_bytes()).into()
}

fn random_secret(prefix: &str) -> Result<String, StoreError> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|_| StoreError::Random)?;
    Ok(format!("{prefix}{}", URL_SAFE_NO_PAD.encode(bytes)))
}

fn sanitize_device_id(value: &str) -> Result<String, StoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_DEVICE_ID_LEN
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return Err(StoreError::InvalidInput("Invalid device ID.".into()));
    }
    Ok(trimmed.to_owned())
}

fn sanitize_device_label(value: &str) -> Result<String, StoreError> {
    let clean: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if clean.is_empty() {
        return Err(StoreError::InvalidInput("Device label is required.".into()));
    }
    Ok(clean.chars().take(MAX_LABEL_LEN).collect())
}
