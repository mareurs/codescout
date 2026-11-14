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

// ---------------------------------------------------------------------------------------------
// Collection mode: `kind: "collection"` and a `rel_path` make a collection readme.
// ---------------------------------------------------------------------------------------------

const NEW_DIR: &str = "docs/trackers/ledgers/frictions";
const NEW_README: &str = "docs/trackers/ledgers/frictions/readme.md";

/// The schema keys of a valid ledger.
fn ledger_schema() -> Value {
    json!({
        "entry_kinds": ["friction"],
        "statuses": {"open": "open", "done": "terminal"},
        "initial_status": "open"
    })
}

/// A collection create at `rel_path` with `extra` as the schema keys.
fn collection_at(rel_path: &str, extra: Value) -> Value {
    json!({
        "repo": "r", "kind": "collection", "rel_path": rel_path, "title": "Frictions",
        "extra": extra, "body": "What goes wrong while we work.\n"
    })
}

fn new_ledger(extra: Value) -> Value {
    collection_at(NEW_README, extra)
}

/// A refused collection call wrote nothing: none of `dirs` exists and the catalog has no row.
fn assert_no_collection_written(root: &Path, ctx: &ToolContext, dirs: &[&str]) {
    for dir in dirs {
        assert!(!root.join(dir).exists(), "{dir} was made");
    }
    assert_eq!(artifact_count(ctx), 0);
}

/// One `<problem>; <remedy>` text: exactly one `; `.
fn assert_one_remedy(text: &str) {
    assert_eq!(text.matches("; ").count(), 1, "{text}");
}

#[tokio::test]
async fn creates_a_ledger_collection_that_load_collection_reads() {
    use crate::librarian::lifecycle::collection::load_collection_dir;
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());

    let v = call(&ctx, new_ledger(ledger_schema())).await.unwrap();

    let mut keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["abs_path", "id", "key"], "{v}");
    assert_eq!(v["key"], json!("frictions"));
    assert!(v["abs_path"].as_str().unwrap().ends_with(NEW_README), "{v}");
    let loaded = load_collection_dir(tmp.path(), NEW_DIR).unwrap();
    assert_eq!(loaded.key.as_str(), "frictions");
    assert_eq!(loaded.schema().entry_kinds, ["friction"]);
    let row = artifact::get(&ctx.catalog.lock(), v["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(row.kind, "collection");
    assert_eq!(row.status, "active", "P-R8");
    assert_eq!(row.title.as_deref(), Some("Frictions"));

    // The collection takes an entry at once, by its key.
    let entry = call(
        &ctx,
        json!({"repo": "r", "collection": "frictions", "kind": "friction", "title": "Slow gate"}),
    )
    .await
    .unwrap();
    assert!(entry["abs_path"].as_str().unwrap().contains(NEW_DIR));
}

#[tokio::test]
async fn the_readme_has_kind_collection_and_no_id() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());

    call(&ctx, new_ledger(ledger_schema())).await.unwrap();

    let text = std::fs::read_to_string(tmp.path().join(NEW_README)).unwrap();
    let (fm, body) = frontmatter::parse(&text).unwrap();
    let fm = fm.unwrap();
    assert_eq!(fm.kind.as_deref(), Some("collection"), "{text}");
    assert_eq!(fm.id, None, "{text}");
    assert!(!text.contains("\nid:"), "{text}");
    assert_eq!(fm.title.as_deref(), Some("Frictions"));
    assert!(body.contains("What goes wrong while we work."), "{body}");
}

#[tokio::test]
async fn a_stream_without_exit_when_is_refused_and_writes_nothing() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    let dir = "docs/trackers/streams/campaign";

    let text = refusal(
        &ctx,
        collection_at(&format!("{dir}/readme.md"), ledger_schema()),
    )
    .await;

    assert!(text.contains("exit_when"), "{text}");
    assert_one_remedy(&text);
    assert_no_collection_written(tmp.path(), &ctx, &[dir]);

    // The same schema with its exit condition is a stream.
    let mut schema = ledger_schema();
    schema["exit_when"] = json!("the campaign ships");
    call(&ctx, collection_at(&format!("{dir}/readme.md"), schema))
        .await
        .unwrap();
}

