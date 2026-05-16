//! `codescout artifact <verb>` — find/get/graph/state-at/create/update/move/link.

use anyhow::{anyhow, Context, Result};
use clap::{Args, Subcommand};
use serde_json::{json, Value};

use crate::cli::{open_ctx, CommonOpts};

#[derive(Debug, Subcommand)]
pub enum Verb {
    /// Find artifacts by filter / tag / kind / semantic query.
    Find(FindArgs),
    /// Read one artifact by id.
    Get(GetArgs),
    /// BFS neighbourhood around an artifact.
    Graph(GraphArgs),
    /// Snapshot an artifact at a past commit or timestamp.
    #[command(name = "state-at")]
    StateAt(StateAtArgs),
    /// Create a new artifact.
    Create(CreateArgs),
    /// Update an existing artifact.
    Update(UpdateArgs),
}

#[derive(Debug, Args)]
pub struct FindArgs {
    /// kind=eq filter, e.g. "tracker"
    #[arg(long)]
    pub kind: Option<String>,
    /// repeatable; each → {"tags":{"contains":<tag>}}
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    /// status=eq filter; disables archived-hide default
    #[arg(long)]
    pub status: Option<String>,
    /// owner=eq filter (owners contains <owner>)
    #[arg(long)]
    pub owner: Option<String>,
    /// topic LIKE %<value>%
    #[arg(long = "has-topic")]
    pub has_topic: Option<String>,
    /// Raw FilterNode JSON; AND-merged with shortcuts.
    #[arg(long)]
    pub filter: Option<String>,
    /// Natural-language semantic search. Requires LIBRARIAN_EMBED_MODEL env.
    #[arg(long)]
    pub semantic: Option<String>,
    /// project|repo|umbrella|all
    #[arg(long, default_value = "project")]
    pub scope: String,
    /// Include archived/superseded by default.
    #[arg(long = "include-archived")]
    pub include_archived: bool,
    /// Filter to augmented (true) or non-augmented (false) artifacts.
    #[arg(long)]
    pub augmented: Option<bool>,
    /// Max results to return.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    /// Pagination offset.
    #[arg(long, default_value_t = 0)]
    pub offset: usize,
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl FindArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}

/// Compile shortcuts + raw --filter into a single FilterNode `Value`.
///
/// A composite raw `--filter` (e.g. `{"and":[...]}`) is pushed in as a single
/// leaf alongside any shortcut leaves; the result keeps the nested structure
/// rather than flattening. `FilterNode::compile_sql` handles nested `and`
/// correctly, so this is by design — do not "fix" by flattening.
pub(crate) fn compile_filter(args: &FindArgs) -> Result<Option<Value>> {
    let mut leaves: Vec<Value> = Vec::new();
    if let Some(k) = &args.kind {
        leaves.push(json!({"kind": {"eq": k}}));
    }
    if let Some(s) = &args.status {
        leaves.push(json!({"status": {"eq": s}}));
    }
    if let Some(o) = &args.owner {
        leaves.push(json!({"owners": {"contains": o}}));
    }
    if let Some(t) = &args.has_topic {
        leaves.push(json!({"topic": {"contains": t}}));
    }
    for tag in &args.tag {
        leaves.push(json!({"tags": {"contains": tag}}));
    }
    if let Some(raw) = &args.filter {
        let parsed: Value = serde_json::from_str(raw)
            .with_context(|| format!("--filter is not valid JSON: {raw}"))?;
        leaves.push(parsed);
    }
    Ok(match leaves.len() {
        0 => None,
        1 => Some(leaves.pop().unwrap()),
        _ => Some(json!({"and": leaves})),
    })
}

pub async fn dispatch(verb: Verb) -> Result<()> {
    match verb {
        Verb::Find(args) => run_find(args).await,
        Verb::Get(args) => run_get(args).await,
        Verb::Graph(args) => run_graph(args).await,
        Verb::StateAt(args) => run_state_at(args).await,
        Verb::Create(args) => run_create(args).await,
        Verb::Update(args) => run_update(args).await,
    }
}

