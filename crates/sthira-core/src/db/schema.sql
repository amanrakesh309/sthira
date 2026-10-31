-- Canonical SQLite WAL + FTS5 + Triggers DDL
-- user_version = 1

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    started_at INTEGER NOT NULL,
    last_event_at INTEGER NOT NULL,
    ended_at INTEGER,
    end_reason TEXT CHECK(end_reason IN ('idle_gap', 'explicit', 'app_close'))
);

CREATE TABLE IF NOT EXISTS ai_proposals (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK(kind IN (
        'attach_to_project', 'artifact_link', 'resolve_question', 'supersede_decision',
        'purpose_pivot', 'canvas_reorg', 'merge_notes',
        'prior_knowledge', 'contradiction', 'answers_open_question'
    )),
    subject_key TEXT NOT NULL, -- Canonical order-independent key, e.g., 'art_a|art_b'
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    evidence TEXT NOT NULL CHECK(json_valid(evidence)), -- Node IDs + verified quotes
    model_id TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN ('pending', 'accepted', 'dismissed', 'expired')),
    created_at INTEGER NOT NULL,
    decided_at INTEGER,
    CHECK ((state = 'pending') = (decided_at IS NULL))
);

-- Dismissed, accepted, or pending proposals for the same (kind, subject_key) can NEVER be re-proposed automatically
CREATE UNIQUE INDEX IF NOT EXISTS ux_proposal_live
    ON ai_proposals(kind, subject_key) WHERE state <> 'expired';

CREATE TABLE IF NOT EXISTS change_sets (
    id TEXT PRIMARY KEY,
    actor TEXT NOT NULL CHECK(actor IN ('user', 'ai', 'system')),
    proposal_id TEXT REFERENCES ai_proposals(id) ON DELETE RESTRICT,
    summary TEXT NOT NULL,
    inverse_payload TEXT NOT NULL CHECK(json_valid(inverse_payload)),
    applied_at INTEGER NOT NULL,
    reverted_at INTEGER,
    reverted_by TEXT REFERENCES change_sets(id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS artifacts (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL CHECK(type IN ('note', 'source', 'snippet')),
    authorship TEXT NOT NULL CHECK(authorship IN ('user', 'ai_generated', 'external')),
    title TEXT,
    content_md TEXT NOT NULL,
    source_url TEXT,
    content_hash TEXT NOT NULL,         -- sha256(NFC, LF-normalized body; YAML/TOML frontmatter excluded)
    version INTEGER NOT NULL DEFAULT 1, -- Optimistic concurrency counter
    file_path TEXT UNIQUE,              -- Relative to .sthira_vault/artifacts/
    last_synced_hash TEXT,              -- Hash at last clean disk<->db sync
    is_unfinished_draft INTEGER NOT NULL DEFAULT 0 CHECK(is_unfinished_draft IN (0, 1)),
    is_pinned INTEGER NOT NULL DEFAULT 0,
    last_accessed_at INTEGER NOT NULL DEFAULT 0,
    deleted_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (type <> 'source' OR authorship = 'external')
);

CREATE VIRTUAL TABLE IF NOT EXISTS artifacts_fts USING fts5(
    artifact_id UNINDEXED,
    title,
    content_md,
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS artifacts_fts_insert AFTER INSERT ON artifacts
BEGIN
    INSERT INTO artifacts_fts(artifact_id, title, content_md)
    VALUES (NEW.id, NEW.title, NEW.content_md);
END;

CREATE TRIGGER IF NOT EXISTS artifacts_fts_update AFTER UPDATE ON artifacts
BEGIN
    DELETE FROM artifacts_fts WHERE artifact_id = OLD.id;
    INSERT INTO artifacts_fts(artifact_id, title, content_md)
    VALUES (NEW.id, NEW.title, NEW.content_md);
END;

CREATE TRIGGER IF NOT EXISTS artifacts_fts_delete AFTER DELETE ON artifacts
BEGIN
    DELETE FROM artifacts_fts WHERE artifact_id = OLD.id;
END;

CREATE TABLE IF NOT EXISTS artifact_revisions (
    id TEXT PRIMARY KEY,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    version INTEGER NOT NULL,
    content_md TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    actor TEXT NOT NULL CHECK(actor IN ('user', 'ai', 'disk_external', 'system')),
    change_set_id TEXT REFERENCES change_sets(id) ON DELETE RESTRICT,
    created_at INTEGER NOT NULL,
    UNIQUE(artifact_id, version)
);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    origin_artifact_id TEXT REFERENCES artifacts(id) ON DELETE RESTRICT, -- Set when promoted from a loose note
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'paused', 'archived')),
    is_pinned INTEGER NOT NULL DEFAULT 0,
    last_accessed_at INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    last_substantive_at INTEGER NOT NULL -- Updated ONLY on substantive work, NEVER on views or minor typo edits
);

