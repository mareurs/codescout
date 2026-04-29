use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Freshness {
    Fresh,
    Unknown,
    Stale,
    Superseded,
}

#[derive(Debug, Clone)]
pub struct FreshnessInputs<'a> {
    /// Newest event on the artifact (any kind), or None if no events.
    pub latest_event_kind: Option<&'a str>,
    /// Newest 'reviewed' event's created_at, or None if no reviewed event yet.
    pub latest_reviewed_at: Option<i64>,
    /// File mtime in ms epoch.
    pub file_updated_at: i64,
    /// Topo distance from HEAD to the latest reviewed event's head_commit.
    /// None = unknown (commits not indexed yet); treated as "within horizon".
    pub topo_distance_from_head: Option<i64>,
    /// Configured horizon (commits).
    pub freshness_horizon: i64,
}

pub fn compute(input: FreshnessInputs<'_>) -> Freshness {
    if input.latest_event_kind == Some("superseded_by") {
        return Freshness::Superseded;
    }
    let Some(reviewed_at) = input.latest_reviewed_at else {
        return Freshness::Unknown;
    };
    if input.file_updated_at > reviewed_at {
        return Freshness::Stale;
    }
    if let Some(d) = input.topo_distance_from_head {
        if d > input.freshness_horizon {
            return Freshness::Stale;
        }
    }
    Freshness::Fresh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> FreshnessInputs<'static> {
        FreshnessInputs {
            latest_event_kind: Some("reviewed"),
            latest_reviewed_at: Some(100),
            file_updated_at: 50,
            topo_distance_from_head: Some(0),
            freshness_horizon: 50,
        }
    }

    #[test]
    fn superseded_short_circuits() {
        let mut i = base();
        i.latest_event_kind = Some("superseded_by");
        assert_eq!(compute(i), Freshness::Superseded);
    }

    #[test]
    fn unknown_when_no_reviewed() {
        let mut i = base();
        i.latest_reviewed_at = None;
        assert_eq!(compute(i), Freshness::Unknown);
    }

    #[test]
    fn stale_when_file_newer() {
        let mut i = base();
        i.file_updated_at = 200;
        assert_eq!(compute(i), Freshness::Stale);
    }

    #[test]
    fn stale_beyond_horizon() {
        let mut i = base();
        i.topo_distance_from_head = Some(100);
        assert_eq!(compute(i), Freshness::Stale);
    }

    #[test]
    fn fresh_within_horizon() {
        assert_eq!(compute(base()), Freshness::Fresh);
    }
}
