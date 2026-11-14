//! Entry mode of `doc(action="create")`: `collection` names a lifecycle collection, and the call
//! makes an entry in it from the collection's schema and `## Entry template`.

use std::path::Path;

use chrono::{Days, NaiveDate};
use serde_json::{json, Value};
use tempfile::TempDir;

use super::create::{call, preflight};
use super::testfixture::repo_with_worktree;
use super::{TestToolContextBuilder, Tool as _, ToolContext};
use crate::librarian::catalog::{artifact, Catalog};
use crate::librarian::current_project::CurrentProject;
use crate::librarian::frontmatter;
use crate::librarian::workspace::Root;

const DIR: &str = "docs/trackers/ledgers/d";
const README: &str = "docs/trackers/ledgers/d/readme.md";
const TEMPLATE: &str = "What was decided, and why.";

/// A ledger `d`: one entry kind, a blocked word and a terminal word, one declared field and a
/// template. Keys are dated.
fn ledger() -> String {
    format!(
        "---\ntitle: Decisions\nentry_kinds: [decision]\n\
         statuses: {{open: open, waiting: blocked, done: terminal}}\ninitial_status: open\n\
         fields:\n  area: {{type: scalar}}\nreview_every: 30d\n---\n\n# Decisions\n\n\
         ## Entry template\n\n{TEMPLATE}\n"
    )
}

/// A ledger whose keys are bare names (`key_style: name`).
const NAMED_LEDGER: &str = "---\ntitle: Named\nentry_kinds: [decision]\nstatuses: {open: open, done: terminal}\ninitial_status: open\nkey_style: name\n---\n";

/// A stream readme.
const STREAM: &str = "---\ntitle: S\nentry_kinds: [f]\nstatuses: {open: open, done: terminal}\ninitial_status: open\nexit_when: it ships\n---\n";

fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

/// The key a dated collection gives the title `Pick X` today.
fn pick_x_key() -> String {
    format!("{}-pick-x", today())
}

fn pick_x_rel() -> String {
    format!("{DIR}/{}.md", pick_x_key())
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A temp directory that is the top level of a fresh git checkout, holding `files`.
fn checkout(files: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    git2::Repository::init(tmp.path()).unwrap();
    for (rel, text) in files {
        write(tmp.path(), rel, text);
    }
    tmp
}

fn mk_ctx(root: &Path) -> ToolContext {
    TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "r".into(),
            path: root.to_path_buf(),
        })
        .build()
}

fn artifact_count(ctx: &ToolContext) -> i64 {
    ctx.catalog
        .lock()
        .conn
        .query_row("SELECT count(*) FROM artifact", [], |r| r.get(0))
        .unwrap()
}

