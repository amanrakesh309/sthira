use serde::{Deserialize, Serialize};

/// Phase 18: Whether a scheduled topic performs grounded web research or a
/// project-progress planning review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TopicType {
    Research,
    Plan,
}

impl TopicType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TopicType::Research => "research",
            TopicType::Plan => "plan",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.to_lowercase().as_str() {
            "plan" => TopicType::Plan,
            _ => TopicType::Research,
        }
    }
}

/// Phase 18: Autonomous execution cadence. `Manual` topics only ever run via
/// an explicit `[ Run Now ]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TopicCadence {
    Daily,
    Weekly,
    Manual,
}

impl TopicCadence {
    pub fn as_str(&self) -> &'static str {
        match self {
            TopicCadence::Daily => "daily",
            TopicCadence::Weekly => "weekly",
            TopicCadence::Manual => "manual",
        }
    }

    pub fn from_str_lossy(raw: &str) -> Self {
        match raw.to_lowercase().as_str() {
            "weekly" => TopicCadence::Weekly,
            "manual" => TopicCadence::Manual,
            _ => TopicCadence::Daily,
        }
    }

    /// Milliseconds after which a topic of this cadence is considered due.
    /// `Manual` returns `None` — it is never picked up by the scheduler.
    pub fn due_interval_ms(&self) -> Option<i64> {
        match self {
            TopicCadence::Daily => Some(24 * 60 * 60 * 1000),
            TopicCadence::Weekly => Some(7 * 24 * 60 * 60 * 1000),
            TopicCadence::Manual => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledTopic {
    pub id: String,
    pub title: String,
    pub prompt_query: String,
    pub topic_type: TopicType,
    pub project_id: Option<String>,
    pub cadence: TopicCadence,
    pub model_id: Option<String>,
    pub search_provider_id: Option<String>,
    pub is_active: bool,
    pub last_run_at: Option<i64>,
    pub created_at: i64,
    /// Phase 18.1: minutes since midnight (UTC), 0..1439. `None` = rolling interval.
    pub schedule_time_minutes: Option<i64>,
    /// Phase 18.1: ISO weekday anchor for `weekly` topics (0=Monday .. 6=Sunday).
    pub schedule_weekday: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduledTopicRun {
    pub id: String,
    pub topic_id: String,
    pub artifact_id: String,
    pub sources_scraped_count: i64,
    pub executed_at: i64,
}

const MINUTE_MS: i64 = 60 * 1000;
pub const DAY_MS: i64 = 24 * 60 * MINUTE_MS;

/// Formats minutes-since-midnight as zero-padded `"HH:MM"` (e.g. `360` -> `"06:00"`).
pub fn format_hhmm(minutes: i64) -> String {
    let m = minutes.clamp(0, 1439);
    format!("{:02}:{:02}", m / 60, m % 60)
}

/// Parses `"HH:MM"` (or `"H:MM"`) into minutes-since-midnight. Returns `None` for
/// anything outside `00:00..=23:59`.
pub fn parse_hhmm(raw: &str) -> Option<i64> {
    let (h, m) = raw.trim().split_once(':')?;
    let hours: i64 = h.trim().parse().ok()?;
    let mins: i64 = m.trim().parse().ok()?;
    if !(0..=23).contains(&hours) || !(0..=59).contains(&mins) {
        return None;
    }
    Some(hours * 60 + mins)
}

/// ISO weekday of an epoch-ms instant: `0 = Monday .. 6 = Sunday`.
///
/// 1970-01-01 was a Thursday, so `(days + 3).rem_euclid(7)` maps correctly.
pub fn iso_weekday_of(ms: i64) -> u8 {
    ((ms.div_euclid(DAY_MS) + 3).rem_euclid(7)) as u8
}

/// The most recent occurrence of a wall-clock moment at or before `now_ms`.
///
/// * `weekday = None` -> a *daily* moment: today at `minutes`, or yesterday if
///   today's time has not arrived yet.
/// * `weekday = Some(d)` -> a *weekly* moment: the most recent day `d` at `minutes`.
///
/// All arithmetic is UTC, matching `format_date_utc` used for digest titles.
pub fn most_recent_scheduled_instant(now_ms: i64, weekday: Option<u8>, minutes: i64) -> i64 {
    let minutes = minutes.clamp(0, 1439);
    let days = now_ms.div_euclid(DAY_MS);
    let today_at_time = days * DAY_MS + minutes * MINUTE_MS;

    match weekday {
        None => {
            if today_at_time <= now_ms {
                today_at_time
            } else {
                today_at_time - DAY_MS
            }
        }
        Some(target) => {
            let today_weekday = iso_weekday_of(now_ms) as i64;
            // Days to step *back* to reach `target` from today.
            let back = (today_weekday - target as i64).rem_euclid(7);
            let candidate = today_at_time - back * DAY_MS;
            if candidate <= now_ms {
                candidate
            } else {
                candidate - 7 * DAY_MS
            }
        }
    }
}

/// The next occurrence of a wall-clock moment strictly after `now_ms`.
pub fn next_scheduled_instant(now_ms: i64, weekday: Option<u8>, minutes: i64) -> i64 {
    let minutes = minutes.clamp(0, 1439);
    let days = now_ms.div_euclid(DAY_MS);
    let today_at_time = days * DAY_MS + minutes * MINUTE_MS;

    match weekday {
        None => {
            if today_at_time > now_ms {
                today_at_time
            } else {
                today_at_time + DAY_MS
            }
        }
        Some(target) => {
            let today_weekday = iso_weekday_of(now_ms) as i64;
            let forward = (target as i64 - today_weekday).rem_euclid(7);
            let candidate = today_at_time + forward * DAY_MS;
            if candidate > now_ms {
                candidate
            } else {
                candidate + 7 * DAY_MS
            }
        }
    }
}


impl ScheduledTopic {
    /// Returns `true` when this topic should execute at `now_ms`.
    ///
    /// Two scheduling modes are supported:
    ///
    /// 1. **Wall-clock** (`schedule_time_minutes` set) — the topic fires for the most
    ///    recent occurrence of its configured time-of-day, but only if that occurrence
    ///    happened *after the topic was created* and *after its last run*. A topic
    ///    created at 10:00 asking for `06:00` therefore waits until tomorrow rather
    ///    than firing immediately.
    /// 2. **Rolling interval** (`schedule_time_minutes` = `None`) — the legacy
    ///    behaviour: never-run topics are due, otherwise `now - last_run` must exceed
    ///    the cadence interval (24h / 7d).
    ///
    /// `manual` cadence is never autonomously due.
    pub fn is_due(&self, now_ms: i64) -> bool {
        if !self.is_active {
            return false;
        }
        let Some(interval) = self.cadence.due_interval_ms() else {
            return false; // manual
        };

        match self.schedule_time_minutes {
            None => match self.last_run_at {
                None => true,
                Some(last) => now_ms.saturating_sub(last) > interval,
            },
            Some(minutes) => {
                let instant = most_recent_scheduled_instant(now_ms, self.schedule_anchor_weekday(), minutes);
                // Never fire for an occurrence that predates the topic itself.
                if self.created_at > instant {
                    return false;
                }
                match self.last_run_at {
                    None => true,
                    Some(last) => last < instant,
                }
            }
        }
    }

    /// The next wall-clock instant this topic is expected to fire, if it has one.
    pub fn next_run_at(&self, now_ms: i64) -> Option<i64> {
        let minutes = self.schedule_time_minutes?;
        if !self.is_active || self.cadence == TopicCadence::Manual {
            return None;
        }
        Some(next_scheduled_instant(
            now_ms,
            self.schedule_anchor_weekday(),
            minutes,
        ))
    }

    /// Weekly topics anchor on their configured weekday, defaulting to the weekday the
    /// topic was created. Daily topics have no weekday anchor.
    fn schedule_anchor_weekday(&self) -> Option<u8> {
        if self.cadence == TopicCadence::Weekly {
            Some(
                self.schedule_weekday
                    .unwrap_or_else(|| iso_weekday_of(self.created_at)),
            )
        } else {
            None
        }
    }
}

/// Convenience predicate mirroring the rolling-interval half of
/// [`ScheduledTopic::is_due`], for callers that only have the individual fields.
pub fn is_topic_due(cadence: TopicCadence, last_run_at: Option<i64>, now_ms: i64) -> bool {
    match cadence.due_interval_ms() {
        None => false,
        Some(interval) => match last_run_at {
            None => true,
            Some(last) => now_ms.saturating_sub(last) > interval,
        },
    }
}

