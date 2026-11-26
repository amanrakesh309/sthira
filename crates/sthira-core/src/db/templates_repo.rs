use crate::{
    models::synthesis_templates::{
        template_allowed_for_workflow, template_mode_compatible, validate_template_draft,
        validate_template_target, SynthesisTemplate, SynthesisTemplateDraft,
    },
    CoreError,
};
use rusqlite::{params, Connection, Row};
use serde_json::Value;

const TEMPLATE_COLUMNS: &str = "id, name, target_mode, description, is_default, template_skeleton_md, created_at, is_builtin, version, guidance, updated_at, archived_at";

#[derive(Debug, Clone, Copy)]
pub struct TemplatePage {
    pub limit: usize,
    pub offset: usize,
}

impl Default for TemplatePage {
    fn default() -> Self {
        Self {
            limit: 100,
            offset: 0,
        }
    }
}

fn row_to_template(r: &Row<'_>) -> Result<SynthesisTemplate, rusqlite::Error> {
    Ok(SynthesisTemplate {
        id: r.get(0)?,
        name: r.get(1)?,
        target_mode: r.get(2)?,
        description: r.get(3)?,
        is_default: r.get::<_, i32>(4)? != 0,
        template_skeleton_md: r.get(5)?,
        created_at: r.get(6)?,
        is_builtin: r.get::<_, i32>(7)? != 0,
        version: r.get(8)?,
        guidance: r.get(9)?,
        updated_at: r.get(10)?,
        archived_at: r.get(11)?,
    })
}

fn invariant(message: impl Into<String>) -> CoreError {
    CoreError::InvariantViolation(message.into())
}

/// Active templates for a workflow. Kept as the narrow, backwards-compatible listing.
pub fn list_synthesis_templates(
    conn: &Connection,
    target_mode: Option<&str>,
) -> Result<Vec<SynthesisTemplate>, CoreError> {
    list_templates(conn, target_mode, false, TemplatePage::default())
}

/// Bounded listing for the Settings manager. Archived rows are opt-in and the page
/// size is clamped so a corrupted store cannot return unbounded content.
pub fn list_templates(
    conn: &Connection,
    target_mode: Option<&str>,
    include_archived: bool,
    page: TemplatePage,
) -> Result<Vec<SynthesisTemplate>, CoreError> {
    if let Some(mode) = target_mode {
        validate_template_target(mode).map_err(invariant)?;
    }
    let limit = page.limit.clamp(1, 200) as i64;
    let offset = page.offset.min(1_000_000) as i64;
    let archived = if include_archived {
        ""
    } else {
        " AND archived_at IS NULL"
    };
    let mode_clause = if target_mode.is_some() {
        " AND target_mode = ?3"
    } else {
        ""
    };
    let sql = format!(
        "SELECT {TEMPLATE_COLUMNS} FROM synthesis_templates
         WHERE 1=1{archived}{mode_clause}
         ORDER BY target_mode ASC, is_builtin DESC, is_default DESC, name COLLATE NOCASE ASC
         LIMIT ?1 OFFSET ?2;"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut templates = Vec::new();
    if let Some(mode) = target_mode {
        let rows = stmt.query_map(
            params![limit, offset, mode.trim().to_ascii_lowercase()],
            row_to_template,
        )?;
        for row in rows {
            templates.push(row?);
        }
    } else {
        let rows = stmt.query_map(params![limit, offset], row_to_template)?;
        for row in rows {
            templates.push(row?);
        }
    }
    Ok(templates)
}

pub fn get_synthesis_template(
    conn: &Connection,
    id: &str,
) -> Result<Option<SynthesisTemplate>, CoreError> {
    let sql = format!("SELECT {TEMPLATE_COLUMNS} FROM synthesis_templates WHERE id = ?1;");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_template(r)?))
    } else {
        Ok(None)
    }
}

/// An explicit template lookup for dispatch. Archived and missing templates are
/// distinguished from "inherit the default" with a useful error.
pub fn require_active_template(
    conn: &Connection,
    id: &str,
    workflow: &str,
) -> Result<SynthesisTemplate, CoreError> {
    validate_template_target(workflow).map_err(invariant)?;
    let template = get_synthesis_template(conn, id)?
        .ok_or_else(|| invariant("The selected template no longer exists. Choose another template or clear the selection."))?;
    if template.archived_at.is_some() {
        return Err(invariant(
            "The selected template was archived. Choose another template or clear the selection.",
        ));
    }
    if !template_allowed_for_workflow(&template, workflow) {
        return Err(invariant(
            "This template belongs to another workflow. Choose a compatible template.",
        ));
    }
    Ok(template)
}

