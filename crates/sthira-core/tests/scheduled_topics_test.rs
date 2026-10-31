use async_trait::async_trait;
use std::sync::Arc;
use sthira_core::{
    db::{scheduled_topics_repo as repo, search_repo},
    engines::scheduled_runner::{
        build_continuity_boundary, execute_topic_run_with_provider, list_due_topics,
    },
    models::scheduled_topics::{is_topic_due, ScheduledTopic, TopicCadence, TopicType},
    search::{ScrapedWebDocument, SearchProvider, SearchResultItem},
    ArtifactType, Authorship, Clock, Engine,
};
use sthira_testkit::{DeterministicFakeModel, FakeClock};
use tempfile::tempdir;

const HOUR_MS: i64 = 60 * 60 * 1000;
const DAY_MS: i64 = 24 * HOUR_MS;

const NVME_URL: &str = "https://nvme.example.test/streaming-architecture";
const QUEUE_URL: &str = "https://storage.example.test/queue-depth-benchmarks";

const NVME_BODY: &str = "# NVMe Streaming Architecture\n\n\
    PCIe Gen5 NVMe streaming sustains twelve gigabytes per second using direct storage queues.\n\n\
    Host memory buffers decouple device latency from application read paths.\n";

const QUEUE_BODY: &str = "# Queue Depth Benchmarks\n\n\
    Queue depth scaling beyond sixty four commands yields diminishing latency returns.\n\n\
    Consumer controllers saturate well before enterprise queue depths.\n";

/// Deterministic, fully offline search backend. Paired with a pre-seeded
/// `web_research_cache` so no network request is ever made.
struct FakeSearchProvider {
    calls: std::sync::Mutex<Vec<String>>,
}

