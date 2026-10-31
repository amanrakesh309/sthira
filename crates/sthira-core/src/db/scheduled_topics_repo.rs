use crate::{
    models::scheduled_topics::{ScheduledTopic, ScheduledTopicRun, TopicCadence, TopicType},
    CoreError,
};
use rusqlite::{params, Connection};

const TOPIC_COLUMNS: &str =
    "id, title, prompt_query, topic_type, project_id, cadence, model_id, search_provider_id, is_active, last_run_at, created_at, schedule_time_minutes, schedule_weekday";

fn row_to_topic(r: &rusqlite::Row<'_>) -> Result<ScheduledTopic, rusqlite::Error> {
    let topic_type: String = r.get(3)?;
    let cadence: String = r.get(5)?;
    let weekday: Option<i64> = r.get(12)?;
    Ok(ScheduledTopic {
        id: r.get(0)?,
        title: r.get(1)?,
        prompt_query: r.get(2)?,
        topic_type: TopicType::from_str_lossy(&topic_type),
        project_id: r.get(4)?,
        cadence: TopicCadence::from_str_lossy(&cadence),
        model_id: r.get(6)?,
        search_provider_id: r.get(7)?,
        is_active: r.get::<_, i64>(8)? != 0,
        last_run_at: r.get(9)?,
        created_at: r.get(10)?,
        schedule_time_minutes: r.get(11)?,
        schedule_weekday: weekday.map(|w| w as u8),
    })
}

pub fn create_topic(conn: &Connection, topic: &ScheduledTopic) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO scheduled_topics
            (id, title, prompt_query, topic_type, project_id, cadence, model_id, search_provider_id, is_active, last_run_at, created_at, schedule_time_minutes, schedule_weekday)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13);",
        params![
            topic.id,
            topic.title,
            topic.prompt_query,
            topic.topic_type.as_str(),
            topic.project_id,
            topic.cadence.as_str(),
            topic.model_id,
            topic.search_provider_id,
            if topic.is_active { 1 } else { 0 },
            topic.last_run_at,
            topic.created_at,
            topic.schedule_time_minutes,
            topic.schedule_weekday.map(|w| w as i64),
        ],
    )?;
    Ok(())
}

pub fn get_topic(conn: &Connection, id: &str) -> Result<Option<ScheduledTopic>, CoreError> {
    let sql = format!("SELECT {TOPIC_COLUMNS} FROM scheduled_topics WHERE id = ?1;");
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query(params![id])?;
    if let Some(r) = rows.next()? {
        Ok(Some(row_to_topic(r)?))
    } else {
        Ok(None)
    }
}

/// Lists every topic (optionally narrowed to a single `topic_type`), newest first.
pub fn list_topics_by_type(
    conn: &Connection,
    topic_type: Option<TopicType>,
) -> Result<Vec<ScheduledTopic>, CoreError> {
    let mut out = Vec::new();
    match topic_type {
        Some(t) => {
            let sql = format!(
                "SELECT {TOPIC_COLUMNS} FROM scheduled_topics WHERE topic_type = ?1 ORDER BY created_at DESC;"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![t.as_str()], row_to_topic)?;
            for r in rows {
                out.push(r?);
            }
        }
        None => {
            let sql =
                format!("SELECT {TOPIC_COLUMNS} FROM scheduled_topics ORDER BY created_at DESC;");
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([], row_to_topic)?;
            for r in rows {
                out.push(r?);
            }
        }
    }
    Ok(out)
}

/// Every active topic whose cadence is autonomous. Cadence-vs-clock evaluation is
/// deliberately left to [`crate::models::scheduled_topics::is_topic_due`] so the
/// scheduler and the tests share one source of truth.
pub fn list_active_topics(conn: &Connection) -> Result<Vec<ScheduledTopic>, CoreError> {
    let sql = format!(
        "SELECT {TOPIC_COLUMNS} FROM scheduled_topics
         WHERE is_active = 1 AND cadence <> 'manual'
         ORDER BY last_run_at IS NOT NULL, last_run_at ASC;"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_topic)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn update_topic_status(
    conn: &Connection,
    topic_id: &str,
    is_active: bool,
) -> Result<(), CoreError> {
    let changed = conn.execute(
        "UPDATE scheduled_topics SET is_active = ?2 WHERE id = ?1;",
        params![topic_id, if is_active { 1 } else { 0 }],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!(
            "Scheduled topic '{}' not found",
            topic_id
        )));
    }
    Ok(())
}

/// Stamps `last_run_at` after a successful execution.
pub fn update_topic_last_run(
    conn: &Connection,
    topic_id: &str,
    last_run_at: i64,
) -> Result<(), CoreError> {
    conn.execute(
        "UPDATE scheduled_topics SET last_run_at = ?2 WHERE id = ?1;",
        params![topic_id, last_run_at],
    )?;
    Ok(())
}

pub fn record_topic_run(conn: &Connection, run: &ScheduledTopicRun) -> Result<(), CoreError> {
    conn.execute(
        "INSERT INTO scheduled_topic_runs (id, topic_id, artifact_id, sources_scraped_count, executed_at)
         VALUES (?1, ?2, ?3, ?4, ?5);",
        params![
            run.id,
            run.topic_id,
            run.artifact_id,
            run.sources_scraped_count,
            run.executed_at,
        ],
    )?;
    Ok(())
}

/// Past runs for a topic, newest first. Used both for the UI history list and to
/// build the anti-duplication boundary for the next execution.
pub fn list_runs_for_topic(
    conn: &Connection,
    topic_id: &str,
) -> Result<Vec<ScheduledTopicRun>, CoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, topic_id, artifact_id, sources_scraped_count, executed_at
         FROM scheduled_topic_runs
         WHERE topic_id = ?1
         ORDER BY executed_at DESC;",
    )?;
    let rows = stmt.query_map(params![topic_id], |r| {
        Ok(ScheduledTopicRun {
            id: r.get(0)?,
            topic_id: r.get(1)?,
            artifact_id: r.get(2)?,
            sources_scraped_count: r.get(3)?,
            executed_at: r.get(4)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn delete_topic(conn: &Connection, topic_id: &str) -> Result<(), CoreError> {
    // Runs cascade via the FK; generated artifacts are intentionally preserved so a
    // digest the user already read is never destroyed by deleting its topic.
    let changed = conn.execute(
        "DELETE FROM scheduled_topics WHERE id = ?1;",
        params![topic_id],
    )?;
    if changed == 0 {
        return Err(CoreError::NotFound(format!(
            "Scheduled topic '{}' not found",
            topic_id
        )));
    }
    Ok(())
}