pub fn get_default_synthesis_template(
    conn: &Connection,
    target_mode: &str,
) -> Result<Option<SynthesisTemplate>, CoreError> {
    validate_template_target(target_mode).map_err(invariant)?;
    let sql = format!(
        "SELECT {TEMPLATE_COLUMNS} FROM synthesis_templates
         WHERE target_mode = ?1 AND is_default = 1 AND archived_at IS NULL LIMIT 1;"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![target_mode.trim().to_ascii_lowercase()])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_template(r)?))
    } else {
        Ok(None)
    }
}

pub fn create_synthesis_template(
    conn: &Connection,
    draft: &SynthesisTemplateDraft,
    now_ms: i64,
) -> Result<SynthesisTemplate, CoreError> {
    validate_template_draft(draft).map_err(invariant)?;
    let target_mode = validate_template_target(&draft.target_mode).map_err(invariant)?;
    let id = draft
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    if get_synthesis_template(conn, &id)?.is_some() {
        return Err(invariant(
            "A template with this identifier already exists. Choose a different identifier.",
        ));
    }
    conn.execute(
        "INSERT INTO synthesis_templates
         (id, name, target_mode, description, is_default, template_skeleton_md, created_at, is_builtin, version, guidance, updated_at, archived_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, 0, 1, ?7, ?6, NULL)",
        params![
            id,
            draft.name.trim(),
            target_mode,
            draft.description.trim(),
            draft.template_skeleton_md,
            now_ms,
            draft.guidance.trim(),
        ],
    )?;
    get_synthesis_template(conn, &id)?
        .ok_or_else(|| invariant("The template could not be read after saving."))
}

pub fn update_synthesis_template(
    conn: &Connection,
    draft: &SynthesisTemplateDraft,
    now_ms: i64,
) -> Result<SynthesisTemplate, CoreError> {
    validate_template_draft(draft).map_err(invariant)?;
    let id = draft
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| invariant("Choose the template to edit."))?;
    let expected = draft.expected_version.ok_or_else(|| {
        invariant(
            "Reload this template before saving so an older draft cannot overwrite newer edits.",
        )
    })?;
    let mode = validate_template_target(&draft.target_mode).map_err(invariant)?;
    let current = require_active_template(conn, id, mode)?;
    if current.is_builtin {
        return Err(invariant(
            "Built-in templates are read-only. Duplicate this template to customize it.",
        ));
    }
    if current.version != expected {
        return Err(invariant(
            "This template changed since you opened it. Reload it and reapply your edits.",
        ));
    }
    let mode_changed = mode != current.target_mode;
    conn.execute(
        "UPDATE synthesis_templates
         SET name=?2, target_mode=?3, description=?4, template_skeleton_md=?5,
             guidance=?6, version=version+1, updated_at=?7,
             is_default=CASE WHEN ?8 THEN 0 ELSE is_default END
         WHERE id=?1 AND version=?9 AND is_builtin=0",
        params![
            id,
            draft.name.trim(),
            mode,
            draft.description.trim(),
            draft.template_skeleton_md,
            draft.guidance.trim(),
            now_ms,
            mode_changed,
            expected,
        ],
    )?;
    get_synthesis_template(conn, id)?
        .ok_or_else(|| invariant("The template was removed while it was being saved."))
}

pub fn duplicate_synthesis_template(
    conn: &Connection,
    source_id: &str,
    now_ms: i64,
) -> Result<SynthesisTemplate, CoreError> {
    let source = get_synthesis_template(conn, source_id)?
        .ok_or_else(|| invariant("The template to duplicate no longer exists."))?;
    let name: String = format!("{} (copy)", source.name)
        .chars()
        .take(120)
        .collect();
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO synthesis_templates
         (id, name, target_mode, description, is_default, template_skeleton_md, created_at, is_builtin, version, guidance, updated_at, archived_at)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, 0, 1, ?7, ?6, NULL)",
        params![
            id,
            name,
            source.target_mode,
            source.description,
            source.template_skeleton_md,
            now_ms,
            source.guidance,
        ],
    )?;
    get_synthesis_template(conn, &id)?
        .ok_or_else(|| invariant("The duplicated template could not be read."))
}

