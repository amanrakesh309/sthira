use std::sync::Arc;
use std::time::Duration;
use sthira_core::Engine;
use tauri::{AppHandle, Emitter};

/// The scheduler wakes every 30 minutes. Cadence windows (24h / 7d) are far
/// larger than the tick, so a missed window is always caught on the next tick.
const TICK_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Payload emitted on `sthira://scheduled-topic-ready` when an autonomous run
/// produces a digest or plan review, so the UI can raise a toast.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScheduledTopicReadyEvent {
    pub topic_id: String,
    pub topic_title: String,
    pub artifact_id: String,
    pub topic_type: String,
}

/// Spawns the background autonomous-research / plan-review loop.
///
/// Runs once immediately on startup (so a topic whose window elapsed while the
/// app was closed fires right away), then on every [`TICK_INTERVAL`] tick.
pub fn spawn_scheduled_topic_scheduler(app_handle: AppHandle, engine: Arc<Engine>) {
    tauri::async_runtime::spawn(async move {
        run_due_topics_once(&app_handle, &engine).await;

        loop {
            tokio::time::sleep(TICK_INTERVAL).await;
            run_due_topics_once(&app_handle, &engine).await;
        }
    });
}

/// Executes every currently-due topic **sequentially**.
///
/// Sequential execution is deliberate: each research run performs multiple
/// scrapes, and serialising them keeps the background work low-impact and avoids
/// hammering a shared search provider or saturating the local model.
async fn run_due_topics_once(app_handle: &AppHandle, engine: &Arc<Engine>) {
    let now_ms = engine.clock().now_ms();

    let due = match sthira_core::engines::scheduled_runner::list_due_topics(engine, now_ms) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[scheduler] failed to enumerate due topics: {e}");
            return;
        }
    };

    if due.is_empty() {
        return;
    }

    eprintln!("[scheduler] {} scheduled topic(s) due", due.len());

    for topic in due {
        match sthira_core::engines::scheduled_runner::execute_topic_run(engine, &topic.id).await {
            Ok(artifact_id) => {
                let _ = app_handle.emit(
                    "sthira://scheduled-topic-ready",
                    ScheduledTopicReadyEvent {
                        topic_id: topic.id.clone(),
                        topic_title: topic.title.clone(),
                        artifact_id,
                        topic_type: topic.topic_type.as_str().to_string(),
                    },
                );
            }
            Err(e) => {
                eprintln!("[scheduler] topic '{}' run failed: {e}", topic.id);
            }
        }
    }
}
