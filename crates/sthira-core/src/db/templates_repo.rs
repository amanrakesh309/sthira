use crate::{models::synthesis_templates::SynthesisTemplate, CoreError};
use rusqlite::{params, Connection, Row};

const TEMPLATE_COLUMNS: &str =
    "id, name, target_mode, description, is_default, template_skeleton_md, created_at";

fn row_to_template(r: &Row<'_>) -> Result<SynthesisTemplate, rusqlite::Error> {
    Ok(SynthesisTemplate {
        id: r.get(0)?,
        name: r.get(1)?,
        target_mode: r.get(2)?,
        description: r.get(3)?,
        is_default: r.get::<_, i32>(4)? != 0,
        template_skeleton_md: r.get(5)?,
        created_at: r.get(6)?,
    })
}

pub fn list_synthesis_templates(
    conn: &Connection,
    target_mode: Option<&str>,
) -> Result<Vec<SynthesisTemplate>, CoreError> {
    let mut templates = Vec::new();

    if let Some(mode) = target_mode {
        let sql = format!(
            "SELECT {TEMPLATE_COLUMNS} FROM synthesis_templates WHERE target_mode = ?1 ORDER BY is_default DESC, name ASC;"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![mode], row_to_template)?;
        for r in rows {
            templates.push(r?);
        }
    } else {
        let sql = format!(
            "SELECT {TEMPLATE_COLUMNS} FROM synthesis_templates ORDER BY target_mode ASC, is_default DESC, name ASC;"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_template)?;
        for r in rows {
            templates.push(r?);
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

pub fn get_default_synthesis_template(
    conn: &Connection,
    target_mode: &str,
) -> Result<Option<SynthesisTemplate>, CoreError> {
    let sql = format!(
        "SELECT {TEMPLATE_COLUMNS} FROM synthesis_templates WHERE target_mode = ?1 AND is_default = 1 LIMIT 1;"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![target_mode])?;

    if let Some(r) = rows.next()? {
        Ok(Some(row_to_template(r)?))
    } else {
        Ok(None)
    }
}

pub fn save_synthesis_template(
    conn: &Connection,
    template: &SynthesisTemplate,
) -> Result<(), CoreError> {
    if template.is_default {
        conn.execute(
            "UPDATE synthesis_templates SET is_default = 0 WHERE target_mode = ?1;",
            params![template.target_mode],
        )?;
    }

    conn.execute(
        "INSERT INTO synthesis_templates (id, name, target_mode, description, is_default, template_skeleton_md, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            target_mode = excluded.target_mode,
            description = excluded.description,
            is_default = excluded.is_default,
            template_skeleton_md = excluded.template_skeleton_md;",
        params![
            template.id,
            template.name,
            template.target_mode,
            template.description,
            if template.is_default { 1 } else { 0 },
            template.template_skeleton_md,
            template.created_at,
        ],
    )?;

    Ok(())
}

pub fn seed_default_synthesis_templates(conn: &Connection, now_ms: i64) -> Result<(), CoreError> {
    let defaults = vec![
        SynthesisTemplate {
            id: "research-comparative-tech".to_string(),
            name: "Comparative Tech Evaluation".to_string(),
            target_mode: "research".to_string(),
            description: "Structured evaluation comparing frameworks, architectures, or libraries with a comparison matrix.".to_string(),
            is_default: true,
            template_skeleton_md: "# {Title}\n\n## Executive Summary\n\n## Contenders & Key Criteria\n\n## Comparison Matrix\n| Solution | Performance | Developer Experience | Maturity | Ecosystem & Trade-offs |\n| :--- | :--- | :--- | :--- | :--- |\n| Option A | High | Moderate | Production | Strong ecosystem, higher learning curve |\n| Option B | Very High | Excellent | Modern | Rapidly evolving, smaller community |\n\n## Grounded Sources & Evidence\n\n## Recommendation & Next Steps\n".to_string(),
            created_at: now_ms,
        },
        SynthesisTemplate {
            id: "research-deep-dive".to_string(),
            name: "Deep Dive Investigation".to_string(),
            target_mode: "research".to_string(),
            description: "In-depth technical breakdown with architecture details, empirical benchmarks, and citations.".to_string(),
            is_default: false,
            template_skeleton_md: "# {Title}\n\n## Overview & Context\n\n## Core Mechanisms & Architecture\n\n## Empirical Findings & Benchmarks\n\n## Contradictions & Trade-offs\n\n## Key Citations & Sources\n\n## Strategic Takeaways\n".to_string(),
            created_at: now_ms,
        },
        SynthesisTemplate {
            id: "plan-milestone-roadmap".to_string(),
            name: "Milestone Roadmap".to_string(),
            target_mode: "plan".to_string(),
            description: "Phased execution roadmap with scope, actionable milestone checklists, and open decisions.".to_string(),
            is_default: true,
            template_skeleton_md: "# {Title}\n\n## Scope & Objectives\n\n## Phase 1: Foundation & Core Setup\n- [ ] Establish base architecture and schemas\n- [ ] Configure core pipelines and dependencies\n- [ ] Verify initial baseline tests\n\n## Phase 2: Feature Implementation\n- [ ] Implement primary execution workflows\n- [ ] Integrate external touchpoints and UI surfaces\n- [ ] Validate end-to-end user journeys\n\n## Open Decisions & Trade-offs\n- Decision: Architectural invariant and migration constraints\n".to_string(),
            created_at: now_ms,
        },
        SynthesisTemplate {
            id: "plan-fast-sprint".to_string(),
            name: "Fast Iteration Sprint".to_string(),
            target_mode: "plan".to_string(),
            description: "High-velocity deliverable breakdown with immediate checklist, risk mitigations, and verification criteria.".to_string(),
            is_default: false,
            template_skeleton_md: "# {Title}\n\n## Goal & Definition of Done\n\n## Immediate Action Items\n- [ ] Priority 1 implementation task\n- [ ] Priority 2 implementation task\n- [ ] Priority 3 implementation task\n\n## Risks & Blockers\n\n## Verification Checklist\n- [ ] Automated regression tests pass\n- [ ] Performance benchmarks satisfied\n".to_string(),
            created_at: now_ms,
        },
    ];

    for t in defaults {
        // Only insert if not already existing, or update skeleton
        conn.execute(
            "INSERT INTO synthesis_templates (id, name, target_mode, description, is_default, template_skeleton_md, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                target_mode = excluded.target_mode,
                description = excluded.description,
                template_skeleton_md = excluded.template_skeleton_md;",
            params![
                t.id,
                t.name,
                t.target_mode,
                t.description,
                if t.is_default { 1 } else { 0 },
                t.template_skeleton_md,
                t.created_at,
            ],
        )?;
    }

    Ok(())
}