/// Transactional single-default assignment. Exactly one row can be the default per
/// workflow because the write clears the previous default before setting the new one.
pub fn set_synthesis_template_default(
    conn: &Connection,
    target_mode: &str,
    id: &str,
) -> Result<(), CoreError> {
    let mode = validate_template_target(target_mode).map_err(invariant)?;
    if mode == "capture" {
        return Err(invariant(
            "Capture structures are optional editor starting points and do not use a workflow default.",
        ));
    }
    require_active_template(conn, id, mode)?;
    conn.execute(
        "UPDATE synthesis_templates SET is_default=0 WHERE target_mode=?1 AND is_default=1",
        params![mode],
    )?;
    conn.execute(
        "UPDATE synthesis_templates SET is_default=1 WHERE id=?1 AND target_mode=?2 AND archived_at IS NULL",
        params![id, mode],
    )?;
    Ok(())
}

pub fn clear_synthesis_template_default(
    conn: &Connection,
    target_mode: &str,
) -> Result<(), CoreError> {
    let mode = validate_template_target(target_mode).map_err(invariant)?;
    conn.execute(
        "UPDATE synthesis_templates SET is_default=0 WHERE target_mode=?1 AND is_default=1",
        params![mode],
    )?;
    Ok(())
}

pub fn archive_synthesis_template(
    conn: &Connection,
    id: &str,
    expected_version: Option<i64>,
    now_ms: i64,
) -> Result<SynthesisTemplate, CoreError> {
    let current = get_synthesis_template(conn, id)?
        .ok_or_else(|| invariant("This template no longer exists."))?;
    if let Some(expected) = expected_version {
        if expected != current.version {
            return Err(invariant(
                "This template changed since it was loaded. Reload it before archiving.",
            ));
        }
    }
    conn.execute(
        "UPDATE synthesis_templates SET archived_at=?2, is_default=0, version=version+1, updated_at=?2 WHERE id=?1",
        params![id, now_ms],
    )?;
    get_synthesis_template(conn, id)?
        .ok_or_else(|| invariant("The archived template could not be read."))
}

pub fn restore_synthesis_template(
    conn: &Connection,
    id: &str,
    now_ms: i64,
) -> Result<SynthesisTemplate, CoreError> {
    conn.execute(
        "UPDATE synthesis_templates SET archived_at=NULL, version=version+1, updated_at=?2 WHERE id=?1",
        params![id, now_ms],
    )?;
    get_synthesis_template(conn, id)?.ok_or_else(|| invariant("This template no longer exists."))
}

/// Only custom templates can be deleted. Built-ins are archived instead so startup
/// seeding cannot silently resurrect content the owner removed.
pub fn delete_synthesis_template(conn: &Connection, id: &str) -> Result<(), CoreError> {
    let current = get_synthesis_template(conn, id)?
        .ok_or_else(|| invariant("This template no longer exists."))?;
    if current.is_builtin {
        return Err(invariant(
            "Built-in templates cannot be deleted. Archive it instead to keep history and stop offering it.",
        ));
    }
    conn.execute("DELETE FROM synthesis_templates WHERE id=?1", params![id])?;
    Ok(())
}

/// One completed generation execution. A `None` snapshot represents an explicit
/// no-template or templates-disabled decision and is recorded with the same immutable
/// history semantics as a template-backed run.
pub struct GenerationRecord<'a> {
    pub workflow: &'a str,
    pub artifact_id: Option<&'a str>,
    pub snapshot: Option<&'a crate::models::synthesis_templates::SynthesisTemplateSnapshot>,
    pub template_mode: &'a str,
    pub generation_key: &'a str,
    pub model_name: &'a str,
    pub provider_name: &'a str,
    pub status: &'a str,
}