CREATE TABLE IF NOT EXISTS project_purposes (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    purpose TEXT NOT NULL,
    origin TEXT NOT NULL CHECK(origin IN ('user_recorded', 'ai_inferred')),
    set_at INTEGER NOT NULL,
    superseded_at INTEGER,
    change_set_id TEXT REFERENCES change_sets(id) ON DELETE RESTRICT
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_current_purpose
    ON project_purposes(project_id) WHERE superseded_at IS NULL;

CREATE TABLE IF NOT EXISTS project_artifacts (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    added_at INTEGER NOT NULL,
    detached_at INTEGER, -- Soft detach preserves historical context
    link_origin TEXT NOT NULL CHECK(link_origin IN ('user_explicit', 'ai_suggested_approved')),
    proposal_id TEXT REFERENCES ai_proposals(id) ON DELETE RESTRICT,
    PRIMARY KEY (project_id, artifact_id),
    CHECK (link_origin = 'user_explicit' OR proposal_id IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS artifact_links (
    source_artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    target_artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    relationship_type TEXT NOT NULL CHECK(relationship_type IN ('supports', 'contradicts', 'extends', 'references')),
    origin TEXT NOT NULL CHECK(origin IN ('user_recorded', 'ai_inferred')),
    proposal_id TEXT REFERENCES ai_proposals(id) ON DELETE RESTRICT,
    approved_at INTEGER,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (source_artifact_id, target_artifact_id, relationship_type),
    CHECK (source_artifact_id <> target_artifact_id),
    CHECK (origin = 'user_recorded' OR (proposal_id IS NOT NULL AND approved_at IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS canvases (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS canvas_items (
    canvas_id TEXT NOT NULL REFERENCES canvases(id) ON DELETE RESTRICT,
    item_id TEXT NOT NULL,
    item_kind TEXT NOT NULL CHECK(item_kind IN ('artifact', 'text', 'frame')),
    artifact_id TEXT REFERENCES artifacts(id) ON DELETE RESTRICT,
    x REAL NOT NULL,
    y REAL NOT NULL,
    w REAL,
    h REAL,
    z INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (canvas_id, item_id),
    CHECK ((item_kind = 'artifact') = (artifact_id IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS canvas_layout_revisions (
    id TEXT PRIMARY KEY,
    canvas_id TEXT NOT NULL REFERENCES canvases(id) ON DELETE RESTRICT,
    layout TEXT NOT NULL CHECK(json_valid(layout)),
    actor TEXT NOT NULL CHECK(actor IN ('user', 'ai')),
    change_set_id TEXT REFERENCES change_sets(id) ON DELETE RESTRICT,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS session_events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT, -- Strict total order even when millisecond timestamps tie
    id TEXT NOT NULL UNIQUE,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT,
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    artifact_id TEXT REFERENCES artifacts(id) ON DELETE RESTRICT,
    actor TEXT NOT NULL CHECK(actor IN ('user', 'ai', 'disk_external', 'system')),
    event_type TEXT NOT NULL CHECK(event_type IN (
        'project_created', 'artifact_created', 'artifact_updated', 'artifact_viewed', 'artifact_deleted',
        'artifact_attached', 'artifact_detached', 'source_captured', 'external_edit_detected',
        'question_raised', 'question_resolved', 'decision_recorded', 'decision_superseded',
        'project_purpose_pivoted', 'artifact_linked', 'proposal_accepted', 'proposal_dismissed',
        'change_set_applied', 'change_set_reverted'
    )),
    materiality TEXT NOT NULL CHECK(materiality IN ('none', 'minor', 'substantive')),
    delta_payload TEXT NOT NULL CHECK(json_valid(delta_payload)),
    change_set_id TEXT REFERENCES change_sets(id) ON DELETE RESTRICT,
    occurred_at INTEGER NOT NULL
);

CREATE TRIGGER IF NOT EXISTS session_events_no_update BEFORE UPDATE ON session_events
BEGIN
    SELECT RAISE(ABORT, 'session_events is strictly append-only');
END;

CREATE TRIGGER IF NOT EXISTS session_events_no_delete BEFORE DELETE ON session_events
BEGIN
    SELECT RAISE(ABORT, 'session_events is strictly append-only');
END;

CREATE TABLE IF NOT EXISTS epistemic_nodes (
    id TEXT PRIMARY KEY,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    context_project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT, -- Informational context when first extracted
    node_type TEXT NOT NULL CHECK(node_type IN ('question', 'decision', 'finding', 'hypothesis', 'rejected_approach')),
    origin TEXT NOT NULL CHECK(origin IN ('user_recorded', 'source_backed', 'ai_inferred')),
    lifecycle TEXT NOT NULL DEFAULT 'open' CHECK(lifecycle IN ('open', 'resolved', 'superseded', 'stale')),
    review_state TEXT NOT NULL DEFAULT 'unreviewed' CHECK(review_state IN ('unreviewed', 'confirmed', 'dismissed')),
    statement TEXT NOT NULL,
    quote TEXT,
    quote_start INTEGER,
    quote_end INTEGER,
    quote_hash TEXT,
    rationale_quote TEXT,
    basis_content_hash TEXT NOT NULL, -- Artifact content_hash when extracted
    resolved_by_node_id TEXT REFERENCES epistemic_nodes(id) ON DELETE RESTRICT,
    superseded_by_node_id TEXT REFERENCES epistemic_nodes(id) ON DELETE RESTRICT,
    extraction_run_id TEXT NOT NULL,
    model_id TEXT NOT NULL,
    first_seen_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK (origin = 'ai_inferred' OR (quote IS NOT NULL AND quote_start IS NOT NULL AND quote_end > quote_start)),
    CHECK (NOT (node_type = 'decision' AND origin = 'ai_inferred'))
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_node_identity
    ON epistemic_nodes(artifact_id, node_type, quote_hash)
    WHERE quote_hash IS NOT NULL AND lifecycle <> 'stale';

-- Prevent Authorship Laundering at the Database Trigger Level
CREATE TRIGGER IF NOT EXISTS nodes_origin_matches_authorship BEFORE INSERT ON epistemic_nodes
BEGIN
    SELECT RAISE(ABORT, 'epistemic origin violates artifact authorship rules')
    FROM artifacts a
    WHERE a.id = NEW.artifact_id AND (
        (NEW.origin = 'user_recorded' AND a.authorship <> 'user')
        OR (NEW.origin = 'source_backed' AND a.authorship <> 'external')
        OR (a.authorship = 'ai_generated' AND NEW.origin <> 'ai_inferred')
    );
END;

CREATE TABLE IF NOT EXISTS embedding_models (
    id TEXT PRIMARY KEY,
    dim INTEGER NOT NULL,
    cosine_threshold REAL NOT NULL,
    calibrated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    content_hash TEXT NOT NULL,
    ord INTEGER NOT NULL,
    text TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS artifact_embeddings (
    artifact_id TEXT PRIMARY KEY REFERENCES artifacts(id) ON DELETE RESTRICT,
    model_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    embedding BLOB NOT NULL,
    created_at INTEGER NOT NULL
);

-- PERSISTENT WORKING MEMORY SNAPSHOTS (Keyed by basis_fingerprint, zero manual is_stale flags)
CREATE TABLE IF NOT EXISTS derived_briefs (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    level INTEGER NOT NULL CHECK(level IN (1, 2)),
    basis_fingerprint TEXT NOT NULL,
    generator_version TEXT NOT NULL,
    model_id TEXT NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)),
    active_working_breadcrumb TEXT NOT NULL,
    mirror_file_path TEXT NOT NULL, -- Points to .sthira_meta/memory_mirrors/projects/<project_id>.md
    generated_at INTEGER NOT NULL,
    PRIMARY KEY (project_id, level)
);

CREATE TABLE IF NOT EXISTS global_working_memory (
    singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
    basis_fingerprint TEXT NOT NULL,
    payload TEXT NOT NULL CHECK(json_valid(payload)), -- Serialized HomeState
    mirror_file_path TEXT NOT NULL,                   -- Points to .sthira_meta/memory_mirrors/global_working_memory.md
    generated_at INTEGER NOT NULL
);

-- Phase 9: Provider-Centric Model Management
CREATE TABLE IF NOT EXISTS providers (
    id TEXT PRIMARY KEY,                       -- 'ollama', 'groq', 'anthropic', 'openai', 'openrouter', 'custom'
    display_name TEXT NOT NULL,                -- 'Ollama (Local)', 'Groq Cloud', 'Anthropic Claude'
    provider_type TEXT NOT NULL CHECK(provider_type IN ('ollama', 'openai_compatible', 'anthropic', 'openrouter')),
    base_url TEXT NOT NULL,                    -- e.g. 'http://localhost:11434', 'https://api.groq.com/openai/v1'
    api_key TEXT,                              -- NULL for local Ollama; token for cloud providers
    is_enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS provider_models (
    id TEXT PRIMARY KEY,                       -- e.g. 'ollama:llama3.2', 'groq:llama-3.3-70b-versatile'
    provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    model_identifier TEXT NOT NULL,            -- raw tag sent in payload, e.g. 'llama3.2:latest'
    display_name TEXT NOT NULL,                -- 'Llama 3.2 (3B)'
    context_window INTEGER NOT NULL DEFAULT 8192,
    supports_structured_output INTEGER NOT NULL DEFAULT 1,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    UNIQUE(provider_id, model_identifier)
);

CREATE INDEX IF NOT EXISTS idx_provider_models_provider ON provider_models(provider_id);

CREATE UNIQUE INDEX IF NOT EXISTS ux_provider_model_unique 
ON provider_models(provider_id, model_identifier);

-- View to guarantee deduplicated active model listings
CREATE VIEW IF NOT EXISTS v_active_models AS
SELECT 
    pm.id,
    pm.provider_id,
    p.display_name AS provider_name,
    p.provider_type,
    pm.model_identifier,
    pm.display_name AS model_display_name,
    pm.context_window,
    pm.is_default
FROM provider_models pm
JOIN providers p ON pm.provider_id = p.id
WHERE pm.is_enabled = 1 AND p.is_enabled = 1
GROUP BY pm.provider_id, pm.model_identifier;

-- Phase 9.2: Extensible Typed Object System (Anytype-Style Entities)
CREATE TABLE IF NOT EXISTS object_types (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    icon TEXT NOT NULL,
    color TEXT NOT NULL,
    schema_json TEXT NOT NULL CHECK(json_valid(schema_json)),
    is_system INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    -- Phase 16: Type & Template Management System
    description TEXT NOT NULL DEFAULT '',
    default_template_md TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS object_instances (
    artifact_id TEXT PRIMARY KEY REFERENCES artifacts(id) ON DELETE CASCADE,
    object_type_id TEXT NOT NULL REFERENCES object_types(id) ON DELETE RESTRICT,
    properties_json TEXT NOT NULL CHECK(json_valid(properties_json)),
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_object_instances_type ON object_instances(object_type_id);

-- Phase 16: Named Object Templates (Notion/Anytype-style structured presets)
CREATE TABLE IF NOT EXISTS object_templates (
    id TEXT PRIMARY KEY,
    object_type_id TEXT NOT NULL REFERENCES object_types(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    icon TEXT NOT NULL DEFAULT '📄',
    is_default INTEGER NOT NULL DEFAULT 0,
    properties_preset_json JSON NOT NULL DEFAULT '{}',
    body_template_md TEXT NOT NULL DEFAULT '',
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_object_templates_type ON object_templates(object_type_id);

-- Phase 10: Search Provider Settings & Web Cache
CREATE TABLE IF NOT EXISTS search_providers (
    id TEXT PRIMARY KEY,                       -- 'searxng', 'brave', 'tavily', 'duckduckgo'
    display_name TEXT NOT NULL,
    provider_type TEXT NOT NULL CHECK(provider_type IN ('searxng', 'brave', 'tavily', 'duckduckgo_scraper')),
    endpoint_url TEXT NOT NULL,
    api_key TEXT,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    is_default INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

-- Search Query & Scraped Page Cache (prevents duplicate API queries & preserves offline provenance)
CREATE TABLE IF NOT EXISTS web_research_cache (
    url_hash TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    title TEXT NOT NULL,
    extracted_text_md TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_web_research_cache_url ON web_research_cache(url);

-- Phase 18: Continuous & Scheduled Research / Planning Topics
CREATE TABLE IF NOT EXISTS scheduled_topics (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,                         -- e.g., "SSD Streaming & NVMe"
    prompt_query TEXT NOT NULL,                  -- Search objective or planning instructions
    topic_type TEXT NOT NULL CHECK(topic_type IN ('research', 'plan')),
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    cadence TEXT NOT NULL CHECK(cadence IN ('daily', 'weekly', 'manual')),
    model_id TEXT,                               -- Specific model override, or NULL for default
    search_provider_id TEXT,                     -- SearXNG, Brave, Tavily, etc.
    is_active INTEGER NOT NULL DEFAULT 1,
    last_run_at INTEGER,
    created_at INTEGER NOT NULL,
    -- Phase 18.1: Wall-clock scheduling. Minutes since midnight (UTC), 0..1439.
    -- NULL keeps the legacy rolling-interval behaviour (every 24h / 7d from last run).
    schedule_time_minutes INTEGER
        CHECK(schedule_time_minutes IS NULL OR (schedule_time_minutes >= 0 AND schedule_time_minutes <= 1439)),
    -- Phase 18.1: Weekday anchor for `weekly` topics (ISO: 0=Monday .. 6=Sunday).
    -- NULL means "anchor on the weekday the topic was created".
    schedule_weekday INTEGER
        CHECK(schedule_weekday IS NULL OR (schedule_weekday >= 0 AND schedule_weekday <= 6))
);

-- Every autonomous execution lands here so prior digests can act as an
-- anti-duplication boundary for the next run.
CREATE TABLE IF NOT EXISTS scheduled_topic_runs (
    id TEXT PRIMARY KEY,
    topic_id TEXT NOT NULL REFERENCES scheduled_topics(id) ON DELETE CASCADE,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    sources_scraped_count INTEGER NOT NULL DEFAULT 0,
    executed_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_scheduled_topics_active ON scheduled_topics(is_active, topic_type);
CREATE INDEX IF NOT EXISTS idx_scheduled_topic_runs_topic ON scheduled_topic_runs(topic_id, executed_at DESC);

