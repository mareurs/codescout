use super::super::routes::DashboardState;
use serde_json::{json, Value};

/// Shared "check usage.db exists -> open it -> run a stats query -> shape a
/// `{available, ...}` JSON response" skeleton behind `get_lsp`/`get_usage`.
/// `label` distinguishes log lines; `no_data_reason` stays per-caller since
/// the two routes use different user-facing wording.
pub fn usage_stats_response<T, F>(
    state: &DashboardState,
    no_data_reason: &str,
    label: &str,
    window: &str,
    query: F,
) -> Value
where
    T: serde::Serialize,
    F: FnOnce(&rusqlite::Connection, &str) -> anyhow::Result<T>,
{
    let db_path = state.project_root.join(".codescout").join("usage.db");
    if !db_path.exists() {
        return json!({
            "available": false,
            "reason": no_data_reason
        });
    }

    let conn = match crate::usage::db::open_db(&state.project_root) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "dashboard", "usage db open failed: {e}");
            return json!({
                "available": false,
                "reason": "Failed to open usage DB."
            });
        }
    };

    match query(&conn, window) {
        Ok(stats) => {
            let mut val = serde_json::to_value(stats).unwrap_or_else(|e| {
                tracing::error!(target: "dashboard", "{label} stats serialize failed: {e}");
                Value::Null
            });
            val["available"] = json!(true);
            val
        }
        Err(e) => {
            tracing::warn!(target: "dashboard", "{label} stats query failed: {e}");
            json!({
                "available": false,
                "reason": "Query failed."
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn reports_unavailable_when_db_missing() {
        let dir = TempDir::new().unwrap();
        let state = DashboardState {
            project_root: dir.path().to_path_buf(),
        };
        let val = usage_stats_response(
            &state,
            "no data yet",
            "test",
            "30d",
            |_conn: &rusqlite::Connection, _window: &str| -> anyhow::Result<Value> {
                Ok(json!({}))
            },
        );
        assert_eq!(val["available"], false);
        assert_eq!(val["reason"], "no data yet");
    }

    #[test]
    fn shapes_available_true_around_query_result_on_existing_db() {
        let dir = TempDir::new().unwrap();
        crate::usage::db::open_db(dir.path()).unwrap();
        let state = DashboardState {
            project_root: dir.path().to_path_buf(),
        };
        let val = usage_stats_response(
            &state,
            "no data yet",
            "test",
            "7d",
            |_conn: &rusqlite::Connection, window: &str| -> anyhow::Result<Value> {
                Ok(json!({ "window": window, "count": 3 }))
            },
        );
        assert_eq!(val["available"], true);
        assert_eq!(val["window"], "7d");
        assert_eq!(val["count"], 3);
    }

    #[test]
    fn reports_unavailable_on_query_failure() {
        let dir = TempDir::new().unwrap();
        crate::usage::db::open_db(dir.path()).unwrap();
        let state = DashboardState {
            project_root: dir.path().to_path_buf(),
        };
        let val = usage_stats_response(
            &state,
            "no data yet",
            "test",
            "30d",
            |_conn: &rusqlite::Connection, _window: &str| -> anyhow::Result<Value> {
                anyhow::bail!("boom")
            },
        );
        assert_eq!(val["available"], false);
        assert_eq!(val["reason"], "Query failed.");
    }
}
