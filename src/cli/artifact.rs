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
}