impl FakeSearchProvider {
    fn new() -> Self {
        Self {
            calls: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn queries(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl SearchProvider for FakeSearchProvider {
    async fn search(&self, query: &str, _limit: usize) -> Result<Vec<SearchResultItem>, String> {
        self.calls.lock().unwrap().push(query.to_string());
        Ok(vec![
            SearchResultItem {
                title: "NVMe Streaming Architecture Deep Dive".to_string(),
                url: NVME_URL.to_string(),
                snippet: "PCIe Gen5 NVMe streaming sustains twelve gigabytes per second.".to_string(),
                score: 1.0,
            },
            SearchResultItem {
                title: "SSD Queue Depth Benchmarks".to_string(),
                url: QUEUE_URL.to_string(),
                snippet: "Queue depth scaling yields diminishing latency returns.".to_string(),
                score: 0.9,
            },
        ])
    }
}

/// Pre-populates the scrape cache so `scrape_and_clean_url` resolves offline.
fn seed_scrape_cache(engine: &Engine) {
    let now = engine.clock().now_ms();
    let writer = engine.db().writer();
    for (url, title, body) in [
        (NVME_URL, "NVMe Streaming Architecture Deep Dive", NVME_BODY),
        (QUEUE_URL, "SSD Queue Depth Benchmarks", QUEUE_BODY),
    ] {
        search_repo::save_cached_web_document(
            &writer,
            &ScrapedWebDocument {
                url: url.to_string(),
                title: title.to_string(),
                content_md: body.to_string(),
                published_date: None,
            },
            now,
        )
        .unwrap();
    }
}

fn setup() -> (tempfile::TempDir, Arc<Engine>, Arc<FakeClock>) {
    let tmp = tempdir().unwrap();
    let db_path = tmp.path().join("scheduled_topics_test.db");
    let vault_dir = tmp.path().join("vault");
    std::fs::create_dir_all(&vault_dir).unwrap();

    let clock = Arc::new(FakeClock::new(1_700_000_000_000));
    let model = Arc::new(DeterministicFakeModel::new("test-model"));

    let engine = Arc::new(
        Engine::open(
            &db_path,
            &vault_dir,
            Arc::clone(&clock) as _,
            Arc::clone(&model) as _,
        )
        .unwrap(),
    );

    seed_scrape_cache(&engine);

    (tmp, engine, clock)
}

fn make_topic(
    engine: &Engine,
    id: &str,
    title: &str,
    topic_type: TopicType,
    cadence: TopicCadence,
    project_id: Option<&str>,
) -> ScheduledTopic {
    make_topic_full(
        engine,
        id,
        title,
        topic_type,
        cadence,
        project_id,
        engine.clock().now_ms(),
        None,
        None,
    )
}

/// Full-control constructor used by the Phase 18.1 wall-clock scheduling tests.
#[allow(clippy::too_many_arguments)]
fn make_topic_full(
    engine: &Engine,
    id: &str,
    title: &str,
    topic_type: TopicType,
    cadence: TopicCadence,
    project_id: Option<&str>,
    created_at: i64,
    schedule_time_minutes: Option<i64>,
    schedule_weekday: Option<u8>,
) -> ScheduledTopic {
    let topic = ScheduledTopic {
        id: id.to_string(),
        title: title.to_string(),
        prompt_query: format!("Track developments in {title}"),
        topic_type,
        project_id: project_id.map(|p| p.to_string()),
        cadence,
        model_id: None,
        search_provider_id: None,
        is_active: true,
        last_run_at: None,
        created_at,
        schedule_time_minutes,
        schedule_weekday,
    };
    let writer = engine.db().writer();
    repo::create_topic(&writer, &topic).unwrap();
    topic
}

/// 06:00 UTC on the given day offset from the fake clock's epoch base.
fn at_time(base_ms: i64, day_offset: i64, minutes: i64) -> i64 {
    let base_day = base_ms.div_euclid(sthira_core::models::scheduled_topics::DAY_MS);
    (base_day + day_offset) * sthira_core::models::scheduled_topics::DAY_MS + minutes * 60_000
}

// ---------------------------------------------------------------------------
// 1. CRUD + type filtering
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_create_list_and_filter_topics_by_type() {
    let (_tmp, engine, _clock) = setup();

    make_topic(&engine, "t-research", "SSD Streaming & NVMe", TopicType::Research, TopicCadence::Daily, None);
    make_topic(&engine, "t-plan", "Architecture Review", TopicType::Plan, TopicCadence::Weekly, None);
    make_topic(&engine, "t-manual", "Ad-hoc Probe", TopicType::Research, TopicCadence::Manual, None);

    let reader = engine.db().reader().unwrap();

    // Unfiltered listing returns everything.
    let all = repo::list_topics_by_type(&reader, None).unwrap();
    assert_eq!(all.len(), 3);

    // Filtering by type.
    let research = repo::list_topics_by_type(&reader, Some(TopicType::Research)).unwrap();
    assert_eq!(research.len(), 2);
    assert!(research.iter().all(|t| t.topic_type == TopicType::Research));

    let plan = repo::list_topics_by_type(&reader, Some(TopicType::Plan)).unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].title, "Architecture Review");
    assert_eq!(plan[0].cadence, TopicCadence::Weekly);
    assert!(plan[0].is_active);

    // Round-trip a single topic.
    let fetched = repo::get_topic(&reader, "t-research").unwrap().unwrap();
    assert_eq!(fetched.title, "SSD Streaming & NVMe");
    assert_eq!(fetched.prompt_query, "Track developments in SSD Streaming & NVMe");
    assert!(fetched.last_run_at.is_none());
    drop(reader);

    // Status toggle + delete (write path uses the writer connection).
    let writer = engine.db().writer();
    repo::update_topic_status(&writer, "t-manual", false).unwrap();
    let reader = engine.db().reader().unwrap();
    assert!(!repo::get_topic(&reader, "t-manual").unwrap().unwrap().is_active);
    drop(reader);

    repo::delete_topic(&writer, "t-manual").unwrap();
    let reader = engine.db().reader().unwrap();
    assert!(repo::get_topic(&reader, "t-manual").unwrap().is_none());
    drop(reader);

    assert!(repo::delete_topic(&writer, "t-manual").is_err());
}

// ---------------------------------------------------------------------------
// 2. Cadence evaluation: only autonomous cadences, only past their window
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_cadence_evaluation_flags_topics_older_than_24_hours() {
    let (_tmp, engine, clock) = setup();
    let now = clock.now_ms();

    // Pure window logic.
    assert!(is_topic_due(TopicCadence::Daily, None, now), "never-run daily topic is due");
    assert!(
        !is_topic_due(TopicCadence::Daily, Some(now - 23 * HOUR_MS), now),
        "daily topic run 23h ago is not due"
    );
    assert!(
        is_topic_due(TopicCadence::Daily, Some(now - 25 * HOUR_MS), now),
        "daily topic run 25h ago is due"
    );
    assert!(
        !is_topic_due(TopicCadence::Weekly, Some(now - 2 * DAY_MS), now),
        "weekly topic run 2d ago is not due"
    );
    assert!(
        is_topic_due(TopicCadence::Weekly, Some(now - 8 * DAY_MS), now),
        "weekly topic run 8d ago is due"
    );
    assert!(
        !is_topic_due(TopicCadence::Manual, None, now),
        "manual topics are never due autonomously"
    );

    // Scheduler-level enumeration.
    make_topic(&engine, "due-old", "Stale Daily", TopicType::Research, TopicCadence::Daily, None);
    make_topic(&engine, "due-fresh", "Fresh Daily", TopicType::Research, TopicCadence::Daily, None);
    make_topic(&engine, "due-weekly", "Fresh Weekly", TopicType::Research, TopicCadence::Weekly, None);
    make_topic(&engine, "due-manual", "Manual Only", TopicType::Research, TopicCadence::Manual, None);

    {
        let writer = engine.db().writer();
        repo::update_topic_last_run(&writer, "due-old", now - 25 * HOUR_MS).unwrap();
        repo::update_topic_last_run(&writer, "due-fresh", now - 2 * HOUR_MS).unwrap();
        repo::update_topic_last_run(&writer, "due-weekly", now - 2 * DAY_MS).unwrap();
    }

    let due = list_due_topics(&engine, now).unwrap();
    let ids: Vec<&str> = due.iter().map(|t| t.id.as_str()).collect();
    assert!(ids.contains(&"due-old"), "25h-old daily topic must be due");
    assert!(!ids.contains(&"due-fresh"), "2h-old daily topic must not be due");
    assert!(!ids.contains(&"due-weekly"), "2d-old weekly topic must not be due");
    assert!(!ids.contains(&"due-manual"), "manual topic must never be scheduled");

    // Pausing removes a topic from the due set entirely.
    {
        let writer = engine.db().writer();
        repo::update_topic_status(&writer, "due-old", false).unwrap();
    }
    let due_after_pause = list_due_topics(&engine, now).unwrap();
    assert!(due_after_pause.is_empty());
}


// ---------------------------------------------------------------------------
// 3. Research execution: citations, External authorship, project association
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_execute_research_topic_produces_grounded_digest_with_citations() {
    let (_tmp, engine, _clock) = setup();

    let project = engine.create_project("Storage Platform", None).unwrap();
    make_topic(
        &engine,
        "t-research",
        "SSD Streaming & NVMe",
        TopicType::Research,
        TopicCadence::Daily,
        Some(&project.id),
    );

    let provider = FakeSearchProvider::new();
    let artifact_id = execute_topic_run_with_provider(&engine, "t-research", Some(&provider))
        .await
        .expect("research run should succeed");

    // The search backend was actually consulted.
    assert!(
        !provider.queries().is_empty(),
        "the configured search provider must be queried"
    );

    let artifact = engine.get_artifact(&artifact_id).unwrap();

    // Digest title carries the ISO date stamp.
    let title = artifact.title.clone().unwrap();
    assert!(title.starts_with("Digest: SSD Streaming & NVMe — "), "got {title}");
    assert_eq!(
        title.rsplit('—').next().unwrap().trim().len(),
        10,
        "expected YYYY-MM-DD"
    );

    // External authorship — web-derived content can never become user-recorded truth.
    assert_eq!(artifact.authorship, Authorship::External);

    // Grounded evidence + numbered citations rendered into the note.
    assert!(artifact.content_md.contains("Grounded Web Evidence & Verified Findings"));
    assert!(artifact.content_md.contains("## Sources"), "numbered source index must be present");
    assert!(artifact.content_md.contains("[Source 1]"), "findings must carry [Source N] markers");
    assert!(artifact.content_md.contains("favicon"), "citations must carry a favicon");
    // Temporal grounding contract.
    assert!(
        artifact.content_md.contains("## Contradictions & Evolution"),
        "temporal grounding block must be present"
    );
    assert!(artifact.content_md.contains("## Synthesis"));
    assert!(
        artifact.content_md.contains("nvme.example.test"),
        "source citation domain must appear in the digest"
    );
    assert!(artifact.content_md.contains("Queries executed"));

    // The generated note is associated with the topic's project (zero duplication).
    {
        let reader = engine.db().reader().unwrap();
        let linked: i64 = reader
            .query_row(
                "SELECT COUNT(*) FROM project_artifacts
                 WHERE project_id = ?1 AND artifact_id = ?2 AND detached_at IS NULL;",
                rusqlite::params![project.id, artifact_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(linked, 1, "digest must be attached to the linked project");
    }

    // Run recorded + last_run_at stamped.
    {
        let reader = engine.db().reader().unwrap();
        let runs = repo::list_runs_for_topic(&reader, "t-research").unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].artifact_id, artifact_id);
        assert!(
            runs[0].sources_scraped_count > 0,
            "scraped source count must be recorded"
        );

        let topic = repo::get_topic(&reader, "t-research").unwrap().unwrap();
        assert!(topic.last_run_at.is_some(), "last_run_at must be stamped");
    }
}


// ---------------------------------------------------------------------------
// 4. Anti-duplication boundary carries prior digests into the next run
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_second_run_applies_continuity_boundary() {
    let (_tmp, engine, _clock) = setup();
    make_topic(
        &engine,
        "t-repeat",
        "NVMe Watch",
        TopicType::Research,
        TopicCadence::Daily,
        None,
    );

    let provider = FakeSearchProvider::new();

    // First run — no boundary yet.
    assert_eq!(build_continuity_boundary(&engine, "t-repeat"), "");
    let first = execute_topic_run_with_provider(&engine, "t-repeat", Some(&provider))
        .await
        .unwrap();

    // The first digest's findings become the boundary for the next run.
    let boundary = build_continuity_boundary(&engine, "t-repeat");
    assert!(
        !boundary.is_empty(),
        "prior digest findings must seed the anti-duplication boundary"
    );
    assert!(boundary.starts_with("- "));

    // Second run embeds the boundary and explicitly excludes covered items.
    let second = execute_topic_run_with_provider(&engine, "t-repeat", Some(&provider))
        .await
        .unwrap();
    assert_ne!(first, second, "each run produces a fresh artifact");

    let second_artifact = engine.get_artifact(&second).unwrap();
    assert!(second_artifact.content_md.contains("Previously Covered"));
    assert!(second_artifact.content_md.contains("Anti-duplication boundary applied"));

    // The inquiry handed to the search planner carries the exclusion instruction.
    let queries = provider.queries();
    assert!(
        queries.iter().any(|q| q.contains("CONTINUITY BOUNDARY")),
        "continuity boundary must be injected into the inquiry"
    );
}

// ---------------------------------------------------------------------------
// 5. Plan review: milestone evaluation, AI authorship, project association
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_execute_plan_topic_produces_plan_review() {
    let (_tmp, engine, _clock) = setup();

    let project = engine.create_project("Retrieval Systems", None).unwrap();
    let note = engine
        .create_artifact(
            ArtifactType::Note,
            Authorship::User,
            Some("Milestone Notes"),
            "# Milestone Notes\n\nDecision: We are standardizing on the 14B model tier due to VRAM limits.\n",
            None,
        )
        .unwrap();
    engine
        .attach_artifact(&project.id, &note.id, "user_explicit", None)
        .unwrap();

    make_topic(
        &engine,
        "t-plan",
        "Weekly Architecture Review",
        TopicType::Plan,
        TopicCadence::Weekly,
        Some(&project.id),
    );

    let artifact_id = execute_topic_run_with_provider(&engine, "t-plan", None)
        .await
        .expect("plan run should succeed");

    let artifact = engine.get_artifact(&artifact_id).unwrap();
    let title = artifact.title.clone().unwrap();

    assert!(
        title.starts_with("Plan Review: Weekly Architecture Review — "),
        "got {title}"
    );
    // Plan reviews are model-derived, never user-recorded truth.
    assert_eq!(artifact.authorship, Authorship::AiGenerated);

    // The review always reports progress, blockers and a next phase.
    assert!(artifact.content_md.contains("## Progress Assessment"));
    assert!(artifact.content_md.contains("## Blockers & Unresolved Dependencies"));
    assert!(artifact.content_md.contains("## Next Phase"));
    assert!(artifact.content_md.contains("Retrieval Systems"));

    {
        let reader = engine.db().reader().unwrap();
        let linked: i64 = reader
            .query_row(
                "SELECT COUNT(*) FROM project_artifacts
                 WHERE project_id = ?1 AND artifact_id = ?2 AND detached_at IS NULL;",
                rusqlite::params![project.id, artifact_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(linked, 1, "plan review must be attached to the project");

        let runs = repo::list_runs_for_topic(&reader, "t-plan").unwrap();
        assert_eq!(runs.len(), 1);
    }

    // A plan topic with no project still produces a review (unscoped workspace).
    make_topic(
        &engine,
        "t-plan-loose",
        "Global Review",
        TopicType::Plan,
        TopicCadence::Manual,
        None,
    );
    let loose_id = execute_topic_run_with_provider(&engine, "t-plan-loose", None)
        .await
        .unwrap();
    let loose = engine.get_artifact(&loose_id).unwrap();
    assert!(loose.content_md.contains("Workspace (unscoped)"));
}

// ---------------------------------------------------------------------------
// 6. Unknown topic is rejected
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_execute_unknown_topic_errors() {
    let (_tmp, engine, _clock) = setup();
    let err = execute_topic_run_with_provider(&engine, "nope", None).await;
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("not found"));
}


// ---------------------------------------------------------------------------
// 7. Phase 18.1: wall-clock scheduling ("run every day at 06:00")
// ---------------------------------------------------------------------------
#[tokio::test]
async fn test_daily_wall_clock_scheduling_fires_at_configured_time() {
    let (_tmp, engine, _clock) = setup();
    let base = engine.clock().now_ms();
    let six_am = 6 * 60;

    // Topic created at 05:00 asking to run at 06:00.
    let topic = make_topic_full(
        &engine,
        "t-daily-6",
        "Morning NVMe Digest",
        TopicType::Research,
        TopicCadence::Daily,
        None,
        at_time(base, 0, 5 * 60), // created 05:00 today
        Some(six_am),
        None,
    );

    // Before 06:00 -> not due yet.
    assert!(
        !topic.is_due(at_time(base, 0, 5 * 60 + 30)),
        "must not fire before its configured time"
    );
    // At/after 06:00 -> due.
    assert!(topic.is_due(at_time(base, 0, six_am)), "must fire at 06:00");
    assert!(
        topic.is_due(at_time(base, 0, 23 * 60)),
        "still due later the same day if it never ran"
    );

    // After running at 06:00 it must not fire again that day...
    let mut ran = topic.clone();
    ran.last_run_at = Some(at_time(base, 0, six_am));
    assert!(
        !ran.is_due(at_time(base, 0, 23 * 60)),
        "must not re-fire the same day after running"
    );
    // ...but must fire again at 06:00 the next day.
    assert!(
        ran.is_due(at_time(base, 1, six_am)),
        "must fire again the next day at 06:00"
    );
    assert!(
        !ran.is_due(at_time(base, 1, 5 * 60 + 59)),
        "must not fire before 06:00 the next day"
    );

    // A topic created *after* today's occurrence waits until tomorrow rather than
    // firing immediately (no surprise run on creation).
    let late = make_topic_full(
        &engine,
        "t-daily-late",
        "Late Creation",
        TopicType::Research,
        TopicCadence::Daily,
        None,
        at_time(base, 0, 10 * 60), // created 10:00
        Some(six_am),
        None,
    );
    assert!(
        !late.is_due(at_time(base, 0, 11 * 60)),
        "topic created after today's 06:00 must not fire immediately"
    );
    assert!(
        late.is_due(at_time(base, 1, six_am)),
        "topic created after today's 06:00 fires tomorrow at 06:00"
    );

    // next_run_at points at the upcoming occurrence.
    assert_eq!(
        late.next_run_at(at_time(base, 0, 11 * 60)),
        Some(at_time(base, 1, six_am))
    );
    assert_eq!(
        topic.next_run_at(at_time(base, 0, 5 * 60)),
        Some(at_time(base, 0, six_am))
    );
}


#[tokio::test]
async fn test_weekly_wall_clock_scheduling_anchors_on_weekday() {
    let (_tmp, engine, _clock) = setup();
    let base = engine.clock().now_ms();
    let six_am = 6 * 60;
    let day_ms = sthira_core::models::scheduled_topics::DAY_MS;

    // Anchor explicitly on Monday (ISO 0), created two weeks back so the window passed.
    let created = at_time(base, -14, 0);
    let topic = make_topic_full(
        &engine,
        "t-weekly-mon",
        "Weekly Review",
        TopicType::Plan,
        TopicCadence::Weekly,
        None,
        created,
        Some(six_am),
        Some(0), // Monday
    );

    // The most recent Monday-06:00 instant drives due-ness.
    let last_monday = sthira_core::most_recent_scheduled_instant(base, Some(0), six_am);
    assert_eq!(
        sthira_core::iso_weekday_of(last_monday),
        0,
        "computed instant must land on a Monday"
    );
    assert!(
        topic.is_due(base),
        "never-run weekly topic whose Monday window has passed is due"
    );

    // After running at that instant it is not due again until the next Monday.
    let mut ran = topic.clone();
    ran.last_run_at = Some(last_monday);
    assert!(!ran.is_due(base), "must not re-fire within the same week");
    assert!(
        ran.is_due(last_monday + 7 * day_ms),
        "must fire again the following Monday"
    );

    // The next run is always a Monday.
    let next = topic.next_run_at(base).unwrap();
    assert_eq!(sthira_core::iso_weekday_of(next), 0);
    assert!(next > base);

    // A weekly topic with no explicit weekday anchors on its creation weekday.
    let anchored = make_topic_full(
        &engine,
        "t-weekly-auto",
        "Auto-anchored",
        TopicType::Plan,
        TopicCadence::Weekly,
        None,
        created,
        Some(six_am),
        None,
    );
    assert_eq!(
        sthira_core::iso_weekday_of(anchored.next_run_at(base).unwrap()),
        sthira_core::iso_weekday_of(created),
        "default weekly anchor is the creation weekday"
    );
}

#[tokio::test]
async fn test_hhmm_parsing_and_formatting_roundtrip() {
    assert_eq!(sthira_core::parse_hhmm("06:00"), Some(360));
    assert_eq!(sthira_core::parse_hhmm("6:05"), Some(365));
    assert_eq!(sthira_core::parse_hhmm("23:59"), Some(1439));
    assert_eq!(sthira_core::parse_hhmm("00:00"), Some(0));

    // Out-of-range values are rejected rather than silently clamped.
    assert_eq!(sthira_core::parse_hhmm("24:00"), None);
    assert_eq!(sthira_core::parse_hhmm("12:60"), None);
    assert_eq!(sthira_core::parse_hhmm("nonsense"), None);
    assert_eq!(sthira_core::parse_hhmm("12"), None);

    assert_eq!(sthira_core::format_hhmm(360), "06:00");
    assert_eq!(sthira_core::format_hhmm(0), "00:00");
    assert_eq!(sthira_core::format_hhmm(1439), "23:59");

    for m in [0_i64, 1, 59, 60, 725, 1439] {
        assert_eq!(sthira_core::parse_hhmm(&sthira_core::format_hhmm(m)), Some(m));
    }
}

#[tokio::test]
async fn test_manual_and_paused_topics_ignore_wall_clock_time() {
    let (_tmp, engine, _clock) = setup();
    let base = engine.clock().now_ms();

    let manual = make_topic_full(
        &engine,
        "t-manual-6",
        "Manual Only",
        TopicType::Research,
        TopicCadence::Manual,
        None,
        at_time(base, -1, 0),
        Some(6 * 60),
        None,
    );
    assert!(!manual.is_due(base), "manual topics never fire autonomously");
    assert_eq!(manual.next_run_at(base), None, "manual topics have no next run");

    // Even with a time set, the scheduler must not pick it up.
    let due = list_due_topics(&engine, base).unwrap();
    assert!(due.iter().all(|t| t.id != "t-manual-6"));

    // Paused wall-clock topics are excluded too.
    make_topic_full(
        &engine,
        "t-paused-6",
        "Paused Daily",
        TopicType::Research,
        TopicCadence::Daily,
        None,
        at_time(base, -1, 0),
        Some(6 * 60),
        None,
    );
    {
        let writer = engine.db().writer();
        repo::update_topic_status(&writer, "t-paused-6", false).unwrap();
    }
    let due_after = list_due_topics(&engine, base).unwrap();
    assert!(due_after.iter().all(|t| t.id != "t-paused-6"));
}

#[tokio::test]
async fn test_wall_clock_topic_runs_end_to_end_and_stamps_last_run() {
    let (_tmp, engine, _clock) = setup();
    let base = engine.clock().now_ms();

    // Due because its 06:00 window already passed today and it was created yesterday.
    make_topic_full(
        &engine,
        "t-sched-run",
        "Scheduled NVMe",
        TopicType::Research,
        TopicCadence::Daily,
        None,
        at_time(base, -1, 0),
        Some(6 * 60),
        None,
    );

    let due = list_due_topics(&engine, base).unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].id, "t-sched-run");

    let provider = FakeSearchProvider::new();
    let artifact_id = execute_topic_run_with_provider(&engine, "t-sched-run", Some(&provider))
        .await
        .unwrap();

    {
        let reader = engine.db().reader().unwrap();
        let topic = repo::get_topic(&reader, "t-sched-run").unwrap().unwrap();
        assert!(topic.last_run_at.is_some());
        assert_eq!(topic.schedule_time_minutes, Some(6 * 60));
    }

    // After running, the topic is no longer due at the same instant.
    let due_after = list_due_topics(&engine, base).unwrap();
    assert!(due_after.iter().all(|t| t.id != "t-sched-run"));
    assert!(!engine.get_artifact(&artifact_id).unwrap().content_md.is_empty());
}

