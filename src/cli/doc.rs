//! `codescout doc <verb>` — find/get/graph/state-at/create/update/move/link/event/augment/refresh.

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
    /// Move an artifact to a new path.
    Move(MoveArgs),
    /// Add a typed edge between two artifacts.
    Link(LinkArgs),
    /// Event log: `doc event list|create`.
    Event {
        #[command(subcommand)]
        verb: super::doc_event::Verb,
    },
    /// Attach or merge an augmentation.
    Augment(super::doc_augment::AugmentArgs),
    /// Refresh lifecycle: `doc refresh gather|list-stale`.
    Refresh {
        #[command(subcommand)]
        verb: super::doc_refresh::Verb,
    },
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
    #[command(flatten)]
    pub common: CommonOpts,
}

impl FindArgs {
    pub fn common(&self) -> CommonOpts {
        self.common.clone()
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
        Verb::Move(args) => run_move(args).await,
        Verb::Link(args) => run_link(args).await,
        Verb::Event { verb } => super::doc_event::dispatch(verb).await,
        Verb::Augment(args) => super::doc_augment::run(args).await,
        Verb::Refresh { verb } => super::doc_refresh::dispatch(verb).await,
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

    let v = crate::librarian::tools::find::call(&ctx, Value::Object(tool_args)).await?;
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
    #[command(flatten)]
    pub common: CommonOpts,
}

impl GetArgs {
    pub fn common(&self) -> CommonOpts {
        self.common.clone()
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

    let v = crate::librarian::tools::get::call(&ctx, Value::Object(tool_args)).await?;
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
    #[command(flatten)]
    pub common: CommonOpts,
}

impl GraphArgs {
    pub fn common(&self) -> CommonOpts {
        self.common.clone()
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
    let v = crate::librarian::tools::graph::call(&ctx, Value::Object(tool_args)).await?;
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
    #[command(flatten)]
    pub common: CommonOpts,
}

impl StateAtArgs {
    pub fn common(&self) -> CommonOpts {
        self.common.clone()
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
    tool_args.insert("id".into(), Value::String(args.id.clone()));
    if let Some(c) = &args.commit {
        tool_args.insert("commit".into(), Value::String(c.clone()));
    }
    if let Some(t) = args.timestamp {
        tool_args.insert("timestamp".into(), Value::Number(t.into()));
    }
    let v = crate::librarian::tools::state_at::call(&ctx, Value::Object(tool_args)).await?;
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
    /// Temporal scope tag written to frontmatter and the catalog (e.g.
    /// `2026-W25`, a date, or `dated_snapshot`). Filterable via `artifact find`.
    #[arg(long = "time-scope")]
    pub time_scope: Option<String>,
    /// Custom frontmatter keys as a JSON object (`@<file>`, `-`, or literal).
    ///
    /// Distinct from `--augment-params`, which seeds AUGMENTATION params.
    /// `extra` is plain YAML frontmatter — where `unverified:`, `opened:` and
    /// `entry_prefix` live.
    #[arg(long)]
    pub extra: Option<String>,
    #[command(flatten)]
    pub common: CommonOpts,
}

impl CreateArgs {
    pub fn common(&self) -> CommonOpts {
        self.common.clone()
    }
}

/// Translate `CreateArgs` into the `artifact create` tool's JSON arguments.
///
/// Split out of [`run_create`] for the reason [`build_update_tool_args`] gives
/// on the update side, and this bug is that reason arriving twice: the CLI holds
/// its own clap struct and hand-marshals every field, so a field can exist on
/// the struct and never reach the tool. The failure is silent — the tool
/// defaults the missing key and reports `created` — and it is only testable if
/// the translation is reachable without a catalog.
/// See `docs/issues/archive/2026-08-30-cli-artifact-drops-time-scope-and-extra.md`.
fn build_create_tool_args(args: &CreateArgs) -> Result<Value> {
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
    if let Some(t) = &args.time_scope {
        tool_args.insert("time_scope".into(), Value::String(t.clone()));
    }
    if let Some(e) = &args.extra {
        let raw = crate::cli::read_at_or_stdin(e)?;
        let parsed: Value = serde_json::from_str(&raw).context("--extra is not valid JSON")?;
        tool_args.insert("extra".into(), parsed);
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
    Ok(Value::Object(tool_args))
}

pub(crate) async fn run_create(args: CreateArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let tool_args = build_create_tool_args(&args)?;

    let v = crate::librarian::tools::create::call(&ctx, tool_args).await?;
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
    /// Bypass the body-shrink guard. Required when a body write would cut the
    /// file by more than 50% in EITHER bytes or lines. Use only when the
    /// shrinkage is intentional (e.g. archiving stale sections, full rewrite).
    ///
    /// Mirrors the MCP tool's `force` param — the guard refusal's hint names
    /// `force=true`, so the CLI has to offer the escape the hint promises.
    #[arg(long)]
    pub force: bool,
    /// Temporal scope tag written to frontmatter and the catalog (e.g.
    /// `2026-W25`, a date, or `dated_snapshot`). Filterable via `artifact find`.
    #[arg(long = "time-scope")]
    pub time_scope: Option<String>,
    /// Custom frontmatter keys as a JSON object (`@<file>`, `-`, or literal).
    ///
    /// Distinct from `--patch-params`, which merge-patches AUGMENTATION params.
    /// `extra` is plain YAML frontmatter — where `unverified:`, `closed:` and
    /// `entry_prefix` live. Each key is upserted; a `null` value deletes it.
    #[arg(long)]
    pub extra: Option<String>,
    #[command(flatten)]
    pub common: CommonOpts,
}

impl UpdateArgs {
    pub fn common(&self) -> CommonOpts {
        self.common.clone()
    }
}

/// Translate `UpdateArgs` into the `artifact update` tool's JSON arguments.
///
/// Split out of [`run_update`] for the same reason [`compile_filter`] is split
/// out of [`run_find`]: the CLI holds its own clap struct and hand-marshals
/// every field, so a field can exist on the struct and never reach the tool.
/// That failure mode is silent — the tool defaults the missing key and reports
/// success — and it is only testable if the translation is reachable without a
/// catalog. See `docs/issues/archive/2026-08-30-cli-artifact-update-has-no-force-escape-for-the-shrink-guard.md`.
fn build_update_tool_args(args: &UpdateArgs) -> Result<Value> {
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
    if let Some(t) = &args.time_scope {
        patch.insert("time_scope".into(), Value::String(t.clone()));
    }
    if let Some(e) = &args.extra {
        let raw = crate::cli::read_at_or_stdin(e)?;
        let parsed: Value = serde_json::from_str(&raw).context("--extra is not valid JSON")?;
        patch.insert("extra".into(), parsed);
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
    if args.force {
        tool_args.insert("force".into(), Value::Bool(true));
    }
    Ok(Value::Object(tool_args))
}

pub(crate) async fn run_update(args: UpdateArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;

    let tool_args = build_update_tool_args(&args)?;

    let v = crate::librarian::tools::update::call(&ctx, tool_args).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

#[derive(Debug, clap::Args)]
pub struct MoveArgs {
    /// Artifact id.
    pub id: String,
    /// Destination path relative to repo root.
    #[arg(long = "new-rel-path")]
    pub new_rel_path: String,
    #[command(flatten)]
    pub common: CommonOpts,
}

impl MoveArgs {
    pub fn common(&self) -> CommonOpts {
        self.common.clone()
    }
}

pub(crate) async fn run_move(args: MoveArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let tool_args = serde_json::json!({
        "id": args.id,
        "new_rel_path": args.new_rel_path,
    });
    let v = crate::librarian::tools::mv::call(&ctx, tool_args).await?;
    crate::cli::format::print(&v, &output)?;
    Ok(())
}

#[derive(Debug, clap::Args)]
pub struct LinkArgs {
    /// Source artifact id.
    #[arg(long)]
    pub src: String,
    /// Destination artifact id.
    #[arg(long)]
    pub dst: String,
    /// Relation type (e.g. supersedes, implements, child).
    #[arg(long)]
    pub rel: String,
    #[command(flatten)]
    pub common: CommonOpts,
}

impl LinkArgs {
    pub fn common(&self) -> CommonOpts {
        self.common.clone()
    }
}

pub(crate) async fn run_link(args: LinkArgs) -> Result<()> {
    let common = args.common();
    let output = common.output();
    let ctx = open_ctx(&common).await?;
    let tool_args = serde_json::json!({
        "src_id": args.src,
        "dst_id": args.dst,
        "rel": args.rel,
    });
    let v = crate::librarian::tools::link::call(&ctx, tool_args).await?;
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
            common: CommonOpts::default(),
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
            common: CommonOpts {
                project: Some(std::path::PathBuf::from("/tmp/proj")),
                json: true,
                no_color: true,
            },
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
            common: CommonOpts::default(),
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
            common: CommonOpts::default(),
        };
        let err = run_state_at(args).await.unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("--commit"), "got: {msg}");
        assert!(msg.contains("--timestamp"), "got: {msg}");
    }

    // --- `artifact update --force` ------------------------------------------
    //
    // The CLI keeps its own clap struct and hand-marshals each field into the
    // tool's JSON, so `--force` can break in two independent ways: the parser
    // can reject it, or the parser can accept it and the marshalling can drop
    // it. The second is the dangerous one — the tool defaults `force` to false
    // and still reports `updated: true`, so a dropped flag looks like a working
    // flag until a guard refusal contradicts it. One test per half.
    //
    // That `force: true` actually bypasses the guard is pinned tool-side by
    // `librarian::tools::update::tests::body_shrink_guard_allows_with_force`;
    // these close the CLI's half of the path.

    fn update_args(id: &str) -> UpdateArgs {
        UpdateArgs {
            id: id.into(),
            title: None,
            status: None,
            owners: None,
            tags: None,
            topic: None,
            body: None,
            patch_params: None,
            commit_refresh: false,
            force: false,
            time_scope: None,
            extra: None,
            common: CommonOpts::default(),
        }
    }

    fn create_args() -> CreateArgs {
        CreateArgs {
            kind: "bug".into(),
            title: "T".into(),
            rel_path: "docs/issues/x.md".into(),
            repo: None,
            status: None,
            owners: None,
            tags: None,
            topic: None,
            body: None,
            augment_prompt: None,
            augment_params: None,
            time_scope: None,
            extra: None,
            common: CommonOpts::default(),
        }
    }

    #[test]
    fn update_parser_accepts_force_flag() {
        use clap::Parser;

        #[derive(Parser)]
        struct Wrap {
            #[command(flatten)]
            args: UpdateArgs,
        }

        let w = Wrap::try_parse_from(["codescout", "abc123", "--force"])
            .expect("`--force` must be a recognised argument");
        assert!(w.args.force);

        // The control: without the flag it stays off, so the assertion above is
        // about the flag and not about a field that is always true.
        let w = Wrap::try_parse_from(["codescout", "abc123"]).unwrap();
        assert!(!w.args.force);
    }

    #[test]
    fn update_force_flag_reaches_the_tool_args() {
        let mut a = update_args("abc123");
        a.force = true;
        let v = build_update_tool_args(&a).unwrap();
        assert_eq!(
            v.get("force"),
            Some(&Value::Bool(true)),
            "`--force` must be marshalled as a TOP-LEVEL key, sibling to `patch` \
             — the tool reads `a.force`, not `patch.force`; got: {v}"
        );
    }

    #[test]
    fn update_omits_force_when_unset() {
        let v = build_update_tool_args(&update_args("abc123")).unwrap();
        assert!(
            v.get("force").is_none(),
            "an unset `--force` must not be sent at all — marshalling it \
             unconditionally would disable the shrink guard for every CLI \
             update; got: {v}"
        );
    }

    // `--time-scope` / `--extra`, both subcommands.
    //
    // `docs/issues/archive/2026-08-30-cli-artifact-drops-time-scope-and-extra.md`. Same
    // two-independent-halves shape as `--force` above: the parser can reject the
    // flag, or accept it and the marshalling can drop it. The second half is the
    // one that bit — before this, neither flag existed at all, so a CLI caller
    // could not set either field and got no signal that they were unreachable.
    //
    // These matter more than their surface suggests: `extra` is where
    // `unverified:`, `opened:`, `closed:` and `entry_prefix` live, and
    // `get_guide("tracker-conventions")` builds its whole queryability argument
    // on those being reachable. A write path that drops them defeats the
    // convention at the moment of writing.

    #[test]
    fn update_parser_accepts_time_scope_and_extra() {
        use clap::Parser;

        #[derive(Parser)]
        struct Wrap {
            #[command(flatten)]
            args: UpdateArgs,
        }

        let w = Wrap::try_parse_from([
            "codescout",
            "abc123",
            "--time-scope",
            "2026-W25",
            "--extra",
            r#"{"unverified":"not re-checked since the rebuild"}"#,
        ])
        .expect("`--time-scope` and `--extra` must be recognised arguments");
        assert_eq!(w.args.time_scope.as_deref(), Some("2026-W25"));
        assert!(w.args.extra.is_some());

        // Control: absent flags stay None, so the assertions above are about the
        // flags rather than about fields that are always populated.
        let w = Wrap::try_parse_from(["codescout", "abc123"]).unwrap();
        assert!(w.args.time_scope.is_none() && w.args.extra.is_none());
    }

    #[test]
    fn update_time_scope_and_extra_reach_the_patch() {
        let mut a = update_args("abc123");
        a.time_scope = Some("2026-W25".into());
        a.extra = Some(r#"{"closed":"2026-08-30","legacy":null}"#.into());
        let v = build_update_tool_args(&a).unwrap();
        let patch = v.get("patch").expect("patch object");

        assert_eq!(
            patch.get("time_scope"),
            Some(&Value::String("2026-W25".into())),
            "`--time-scope` must land INSIDE `patch` — the tool reads \
             `patch.time_scope`, not a top-level key; got: {v}"
        );
        // Parsed as JSON, not passed through as a string: the tool expects an
        // object, and a string here would be accepted-and-wrong rather than
        // rejected. `null` must survive, because that is how `extra` deletes a key.
        assert_eq!(
            patch.get("extra"),
            Some(&serde_json::json!({"closed": "2026-08-30", "legacy": null})),
            "`--extra` must be parsed into an object with nulls preserved; got: {v}"
        );
    }

    #[test]
    fn update_omits_time_scope_and_extra_when_unset() {
        let v = build_update_tool_args(&update_args("abc123")).unwrap();
        let patch = v.get("patch").expect("patch object");
        assert!(
            patch.get("time_scope").is_none() && patch.get("extra").is_none(),
            "unset flags must not be sent at all — marshalling `extra` \
             unconditionally would send an empty object, and the tool upserts \
             every key it is given; got: {v}"
        );
    }

    #[test]
    fn update_rejects_extra_that_is_not_json() {
        let mut a = update_args("abc123");
        a.extra = Some("closed: 2026-08-30".into());
        let err = build_update_tool_args(&a)
            .expect_err("non-JSON `--extra` must fail loudly, not be dropped");
        assert!(
            err.to_string().contains("--extra"),
            "the error must name the flag the caller got wrong: {err}"
        );
    }

    #[test]
    fn create_parser_accepts_time_scope_and_extra() {
        use clap::Parser;

        #[derive(Parser)]
        struct Wrap {
            #[command(flatten)]
            args: CreateArgs,
        }

        let w = Wrap::try_parse_from([
            "codescout",
            "--kind",
            "bug",
            "--title",
            "T",
            "--rel-path",
            "docs/issues/x.md",
            "--time-scope",
            "dated_snapshot",
            "--extra",
            r#"{"opened":"2026-08-30"}"#,
        ])
        .expect("`--time-scope` and `--extra` must be recognised on create too");
        assert_eq!(w.args.time_scope.as_deref(), Some("dated_snapshot"));
        assert!(w.args.extra.is_some());
    }

    #[test]
    fn create_time_scope_and_extra_reach_the_tool_args() {
        let mut a = create_args();
        a.time_scope = Some("dated_snapshot".into());
        a.extra = Some(r#"{"opened":"2026-08-30","severity":"low"}"#.into());
        let v = build_create_tool_args(&a).unwrap();

        // Create takes both as TOP-LEVEL keys, unlike update which nests them in
        // `patch`. Asserting the level is the point: a value marshalled to the
        // wrong depth is silently defaulted by the tool, which is this bug.
        assert_eq!(
            v.get("time_scope"),
            Some(&Value::String("dated_snapshot".into())),
            "`--time-scope` must be a top-level key on create; got: {v}"
        );
        assert_eq!(
            v.get("extra"),
            Some(&serde_json::json!({"opened": "2026-08-30", "severity": "low"})),
            "`--extra` must be a parsed top-level object on create; got: {v}"
        );
    }

    #[test]
    fn create_omits_time_scope_and_extra_when_unset() {
        let v = build_create_tool_args(&create_args()).unwrap();
        assert!(
            v.get("time_scope").is_none() && v.get("extra").is_none(),
            "unset flags must not be sent at all; got: {v}"
        );
    }

    #[test]
    fn create_still_marshals_every_pre_existing_field() {
        // Characterization guard for the `build_create_tool_args` extraction.
        // The refactor moved ~60 lines out of `run_create`; this pins that the
        // move was behaviour-preserving rather than merely compiling, which is
        // the failure mode the extraction exists to make testable in the first
        // place.
        let mut a = create_args();
        a.repo = Some("codescout".into());
        a.status = Some("open".into());
        a.owners = Some("marius, other".into());
        a.tags = Some("cli, librarian".into());
        a.topic = Some("parity".into());
        a.augment_prompt = Some("keep it current".into());

        let v = build_create_tool_args(&a).unwrap();
        assert_eq!(v.get("kind"), Some(&Value::String("bug".into())));
        assert_eq!(v.get("title"), Some(&Value::String("T".into())));
        assert_eq!(
            v.get("rel_path"),
            Some(&Value::String("docs/issues/x.md".into()))
        );
        assert_eq!(v.get("repo"), Some(&Value::String("codescout".into())));
        assert_eq!(v.get("status"), Some(&Value::String("open".into())));
        // Comma-split with surrounding whitespace trimmed.
        assert_eq!(
            v.get("owners"),
            Some(&serde_json::json!(["marius", "other"]))
        );
        assert_eq!(
            v.get("tags"),
            Some(&serde_json::json!(["cli", "librarian"]))
        );
        assert_eq!(v.get("topic"), Some(&Value::String("parity".into())));
        // Body is defaulted to empty rather than omitted — the server requires it.
        assert_eq!(v.get("body"), Some(&Value::String(String::new())));
        assert_eq!(
            v.get("augment").and_then(|a| a.get("prompt")),
            Some(&Value::String("keep it current".into())),
            "augment.prompt must survive the extraction; got: {v}"
        );
    }
}