/// The files directly in the collection folder, sorted.
fn collection_files(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root.join(DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// The call nothing may have written for: the readme alone in the folder, and no row.
fn assert_nothing_written(root: &Path, ctx: &ToolContext) {
    assert_eq!(collection_files(root), ["readme.md"]);
    assert_eq!(artifact_count(ctx), 0);
}

fn pick_x(extra: Value) -> Value {
    json!({
        "repo": "r", "collection": "d", "kind": "decision", "title": "Pick X", "extra": extra
    })
}

async fn refusal(ctx: &ToolContext, args: Value) -> String {
    format!("{:#}", call(ctx, args).await.unwrap_err())
}

#[tokio::test]
async fn creates_an_entry_from_the_schema_and_template() {
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());

    let v = call(&ctx, pick_x(json!({}))).await.unwrap();

    let rel = pick_x_rel();
    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["abs_path", "id", "key"], "{v}");
    assert_eq!(v["key"], json!(pick_x_key()));
    assert!(
        v["abs_path"].as_str().unwrap().ends_with(&rel),
        "{v} should name {rel}"
    );
    let text = std::fs::read_to_string(tmp.path().join(&rel)).unwrap();
    let (fm, body) = frontmatter::parse(&text).unwrap();
    let fm = fm.unwrap();
    assert_eq!(fm.id, None, "an entry carries no id: {text}");
    assert!(!text.contains("\nid:"), "{text}");
    assert_eq!(fm.kind.as_deref(), Some("entry"));
    assert_eq!(fm.status.as_deref(), Some("open"), "the initial_status");
    assert_eq!(fm.title.as_deref(), Some("Pick X"));
    assert_eq!(fm.extra.get("entry_kind"), Some(&json!("decision")));
    assert_eq!(body.trim(), TEMPLATE);

    let id = v["id"].as_str().unwrap();
    let row = artifact::get(&ctx.catalog.lock(), id).unwrap().unwrap();
    assert_eq!(row.kind, "entry");
    assert_eq!(row.status, "open");
    assert_eq!(row.title.as_deref(), Some("Pick X"));
    let slug: Option<String> = ctx
        .catalog
        .lock()
        .conn
        .query_row("SELECT slug FROM artifact WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(slug.is_some(), "an entry row has a slug");
}

#[tokio::test]
async fn rel_path_and_collection_together_are_refused() {
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());
    let mut args = pick_x(json!({}));
    args["rel_path"] = json!(pick_x_rel());

    let text = refusal(&ctx, args.clone()).await;
    assert!(text.contains("rel_path"), "{text}");
    assert_eq!(text.matches("; ").count(), 1, "{text}");
    assert_nothing_written(tmp.path(), &ctx);

    args["action"] = json!("create");
    let text = format!("{:#}", preflight(&args).unwrap_err());
    assert!(text.contains("rel_path"), "preflight refuses it: {text}");
}

#[tokio::test]
async fn kind_collection_and_collection_together_are_refused() {
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());
    let mut args = pick_x(json!({}));
    args["kind"] = json!("collection");

    let text = refusal(&ctx, args.clone()).await;
    assert!(text.contains("\"collection\""), "{text}");
    assert_eq!(text.matches("; ").count(), 1, "{text}");
    assert_nothing_written(tmp.path(), &ctx);
    args["action"] = json!("create");
    assert!(preflight(&args).is_err());
}

#[tokio::test]
async fn a_blocked_entry_takes_revisit_by_from_extra() {
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());
    let date = today().checked_add_days(Days::new(7)).unwrap();
    let mut args = pick_x(json!({
        "revisit_by": date.to_string(), "valid": "dated 2026-11-14", "area": "core"
    }));
    args["status"] = json!("waiting");

    let v = call(&ctx, args).await.unwrap();

    let text = std::fs::read_to_string(tmp.path().join(pick_x_rel())).unwrap();
    assert!(text.contains("\nstatus: waiting\n"), "{text}");
    assert!(text.contains(&format!("\nrevisit_by: {date}\n")), "{text}");
    assert!(text.contains("\nvalid: dated 2026-11-14\n"), "{text}");
    assert!(text.contains("\narea: core\n"), "{text}");
    let row = artifact::get(&ctx.catalog.lock(), v["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(row.status, "waiting", "the entry's own status word");
}

#[tokio::test]
async fn a_past_revisit_by_is_refused() {
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());
    let date = today().checked_sub_days(Days::new(1)).unwrap();
    let mut args = pick_x(json!({"revisit_by": date.to_string()}));
    args["status"] = json!("waiting");

    let text = refusal(&ctx, args).await;

    assert!(text.contains("is before today"), "{text}");
    assert_eq!(text.matches("; ").count(), 1, "{text}");
    assert_nothing_written(tmp.path(), &ctx);
}

/// The second call is refused by the key check (the first file is listed), and, with the key
/// check blind to the first file (git ignores it), by the exclusive create. Either way the first
/// file, its row, its `created_at` and its `file_sha256` stay as they were.
#[tokio::test]
async fn a_second_create_of_the_same_title_on_the_same_day_is_refused_and_leaves_the_first() {
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());
    let first = call(&ctx, pick_x(json!({}))).await.unwrap();
    let id = first["id"].as_str().unwrap().to_string();
    let full = tmp.path().join(pick_x_rel());
    let bytes = std::fs::read(&full).unwrap();
    let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();

    let listed = refusal(&ctx, pick_x(json!({"area": "other"}))).await;
    assert!(listed.contains("path exists"), "{listed}");
    assert!(listed.contains(&pick_x_rel()), "{listed}");

    write(tmp.path(), ".gitignore", &format!("/{}\n", pick_x_rel()));
    let ignored = refusal(&ctx, pick_x(json!({"area": "other"}))).await;
    assert!(ignored.contains("path exists"), "{ignored}");

    assert_eq!(std::fs::read(&full).unwrap(), bytes);
    let after = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
    assert_eq!(after.created_at, row.created_at);
    assert_eq!(after.file_sha256, row.file_sha256);
    assert_eq!(after, row);
    assert_eq!(artifact_count(&ctx), 1);
}

