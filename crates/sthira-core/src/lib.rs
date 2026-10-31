pub mod db;
pub mod discovery;
pub mod engines;
pub mod epistemic;
pub mod history;
pub mod materiality;
pub mod memory;
pub mod models;
pub mod reversibility;
pub mod router;
pub mod vault;
pub mod llm;
pub mod search;

pub use engines::{
    execute_adaptive_stream, generate_simulated_token_stream, AdaptiveStreamParser, RouteOutcome,
    WebResearchFinding, WebResearchResult,
};
pub use search::{
    build_search_provider, scraper, ScrapedWebDocument, SearchProvider, SearchProviderEntry,
    SearchResultItem,
};
pub use models::objects::{
    ObjectInstance, ObjectItem, ObjectTemplate, ObjectType, PropertyDefinition, PropertyType,
};
pub use models::scheduled_topics::{
    format_hhmm, is_topic_due, iso_weekday_of, most_recent_scheduled_instant,
    next_scheduled_instant, parse_hhmm, ScheduledTopic, ScheduledTopicRun, TopicCadence, TopicType,
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
pub use models::router::{
    OperationalMode, PlanProposedAction, ProviderEntry, ProviderModelEntry, StructuredPlanPreview,
};
pub use router::{Approval, IntentType, RouterPlan};
pub use memory::{
    BriefClaim, ClaimRendering, HomeState, PaneHintReason, ProjectThreadSummary, ResumeBrief,
    ResumePaneHint,
};
pub use history::{Attribution, Milestone, MilestoneType, NarrativeSegment, ProjectHistory};
pub use discovery::{AiProposalRecord, ContradictionAnalysis, DiscoveryCandidate};
pub use llm::{
    ComparisonRowPayload, DecisionPointPayload, OllamaProvider, OpenAiCompatibleProvider,
    PlanPhasePayload, ProgressiveStreamChunk, TaskTier, TieredModelProvider,
};

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
            let now_ms = clock.now_ms();
            let _ = db::providers_repo::clean_duplicate_models(&writer);
            let _ = db::providers_repo::seed_default_providers_if_empty(&writer, now_ms);
            let _ = db::objects_repo::seed_default_object_types_if_empty(&writer, now_ms);
            let _ = db::search_repo::seed_default_search_providers_if_empty(&writer, now_ms);
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

    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
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

    pub fn get_project(&self, id: &str) -> Result<Project, CoreError> {
        let reader = self.db.reader()?;
        reader
            .query_row(
                "SELECT id, name, origin_artifact_id, status, created_at, last_substantive_at
                 FROM projects WHERE id = ?1;",
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

    pub fn create_artifact(
        &self,
        artifact_type: ArtifactType,
        authorship: Authorship,
        title: Option<&str>,
        content_md: &str,
        source_url: Option<&str>,
    ) -> Result<Artifact, CoreError> {
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

        let writer = self.db.writer();
        writer.execute(
            "INSERT INTO artifacts (id, type, authorship, title, content_md, source_url, content_hash, version, file_path, last_synced_hash, is_unfinished_draft, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?7, ?9, ?10, ?10);",
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
        writer.execute(
            "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, created_at)
             VALUES (?1, ?2, 1, ?3, ?4, 'user', ?5);",
            params![rev_id, artifact_id, normalized_body, content_hash, now_ms],
        )?;

        let artifact = Artifact {
            id: artifact_id.clone(),
            artifact_type,
            authorship,
            title: title.map(|s| s.to_string()),
            content_md: normalized_body.clone(),
            source_url: source_url.map(|s| s.to_string()),
            content_hash,
            version: 1,
            file_path: Some(file_path),
            last_synced_hash: None,
            is_unfinished_draft: is_unfinished,
            deleted_at: None,
            created_at: now_ms,
            updated_at: now_ms,
        };

        // Write file to vault disk
        let full_path = self.vault_dir.join(artifact.file_path.as_ref().unwrap());
        let rendered = vault::render_file_with_frontmatter(&artifact);
        let _ = std::fs::write(&full_path, rendered);

        self.log_event_tx(
            &writer,
            None,
            Some(&artifact_id),
            Actor::User,
            EventType::ArtifactCreated,
            Materiality::Substantive,
            serde_json::json!({ "title": title }),
            None,
            now_ms,
        )?;

        Ok(artifact)
    }

    pub fn get_artifact(&self, id: &str) -> Result<Artifact, CoreError> {
        let reader = self.db.reader()?;
        reader
            .query_row(
                "SELECT id, type, authorship, title, content_md, source_url, content_hash, version, file_path, last_synced_hash, is_unfinished_draft, deleted_at, created_at, updated_at
                 FROM artifacts WHERE id = ?1;",
                params![id],
                |r| {
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
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    CoreError::NotFound(format!("Artifact {id} not found"))
                }
                other => CoreError::Db(other),
            })
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

    pub fn update_artifact(
        &self,
        artifact_id: &str,
        new_content_md: &str,
        actor: Actor,
        context_project_id: Option<&str>,
    ) -> Result<Artifact, CoreError> {
        let now_ms = self.clock.now_ms();
        let normalized = vault::normalize_content(new_content_md);
        let new_hash = vault::compute_content_hash(&normalized);

        let writer = self.db.writer();
        let (current_version, _old_hash, old_content): (i64, String, String) = writer.query_row(
            "SELECT version, content_hash, content_md FROM artifacts WHERE id = ?1;",
            params![artifact_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

        let materiality = materiality::compute_materiality(&old_content, &normalized);

        let is_unfinished = epistemic::is_mid_sentence(&normalized);
        let draft_val = if is_unfinished { 1 } else { 0 };

        let new_version = current_version + 1;
        writer.execute(
            "UPDATE artifacts
             SET content_md = ?1, content_hash = ?2, version = ?3, is_unfinished_draft = ?4, updated_at = ?5
             WHERE id = ?6;",
            params![normalized, new_hash, new_version, draft_val, now_ms, artifact_id],
        )?;

        let actor_str = match actor {
            Actor::User => "user",
            Actor::Ai => "ai",
            Actor::DiskExternal => "disk_external",
            Actor::System => "system",
        };

        let rev_id = uuid::Uuid::new_v4().to_string();
        writer.execute(
            "INSERT INTO artifact_revisions (id, artifact_id, version, content_md, content_hash, actor, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7);",
            params![rev_id, artifact_id, new_version, normalized, new_hash, actor_str, now_ms],
        )?;

        // If updated inside a context project, bump only that project's last_substantive_at
        if let Some(pid) = context_project_id {
            if materiality == Materiality::Substantive {
                writer.execute(
                    "UPDATE projects SET last_substantive_at = ?1 WHERE id = ?2;",
                    params![now_ms, pid],
                )?;
            }
        }

        self.log_event_tx(
            &writer,
            context_project_id,
            Some(artifact_id),
            actor,
            EventType::ArtifactUpdated,
            materiality,
            serde_json::json!({ "version": new_version }),
            None,
            now_ms,
        )?;

        drop(writer);
        self.get_artifact(artifact_id)
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
            "UPDATE artifacts SET is_unfinished_draft = ?1, updated_at = ?2 WHERE id = ?3;",
            params![val, now_ms, artifact_id],
        )?;
        Ok(())
    }

    pub fn create_canvas(
        &self,
        project_id: Option<&str>,
        name: &str,
    ) -> Result<String, CoreError> {
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
        reversibility::apply_change_set(&self.db, id, actor, proposal_id, summary, inverse_payload, now_ms)
    }

    pub fn revert(&self, target_cs_id: &str) -> Result<String, CoreError> {
        let now_ms = self.clock.now_ms();
        reversibility::revert_change_set(&self.db, target_cs_id, now_ms, |tx, reverting_cs_id, timestamp| {
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
        })
    }

    pub fn revert_change_set(&self, target_cs_id: &str) -> Result<String, CoreError> {
        self.revert(target_cs_id)
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

    fn resolve_session_id(&self, conn: &rusqlite::Connection, now_ms: i64) -> Result<String, CoreError> {
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
}