/// Record the exact dispatch-time template (or explicit no-template decision) that
/// produced a saved artifact. Repeated callbacks with the same `generation_key` are
/// idempotent. This is execution history, not user knowledge.
pub fn record_generation_run(
    conn: &Connection,
    record: &GenerationRecord<'_>,
    now_ms: i64,
) -> Result<bool, CoreError> {
    validate_template_target(record.workflow).map_err(invariant)?;
    if record.generation_key.trim().is_empty() {
        return Err(invariant("A generation record needs a stable key."));
    }
    if !matches!(
        record.template_mode,
        "inherit" | "explicit" | "none" | "disabled"
    ) {
        return Err(invariant(
            "Unsupported template decision for execution history.",
        ));
    }
    if !matches!(record.status, "completed" | "partial") {
        return Err(invariant(
            "Only completed or partial generations are recorded.",
        ));
    }
    let (template_id, template_version, template_name, skeleton, guidance) = match record.snapshot {
        Some(snapshot) => {
            if !template_mode_compatible(record.workflow, &snapshot.target_mode) {
                return Err(invariant(
                    "The recorded template belongs to another workflow.",
                ));
            }
            if !matches!(record.template_mode, "inherit" | "explicit") {
                return Err(invariant(
                    "A template snapshot needs an inherit or explicit decision.",
                ));
            }
            (
                snapshot.id.as_str(),
                snapshot.version,
                snapshot.name.as_str(),
                snapshot.skeleton_md.as_str(),
                snapshot.guidance.as_str(),
            )
        }
        None => {
            if !matches!(record.template_mode, "inherit" | "none" | "disabled") {
                return Err(invariant(
                    "An explicit template decision needs a template snapshot.",
                ));
            }
            ("__none__", 0, "No template", "", "")
        }
    };
    let changed = conn.execute(
        "INSERT INTO synthesis_template_runs
         (id, workflow, artifact_id, template_id, template_version, template_name, skeleton_md, guidance, created_at, template_mode, generation_key, status, model_name, provider_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(generation_key) WHERE generation_key IS NOT NULL DO NOTHING",
        params![
            uuid::Uuid::new_v4().to_string(),
            record.workflow,
            record.artifact_id,
            template_id,
            template_version,
            template_name,
            skeleton,
            guidance,
            now_ms,
            record.template_mode,
            record.generation_key,
            record.status,
            record.model_name,
            record.provider_name,
        ],
    )?;
    Ok(changed == 1)
}

/// Compatibility wrapper for an explicit template-backed generation.
pub fn record_template_run(
    conn: &Connection,
    workflow: &str,
    artifact_id: Option<&str>,
    snapshot: &crate::models::synthesis_templates::SynthesisTemplateSnapshot,
    now_ms: i64,
) -> Result<(), CoreError> {
    record_generation_run(
        conn,
        &GenerationRecord {
            workflow,
            artifact_id,
            snapshot: Some(snapshot),
            template_mode: "explicit",
            generation_key: &uuid::Uuid::new_v4().to_string(),
            model_name: "",
            provider_name: "",
            status: "completed",
        },
        now_ms,
    )?;
    Ok(())
}

/// Look up the template snapshots behind a saved artifact for lineage display.
pub fn list_template_runs_for_artifact(
    conn: &Connection,
    artifact_id: &str,
    limit: usize,
) -> Result<Vec<Value>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT workflow, template_id, template_version, template_name, created_at, template_mode, status, model_name, provider_name
         FROM synthesis_template_runs WHERE artifact_id=?1 ORDER BY created_at DESC LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![artifact_id, limit.clamp(1, 20) as i64], |r| {
            Ok(serde_json::json!({
                "workflow": r.get::<_, String>(0)?,
                "template_id": r.get::<_, String>(1)?,
                "template_version": r.get::<_, i64>(2)?,
                "template_name": r.get::<_, String>(3)?,
                "created_at": r.get::<_, i64>(4)?,
                "template_mode": r.get::<_, String>(5)?,
                "status": r.get::<_, String>(6)?,
                "model_name": r.get::<_, String>(7)?,
                "provider_name": r.get::<_, String>(8)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

struct BuiltinSeed {
    id: &'static str,
    name: &'static str,
    target_mode: &'static str,
    description: &'static str,
    skeleton: &'static str,
    guidance: &'static str,
}

const DEEP_DIVE: BuiltinSeed = BuiltinSeed {
    id: "research-deep-dive",
    name: "Research: Deep Dive",
    target_mode: "research",
    description: "A focused investigation: what is established, what competes, and what remains uncertain.",
    skeleton: "# {{title}}\n\n## Question and scope\n\n## Context\n\n## What the evidence shows\n\n## Competing interpretations and limits\n\n## Uncertainty and open questions\n\n## Sources\n",
    guidance: "Use only supplied saved or retrieved evidence. Attribute claims to sources, keep uncertainty visible, and do not invent citations or findings.",
};
const EVIDENCE_COMPARISON: BuiltinSeed = BuiltinSeed {
    id: "research-evidence-comparison",
    name: "Research: Evidence Comparison",
    target_mode: "research",
    description: "Compare claims or options against shared criteria and record where evidence is thin.",
    skeleton: "# {{title}}\n\n## Question\n\n## Options or claims compared\n\n## Criteria\n\n## Evidence table\n| Claim or option | Supporting evidence | Limits or contradictions |\n| --- | --- | --- |\n| | | |\n\n## What the evidence does not establish\n\n## Sources\n",
    guidance: "Compare like with like. Leave cells blank when no supplied evidence supports them rather than estimating.",
};
const FOUR_LENSES: BuiltinSeed = BuiltinSeed {
    id: "research-cultural-four-lenses",
    name: "Research: Cultural Tradition — Four Lenses",
    target_mode: "research",
    description: "An evidence-led guide to a tradition through description, origins, context, and present understanding.",
    skeleton: "# {{title}}\n\n## Scope\n- Tradition/practice:\n- Region/community:\n- Time period:\n- Research question:\n\n## 1. What is it?\nDescription, variations, and relevant context.\n\n## 2. Why did it form?\nHistorical evidence, proposed explanations, and uncertainty.\n\n## 3. Why might it have worked in its original context?\nClaimed functions, supporting evidence, limitations, and alternatives.\n\n## 4. How is it understood today?\nContemporary perspectives, changes, disagreements, and regional differences.\n\n## Evidence and competing interpretations\n\n## What remains uncertain\n\n## Sources\n",
    guidance: "Treat explanations of a tradition as evidence-dependent interpretations, not universal claims. Distinguish recorded practice, historical evidence, and proposed explanations. Preserve disagreement and uncertainty, and never present generated prose as user-recorded fact.",
};
const MILESTONE_PLAN: BuiltinSeed = BuiltinSeed {
    id: "plan-milestone-roadmap",
    name: "Plan: Milestone Roadmap",
    target_mode: "plan",
    description: "Phased milestones with explicit scope, open decisions, and verification criteria.",
    skeleton: "# {{title}}\n\n## Goal and definition of done\n\n## Scope and non-goals\n\n## Phase 1 — Foundation\n- [ ] \n\n## Phase 2 — Implementation\n- [ ] \n\n## Phase 3 — Verification\n- [ ] \n\n## Open decisions and risks\n\n## Verification criteria\n",
    guidance: "Produce a feasible sequence for this specific goal. Do not claim work is complete and do not invent metrics or benchmark values.",
};
const MVP_PLAN: BuiltinSeed = BuiltinSeed {
    id: "plan-mvp-roadmap",
    name: "Plan: MVP / Product Roadmap",
    target_mode: "plan",
    description: "A product roadmap that isolates the smallest useful release and its follow-on work.",
    skeleton: "# {{title}}\n\n## Product intent and user problem\n\n## MVP scope\n\n## Out of scope\n\n## Milestones\n### Milestone 1 — \n- [ ] \n\n### Milestone 2 — \n- [ ] \n\n## Risks and assumptions\n\n## Success measures (define, do not invent)\n",
    guidance: "Define measures as proposals for the owner to decide; do not fabricate targets or adoption numbers.",
};
const EDITORIAL_PLAN: BuiltinSeed = BuiltinSeed {
    id: "plan-editorial-launch",
    name: "Plan: Editorial Launch Plan",
    target_mode: "plan",
    description: "A publishing plan covering audience, editorial sequence, review, and launch work.",
    skeleton: "# {{title}}\n\n## Audience and purpose\n\n## Editorial scope\n\n## Publication sequence\n- [ ] Draft\n- [ ] Fact-check and source review\n- [ ] Edit\n- [ ] Publish\n\n## Review and fact-checking\n\n## Launch channels\n\n## Open decisions\n",
    guidance: "Keep the sequence realistic for the stated team. Do not state that review or publication has already happened.",
};
const FAST_SPRINT: BuiltinSeed = BuiltinSeed {
    id: "plan-fast-sprint",
    name: "Plan: Fast Iteration Sprint",
    target_mode: "plan",
    description: "A short, bounded sprint with immediate actions, risks, and verification.",
    skeleton: "# {{title}}\n\n## Goal\n\n## Definition of done\n\n## Immediate actions\n- [ ] \n\n## Risks and blockers\n\n## Verification\n- [ ] \n",
    guidance: "Keep the sprint small enough to finish. Mark tasks as proposed, not completed.",
};
const READING_NOTE: BuiltinSeed = BuiltinSeed {
    id: "capture-reading-note",
    name: "Capture: Reading Note",
    target_mode: "capture",
    description: "An optional note structure for a reading session. Capture never calls a model.",
    skeleton: "# {{title}}\n\n## Citation\n\n## Why I read this\n\n## Key points in my own words\n\n## Quotes to verify\n\n## Questions\n\n## Related notes\n",
    guidance: "Optional starting structure only. Sthira does not fill this in with AI text during Capture; placeholders are not recorded facts or verified evidence.",
};

/// Startup seeding is additive and idempotent. It never overwrites names, structure,
/// guidance, defaults, or archive state that may belong to the owner.
pub fn seed_default_synthesis_templates(conn: &Connection, now_ms: i64) -> Result<(), CoreError> {
    let seeds = [
        DEEP_DIVE,
        EVIDENCE_COMPARISON,
        FOUR_LENSES,
        MILESTONE_PLAN,
        MVP_PLAN,
        EDITORIAL_PLAN,
        FAST_SPRINT,
        READING_NOTE,
    ];
    for seed in seeds {
        conn.execute(
            "INSERT INTO synthesis_templates
             (id, name, target_mode, description, is_default, template_skeleton_md, created_at, is_builtin, version, guidance, updated_at, archived_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, 1, 1, ?7, ?6, NULL)
             ON CONFLICT(id) DO NOTHING",
            params![
                seed.id,
                seed.name,
                seed.target_mode,
                seed.description,
                seed.skeleton,
                now_ms,
                seed.guidance,
            ],
        )?;
    }
    Ok(())
}

/// Upgrade only those legacy built-in rows whose stored content still exactly matches
/// the old seed text. A user-edited built-in is left untouched even though it remains
/// marked as built-in (the owner can duplicate it).
pub fn upgrade_legacy_builtin_content(conn: &Connection) -> rusqlite::Result<()> {
    let legacy = [
        (
            DEEP_DIVE,
            "# {Title}\n\n## Overview & Context\n\n## Core Mechanisms & Architecture\n\n## Empirical Findings & Benchmarks\n\n## Contradictions & Trade-offs\n\n## Key Citations & Sources\n\n## Strategic Takeaways\n",
        ),
        (
            MILESTONE_PLAN,
            "# {Title}\n\n## Scope & Objectives\n\n## Phase 1: Foundation & Core Setup\n- [ ] Establish base architecture and schemas\n- [ ] Configure core pipelines and dependencies\n- [ ] Verify initial baseline tests\n\n## Phase 2: Feature Implementation\n- [ ] Implement primary execution workflows\n- [ ] Integrate external touchpoints and UI surfaces\n- [ ] Validate end-to-end user journeys\n\n## Open Decisions & Trade-offs\n- Decision: Architectural invariant and migration constraints\n",
        ),
        (
            FAST_SPRINT,
            "# {Title}\n\n## Goal & Definition of Done\n\n## Immediate Action Items\n- [ ] Priority 1 implementation task\n- [ ] Priority 2 implementation task\n- [ ] Priority 3 implementation task\n\n## Risks & Blockers\n\n## Verification Checklist\n- [ ] Automated regression tests pass\n- [ ] Performance benchmarks satisfied\n",
        ),
    ];
    for (seed, old_skeleton) in legacy {
        conn.execute(
            "UPDATE synthesis_templates
             SET name=?2, description=?3, template_skeleton_md=?4, guidance=?5,
                 is_builtin=1, version=CASE WHEN version<2 THEN 2 ELSE version END,
                 updated_at=created_at
             WHERE id=?1 AND template_skeleton_md=?6",
            params![
                seed.id,
                seed.name,
                seed.description,
                seed.skeleton,
                seed.guidance,
                old_skeleton,
            ],
        )?;
    }
    // Legacy rows are known built-ins even if their content was already customized.
    conn.execute(
        "UPDATE synthesis_templates SET is_builtin=1 WHERE id IN
         ('research-deep-dive','plan-milestone-roadmap','plan-fast-sprint')",
        [],
    )?;
    Ok(())
}