#[tokio::test]
async fn an_unknown_status_class_is_refused() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    let mut schema = ledger_schema();
    schema["statuses"] = json!({"open": "open", "done": "terminal", "odd": "finished"});

    let text = refusal(&ctx, new_ledger(schema)).await;

    assert!(text.contains("finished"), "{text}");
    assert_one_remedy(&text);
    assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
}

#[tokio::test]
async fn several_schema_problems_are_joined_into_one_text() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    let mut schema = ledger_schema();
    schema["statuses"] = json!({"open": "open", "odd": "finished"});
    schema["review_evry"] = json!("30d");

    let text = refusal(&ctx, new_ledger(schema)).await;

    assert!(text.contains("review_evry"), "{text}");
    assert!(text.contains("finished"), "{text}");
    assert!(text.contains(" | "), "{text}");
    assert_one_remedy(&text);
    assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
}

#[tokio::test]
async fn a_nested_path_is_refused() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    for rel in [
        "docs/trackers/ledgers/frictions/sub/readme.md",
        "docs/trackers/ledgers/frictions/notes.md",
        "docs/trackers/ledgers/frictions/README.md",
    ] {
        let text = refusal(&ctx, collection_at(rel, ledger_schema())).await;
        assert!(text.contains(rel), "{text}");
        assert_one_remedy(&text);
        assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
    }
}

#[tokio::test]
async fn an_archive_path_is_refused() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    for rel in [
        "docs/trackers/ledgers/archive/frictions/readme.md",
        "docs/trackers/ledgers/archive/readme.md",
    ] {
        let text = refusal(&ctx, collection_at(rel, ledger_schema())).await;
        assert!(text.contains("archive"), "{text}");
        assert_one_remedy(&text);
        assert_no_collection_written(
            tmp.path(),
            &ctx,
            &["docs/trackers/ledgers/archive", NEW_DIR],
        );
    }
}

#[tokio::test]
async fn a_folder_that_already_holds_files_is_refused() {
    let notes = format!("{NEW_DIR}/notes.md");
    let tmp = checkout(&[(&notes, "# Notes\n")]);
    let ctx = mk_ctx(tmp.path());

    let text = refusal(&ctx, new_ledger(ledger_schema())).await;

    assert!(text.contains(NEW_DIR), "{text}");
    assert_one_remedy(&text);
    assert!(!tmp.path().join(NEW_README).exists());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(&notes)).unwrap(),
        "# Notes\n"
    );
    assert_eq!(artifact_count(&ctx), 0);
}

#[tokio::test]
async fn an_empty_existing_folder_takes_the_collection() {
    let tmp = checkout(&[]);
    std::fs::create_dir_all(tmp.path().join(NEW_DIR)).unwrap();
    let ctx = mk_ctx(tmp.path());

    call(&ctx, new_ledger(ledger_schema())).await.unwrap();

    assert!(tmp.path().join(NEW_README).is_file());
}

#[tokio::test]
async fn a_path_outside_the_lifecycle_folders_is_refused() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    for (rel, dir) in [
        (
            "docs/trackers/misc/frictions/readme.md",
            "docs/trackers/misc",
        ),
        ("docs/frictions/readme.md", "docs/frictions"),
        (
            "docs/trackers/frictions/readme.md",
            "docs/trackers/frictions",
        ),
        ("docs/trackers/ledgers/readme.md", "docs/trackers/ledgers"),
        ("docs/trackers/ledgers/Bad_Name/readme.md", "docs/trackers"),
    ] {
        let text = refusal(&ctx, collection_at(rel, ledger_schema())).await;
        assert!(text.contains(rel), "{text}");
        assert_one_remedy(&text);
        assert_no_collection_written(tmp.path(), &ctx, &[dir]);
    }
}

#[tokio::test]
async fn a_path_with_a_parent_component_is_refused() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());

    let text = refusal(
        &ctx,
        collection_at(
            "docs/trackers/ledgers/../ledgers/frictions/readme.md",
            ledger_schema(),
        ),
    )
    .await;

    assert!(text.contains(".."), "{text}");
    assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
}

#[tokio::test]
async fn a_readme_key_equal_to_an_existing_stem_is_refused() {
    let issue = "docs/issues/frictions.md";
    let tmp = checkout(&[(issue, "# BUG: x\n")]);
    let ctx = mk_ctx(tmp.path());

    let text = refusal(&ctx, new_ledger(ledger_schema())).await;

    assert!(text.contains(issue), "names the other path: {text}");
    assert!(text.contains(NEW_README), "names the new path: {text}");
    assert_one_remedy(&text);
    assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
}

