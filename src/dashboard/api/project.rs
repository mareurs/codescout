use super::super::routes::DashboardState;
use crate::config::project::ProjectConfig;
use crate::util::fs::to_forward_slash;
use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

pub async fn get_project_info(State(state): State<DashboardState>) -> Json<Value> {
    let root = &state.project_root;
    let config = ProjectConfig::load_or_default(root)
        .unwrap_or_else(|_| ProjectConfig::default_for("unknown".to_string()));
    let name = config.project.name.clone();

    // Detect languages by scanning file extensions
    let languages = detect_languages(root);

    // Git info
    let (git_branch, git_dirty) = git_info(root);

    Json(json!({
        "name": name,
        "root": to_forward_slash(root),
        "languages": languages,
        "git_branch": git_branch,
        "git_dirty": git_dirty,
    }))
}

fn detect_languages(root: &std::path::Path) -> Vec<String> {
    let mut langs = std::collections::BTreeSet::new();
    for entry in walkdir::WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .take(500)
    {
        if let Some(lang) = crate::ast::detect_language(entry.path()) {
            langs.insert(lang.to_string());
        }
    }
    langs.into_iter().collect()
}

fn git_info(root: &std::path::Path) -> (Option<String>, bool) {
    match crate::git::open_repo(root) {
        Ok(repo) => {
            let branch = repo
                .head()
                .ok()
                .and_then(|h| h.shorthand().map(String::from));
            let dirty = repo.statuses(None).map(|s| !s.is_empty()).unwrap_or(false);
            (branch, dirty)
        }
        Err(_) => (None, false),
    }
}