pub(crate) async fn run_find(args: FindArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();

    if args.semantic.is_some() && std::env::var("LIBRARIAN_EMBED_MODEL").is_err() {
        return Err(anyhow!(
            "--semantic requires the embedding service. Set LIBRARIAN_EMBED_MODEL \
             (and optionally LIBRARIAN_EMBED_URL, LIBRARIAN_EMBED_API_KEY) and re-run."
        ));
    }

    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    if let Some(f) = compile_filter(&args)? {
        tool_args.insert("filter".into(), f);
    }
    if let Some(s) = &args.semantic {
        tool_args.insert("semantic".into(), Value::String(s.clone()));
    }
    tool_args.insert("scope".into(), Value::String(args.scope.clone()));
    tool_args.insert(
        "include_archived".into(),
        Value::Bool(args.include_archived),
    );
    if let Some(a) = args.augmented {
        tool_args.insert("augmented".into(), Value::Bool(a));
    }
    tool_args.insert("limit".into(), Value::Number(args.limit.into()));
    tool_args.insert("offset".into(), Value::Number(args.offset.into()));

    let v = librarian_mcp::tools::find::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

#[derive(Debug, clap::Args)]
pub struct GetArgs {
    /// Artifact id.
    pub id: String,
    /// Include the full body.
    #[arg(long)]
    pub full: bool,
    /// Fetch a specific section by heading.
    #[arg(long)]
    pub heading: Option<String>,
    /// 1-indexed start of line slice.
    #[arg(long = "start-line")]
    pub start_line: Option<usize>,
    /// 1-indexed inclusive end of line slice.
    #[arg(long = "end-line")]
    pub end_line: Option<usize>,
    /// Include link edges in the response.
    #[arg(long = "include-links")]
    pub include_links: bool,
    /// Filter links by direction (in|out|both).
    #[arg(long = "links-direction")]
    pub links_direction: Option<String>,
    /// Filter links to this rel type.
    #[arg(long = "links-rel")]
    pub links_rel: Option<String>,
    /// Include observations in the response.
    #[arg(long = "include-observations")]
    pub include_observations: bool,
    /// Include events in the response.
    #[arg(long = "include-events")]
    pub include_events: bool,
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl GetArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}

pub(crate) async fn run_get(args: GetArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    tool_args.insert("id".into(), Value::String(args.id.clone()));
    tool_args.insert("full".into(), Value::Bool(args.full));
    if let Some(h) = &args.heading {
        tool_args.insert("heading".into(), Value::String(h.clone()));
    }
    if let Some(s) = args.start_line {
        tool_args.insert("start_line".into(), Value::Number(s.into()));
    }
    if let Some(e) = args.end_line {
        tool_args.insert("end_line".into(), Value::Number(e.into()));
    }
    if args.include_links {
        tool_args.insert("include_links".into(), Value::Bool(true));
    }
    if let Some(d) = &args.links_direction {
        tool_args.insert("links_direction".into(), Value::String(d.clone()));
    }
    if let Some(r) = &args.links_rel {
        tool_args.insert("links_rel".into(), Value::String(r.clone()));
    }
    if args.include_observations {
        tool_args.insert("include_observations".into(), Value::Bool(true));
    }
    if args.include_events {
        tool_args.insert("include_events".into(), Value::Bool(true));
    }

    let v = librarian_mcp::tools::get::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

#[derive(Debug, clap::Args)]
pub struct GraphArgs {
    /// Artifact id.
    pub id: String,
    /// BFS depth (1..=3).
    #[arg(long, default_value_t = 1)]
    pub depth: u8,
    /// Comma-separated list of rel types to include (e.g. "supersedes,implements").
    #[arg(long)]
    pub rels: Option<String>,
    /// Include event/source nodes via event edges.
    #[arg(long = "include-events")]
    pub include_events: bool,
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl GraphArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}

pub(crate) async fn run_graph(args: GraphArgs) -> Result<()> {
    if !(1..=3).contains(&args.depth) {
        return Err(anyhow!("--depth must be in 1..=3 (got {})", args.depth));
    }
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    tool_args.insert("id".into(), Value::String(args.id.clone()));
    tool_args.insert("depth".into(), Value::Number(args.depth.into()));
    if let Some(r) = &args.rels {
        let list: Vec<Value> = r
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| Value::String(s.trim().to_string()))
            .collect();
        tool_args.insert("rels".into(), Value::Array(list));
    }
    if args.include_events {
        tool_args.insert("include_events".into(), Value::Bool(true));
    }
    let v = librarian_mcp::tools::graph::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

#[derive(Debug, clap::Args)]
pub struct StateAtArgs {
    /// Artifact id.
    pub id: String,
    /// Git commit hash to time-travel to. Mutually exclusive with --timestamp.
    #[arg(long, conflicts_with = "timestamp")]
    pub commit: Option<String>,
    /// Unix epoch ms to time-travel to. Mutually exclusive with --commit.
    #[arg(long, conflicts_with = "commit")]
    pub timestamp: Option<i64>,
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl StateAtArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}

pub(crate) async fn run_state_at(args: StateAtArgs) -> Result<()> {
    if args.commit.is_none() && args.timestamp.is_none() {
        return Err(anyhow!(
            "state-at requires exactly one of --commit <sha> or --timestamp <ms>"
        ));
    }
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let mut tool_args = serde_json::Map::new();
    tool_args.insert("artifact_id".into(), Value::String(args.id.clone()));
    if let Some(c) = &args.commit {
        tool_args.insert("commit".into(), Value::String(c.clone()));
    }
    if let Some(t) = args.timestamp {
        tool_args.insert("timestamp".into(), Value::Number(t.into()));
    }
    let v = librarian_mcp::tools::state_at::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

#[derive(Debug, clap::Args)]
pub struct CreateArgs {
    /// Artifact kind (e.g. spec, plan, tracker, adr).
    #[arg(long)]
    pub kind: String,
    /// Human-readable title.
    #[arg(long)]
    pub title: String,
    /// Relative path for the new file (e.g. docs/specs/foo.md).
    #[arg(long = "rel-path")]
    pub rel_path: String,
    /// Workspace root name (git repo basename); omit to infer from active project.
    #[arg(long)]
    pub repo: Option<String>,
    /// Initial status.
    #[arg(long)]
    pub status: Option<String>,
    /// Comma-separated owner list.
    #[arg(long)]
    pub owners: Option<String>,
    /// Comma-separated tag list.
    #[arg(long)]
    pub tags: Option<String>,
    /// Topic keyword for search.
    #[arg(long)]
    pub topic: Option<String>,
    /// Body content: `@<file>` reads from file, `-` reads stdin, else literal string.
    #[arg(long)]
    pub body: Option<String>,
    /// Persistent augmentation prompt (or `@<file>` / `-`).
    #[arg(long = "augment-prompt")]
    pub augment_prompt: Option<String>,
    /// Augmentation params JSON (`@<file>` / `-` / literal JSON string).
    #[arg(long = "augment-params")]
    pub augment_params: Option<String>,
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl CreateArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}

pub(crate) async fn run_create(args: CreateArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    tool_args.insert("kind".into(), Value::String(args.kind.clone()));
    tool_args.insert("title".into(), Value::String(args.title.clone()));
    tool_args.insert("rel_path".into(), Value::String(args.rel_path.clone()));
    if let Some(r) = &args.repo {
        tool_args.insert("repo".into(), Value::String(r.clone()));
    }
    if let Some(s) = &args.status {
        tool_args.insert("status".into(), Value::String(s.clone()));
    }
    if let Some(o) = &args.owners {
        let list: Vec<Value> = o
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| Value::String(s.trim().into()))
            .collect();
        tool_args.insert("owners".into(), Value::Array(list));
    }
    if let Some(t) = &args.tags {
        let list: Vec<Value> = t
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| Value::String(s.trim().into()))
            .collect();
        tool_args.insert("tags".into(), Value::Array(list));
    }
    if let Some(t) = &args.topic {
        tool_args.insert("topic".into(), Value::String(t.clone()));
    }
    if let Some(b) = &args.body {
        tool_args.insert(
            "body".into(),
            Value::String(crate::cli::read_at_or_stdin(b)?),
        );
    } else {
        // Server requires a body field; default to empty when caller omits --body.
        tool_args.insert("body".into(), Value::String(String::new()));
    }
    if args.augment_prompt.is_some() || args.augment_params.is_some() {
        let mut aug = serde_json::Map::new();
        if let Some(p) = &args.augment_prompt {
            aug.insert(
                "prompt".into(),
                Value::String(crate::cli::read_at_or_stdin(p)?),
            );
        }
        if let Some(params) = &args.augment_params {
            let raw = crate::cli::read_at_or_stdin(params)?;
            let parsed: Value =
                serde_json::from_str(&raw).context("--augment-params is not valid JSON")?;
            aug.insert("params".into(), parsed);
        }
        tool_args.insert("augment".into(), Value::Object(aug));
    }

    let v = librarian_mcp::tools::create::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    /// Artifact id.
    pub id: String,
    /// New title.
    #[arg(long)]
    pub title: Option<String>,
    /// New status.
    #[arg(long)]
    pub status: Option<String>,
    /// Comma-separated owner list (replaces existing list).
    #[arg(long)]
    pub owners: Option<String>,
    /// Comma-separated tag list (replaces existing list).
    #[arg(long)]
    pub tags: Option<String>,
    /// New topic.
    #[arg(long)]
    pub topic: Option<String>,
    /// Body content: `@<file>`, `-`, or literal.
    #[arg(long)]
    pub body: Option<String>,
    /// RFC 7396 merge-patch on augmentation params (`@<file>`, `-`, or literal JSON).
    #[arg(long = "patch-params")]
    pub patch_params: Option<String>,
    /// Record a completed refresh cycle atomically.
    #[arg(long = "commit-refresh")]
    pub commit_refresh: bool,
    /// Optional project root override (defaults to cwd).
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Emit JSON to stdout.
    #[arg(long)]
    pub json: bool,
    /// Force no color (also implicit when stdout is not a TTY).
    #[arg(long = "no-color")]
    pub no_color: bool,
}

impl UpdateArgs {
    pub fn common(&self) -> CommonOpts {
        CommonOpts {
            project: self.project.clone(),
            json: self.json,
            no_color: self.no_color,
        }
    }
}

pub(crate) async fn run_update(args: UpdateArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let mut tool_args = serde_json::Map::new();
    tool_args.insert("id".into(), Value::String(args.id.clone()));

    let mut patch = serde_json::Map::new();
    if let Some(t) = &args.title {
        patch.insert("title".into(), Value::String(t.clone()));
    }
    if let Some(s) = &args.status {
        patch.insert("status".into(), Value::String(s.clone()));
    }
    if let Some(o) = &args.owners {
        let list: Vec<Value> = o
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| Value::String(s.trim().into()))
            .collect();
        patch.insert("owners".into(), Value::Array(list));
    }
    if let Some(t) = &args.tags {
        let list: Vec<Value> = t
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| Value::String(s.trim().into()))
            .collect();
        patch.insert("tags".into(), Value::Array(list));
    }
    if let Some(t) = &args.topic {
        patch.insert("topic".into(), Value::String(t.clone()));
    }
    if let Some(b) = &args.body {
        patch.insert(
            "body".into(),
            Value::String(crate::cli::read_at_or_stdin(b)?),
        );
    }
    if let Some(pp) = &args.patch_params {
        let raw = crate::cli::read_at_or_stdin(pp)?;
        let parsed: Value =
            serde_json::from_str(&raw).context("--patch-params is not valid JSON")?;
        patch.insert("params".into(), parsed);
    }
    tool_args.insert("patch".into(), Value::Object(patch));
    if args.commit_refresh {
        tool_args.insert("commit_refresh".into(), Value::Bool(true));
    }

    let v = librarian_mcp::tools::update::call(&ctx, Value::Object(tool_args)).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_with_tag(tags: &[&str]) -> FindArgs {
        FindArgs {
            kind: None,
            tag: tags.iter().map(|s| s.to_string()).collect(),
            status: None,
            owner: None,
            has_topic: None,
            filter: None,
            semantic: None,
            scope: "project".into(),
            include_archived: false,
            augmented: None,
            limit: 50,
            offset: 0,
            project: None,
            json: false,
            no_color: false,
        }
    }

    #[test]
    fn compile_filter_single_tag_yields_leaf() {
        let a = args_with_tag(&["goal"]);
        let f = compile_filter(&a).unwrap().unwrap();
        assert_eq!(f, json!({"tags": {"contains": "goal"}}));
    }

    #[test]
    fn compile_filter_two_tags_and_joined() {
        let a = args_with_tag(&["goal", "p1"]);
        let f = compile_filter(&a).unwrap().unwrap();
        assert_eq!(
            f,
            json!({"and": [
                {"tags": {"contains": "goal"}},
                {"tags": {"contains": "p1"}}
            ]})
        );
    }

    #[test]
    fn compile_filter_kind_status_tag_combined() {
        let mut a = args_with_tag(&["goal"]);
        a.kind = Some("tracker".into());
        a.status = Some("active".into());
        let f = compile_filter(&a).unwrap().unwrap();
        assert_eq!(
            f,
            json!({"and": [
                {"kind": {"eq": "tracker"}},
                {"status": {"eq": "active"}},
                {"tags": {"contains": "goal"}}
            ]})
        );
    }

    #[test]
    fn compile_filter_raw_filter_parses_and_joins() {
        let mut a = args_with_tag(&[]);
        a.filter = Some(r#"{"kind":{"eq":"spec"}}"#.into());
        let f = compile_filter(&a).unwrap().unwrap();
        assert_eq!(f, json!({"kind": {"eq": "spec"}}));
    }

    #[test]
    fn compile_filter_raw_filter_bad_json_errors() {
        let mut a = args_with_tag(&[]);
        a.filter = Some("{not json".into());
        let err = compile_filter(&a).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("--filter is not valid JSON"));
    }

    #[test]
    fn compile_filter_none_when_no_shortcuts_or_filter() {
        let a = args_with_tag(&[]);
        assert!(compile_filter(&a).unwrap().is_none());
    }

    #[test]
    fn get_args_common_carries_project_json_no_color() {
        let a = GetArgs {
            id: "abc".into(),
            full: false,
            heading: None,
            start_line: None,
            end_line: None,
            include_links: false,
            links_direction: None,
            links_rel: None,
            include_observations: false,
            include_events: false,
            project: Some(std::path::PathBuf::from("/tmp/proj")),
            json: true,
            no_color: true,
        };
        let c = a.common();
        assert_eq!(c.project, Some(std::path::PathBuf::from("/tmp/proj")));
        assert!(c.json);
        assert!(c.no_color);
    }

    #[tokio::test]
    async fn run_graph_rejects_depth_zero() {
        let args = GraphArgs {
            id: "abc".into(),
            depth: 0,
            rels: None,
            include_events: false,
            project: None,
            json: false,
            no_color: false,
        };
        let err = run_graph(args).await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("--depth must be in 1..=3"), "got: {msg}");
    }

    #[tokio::test]
    async fn run_state_at_rejects_missing_cutoff() {
        let args = StateAtArgs {
            id: "abc".into(),
            commit: None,
            timestamp: None,
            project: None,
            json: false,
            no_color: false,
        };
        let err = run_state_at(args).await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("--commit"), "got: {msg}");
        assert!(msg.contains("--timestamp"), "got: {msg}");
    }
}
