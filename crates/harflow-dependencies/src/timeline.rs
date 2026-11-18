use chrono::DateTime;
use harflow_normalizer::{NormalizedEntry, NormalizedSession};

/// A normalized entry positioned on a single session's chronological timeline.
#[derive(Debug, Clone)]
pub struct TimelineEntry<'a> {
    pub session_id: &'a str,
    pub order_index: usize,
    pub timestamp_ms: Option<i64>,
    pub entry: &'a NormalizedEntry,
}

/// Strict chronological sequence of entries isolated to a single `NormalizedSession`.
#[derive(Debug, Clone)]
pub struct SessionTimeline<'a> {
    pub session_id: &'a str,
    pub entries: Vec<TimelineEntry<'a>>,
}

/// Builds per-session chronological timelines without mixing entries across distinct sessions.
#[must_use]
pub fn build_session_timelines(sessions: &[NormalizedSession]) -> Vec<SessionTimeline<'_>> {
    sessions
        .iter()
        .map(|session| {
            let mut last_ts: Option<i64> = None;
            let mut keyed: Vec<(Option<i64>, usize, TimelineEntry<'_>)> = session
                .entries
                .iter()
                .enumerate()
                .map(|(idx, entry)| {
                    let parsed = parse_rfc3339_millis(&entry.started_date_time);
                    let effective_ts = parsed.or(last_ts);
                    if parsed.is_some() {
                        last_ts = parsed;
                    }
                    (
                        effective_ts,
                        idx,
                        TimelineEntry {
                            session_id: session.session_id.as_str(),
                            order_index: idx,
                            timestamp_ms: parsed,
                            entry,
                        },
                    )
                })
                .collect();

            keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

            SessionTimeline {
                session_id: session.session_id.as_str(),
                entries: keyed.into_iter().map(|(_, _, item)| item).collect(),
            }
        })
        .collect()
}

fn parse_rfc3339_millis(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(trimmed)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

#[cfg(test)]
mod tests {
    use harflow_normalizer::{NormalizedEntry, NormalizedSession};

    use super::*;

    fn entry(id: &str, started: &str) -> NormalizedEntry {
        NormalizedEntry {
            id: id.into(),
            session_id: "sess-0001".into(),
            started_date_time: started.into(),
            method: "GET".into(),
            host: "api.example.test".into(),
            path: "/v1/items".into(),
            response_status: 200,
            ..NormalizedEntry::default()
        }
    }

    #[test]
    fn sorts_within_session_chronologically_and_isolates_sessions() {
        let s1 = NormalizedSession {
            session_id: "sess-a".into(),
            source_name: "a.har".into(),
            har_version: "1.2".into(),
            entries: vec![
                entry("late", "2026-11-18T10:00:05.000Z"),
                entry("early", "2026-11-18T10:00:01.000Z"),
                entry("middle_no_ts", ""),
            ],
            redacted_secrets_count: 0,
        };
        let s2 = NormalizedSession {
            session_id: "sess-b".into(),
            source_name: "b.har".into(),
            har_version: "1.2".into(),
            entries: vec![entry("b1", "2026-11-18T09:00:00.000Z")],
            redacted_secrets_count: 0,
        };

        let sessions = [s1, s2];
        let timelines = build_session_timelines(&sessions);
        assert_eq!(timelines.len(), 2);
        let ids_a: Vec<&str> = timelines[0]
            .entries
            .iter()
            .map(|e| e.entry.id.as_str())
            .collect();
        // middle_no_ts inherits early's timestamp (10:00:01) with higher order_index, placing it after early and before late.
        assert_eq!(ids_a, vec!["early", "middle_no_ts", "late"]);
        assert_eq!(timelines[1].session_id, "sess-b");
        assert_eq!(timelines[1].entries[0].entry.id, "b1");
    }
}