#[tokio::test]
async fn a_key_equal_to_a_docs_issues_stem_is_refused() {
    let issue = format!("docs/issues/{}.md", pick_x_key());
    let tmp = checkout(&[(README, &ledger()), (&issue, "# BUG: x\n")]);
    let ctx = mk_ctx(tmp.path());

    let text = refusal(&ctx, pick_x(json!({}))).await;

    assert!(text.contains(&issue), "names the other path: {text}");
    assert!(text.contains(&pick_x_rel()), "names the new path: {text}");
    assert_eq!(text.matches("; ").count(), 1, "{text}");
    assert_nothing_written(tmp.path(), &ctx);
}

#[tokio::test]
async fn a_key_equal_to_another_collections_readme_folder_is_refused() {
    let other = "docs/trackers/streams/pick-x/readme.md";
    let tmp = checkout(&[(README, NAMED_LEDGER), (other, STREAM)]);
    let ctx = mk_ctx(tmp.path());

    let text = refusal(&ctx, pick_x(json!({}))).await;

    assert!(text.contains(other), "names the other path: {text}");
    assert!(text.contains(&format!("{DIR}/pick-x.md")), "{text}");
    assert_nothing_written(tmp.path(), &ctx);
}

#[cfg(unix)]
#[tokio::test]
async fn a_failed_file_write_undoes_the_row() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());
    let dir = tmp.path().join(DIR);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::write(dir.join("probe.md"), "x").is_ok() {
        // Running as a user the mode does not bind (root): nothing to measure.
        std::fs::remove_file(dir.join("probe.md")).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let result = call(&ctx, pick_x(json!({}))).await;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();

    let err = result.unwrap_err();
    assert!(
        format!("{err:#}").contains(&pick_x_key()),
        "names the file: {err:#}"
    );
    assert_nothing_written(tmp.path(), &ctx);
}

#[tokio::test]
async fn owners_topic_time_scope_and_augment_are_refused_in_entry_mode() {
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());
    for (key, value) in [
        ("owners", json!(["me"])),
        ("topic", json!("t")),
        ("time_scope", json!("2026-W46")),
        ("augment", json!({"prompt": "p"})),
    ] {
        let mut args = pick_x(json!({}));
        args[key] = value;
        let text = refusal(&ctx, args.clone()).await;
        assert!(text.contains(key), "{key}: {text}");
        assert_eq!(text.matches("; ").count(), 1, "{text}");
        args["action"] = json!("create");
        let pre = format!("{:#}", preflight(&args).unwrap_err());
        assert_eq!(pre, text, "preflight gives the same refusal");
    }
    assert_nothing_written(tmp.path(), &ctx);
}

/// A malformed entry call is refused by `preflight`, which runs before the worktree write guard,
/// so the caller gets the call's own error.
#[test]
fn a_malformed_entry_call_is_refused_by_preflight_with_its_own_error() {
    for (args, says) in [
        (
            json!({"action": "create", "collection": "d", "title": "T"}),
            "`kind`",
        ),
        (
            json!({"action": "create", "collection": "d", "kind": "decision"}),
            "`title`",
        ),
        (
            json!({"action": "create", "collection": "d", "kind": "decision", "title": "T",
                   "extra": {"revisit_by": "next week"}}),
            "revisit_by",
        ),
        (
            json!({"action": "create", "collection": "d", "kind": "decision", "title": "T",
                   "extra": {"valid": 3}}),
            "valid",
        ),
    ] {
        let text = format!("{:#}", preflight(&args).unwrap_err());
        assert!(text.contains(says), "{args} -> {text}");
        assert!(!text.contains("requires 'rel_path'"), "{args} -> {text}");
        assert_eq!(text.matches("; ").count(), 1, "{args} -> {text}");
    }
}

