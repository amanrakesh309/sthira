pub mod conversation;
pub mod db;
pub mod discovery;
pub mod engines;
pub mod epistemic;
pub mod history;
pub mod llm;
pub mod materiality;
pub mod memory;
pub mod models;
pub mod naming;
pub mod references;
pub mod resume;
pub mod reversibility;
pub mod router;
pub mod search;
pub mod vault;

pub use engines::{
    execute_adaptive_stream, generate_simulated_token_stream, AdaptiveStreamParser,
    RefineArtifactOutcome, RouteOutcome, WebResearchFinding, WebResearchResult,
};
pub use models::objects::{
    ObjectInstance, ObjectItem, ObjectTemplate, ObjectType, PropertyDefinition, PropertyType,
};
pub use models::scheduled_topics::{
    format_hhmm, is_topic_due, iso_weekday_of, most_recent_scheduled_instant,
    next_scheduled_instant, parse_hhmm, ScheduledTopic, ScheduledTopicRun, TopicCadence, TopicType,
};
pub use models::synthesis_templates::SynthesisTemplate;
pub use search::{
    build_search_provider, scraper, ScrapedWebDocument, SearchError, SearchErrorKind,
    SearchProvider, SearchProviderEntry, SearchResultItem,
};

/// Converts epoch milliseconds to an ISO-8601 `YYYY-MM-DD` civil date (UTC).
///
/// Implemented inline (Howard Hinnant's `civil_from_days`) so the core keeps a
/// dependency-free, fully deterministic date stamp shared by digest titles and the
/// research pipeline's temporal grounding instruction.
pub fn format_date_utc(now_ms: i64) -> String {
    let days = now_ms.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
pub use discovery::{AiProposalRecord, ContradictionAnalysis, DiscoveryCandidate};
pub use history::{Attribution, Milestone, MilestoneType, NarrativeSegment, ProjectHistory};
pub use llm::{
    ComparisonRowPayload, DecisionPointPayload, OllamaProvider, OpenAiCompatibleProvider,
    PlanPhasePayload, ProgressiveStreamChunk, TaskTier, TieredModelProvider,
};
pub use memory::{
    BriefClaim, ClaimRendering, HomeState, PaneHintReason, ProjectThreadSummary, ResumeBrief,
    ResumePaneHint,
};
pub use models::router::{
    OperationalMode, PlanProposedAction, ProviderEntry, ProviderModelEntry, StructuredPlanPreview,
};
pub use router::{Approval, IntentType, RouterPlan};

use rusqlite::params;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const IDLE_GAP_MS: i64 = 30 * 60 * 1000; // 30 minutes

pub trait Clock: Send + Sync {
    fn now_ms(&self) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Locality {
    Local,
    Cloud,
}

#[async_trait::async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> &str;
    fn locality(&self) -> Locality;
    async fn generate_json(
        &self,
        schema: &serde_json::Value,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, CoreError>;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, CoreError>;
}

#[non_exhaustive]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreEvent {
    ArtifactChanged { id: String },
    ExtractionFinished { id: String },
    ProposalsReady { count: u32 },
    ChangeSetApplied { id: String },
    ChangeSetReverted { id: String },
    ExternalEditDetected { id: String },
}

#[derive(thiserror::Error, Debug)]
pub enum CoreError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Quote verification error: {0}")]
    QuoteVerification(String),
    #[error("Approval required: {0}")]
    ApprovalRequired(String),
    #[error("Model error: {0}")]
    Model(String),
    #[error("Request error: {0}")]
    Request(#[from] crate::llm::lifecycle::RequestErrorKind),
    #[error("Search error: {0}")]
    ExternalSearch(#[from] crate::search::SearchError),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invariant violation: {0}")]
    InvariantViolation(String),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Note,
    Source,
    Snippet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authorship {
    User,
    AiGenerated,
    External,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Artifact {
    pub id: String,
    pub artifact_type: ArtifactType,
    pub authorship: Authorship,
    pub title: Option<String>,
    pub content_md: String,
    pub source_url: Option<String>,
    pub content_hash: String,
    pub version: i64,
    pub file_path: Option<String>,
    pub last_synced_hash: Option<String>,
    pub is_unfinished_draft: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(default)]
    pub derived_from_artifact_id: Option<String>,
    #[serde(default)]
    pub derivation_type: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SaveArtifactOutcome {
    pub artifact: Artifact,
    pub mirror_synced: bool,
    pub mirror_sync_error: Option<String>,
    pub unchanged: bool,
    pub repaired_mirror_only: bool,
}

#[derive(Debug, Clone)]
pub struct SaveArtifactRequest<'a> {
    pub content_md: &'a str,
    pub title: Option<&'a str>,
    pub actor: Actor,
    pub context_project_id: Option<&'a str>,
    pub authorship: Option<Authorship>,
    pub is_unfinished_draft: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Active,
    Paused,
    Archived,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub origin_artifact_id: Option<String>,
    pub status: ProjectStatus,
    pub created_at: i64,
    pub last_substantive_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasItemKind {
    Artifact,
    Text,
    Frame,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CanvasItem {
    pub canvas_id: String,
    pub item_id: String,
    pub item_kind: CanvasItemKind,
    pub artifact_id: Option<String>,
    pub x: f64,
    pub y: f64,
    pub w: Option<f64>,
    pub h: Option<f64>,
    pub z: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    User,
    Ai,
    DiskExternal,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Materiality {
    None,
    Minor,
    Substantive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    ProjectCreated,
    ArtifactCreated,
    ArtifactUpdated,
    ArtifactViewed,
    ArtifactDeleted,
    ArtifactAttached,
    ArtifactDetached,
    SourceCaptured,
    ExternalEditDetected,
    QuestionRaised,
    QuestionResolved,
    DecisionRecorded,
    DecisionSuperseded,
    ProjectPurposePivoted,
    ArtifactLinked,
    ProposalAccepted,
    ProposalDismissed,
    ChangeSetApplied,
    ChangeSetReverted,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionEvent {
    pub seq: i64,
    pub id: String,
    pub session_id: String,
    pub project_id: Option<String>,
    pub artifact_id: Option<String>,
    pub actor: Actor,
    pub event_type: EventType,
    pub materiality: Materiality,
    pub delta_payload: serde_json::Value,
    pub change_set_id: Option<String>,
    pub occurred_at: i64,
}

#[derive(Clone)]
pub struct Engine {
    db: db::Db,
    vault_dir: PathBuf,
    clock: Arc<dyn Clock>,
    model: Arc<dyn ModelProvider>,
    event_tx: tokio::sync::broadcast::Sender<CoreEvent>,
}

impl Engine {
    pub fn open(
        db_path: impl AsRef<Path>,
        vault_dir: impl AsRef<Path>,
        clock: Arc<dyn Clock>,
        model: Arc<dyn ModelProvider>,
    ) -> Result<Self, CoreError> {
        let db = db::Db::open(db_path)?;
        let vault_dir = vault_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&vault_dir)?;
        let (event_tx, _) = tokio::sync::broadcast::channel(256);

        {
            let writer = db.writer();
            let existing_instance: Option<String> = writer
                .query_row(
                    "SELECT value FROM vault_meta WHERE key = 'vault_instance_id';",
                    [],
                    |r| r.get(0),
                )
                .ok();
            if existing_instance.is_none() {
                let instance_id = uuid::Uuid::new_v4().to_string();
                writer.execute(
                    "INSERT OR IGNORE INTO vault_meta (key, value) VALUES ('vault_instance_id', ?1);",
                    params![instance_id],
                )?;
            }
            let now_ms = clock.now_ms();
            let _ = db::providers_repo::clean_duplicate_models(&writer);
            // AI setup is explicit. An empty catalog must stay empty until the user
            // connects a provider; installed runtimes and models are never assumed.
            let _ = db::objects_repo::seed_default_object_types_if_empty(&writer, now_ms);
            let _ = db::search_repo::seed_default_search_providers_if_empty(&writer, now_ms);
            let _ = db::templates_repo::seed_default_synthesis_templates(&writer, now_ms);
        }

        Ok(Self {
            db,
            vault_dir,
            clock,
            model,
            event_tx,
        })
    }

    pub fn db(&self) -> &db::Db {
        &self.db
    }

    pub fn vault_dir(&self) -> &Path {
        &self.vault_dir
    }

    pub fn vault_instance_id(&self) -> Result<String, CoreError> {
        {
            let reader = self.db.reader()?;
            if let Ok(id) = reader.query_row(
                "SELECT value FROM vault_meta WHERE key = 'vault_instance_id';",
                [],
                |r| r.get::<_, String>(0),
            ) {
                return Ok(id);
            }
        }
        let writer = self.db.writer();
        if let Ok(id) = writer.query_row(
            "SELECT value FROM vault_meta WHERE key = 'vault_instance_id';",
            [],
            |r| r.get::<_, String>(0),
        ) {
            return Ok(id);
        }
        let new_id = uuid::Uuid::new_v4().to_string();
        writer.execute(
            "INSERT INTO vault_meta (key, value) VALUES ('vault_instance_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            params![new_id],
        )?;
        Ok(new_id)
    }

    pub fn rotate_vault_instance_id_tx(conn: &rusqlite::Connection) -> Result<String, CoreError> {
        let new_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO vault_meta (key, value) VALUES ('vault_instance_id', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value;",
            params![new_id],
        )?;
        Ok(new_id)
    }

    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// Scope an individual run to its resolved model without changing other runs.
    pub fn with_model(&self, model: Arc<dyn ModelProvider>) -> Self {
        Self {
            model,
            ..self.clone()
        }
    }

    pub fn model(&self) -> &Arc<dyn ModelProvider> {
        &self.model
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<CoreEvent> {
        self.event_tx.subscribe()
    }

    pub fn emit_event(&self, event: CoreEvent) {
        let _ = self.event_tx.send(event);
    }

    pub fn create_project(
        &self,
        name: &str,
        origin_artifact_id: Option<&str>,
    ) -> Result<Project, CoreError> {
        let now_ms = self.clock.now_ms();
        let project_id = uuid::Uuid::new_v4().to_string();

        let writer = self.db.writer();
        writer.execute(
            "INSERT INTO projects (id, name, origin_artifact_id, status, created_at, last_substantive_at)
             VALUES (?1, ?2, ?3, 'active', ?4, ?4);",
            params![project_id, name, origin_artifact_id, now_ms],
        )?;

        let project = Project {
            id: project_id.clone(),
            name: name.to_string(),
            origin_artifact_id: origin_artifact_id.map(|s| s.to_string()),
            status: ProjectStatus::Active,
            created_at: now_ms,
            last_substantive_at: now_ms,
        };

        self.log_event_tx(
            &writer,
            Some(&project_id),
            None,
            Actor::User,
            EventType::ProjectCreated,
            Materiality::Substantive,
            serde_json::json!({ "name": name }),
            None,
            now_ms,
        )?;

        Ok(project)
    }

    pub fn set_project_purpose(
        &self,
        project_id: &str,
        purpose: &str,
    ) -> Result<String, CoreError> {
        let now_ms = self.clock.now_ms();
        let writer = self.db.writer();
        writer.execute(
            "UPDATE project_purposes SET superseded_at = ?1 WHERE project_id = ?2 AND superseded_at IS NULL;",
            params![now_ms, project_id],
        )?;
        let purpose_id = uuid::Uuid::new_v4().to_string();
        writer.execute(
            "INSERT INTO project_purposes (id, project_id, purpose, origin, set_at)
             VALUES (?1, ?2, ?3, 'user_recorded', ?4);",
            params![purpose_id, project_id, purpose, now_ms],
        )?;
        Ok(purpose_id)
    }

    fn map_artifact_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Artifact> {
        let type_str: String = r.get(1)?;
        let auth_str: String = r.get(2)?;
        Ok(Artifact {
            id: r.get(0)?,
            artifact_type: match type_str.as_str() {
                "source" => ArtifactType::Source,
                "snippet" => ArtifactType::Snippet,
                _ => ArtifactType::Note,
            },
            authorship: match auth_str.as_str() {
                "ai_generated" => Authorship::AiGenerated,
                "external" => Authorship::External,
                _ => Authorship::User,
            },
            title: r.get(3)?,
            content_md: r.get(4)?,
            source_url: r.get(5)?,
            content_hash: r.get(6)?,
            version: r.get(7)?,
            file_path: r.get(8)?,
            last_synced_hash: r.get(9)?,
            is_unfinished_draft: r.get::<_, i64>(10)? == 1,
            deleted_at: r.get(11)?,
            created_at: r.get(12)?,
            updated_at: r.get(13)?,
            derived_from_artifact_id: r.get(14)?,
            derivation_type: r.get(15)?,
        })
    }

    fn get_artifact_tx(conn: &rusqlite::Connection, id: &str) -> Result<Artifact, CoreError> {
        conn.query_row(
            "SELECT id, type, authorship, title, content_md, source_url, content_hash, version, file_path, last_synced_hash, is_unfinished_draft, deleted_at, created_at, updated_at, derived_from_artifact_id, derivation_type
             FROM artifacts WHERE id = ?1 AND deleted_at IS NULL;",
            params![id],
            Self::map_artifact_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                CoreError::NotFound(format!("Artifact {id} not found"))
            }
            other => CoreError::Db(other),
        })
    }

    pub fn get_project(&self, id: &str) -> Result<Project, CoreError> {
        let reader = self.db.reader()?;
        reader
            .query_row(
                "SELECT id, name, origin_artifact_id, status, created_at, last_substantive_at
                 FROM projects WHERE id = ?1 AND deleted_at IS NULL;",
                params![id],
                |r| {
                    let status_str: String = r.get(3)?;
                    Ok(Project {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        origin_artifact_id: r.get(2)?,
                        status: match status_str.as_str() {
                            "paused" => ProjectStatus::Paused,
                            "archived" => ProjectStatus::Archived,
                            _ => ProjectStatus::Active,
                        },
                        created_at: r.get(4)?,
                        last_substantive_at: r.get(5)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CoreError::NotFound(format!("Project {id} not found"))
                }
                other => CoreError::Db(other),
            })
    }

    fn create_artifact_inner(
        &self,
        artifact_type: ArtifactType,
        authorship: Authorship,
        title: Option<&str>,
        content_md: &str,
        source_url: Option<&str>,
        target_project_id: Option<&str>,
    ) -> Result<(Artifact, Option<String>), CoreError> {
        let now_ms = self.clock.now_ms();
        let artifact_id = uuid::Uuid::new_v4().to_string();
        let normalized_body = vault::normalize_content(content_md);
        let content_hash = vault::compute_content_hash(&normalized_body);
        let file_path = format!("{}.md", artifact_id);

        let type_str = match artifact_type {
            ArtifactType::Note => "note",
            ArtifactType::Source => "source",
            ArtifactType::Snippet => "snippet",
        };
        let auth_str = match authorship {
            Authorship::User => "user",
            Authorship::AiGenerated => "ai_generated",
            Authorship::External => "external",
        };

        let is_unfinished = epistemic::is_mid_sentence(&normalized_body);
        let draft_val = if is_unfinished { 1 } else { 0 };

        let mut writer = self.db.writer();
        let tx = writer.transaction()?;

        if let Some(pid) = target_project_id {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL);",
                params![pid],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(CoreError::NotFound(format!("Project {pid} not found")));
            }
        }

        tx.execute(
            "INSERT INTO artifacts (id, type, authorship, title, content_md, source_url, content_hash, version, file_path, last_synced_hash, is_unfinished_draft, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, NULL, ?9, ?10, ?10);",
            params![
                artifact_id,
                type_str,
                auth_str,
                title,
                normalized_body,
                source_url,
                content_hash,
                file_path,
                draft_val,
                now_ms,
            ],
        )?;

        let rev_id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, created_at)
             VALUES (?1, ?2, 1, ?3, ?4, 'user', ?5);",
            params![rev_id, artifact_id, normalized_body, content_hash, now_ms],
        )?;

        self.log_event_tx(
            &tx,
            target_project_id,
            Some(&artifact_id),
            Actor::User,
            EventType::ArtifactCreated,
            Materiality::Substantive,
            serde_json::json!({ "title": title }),
            None,
            now_ms,
        )?;

        if let Some(pid) = target_project_id {
            tx.execute(
                "INSERT INTO project_artifacts (project_id, artifact_id, added_at, link_origin, proposal_id)
                 VALUES (?1, ?2, ?3, 'user_explicit', NULL)
                 ON CONFLICT(project_id, artifact_id) DO UPDATE SET detached_at = NULL;",
                params![pid, artifact_id, now_ms],
            )?;
            self.log_event_tx(
                &tx,
                Some(pid),
                Some(&artifact_id),
                Actor::User,
                EventType::ArtifactAttached,
                Materiality::Substantive,
                serde_json::json!({ "link_origin": "user_explicit" }),
                None,
                now_ms,
            )?;
        }

        tx.commit()?;
        drop(writer);

        let mut artifact = Artifact {
            id: artifact_id.clone(),
            artifact_type,
            authorship,
            title: title.map(|s| s.to_string()),
            content_md: normalized_body,
            source_url: source_url.map(|s| s.to_string()),
            content_hash: content_hash.clone(),
            version: 1,
            file_path: Some(file_path),
            last_synced_hash: None,
            is_unfinished_draft: is_unfinished,
            deleted_at: None,
            created_at: now_ms,
            updated_at: now_ms,
            derived_from_artifact_id: None,
            derivation_type: None,
        };

        let mirror_error = match vault::write_artifact_mirror(&self.vault_dir, &artifact) {
            Ok(()) => {
                let writer = self.db.writer();
                match writer.execute(
                    "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2 AND deleted_at IS NULL;",
                    params![content_hash, artifact_id],
                ) {
                    Ok(_) => {
                        artifact.last_synced_hash = Some(content_hash);
                        None
                    }
                    Err(err) => Some(format!("sync metadata update failed: {err}")),
                }
            }
            Err(err) => Some(err.to_string()),
        };

        Ok((artifact, mirror_error))
    }

    pub fn create_artifact(
        &self,
        artifact_type: ArtifactType,
        authorship: Authorship,
        title: Option<&str>,
        content_md: &str,
        source_url: Option<&str>,
    ) -> Result<Artifact, CoreError> {
        let (artifact, _) = self.create_artifact_inner(
            artifact_type,
            authorship,
            title,
            content_md,
            source_url,
            None,
        )?;
        Ok(artifact)
    }

    pub fn create_and_save_artifact(
        &self,
        artifact_type: ArtifactType,
        authorship: Authorship,
        title: Option<&str>,
        content_md: &str,
        source_url: Option<&str>,
        target_project_id: Option<&str>,
    ) -> Result<SaveArtifactOutcome, CoreError> {
        let (artifact, mirror_sync_error) = self.create_artifact_inner(
            artifact_type,
            authorship,
            title,
            content_md,
            source_url,
            target_project_id,
        )?;
        let mirror_synced = mirror_sync_error.is_none();
        Ok(SaveArtifactOutcome {
            artifact,
            mirror_synced,
            mirror_sync_error,
            unchanged: false,
            repaired_mirror_only: false,
        })
    }

    pub fn get_artifact(&self, id: &str) -> Result<Artifact, CoreError> {
        let reader = self.db.reader()?;
        Self::get_artifact_tx(&reader, id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_derived_artifact_with_cancellation(
        &self,
        artifact_type: ArtifactType,
        authorship: Authorship,
        title: Option<&str>,
        body: &str,
        source_url: Option<&str>,
        derived_from_artifact_id: Option<&str>,
        derivation_type: Option<&str>,
        target_project_id: Option<&str>,
        cancel_token: Option<&crate::llm::lifecycle::CancellationToken>,
    ) -> Result<Artifact, CoreError> {
        let mut writer = self.db.writer();

        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(CoreError::Request(
                    crate::llm::lifecycle::RequestErrorKind::Cancelled,
                ));
            }
        }

        let now_ms = self.clock.now_ms();

        if let Some(token) = cancel_token {
            token.try_begin_commit()?;
        }

        let artifact_id = uuid::Uuid::new_v4().to_string();
        let normalized_body = vault::normalize_content(body);
        let content_hash = vault::compute_content_hash(&normalized_body);
        let file_path = format!("{}.md", artifact_id);

        let type_str = match artifact_type {
            ArtifactType::Note => "note",
            ArtifactType::Source => "source",
            ArtifactType::Snippet => "snippet",
        };
        let auth_str = match authorship {
            Authorship::User => "user",
            Authorship::AiGenerated => "ai_generated",
            Authorship::External => "external",
        };

        let is_unfinished = epistemic::is_mid_sentence(&normalized_body);
        let draft_val = if is_unfinished { 1 } else { 0 };

        let tx = writer.transaction()?;

        if let Some(pid) = target_project_id {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL);",
                params![pid],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(CoreError::NotFound(format!("Project {pid} not found")));
            }
        }

        tx.execute(
            "INSERT INTO artifacts (id, type, authorship, title, content_md, source_url, content_hash, version, file_path, last_synced_hash, is_unfinished_draft, derived_from_artifact_id, derivation_type, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, NULL, ?9, ?10, ?11, ?12, ?12);",
            params![
                artifact_id,
                type_str,
                auth_str,
                title,
                normalized_body,
                source_url,
                content_hash,
                file_path,
                draft_val,
                derived_from_artifact_id,
                derivation_type,
                now_ms,
            ],
        )?;

        let rev_id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, created_at)
             VALUES (?1, ?2, 1, ?3, ?4, 'ai', ?5);",
            params![rev_id, artifact_id, normalized_body, content_hash, now_ms],
        )?;

        self.log_event_tx(
            &tx,
            target_project_id,
            Some(&artifact_id),
            Actor::Ai,
            EventType::ArtifactCreated,
            Materiality::Substantive,
            serde_json::json!({
                "title": title,
                "derived_from": derived_from_artifact_id,
                "derivation_type": derivation_type
            }),
            None,
            now_ms,
        )?;

        if let Some(pid) = target_project_id {
            tx.execute(
                "INSERT INTO project_artifacts (project_id, artifact_id, added_at, link_origin, proposal_id)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(project_id, artifact_id) DO UPDATE SET detached_at = NULL;",
                params![pid, artifact_id, now_ms, "user_explicit", Option::<&str>::None],
            )?;

            self.log_event_tx(
                &tx,
                Some(pid),
                Some(&artifact_id),
                Actor::User,
                EventType::ArtifactAttached,
                Materiality::Substantive,
                serde_json::json!({ "link_origin": "user_explicit" }),
                None,
                now_ms,
            )?;
        }

        tx.commit()?;
        drop(writer);

        let mut artifact = Artifact {
            id: artifact_id.clone(),
            artifact_type,
            authorship,
            title: title.map(|s| s.to_string()),
            content_md: normalized_body,
            source_url: source_url.map(|s| s.to_string()),
            content_hash: content_hash.clone(),
            version: 1,
            file_path: Some(file_path),
            last_synced_hash: None,
            is_unfinished_draft: is_unfinished,
            deleted_at: None,
            created_at: now_ms,
            updated_at: now_ms,
            derived_from_artifact_id: derived_from_artifact_id.map(|s| s.to_string()),
            derivation_type: derivation_type.map(|s| s.to_string()),
        };

        // Write file to vault disk only after database commit succeeds
        if vault::write_artifact_mirror(&self.vault_dir, &artifact).is_ok() {
            let writer = self.db.writer();
            if writer
                .execute(
                    "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2 AND deleted_at IS NULL;",
                    params![content_hash, artifact_id],
                )
                .is_ok()
            {
                artifact.last_synced_hash = Some(content_hash);
            }
        }

        if let Some(token) = cancel_token {
            token.finish_commit();
        }

        Ok(artifact)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_derived_artifact(
        &self,
        artifact_type: ArtifactType,
        authorship: Authorship,
        title: Option<&str>,
        body: &str,
        source_url: Option<&str>,
        derived_from_artifact_id: Option<&str>,
        derivation_type: Option<&str>,
        target_project_id: Option<&str>,
    ) -> Result<Artifact, CoreError> {
        self.create_derived_artifact_with_cancellation(
            artifact_type,
            authorship,
            title,
            body,
            source_url,
            derived_from_artifact_id,
            derivation_type,
            target_project_id,
            None,
        )
    }

    pub fn apply_refinement_with_cancellation(
        &self,
        artifact_id: &str,
        new_content_md: &str,
        summary: &str,
        inverse_payload: &serde_json::Value,
        cancel_token: Option<&crate::llm::lifecycle::CancellationToken>,
    ) -> Result<(Artifact, String), CoreError> {
        let mut writer = self.db.writer();

        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(CoreError::Request(
                    crate::llm::lifecycle::RequestErrorKind::Cancelled,
                ));
            }
        }

        let now_ms = self.clock.now_ms();

        if let Some(token) = cancel_token {
            token.try_begin_commit()?;
        }

        let cs_id = uuid::Uuid::new_v4().to_string();
        let actor_str = "ai";

        let tx = writer.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        // 1. Fetch current persisted artifact atomically under the writer lock
        let existing = Self::get_artifact_tx(&tx, artifact_id)?;
        let current_version = existing.version;
        let old_content = existing.content_md.clone();

        let normalized = vault::normalize_content(new_content_md);
        let materiality = materiality::compute_materiality(&old_content, &normalized);
        let new_hash = vault::compute_content_hash(&normalized);
        let is_unfinished = epistemic::is_mid_sentence(&normalized);
        let draft_val = if is_unfinished { 1 } else { 0 };
        let new_version = current_version + 1;
        let auth_str = match existing.authorship {
            Authorship::User => "user",
            Authorship::AiGenerated => "ai_generated",
            Authorship::External => "external",
        };

        // Enrich restore_artifact inverse operation with the authoritative committed
        // revision/version and metadata basis observed inside the writer transaction.
        let mut resolved_inverse = inverse_payload.clone();
        if let Some(op) = resolved_inverse
            .get_mut("operations")
            .and_then(|v| v.as_array_mut())
            .and_then(|ops| ops.first_mut())
            .and_then(|op| op.as_object_mut())
        {
            if op.get("type").and_then(|v| v.as_str()) == Some("restore_artifact") {
                op.insert(
                    "refined_version".to_string(),
                    serde_json::json!(new_version),
                );
                op.insert(
                    "refined_content_md".to_string(),
                    serde_json::json!(normalized),
                );
                op.insert(
                    "refined_content_hash".to_string(),
                    serde_json::json!(new_hash),
                );
                op.insert(
                    "refined_title".to_string(),
                    serde_json::json!(existing.title),
                );
                op.insert(
                    "refined_authorship".to_string(),
                    serde_json::json!(auth_str),
                );
                op.insert(
                    "refined_is_unfinished_draft".to_string(),
                    serde_json::json!(is_unfinished),
                );
            }
        }
        let inverse_str = serde_json::to_string(&resolved_inverse)?;

        // 2. Insert change set for reversibility
        tx.execute(
            "INSERT INTO change_sets (id, actor, proposal_id, summary, inverse_payload, applied_at)
             VALUES (?1, ?2, NULL, ?3, ?4, ?5);",
            params![cs_id, actor_str, summary, inverse_str, now_ms],
        )?;

        tx.execute(
            "UPDATE artifacts
             SET content_md = ?1, content_hash = ?2, version = ?3, is_unfinished_draft = ?4, updated_at = ?5
             WHERE id = ?6 AND deleted_at IS NULL;",
            params![normalized, new_hash, new_version, draft_val, now_ms, artifact_id],
        )?;

        let rev_id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, change_set_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8);",
            params![rev_id, artifact_id, new_version, normalized, new_hash, actor_str, cs_id, now_ms],
        )?;

        let project_ids: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT pa.project_id
                 FROM project_artifacts pa
                 JOIN projects p ON p.id = pa.project_id
                 WHERE pa.artifact_id = ?1 AND pa.detached_at IS NULL AND p.deleted_at IS NULL
                 ORDER BY pa.added_at DESC;",
            )?;
            let rows = stmt.query_map(params![artifact_id], |r| r.get(0))?;
            rows.flatten().collect()
        };

        if materiality == Materiality::Substantive {
            for pid in &project_ids {
                tx.execute(
                    "UPDATE projects SET last_substantive_at = ?1 WHERE id = ?2 AND deleted_at IS NULL;",
                    params![now_ms, pid],
                )?;
            }
        }

        self.log_event_tx(
            &tx,
            project_ids.first().map(|s| s.as_str()),
            Some(artifact_id),
            Actor::Ai,
            EventType::ArtifactUpdated,
            materiality,
            serde_json::json!({ "version": new_version }),
            Some(&cs_id),
            now_ms,
        )?;

        let mut updated_art = Self::get_artifact_tx(&tx, artifact_id)?;

        tx.commit()?;
        drop(writer);

        if vault::write_artifact_mirror(&self.vault_dir, &updated_art).is_ok() {
            let writer = self.db.writer();
            if writer
                .execute(
                    "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2 AND deleted_at IS NULL;",
                    params![new_hash, artifact_id],
                )
                .is_ok()
            {
                updated_art.last_synced_hash = Some(new_hash);
            }
        }

        if let Some(token) = cancel_token {
            token.finish_commit();
        }

        Ok((updated_art, cs_id))
    }

    pub fn attach_artifact(
        &self,
        project_id: &str,
        artifact_id: &str,
        link_origin: &str,
        proposal_id: Option<&str>,
    ) -> Result<(), CoreError> {
        let now_ms = self.clock.now_ms();
        let writer = self.db.writer();

        writer.execute(
            "INSERT INTO project_artifacts (project_id, artifact_id, added_at, link_origin, proposal_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(project_id, artifact_id) DO UPDATE SET detached_at = NULL;",
            params![project_id, artifact_id, now_ms, link_origin, proposal_id],
        )?;

        self.log_event_tx(
            &writer,
            Some(project_id),
            Some(artifact_id),
            Actor::User,
            EventType::ArtifactAttached,
            Materiality::Substantive,
            serde_json::json!({ "link_origin": link_origin }),
            None,
            now_ms,
        )?;

        Ok(())
    }

    pub fn detach_artifact(&self, project_id: &str, artifact_id: &str) -> Result<(), CoreError> {
        let now_ms = self.clock.now_ms();
        let writer = self.db.writer();

        writer.execute(
            "UPDATE project_artifacts SET detached_at = ?1 WHERE project_id = ?2 AND artifact_id = ?3;",
            params![now_ms, project_id, artifact_id],
        )?;

        self.log_event_tx(
            &writer,
            Some(project_id),
            Some(artifact_id),
            Actor::User,
            EventType::ArtifactDetached,
            Materiality::Minor,
            serde_json::json!({ "project_id": project_id, "artifact_id": artifact_id }),
            None,
            now_ms,
        )?;

        Ok(())
    }

    /// Atomically relocates an artifact between projects (or from standalone into a
    /// project) inside a **single SQLite transaction**.
    ///
    /// `from_project_id` is *soft*-detached (`detached_at` is stamped rather than the row
    /// being deleted) so historical context is preserved and the link remains
    /// re-attachable — matching `Engine::attach_artifact`'s
    /// `ON CONFLICT ... DO UPDATE SET detached_at = NULL` contract. `to_project_id` is
    /// then attached.
    ///
    /// Both the `artifact_detached` and `artifact_attached` rows are written to the
    /// append-only `session_events` log **within the same transaction**, so the
    /// provenance chain that resume briefs, project history and reversibility consume
    /// stays gapless. This is why the relocation lives in the engine rather than the
    /// desktop command layer: `log_event_tx` is crate-private and re-implementing the
    /// event log outside `sthira-core` would violate the append-only invariant.
    pub fn move_artifact_to_project(
        &self,
        artifact_id: &str,
        from_project_id: Option<&str>,
        to_project_id: &str,
    ) -> Result<(), CoreError> {
        let now_ms = self.clock.now_ms();
        let mut writer = self.db.writer();
        let tx = writer.transaction()?;

        if let Some(from) = from_project_id {
            tx.execute(
                "UPDATE project_artifacts SET detached_at = ?1
                 WHERE project_id = ?2 AND artifact_id = ?3 AND detached_at IS NULL;",
                params![now_ms, from, artifact_id],
            )?;

            self.log_event_tx(
                &tx,
                Some(from),
                Some(artifact_id),
                Actor::User,
                EventType::ArtifactDetached,
                Materiality::Minor,
                serde_json::json!({
                    "project_id": from,
                    "artifact_id": artifact_id,
                    "reason": "move_to_project",
                }),
                None,
                now_ms,
            )?;
        }

        tx.execute(
            "INSERT INTO project_artifacts (project_id, artifact_id, added_at, link_origin, proposal_id)
             VALUES (?1, ?2, ?3, 'user_explicit', NULL)
             ON CONFLICT(project_id, artifact_id) DO UPDATE SET detached_at = NULL;",
            params![to_project_id, artifact_id, now_ms],
        )?;

        self.log_event_tx(
            &tx,
            Some(to_project_id),
            Some(artifact_id),
            Actor::User,
            EventType::ArtifactAttached,
            Materiality::Substantive,
            serde_json::json!({ "link_origin": "user_explicit", "reason": "move_to_project" }),
            None,
            now_ms,
        )?;

        tx.commit()?;

        Ok(())
    }

    fn apply_artifact_update_tx(
        &self,
        tx: &rusqlite::Connection,
        existing: &Artifact,
        request: &SaveArtifactRequest<'_>,
        now_ms: i64,
    ) -> Result<Artifact, CoreError> {
        let normalized = vault::normalize_content(request.content_md);
        let new_hash = vault::compute_content_hash(&normalized);
        let effective_title = match request.title {
            Some(t) => Some(t.to_string()),
            None => existing.title.clone(),
        };
        let effective_authorship = request.authorship.unwrap_or(existing.authorship);
        let is_unfinished = request
            .is_unfinished_draft
            .unwrap_or_else(|| epistemic::is_mid_sentence(&normalized));
        let draft_val = if is_unfinished { 1 } else { 0 };
        let materiality = materiality::compute_materiality(&existing.content_md, &normalized);
        let new_version = existing.version + 1;

        let active_project_id = if let Some(pid) = request.context_project_id {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL);",
                params![pid],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(CoreError::NotFound(format!("Project {pid} not found")));
            }
            Some(pid)
        } else {
            None
        };

        let auth_str = match effective_authorship {
            Authorship::User => "user",
            Authorship::AiGenerated => "ai_generated",
            Authorship::External => "external",
        };
        let content_changed = normalized != existing.content_md;
        let next_synced_hash =
            if content_changed && existing.last_synced_hash.as_deref() != Some(new_hash.as_str()) {
                existing.last_synced_hash.as_deref()
            } else {
                None
            };

        let updated_rows = tx.execute(
            "UPDATE artifacts
             SET title = ?1, content_md = ?2, content_hash = ?3, authorship = ?4, version = ?5, is_unfinished_draft = ?6, updated_at = ?7, last_synced_hash = ?8
             WHERE id = ?9 AND deleted_at IS NULL;",
            params![
                effective_title,
                normalized,
                new_hash,
                auth_str,
                new_version,
                draft_val,
                now_ms,
                next_synced_hash,
                existing.id,
            ],
        )?;
        if updated_rows == 0 {
            return Err(CoreError::NotFound(format!(
                "Artifact {} not found",
                existing.id
            )));
        }

        let actor_str = match request.actor {
            Actor::User => "user",
            Actor::Ai => "ai",
            Actor::DiskExternal => "disk_external",
            Actor::System => "system",
        };

        let rev_id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![
                rev_id,
                existing.id,
                new_version,
                normalized,
                new_hash,
                actor_str,
                now_ms
            ],
        )?;

        if let Some(pid) = active_project_id {
            if materiality == Materiality::Substantive {
                tx.execute(
                    "UPDATE projects SET last_substantive_at = ?1 WHERE id = ?2 AND deleted_at IS NULL;",
                    params![now_ms, pid],
                )?;
            }
        }

        self.log_event_tx(
            tx,
            active_project_id,
            Some(&existing.id),
            request.actor,
            EventType::ArtifactUpdated,
            materiality,
            serde_json::json!({ "version": new_version, "title": effective_title }),
            None,
            now_ms,
        )?;

        Self::get_artifact_tx(tx, &existing.id)
    }

    /// Transactionally updates an artifact in SQLite without writing `.md` disk mirrors.
    /// Used by core sync/test paths where `reconcile_vault` manages disk export separately.
    pub fn update_artifact(
        &self,
        artifact_id: &str,
        new_content_md: &str,
        actor: Actor,
        context_project_id: Option<&str>,
    ) -> Result<Artifact, CoreError> {
        let now_ms = self.clock.now_ms();
        let mut writer = self.db.writer();
        let tx = writer.transaction()?;
        let existing = Self::get_artifact_tx(&tx, artifact_id)?;
        let req = SaveArtifactRequest {
            content_md: new_content_md,
            title: None,
            actor,
            context_project_id,
            authorship: None,
            is_unfinished_draft: None,
        };
        let updated = self.apply_artifact_update_tx(&tx, &existing, &req, now_ms)?;
        tx.commit()?;
        Ok(updated)
    }

    /// Atomically saves user/model changes in a single SQLite transaction, performs exact
    /// normalized no-op detection, and publishes the `.md` mirror post-commit while returning
    /// honest mirror synchronization status (`SaveArtifactOutcome`).
    pub fn save_artifact(
        &self,
        artifact_id: &str,
        request: SaveArtifactRequest<'_>,
    ) -> Result<SaveArtifactOutcome, CoreError> {
        let now_ms = self.clock.now_ms();
        let normalized = vault::normalize_content(request.content_md);

        let mut writer = self.db.writer();
        let tx = writer.transaction()?;
        let mut existing = Self::get_artifact_tx(&tx, artifact_id)?;

        let effective_title = match request.title {
            Some(t) => Some(t.to_string()),
            None => existing.title.clone(),
        };
        let effective_authorship = request.authorship.unwrap_or(existing.authorship);
        let is_unfinished = request
            .is_unfinished_draft
            .unwrap_or_else(|| epistemic::is_mid_sentence(&normalized));

        let is_unchanged = normalized == existing.content_md
            && effective_title == existing.title
            && effective_authorship == existing.authorship
            && is_unfinished == existing.is_unfinished_draft;

        if is_unchanged {
            // Still validate explicit project context if caller supplied one
            if let Some(pid) = request.context_project_id {
                let exists: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL);",
                    params![pid],
                    |r| r.get(0),
                )?;
                if !exists {
                    return Err(CoreError::NotFound(format!("Project {pid} not found")));
                }
            }
            drop(tx);
            drop(writer);

            if let Some(ref rel_path) = existing.file_path {
                let full_path = self.vault_dir.join(rel_path);
                let needs_repair = !full_path.exists()
                    || existing.last_synced_hash.as_deref() != Some(existing.content_hash.as_str());
                if needs_repair {
                    return match vault::write_artifact_mirror(&self.vault_dir, &existing) {
                        Ok(()) => {
                            let writer = self.db.writer();
                            match writer.execute(
                                "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2 AND deleted_at IS NULL;",
                                params![existing.content_hash, existing.id],
                            ) {
                                Ok(_) => {
                                    existing.last_synced_hash = Some(existing.content_hash.clone());
                                    Ok(SaveArtifactOutcome {
                                        artifact: existing,
                                        mirror_synced: true,
                                        mirror_sync_error: None,
                                        unchanged: true,
                                        repaired_mirror_only: true,
                                    })
                                }
                                Err(err) => Ok(SaveArtifactOutcome {
                                    artifact: existing,
                                    mirror_synced: false,
                                    mirror_sync_error: Some(format!(
                                        "sync metadata update failed: {err}"
                                    )),
                                    unchanged: true,
                                    repaired_mirror_only: false,
                                }),
                            }
                        }
                        Err(err) => Ok(SaveArtifactOutcome {
                            artifact: existing,
                            mirror_synced: false,
                            mirror_sync_error: Some(err.to_string()),
                            unchanged: true,
                            repaired_mirror_only: false,
                        }),
                    };
                }
            }

            return Ok(SaveArtifactOutcome {
                artifact: existing,
                mirror_synced: true,
                mirror_sync_error: None,
                unchanged: true,
                repaired_mirror_only: false,
            });
        }

        let mut updated = self.apply_artifact_update_tx(&tx, &existing, &request, now_ms)?;
        tx.commit()?;
        drop(writer);

        match vault::write_artifact_mirror(&self.vault_dir, &updated) {
            Ok(()) => {
                let writer = self.db.writer();
                match writer.execute(
                    "UPDATE artifacts SET last_synced_hash = ?1 WHERE id = ?2 AND deleted_at IS NULL;",
                    params![updated.content_hash, updated.id],
                ) {
                    Ok(_) => {
                        updated.last_synced_hash = Some(updated.content_hash.clone());
                        Ok(SaveArtifactOutcome {
                            artifact: updated,
                            mirror_synced: true,
                            mirror_sync_error: None,
                            unchanged: false,
                            repaired_mirror_only: false,
                        })
                    }
                    Err(err) => Ok(SaveArtifactOutcome {
                        artifact: updated,
                        mirror_synced: false,
                        mirror_sync_error: Some(format!("sync metadata update failed: {err}")),
                        unchanged: false,
                        repaired_mirror_only: false,
                    }),
                }
            }
            Err(err) => Ok(SaveArtifactOutcome {
                artifact: updated,
                mirror_synced: false,
                mirror_sync_error: Some(err.to_string()),
                unchanged: false,
                repaired_mirror_only: false,
            }),
        }
    }

    pub fn set_artifact_unfinished_draft(
        &self,
        artifact_id: &str,
        is_draft: bool,
    ) -> Result<(), CoreError> {
        let now_ms = self.clock.now_ms();
        let writer = self.db.writer();
        let val = if is_draft { 1 } else { 0 };
        writer.execute(
            "UPDATE artifacts SET is_unfinished_draft = ?1, updated_at = ?2 WHERE id = ?3 AND deleted_at IS NULL;",
            params![val, now_ms, artifact_id],
        )?;
        Ok(())
    }

    pub fn create_canvas(&self, project_id: Option<&str>, name: &str) -> Result<String, CoreError> {
        let now_ms = self.clock.now_ms();
        let canvas_id = uuid::Uuid::new_v4().to_string();
        let writer = self.db.writer();

        writer.execute(
            "INSERT INTO canvases (id, project_id, name, created_at)
             VALUES (?1, ?2, ?3, ?4);",
            params![canvas_id, project_id, name, now_ms],
        )?;

        Ok(canvas_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_canvas_item(
        &self,
        canvas_id: &str,
        item_id: &str,
        item_kind: CanvasItemKind,
        artifact_id: Option<&str>,
        x: f64,
        y: f64,
        w: Option<f64>,
        h: Option<f64>,
        z: i64,
    ) -> Result<(), CoreError> {
        let kind_str = match item_kind {
            CanvasItemKind::Artifact => "artifact",
            CanvasItemKind::Text => "text",
            CanvasItemKind::Frame => "frame",
        };

        let writer = self.db.writer();
        writer.execute(
            "INSERT INTO canvas_items (canvas_id, item_id, item_kind, artifact_id, x, y, w, h, z)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9);",
            params![canvas_id, item_id, kind_str, artifact_id, x, y, w, h, z],
        )?;

        Ok(())
    }

    pub fn move_canvas_item(
        &self,
        canvas_id: &str,
        item_id: &str,
        x: f64,
        y: f64,
    ) -> Result<(), CoreError> {
        let writer = self.db.writer();
        writer.execute(
            "UPDATE canvas_items SET x = ?1, y = ?2 WHERE canvas_id = ?3 AND item_id = ?4;",
            params![x, y, canvas_id, item_id],
        )?;
        Ok(())
    }

    pub fn promote_to_project(
        &self,
        artifact_ids: &[&str],
        project_name: &str,
    ) -> Result<Project, CoreError> {
        let now_ms = self.clock.now_ms();
        let origin_art = artifact_ids.first().copied();
        let project = self.create_project(project_name, origin_art)?;

        let writer = self.db.writer();
        for &art_id in artifact_ids {
            writer.execute(
                "INSERT INTO project_artifacts (project_id, artifact_id, added_at, link_origin)
                 VALUES (?1, ?2, ?3, 'user_explicit')
                 ON CONFLICT(project_id, artifact_id) DO NOTHING;",
                params![project.id, art_id, now_ms],
            )?;
        }

        // Seed initial project_purposes row
        let purpose_id = uuid::Uuid::new_v4().to_string();
        writer.execute(
            "INSERT INTO project_purposes (id, project_id, purpose, origin, set_at)
             VALUES (?1, ?2, ?3, 'user_recorded', ?4);",
            params![purpose_id, project.id, project_name, now_ms],
        )?;

        Ok(project)
    }

    pub fn apply_change_set(
        &self,
        id: &str,
        actor: Actor,
        proposal_id: Option<&str>,
        summary: &str,
        inverse_payload: &serde_json::Value,
    ) -> Result<(), CoreError> {
        let now_ms = self.clock.now_ms();
        reversibility::apply_change_set(
            &self.db,
            id,
            actor,
            proposal_id,
            summary,
            inverse_payload,
            now_ms,
        )
    }

    pub fn revert(&self, target_cs_id: &str) -> Result<String, CoreError> {
        let now_ms = self.clock.now_ms();
        reversibility::revert_change_set(
            &self.db,
            target_cs_id,
            now_ms,
            |tx, reverting_cs_id, timestamp| {
                let event_id = uuid::Uuid::new_v4().to_string();
                let session_id = self.resolve_session_id(tx, timestamp)?;

                tx.execute(
                "INSERT INTO session_events (id, session_id, actor, event_type, materiality, delta_payload, change_set_id, occurred_at)
                 VALUES (?1, ?2, 'system', 'change_set_reverted', 'substantive', ?3, ?4, ?5);",
                params![
                    event_id,
                    session_id,
                    serde_json::json!({ "reverted_change_set_id": target_cs_id }).to_string(),
                    reverting_cs_id,
                    timestamp,
                ],
            )?;
                Ok(())
            },
        )
    }

    pub fn revert_change_set(&self, target_cs_id: &str) -> Result<String, CoreError> {
        self.revert(target_cs_id)
    }

    /// Guarded 1-click undo for a document refinement, refusing if newer saved
    /// revisions superseded the refined revision, committing SQLite atomically
    /// before publishing the `.md` mirror, and reporting honest mirror status.
    pub fn undo_refinement(
        &self,
        artifact_id: &str,
        change_set_id: &str,
    ) -> Result<SaveArtifactOutcome, String> {
        let mut writer = self.db.writer();
        let tx = writer
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(|e| e.to_string())?;

        let current = Self::get_artifact_tx(&tx, artifact_id).map_err(|e| e.to_string())?;

        let (inverse_payload_str, reverted_at): (String, Option<i64>) = tx
            .query_row(
                "SELECT inverse_payload, reverted_at FROM change_sets WHERE id = ?1;",
                params![change_set_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| "This refinement cannot be undone.".to_string())?;

        if reverted_at.is_some() {
            return Err("This refinement has already been undone.".to_string());
        }

        let inverse: serde_json::Value = serde_json::from_str(&inverse_payload_str)
            .map_err(|_| "This refinement cannot be undone.".to_string())?;
        let op = inverse
            .get("operations")
            .and_then(|v| v.as_array())
            .and_then(|ops| ops.first())
            .ok_or_else(|| "This refinement cannot be undone.".to_string())?;

        if op.get("type").and_then(|v| v.as_str()) != Some("restore_artifact")
            || op.get("artifact_id").and_then(|v| v.as_str()) != Some(artifact_id)
        {
            return Err("This refinement cannot be undone.".to_string());
        }

        let original_md = op
            .get("content_md")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "This refinement cannot be undone.".to_string())?;
        let expected_refined_md = op
            .get("refined_content_md")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "This refinement cannot be undone.".to_string())?;
        let expected_refined_hash = op
            .get("refined_content_hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "This refinement cannot be undone.".to_string())?;

        let (committed_version, committed_md, committed_hash): (i64, String, String) = tx
            .query_row(
                "SELECT version, content_md, content_hash
                 FROM artifact_revisions
                 WHERE artifact_id = ?1 AND change_set_id = ?2
                 ORDER BY version DESC LIMIT 1;",
                params![artifact_id, change_set_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| "This refinement cannot be undone.".to_string())?;

        let latest_revision_version: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM artifact_revisions WHERE artifact_id = ?1;",
                params![artifact_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;

        let expected_version_ok = op
            .get("refined_version")
            .and_then(|v| v.as_i64())
            .is_none_or(|v| v == current.version);
        let expected_title_ok = op
            .get("refined_title")
            .is_none_or(|v| v.as_str() == current.title.as_deref());
        let expected_draft_ok = op
            .get("refined_is_unfinished_draft")
            .and_then(|v| v.as_bool())
            .is_none_or(|d| d == current.is_unfinished_draft);
        let current_auth = match current.authorship {
            Authorship::User => "user",
            Authorship::AiGenerated => "ai_generated",
            Authorship::External => "external",
        };
        let expected_auth_ok = op
            .get("refined_authorship")
            .and_then(|v| v.as_str())
            .is_none_or(|a| a == current_auth);

        if current.version != committed_version
            || latest_revision_version != committed_version
            || current.content_md != committed_md
            || current.content_hash != committed_hash
            || current.content_md != expected_refined_md
            || current.content_hash != expected_refined_hash
            || !expected_version_ok
            || !expected_title_ok
            || !expected_draft_ok
            || !expected_auth_ok
        {
            return Err(
                "The document changed after this refinement. Undo would overwrite newer work."
                    .to_string(),
            );
        }

        let now = self.clock.now_ms();
        let undo_cs = uuid::Uuid::new_v4().to_string();
        let restored_md = vault::normalize_content(original_md);
        let restored_hash = vault::compute_content_hash(&restored_md);
        let is_unfinished = epistemic::is_mid_sentence(&restored_md);
        let draft_val = if is_unfinished { 1 } else { 0 };
        let new_version = current.version + 1;

        // Keep `last_synced_hash` NOT equal to `restored_hash` inside SQLite until
        // post-commit `.md` mirror publication succeeds.
        let content_changed = restored_md != current.content_md;
        let next_synced_hash = if content_changed
            && current.last_synced_hash.as_deref() != Some(restored_hash.as_str())
        {
            current.last_synced_hash.as_deref()
        } else {
            None
        };

        tx.execute(
            "INSERT INTO change_sets (id, actor, summary, inverse_payload, applied_at)
             VALUES (?1, 'system', 'Undid refinement', ?2, ?3);",
            params![
                undo_cs,
                serde_json::json!({ "reverted_change_set_id": change_set_id }).to_string(),
                now
            ],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "UPDATE change_sets SET reverted_at = ?1, reverted_by = ?2 WHERE id = ?3;",
            params![now, undo_cs, change_set_id],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "UPDATE artifacts
             SET content_md = ?1, content_hash = ?2, last_synced_hash = ?3, version = ?4, updated_at = ?5, is_unfinished_draft = ?6
             WHERE id = ?7 AND deleted_at IS NULL;",
            params![
                restored_md,
                restored_hash,
                next_synced_hash,
                new_version,
                now,
                draft_val,
                artifact_id
            ],
        )
        .map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, change_set_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'system', ?6, ?7);",
            params![
                uuid::Uuid::new_v4().to_string(),
                artifact_id,
                new_version,
                restored_md,
                restored_hash,
                undo_cs,
                now
            ],
        )
        .map_err(|e| e.to_string())?;

        let project_id: Option<String> = tx
            .query_row(
                "SELECT pa.project_id
                 FROM project_artifacts pa
                 JOIN projects p ON p.id = pa.project_id
                 WHERE pa.artifact_id = ?1 AND pa.detached_at IS NULL AND p.deleted_at IS NULL
                 ORDER BY pa.added_at DESC LIMIT 1;",
                params![artifact_id],
                |r| r.get(0),
            )
            .ok();

        self.log_event_tx(
            &tx,
            project_id.as_deref(),
            Some(artifact_id),
            Actor::System,
            EventType::ChangeSetReverted,
            Materiality::Substantive,
            serde_json::json!({ "reverted_change_set_id": change_set_id }),
            Some(&undo_cs),
            now,
        )
        .map_err(|e| e.to_string())?;

        let mut restored = Self::get_artifact_tx(&tx, artifact_id).map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;
        drop(writer);

        let outcome = if let Some(ref _path) = restored.file_path {
            let still_current = self
                .get_artifact(artifact_id)
                .map(|latest| latest.version == restored.version)
                .unwrap_or(false);
            if !still_current {
                SaveArtifactOutcome {
                    artifact: restored,
                    mirror_synced: false,
                    mirror_sync_error: Some(
                        "superseded by a newer revision before mirror write".to_string(),
                    ),
                    unchanged: false,
                    repaired_mirror_only: false,
                }
            } else {
                match vault::write_artifact_mirror(&self.vault_dir, &restored) {
                    Ok(()) => {
                        let writer = self.db.writer();
                        match writer.execute(
                            "UPDATE artifacts
                             SET last_synced_hash = ?1
                             WHERE id = ?2 AND deleted_at IS NULL AND version = ?3 AND content_hash = ?1;",
                            params![restored.content_hash, restored.id, restored.version],
                        ) {
                            Ok(rows) if rows > 0 => {
                                restored.last_synced_hash = Some(restored.content_hash.clone());
                                SaveArtifactOutcome {
                                    artifact: restored,
                                    mirror_synced: true,
                                    mirror_sync_error: None,
                                    unchanged: false,
                                    repaired_mirror_only: false,
                                }
                            }
                            Ok(_) => SaveArtifactOutcome {
                                artifact: restored,
                                mirror_synced: false,
                                mirror_sync_error: Some(
                                    "superseded before sync metadata update".to_string(),
                                ),
                                unchanged: false,
                                repaired_mirror_only: false,
                            },
                            Err(err) => SaveArtifactOutcome {
                                artifact: restored,
                                mirror_synced: false,
                                mirror_sync_error: Some(format!(
                                    "sync metadata update failed: {err}"
                                )),
                                unchanged: false,
                                repaired_mirror_only: false,
                            },
                        }
                    }
                    Err(err) => SaveArtifactOutcome {
                        artifact: restored,
                        mirror_synced: false,
                        mirror_sync_error: Some(err.to_string()),
                        unchanged: false,
                        repaired_mirror_only: false,
                    },
                }
            }
        } else {
            SaveArtifactOutcome {
                artifact: restored,
                mirror_synced: true,
                mirror_sync_error: None,
                unchanged: false,
                repaired_mirror_only: false,
            }
        };

        self.emit_event(CoreEvent::ArtifactChanged {
            id: artifact_id.to_string(),
        });
        self.emit_event(CoreEvent::ChangeSetReverted { id: undo_cs });
        Ok(outcome)
    }

    pub fn reconcile_vault(&self) -> Result<vault::ReconcileResult, CoreError> {
        let now_ms = self.clock.now_ms();
        vault::reconcile_vault(
            &self.db,
            &self.vault_dir,
            now_ms,
            |conn, art_id, proj_id, actor, evt, mat, delta, timestamp| {
                self.log_event_tx(
                    conn,
                    proj_id,
                    Some(art_id),
                    actor,
                    evt,
                    mat,
                    delta,
                    None,
                    timestamp,
                )?;
                Ok(())
            },
        )
    }

    pub fn log_event(
        &self,
        project_id: Option<&str>,
        artifact_id: Option<&str>,
        actor: Actor,
        event_type: EventType,
        materiality: Materiality,
        delta_payload: serde_json::Value,
    ) -> Result<SessionEvent, CoreError> {
        let now_ms = self.clock.now_ms();
        let writer = self.db.writer();
        self.log_event_tx(
            &writer,
            project_id,
            artifact_id,
            actor,
            event_type,
            materiality,
            delta_payload,
            None,
            now_ms,
        )
    }

    pub async fn extract_epistemic_nodes(
        &self,
        artifact_id: &str,
    ) -> Result<Vec<String>, CoreError> {
        epistemic::extractor::extract_epistemic_nodes(self, artifact_id).await
    }

    pub fn get_project_history(&self, project_id: &str) -> Result<ProjectHistory, CoreError> {
        history::get_project_history(self, project_id)
    }

    pub async fn detect_purpose_pivot(
        &self,
        project_id: &str,
    ) -> Result<Option<String>, CoreError> {
        history::detect_purpose_pivot(self, project_id).await
    }

    pub async fn run_discovery(
        &self,
        project_id: &str,
        artifact_id: &str,
    ) -> Result<Vec<String>, CoreError> {
        discovery::run_discovery(self, project_id, artifact_id).await
    }

    pub fn list_dismissed_proposals(&self) -> Result<Vec<discovery::AiProposalRecord>, CoreError> {
        discovery::list_dismissed_proposals(self)
    }

    pub fn undismiss_proposal(&self, id: &str) -> Result<(), CoreError> {
        discovery::undismiss_proposal(self, id)
    }

    pub fn apply_discovery_link(&self, proposal_id: &str) -> Result<String, CoreError> {
        discovery::apply_discovery_link(self, proposal_id)
    }

    pub fn store_custom_embedding(
        &self,
        artifact_id: &str,
        content_hash: &str,
        embedding: &[f32],
    ) -> Result<(), CoreError> {
        discovery::store_custom_embedding(
            &self.db,
            artifact_id,
            self.model.id(),
            content_hash,
            embedding,
            self.clock.now_ms(),
        )
    }

    fn resolve_session_id(
        &self,
        conn: &rusqlite::Connection,
        now_ms: i64,
    ) -> Result<String, CoreError> {
        let latest_session: Option<(String, i64, Option<i64>)> = conn
            .query_row(
                "SELECT id, last_event_at, ended_at FROM sessions ORDER BY started_at DESC LIMIT 1;",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();

        let session_id = match latest_session {
            Some((id, last_event_at, ended_at)) => {
                if ended_at.is_some() || (now_ms - last_event_at) > IDLE_GAP_MS {
                    if ended_at.is_none() {
                        conn.execute(
                            "UPDATE sessions SET ended_at = ?1, end_reason = 'idle_gap' WHERE id = ?2;",
                            params![last_event_at, id],
                        )?;
                    }
                    let new_id = uuid::Uuid::new_v4().to_string();
                    conn.execute(
                        "INSERT INTO sessions (id, started_at, last_event_at) VALUES (?1, ?2, ?2);",
                        params![new_id, now_ms],
                    )?;
                    new_id
                } else {
                    conn.execute(
                        "UPDATE sessions SET last_event_at = ?1 WHERE id = ?2;",
                        params![now_ms, id],
                    )?;
                    id
                }
            }
            None => {
                let new_id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO sessions (id, started_at, last_event_at) VALUES (?1, ?2, ?2);",
                    params![new_id, now_ms],
                )?;
                new_id
            }
        };

        Ok(session_id)
    }

    #[allow(clippy::too_many_arguments)]
    fn log_event_tx(
        &self,
        conn: &rusqlite::Connection,
        project_id: Option<&str>,
        artifact_id: Option<&str>,
        actor: Actor,
        event_type: EventType,
        materiality: Materiality,
        delta_payload: serde_json::Value,
        change_set_id: Option<&str>,
        now_ms: i64,
    ) -> Result<SessionEvent, CoreError> {
        let session_id = self.resolve_session_id(conn, now_ms)?;
        let event_id = uuid::Uuid::new_v4().to_string();

        let actor_str = match actor {
            Actor::User => "user",
            Actor::Ai => "ai",
            Actor::DiskExternal => "disk_external",
            Actor::System => "system",
        };
        let event_str = match event_type {
            EventType::ProjectCreated => "project_created",
            EventType::ArtifactCreated => "artifact_created",
            EventType::ArtifactUpdated => "artifact_updated",
            EventType::ArtifactViewed => "artifact_viewed",
            EventType::ArtifactDeleted => "artifact_deleted",
            EventType::ArtifactAttached => "artifact_attached",
            EventType::ArtifactDetached => "artifact_detached",
            EventType::SourceCaptured => "source_captured",
            EventType::ExternalEditDetected => "external_edit_detected",
            EventType::QuestionRaised => "question_raised",
            EventType::QuestionResolved => "question_resolved",
            EventType::DecisionRecorded => "decision_recorded",
            EventType::DecisionSuperseded => "decision_superseded",
            EventType::ProjectPurposePivoted => "project_purpose_pivoted",
            EventType::ArtifactLinked => "artifact_linked",
            EventType::ProposalAccepted => "proposal_accepted",
            EventType::ProposalDismissed => "proposal_dismissed",
            EventType::ChangeSetApplied => "change_set_applied",
            EventType::ChangeSetReverted => "change_set_reverted",
        };
        let mat_str = match materiality {
            Materiality::None => "none",
            Materiality::Minor => "minor",
            Materiality::Substantive => "substantive",
        };

        conn.execute(
            "INSERT INTO session_events (id, session_id, project_id, artifact_id, actor, event_type, materiality, delta_payload, change_set_id, occurred_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);",
            params![
                event_id,
                session_id,
                project_id,
                artifact_id,
                actor_str,
                event_str,
                mat_str,
                delta_payload.to_string(),
                change_set_id,
                now_ms,
            ],
        )?;

        let seq = conn.last_insert_rowid();

        Ok(SessionEvent {
            seq,
            id: event_id,
            session_id,
            project_id: project_id.map(|s| s.to_string()),
            artifact_id: artifact_id.map(|s| s.to_string()),
            actor,
            event_type,
            materiality,
            delta_payload,
            change_set_id: change_set_id.map(|s| s.to_string()),
            occurred_at: now_ms,
        })
    }

    pub fn list_synthesis_templates(
        &self,
        target_mode: Option<&str>,
    ) -> Result<Vec<SynthesisTemplate>, CoreError> {
        let reader = self.db.reader()?;
        db::templates_repo::list_synthesis_templates(&reader, target_mode)
    }

    pub fn get_synthesis_template(&self, id: &str) -> Result<Option<SynthesisTemplate>, CoreError> {
        let reader = self.db.reader()?;
        db::templates_repo::get_synthesis_template(&reader, id)
    }

    pub fn get_default_synthesis_template(
        &self,
        target_mode: &str,
    ) -> Result<Option<SynthesisTemplate>, CoreError> {
        let reader = self.db.reader()?;
        db::templates_repo::get_default_synthesis_template(&reader, target_mode)
    }
}