#[tokio::test]
async fn an_archived_collection_blocks_a_live_one_with_its_key() {
    let archived = "docs/trackers/ledgers/archive/frictions/readme.md";
    let tmp = checkout(&[(archived, "---\ntitle: Old\n---\n")]);
    let ctx = mk_ctx(tmp.path());

    let text = refusal(&ctx, new_ledger(ledger_schema())).await;

    assert!(text.contains(archived), "{text}");
    assert_one_remedy(&text);
    assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
}

#[tokio::test]
async fn a_title_with_a_newline_or_no_text_is_refused() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    for title in ["Frictions\nand more", "Frictions\r", "   "] {
        let mut args = new_ledger(ledger_schema());
        args["title"] = json!(title);
        let text = refusal(&ctx, args.clone()).await;
        assert!(text.contains("title"), "{title:?}: {text}");
        assert_one_remedy(&text);
        args["action"] = json!("create");
        let pre = format!("{:#}", preflight(&args).unwrap_err());
        assert_eq!(pre, text, "preflight gives the same refusal");
        assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
    }
}

#[tokio::test]
async fn title_id_and_kind_in_extra_name_their_own_remedy() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    for key in ["title", "id", "kind"] {
        let mut schema = ledger_schema();
        schema[key] = json!("x");
        let text = refusal(&ctx, new_ledger(schema)).await;
        assert!(text.contains(&format!("extra.{key}")), "{key}: {text}");
        assert_one_remedy(&text);
        assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
    }
}

#[tokio::test]
async fn a_collection_takes_no_owners_topic_time_scope_augment_or_status() {
    let tmp = checkout(&[]);
    let ctx = mk_ctx(tmp.path());
    for (key, value) in [
        ("owners", json!(["me"])),
        ("topic", json!("t")),
        ("time_scope", json!("2026-W46")),
        ("augment", json!({"prompt": "p"})),
        ("status", json!("done")),
    ] {
        let mut args = new_ledger(ledger_schema());
        args[key] = value;
        let text = refusal(&ctx, args.clone()).await;
        assert!(text.contains(key), "{key}: {text}");
        assert_one_remedy(&text);
        args["action"] = json!("create");
        let pre = format!("{:#}", preflight(&args).unwrap_err());
        assert_eq!(pre, text, "preflight gives the same refusal");
    }
    assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
}

#[test]
fn a_collection_call_without_rel_path_or_title_is_refused_by_preflight() {
    for (args, says) in [
        (
            json!({"action": "create", "kind": "collection", "title": "T"}),
            "`rel_path`",
        ),
        (
            json!({"action": "create", "kind": "collection", "rel_path": NEW_README}),
            "`title`",
        ),
    ] {
        let text = format!("{:#}", preflight(&args).unwrap_err());
        assert!(text.contains(says), "{args} -> {text}");
        assert!(!text.contains("missing field"), "{args} -> {text}");
        assert_one_remedy(&text);
    }
    // `body` is optional: the readme prose may be empty.
    preflight(&json!({
        "action": "create", "kind": "collection", "rel_path": NEW_README, "title": "T"
    }))
    .unwrap();
}

#[tokio::test]
async fn a_collection_create_obeys_requirements_toml() {
    let tmp = checkout(&[(
        ".codescout/requirements.toml",
        "[[require]]\nkind = \"collection\"\nsections = [\"Purpose\"]\nmin_section_chars = 40\n",
    )]);
    let ctx = mk_ctx(tmp.path());

    let text = refusal(&ctx, new_ledger(ledger_schema())).await;

    assert!(text.contains("## Purpose"), "{text}");
    assert_no_collection_written(tmp.path(), &ctx, &[NEW_DIR]);
}

#[tokio::test]
async fn a_root_below_the_git_top_level_refuses_a_collection() {
    let tmp = checkout(&[]);
    let sub = tmp.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let ctx = mk_ctx(&sub);

    let text = refusal(&ctx, new_ledger(ledger_schema())).await;

    assert!(text.contains("top level"), "{text}");
    assert_one_remedy(&text);
    assert_no_collection_written(&sub, &ctx, &[NEW_DIR]);
}