#[tokio::test]
async fn an_unknown_collection_names_the_remedy() {
    let tmp = checkout(&[(README, &ledger())]);
    let ctx = mk_ctx(tmp.path());
    let mut args = pick_x(json!({}));
    args["collection"] = json!("nope");

    let text = refusal(&ctx, args).await;

    assert!(text.contains("\"nope\""), "{text}");
    assert!(
        text.contains("pass the key of an existing collection"),
        "{text}"
    );
    assert_nothing_written(tmp.path(), &ctx);
}

#[tokio::test]
async fn a_requirements_toml_rule_is_enforced_on_entry_create() {
    let tmp = checkout(&[
        (README, &ledger()),
        (
            ".codescout/requirements.toml",
            "[[require]]\nkind = \"entry\"\nsections = [\"Decision\"]\nmin_section_chars = 40\n",
        ),
    ]);
    let ctx = mk_ctx(tmp.path());

    let text = refusal(&ctx, pick_x(json!({}))).await;

    assert!(text.contains("## Decision"), "{text}");
    assert_nothing_written(tmp.path(), &ctx);
}

/// The readme exists only in the worktree's checkout: the session reads the schema there and
/// writes the entry there.
#[tokio::test]
async fn creates_in_a_worktree_session_from_the_worktrees_schema() {
    let (_tmp, main, worktree) = repo_with_worktree();
    write(&worktree, README, &ledger());
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "r".into(),
            path: main.clone(),
        })
        .with_current_project(std::sync::Arc::new(CurrentProject {
            abs_path: worktree.clone(),
            git_root: worktree.clone(),
            main_root: Some(main.clone()),
            umbrella: None,
        }))
        .build();

    let v = call(
        &ctx,
        json!({"collection": "d", "kind": "decision", "title": "Pick X"}),
    )
    .await
    .unwrap();

    assert!(worktree.join(pick_x_rel()).is_file());
    assert!(!main.join(pick_x_rel()).exists());
    assert!(!main.join(DIR).exists());
    let abs = v["abs_path"].as_str().unwrap();
    assert!(
        abs.starts_with(&crate::util::fs::to_forward_slash(&worktree)),
        "{abs}"
    );
}

/// The key check lists the checkout from its top level; a project root below it is refused
/// before anything is written.
#[tokio::test]
async fn a_root_below_the_git_top_level_is_refused() {
    let tmp = checkout(&[]);
    let sub = tmp.path().join("sub");
    write(&sub, README, &ledger());
    let ctx = mk_ctx(&sub);

    let text = refusal(&ctx, pick_x(json!({}))).await;

    assert!(text.contains("top level"), "{text}");
    assert_eq!(text.matches("; ").count(), 1, "{text}");
    assert_nothing_written(&sub, &ctx);
}

/// `doc`'s advertised surface takes `collection` for `create` and is no larger than before:
/// the characters the new key costs come out of the description. The budget gate itself is
/// `server::tests::tool_surface_under_budget`.
#[test]
fn the_advertised_surface_is_net_neutral() {
    let tool = super::artifact::Artifact;
    let schema = tool.input_schema();
    let surface = tool.description().chars().count() + schema.to_string().chars().count();
    println!("doc surface: {surface}");
    let collection = &schema["properties"]["collection"];
    assert_eq!(collection["type"], json!("string"));
    let desc = collection["description"].as_str().unwrap_or_default();
    assert!(desc.starts_with("create: "), "{desc}");
    assert!(
        surface <= DOC_SURFACE_BEFORE_COLLECTION,
        "doc's description + schema grew: {surface} > {DOC_SURFACE_BEFORE_COLLECTION}"
    );
}

/// `doc`'s description plus its serialised schema without `collection`, measured at 0e7c82334.
const DOC_SURFACE_BEFORE_COLLECTION: usize = 19_512;
