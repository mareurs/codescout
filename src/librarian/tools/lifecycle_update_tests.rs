//! `doc(action="update")` on a lifecycle entry: a file that `layout::place` calls an entry is
//! updated through `update_entry`, and the catalog row follows the file.

use std::path::Path;

use chrono::{Days, NaiveDate};
use serde_json::{json, Value};
use tempfile::TempDir;

use super::testfixture::repo_with_worktree;
use super::{TestToolContextBuilder, ToolContext};
use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};
use crate::librarian::catalog::Catalog;
use crate::librarian::current_project::CurrentProject;
use crate::librarian::frontmatter;
use crate::librarian::workspace::Root;

const README: &str = "docs/trackers/ledgers/d/readme.md";
const ENTRY: &str = "docs/trackers/ledgers/d/2026-11-01-pick-x.md";

/// A ledger `d`: one entry kind, two open words (`fixed` among them, a word the bug stamp of the
/// generic path reacts to), a blocked word and a terminal word, and one declared field.
const LEDGER: &str = "---\ntitle: Decisions\nentry_kinds: [decision]\n\
    statuses: {open: open, fixed: open, waiting: blocked, done: terminal}\n\
    initial_status: open\nfields:\n  area: {type: scalar}\nreview_every: 30d\n---\n\n# Decisions\n";

/// The frontmatter lines of an open entry, without its `kind` line.
const OPEN_FIELDS: &str =
    "entry_kind: decision\ntitle: Pick X\nstatus: open\nstatus_since: 2026-11-01\ncreated: 2026-11-01\n";

fn today() -> NaiveDate {
    chrono::Utc::now().date_naive()
}

/// An entry document: `front` is its frontmatter lines, and its body is one `## Decision`
/// section.
fn entry(front: &str) -> String {
    format!("---\n{front}---\n\n## Decision\n\nWe pick X.\n")
}

/// An open entry with `kind: entry`.
fn open_entry() -> String {
    entry(&format!("kind: entry\n{OPEN_FIELDS}"))
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

fn mk_ctx(root: &Path) -> ToolContext {
    TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "r".into(),
            path: root.to_path_buf(),
        })
        .build()
}

/// A temp project holding the ledger readme and `files`.
fn project(files: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), README, LEDGER);
    for (rel, text) in files {
        write(tmp.path(), rel, text);
    }
    tmp
}

/// Write `text` at `rel` under `root` and give it a catalog row of `kind` and `status`, as a
/// reindex would. Returns the row's id.
fn seed(ctx: &ToolContext, root: &Path, rel: &str, text: &str, kind: &str, status: &str) -> String {
    write(root, rel, text);
    let abs = root.join(rel);
    let id = crate::librarian::ids::artifact_id_from_abs(&abs);
    let row = TestArtifactRowBuilder::new(&id)
        .with_abs_path(abs)
        .with_kind(kind)
        .with_status(status)
        .with_title("Pick X")
        .build();
    artifact::upsert(&ctx.catalog.lock(), &row).unwrap();
    id
}

async fn update(ctx: &ToolContext, id: &str, patch: Value) -> anyhow::Result<Value> {
    super::update::call(ctx, json!({"id": id, "patch": patch})).await
}

async fn refusal(ctx: &ToolContext, id: &str, patch: Value) -> String {
    format!("{:#}", update(ctx, id, patch).await.unwrap_err())
}

fn row(ctx: &ToolContext, id: &str) -> artifact::ArtifactRow {
    artifact::get(&ctx.catalog.lock(), id).unwrap().unwrap()
}

/// A text shown to an agent: one `<problem>; <remedy>` pair.
fn assert_one_remedy(text: &str) {
    assert_eq!(text.matches("; ").count(), 1, "one `; ` in: {text}");
}

fn has_line(text: &str, line: &str) -> bool {
    text.lines().any(|l| l == line)
}

#[tokio::test]
async fn a_status_update_sets_status_since_and_closed_through_the_tool() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), ENTRY, &open_entry(), "entry", "open");

    update(&ctx, &id, json!({"status": "done"})).await.unwrap();

    let text = read(tmp.path(), ENTRY);
    let today = today();
    assert!(has_line(&text, "status: done"), "{text}");
    assert!(has_line(&text, &format!("status_since: {today}")), "{text}");
    assert!(has_line(&text, &format!("closed: {today}")), "{text}");
    assert!(
        text.ends_with("## Decision\n\nWe pick X.\n"),
        "body kept: {text}"
    );
    assert_eq!(row(&ctx, &id).status, "done");
}

/// C2: after a status, title and tags update, the row holds what the FILE holds: the tags as
/// `update_entry` cleaned them, not as the patch sent them.
#[tokio::test]
async fn the_catalog_row_follows_the_file_after_an_entry_update() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), ENTRY, &open_entry(), "entry", "open");

    update(
        &ctx,
        &id,
        json!({"status": "done", "title": "Pick Y", "tags": [" b", "a", "b"]}),
    )
    .await
    .unwrap();

    let text = read(tmp.path(), ENTRY);
    let fm = frontmatter::parse(&text).unwrap().0.unwrap();
    assert_eq!(fm.tags, ["b", "a"], "{text}");
    let r = row(&ctx, &id);
    assert_eq!(r.status, "done");
    assert_eq!(r.title, fm.title);
    assert_eq!(r.title.as_deref(), Some("Pick Y"));
    assert_eq!(r.tags, fm.tags);
    assert_eq!(
        r.file_sha256,
        crate::librarian::util::sha_of_bytes(text.as_bytes())
    );

    let found = super::find::call(&ctx, json!({"status": "done"}))
        .await
        .unwrap();
    let ids: Vec<&str> = found["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|i| i["id"].as_str())
        .collect();
    assert!(ids.contains(&id.as_str()), "{found}");
}

/// C3: a patch with only body edits takes the generic path, which does not parse the entry.
#[tokio::test]
async fn a_body_only_edit_of_an_entry_applies() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), ENTRY, &open_entry(), "entry", "open");

    update(
        &ctx,
        &id,
        json!({"body_edits": [{"heading": "## Decision", "action": "replace", "content": "We pick Y."}]}),
    )
    .await
    .unwrap();

    let text = read(tmp.path(), ENTRY);
    assert!(text.contains("We pick Y."), "{text}");
    assert!(!text.contains("We pick X."), "{text}");
    assert!(
        text.starts_with(&format!("---\nkind: entry\n{OPEN_FIELDS}---\n")),
        "frontmatter untouched: {text}"
    );
}

/// C3: `commit_refresh` alone is a legitimate empty patch, and on an entry it must not reach
/// `update_entry`, which would refuse it as `EmptyUpdate`.
#[tokio::test]
async fn a_commit_refresh_only_update_of_an_entry_does_not_hit_empty_update() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), ENTRY, &open_entry(), "entry", "open");

    let v = super::update::call(&ctx, json!({"id": id, "commit_refresh": true}))
        .await
        .unwrap();

    assert_eq!(v["updated"], true, "{v}");
}

#[tokio::test]
async fn an_undeclared_status_is_refused_for_an_entry_and_not_for_a_tracker() {
    let tracker = "docs/trackers/notes.md";
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), ENTRY, &open_entry(), "entry", "open");
    let before = read(tmp.path(), ENTRY);
    let row_before = row(&ctx, &id);

    let text = refusal(&ctx, &id, json!({"status": "bogus"})).await;

    assert!(text.contains("\"bogus\" is not declared"), "{text}");
    assert_one_remedy(&text);
    assert_eq!(read(tmp.path(), ENTRY), before, "the file is untouched");
    assert_eq!(row(&ctx, &id), row_before, "the row is untouched");

    let tracker_id = seed(
        &ctx,
        tmp.path(),
        tracker,
        "---\nkind: tracker\ntitle: Notes\nstatus: active\n---\n\n# Notes\n",
        "tracker",
        "active",
    );
    update(&ctx, &tracker_id, json!({"status": "bogus"}))
        .await
        .unwrap();
    assert!(has_line(&read(tmp.path(), tracker), "status: bogus"));
    assert_eq!(row(&ctx, &tracker_id).status, "bogus");
}

/// The route is the path, never the frontmatter `kind`.
#[tokio::test]
async fn an_entry_without_kind_entry_still_takes_the_entry_path() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), ENTRY, &entry(OPEN_FIELDS), "spec", "open");

    update(&ctx, &id, json!({"status": "done"})).await.unwrap();

    let text = read(tmp.path(), ENTRY);
    let today = today();
    assert!(has_line(&text, &format!("status_since: {today}")), "{text}");
    assert!(has_line(&text, &format!("closed: {today}")), "{text}");
}

/// The bug stamp of the generic path keys on the row's kind and the status word; an entry whose
/// file says `kind: bug` is still an entry, and an open-class `fixed` takes no `closed`.
#[tokio::test]
async fn an_entry_with_kind_bug_gets_no_closed_stamp() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(
        &ctx,
        tmp.path(),
        ENTRY,
        &entry(&format!("kind: bug\n{OPEN_FIELDS}")),
        "bug",
        "open",
    );

    update(&ctx, &id, json!({"status": "fixed"})).await.unwrap();

    let text = read(tmp.path(), ENTRY);
    assert!(has_line(&text, "status: fixed"), "{text}");
    assert!(!text.contains("closed:"), "{text}");
    assert!(
        has_line(&text, &format!("status_since: {}", today())),
        "{text}"
    );
}

/// I-1 through the tool: a stray key and an undeclared status, repaired in one call.
#[tokio::test]
async fn one_call_repairs_a_stray_key_and_a_bad_status() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let broken = entry(
        "kind: entry\nentry_kind: decision\ntitle: Pick X\nstatus: bogus\n\
         status_since: 2026-11-01\ncreated: 2026-11-01\nstray: x\n",
    );
    let id = seed(&ctx, tmp.path(), ENTRY, &broken, "entry", "bogus");

    update(
        &ctx,
        &id,
        json!({"status": "open", "extra": {"stray": null}}),
    )
    .await
    .unwrap();

    let text = read(tmp.path(), ENTRY);
    assert!(has_line(&text, "status: open"), "{text}");
    assert!(
        has_line(&text, &format!("status_since: {}", today())),
        "the entry path wrote it: {text}"
    );
    assert!(!text.contains("stray"), "{text}");
    assert_eq!(row(&ctx, &id).status, "open");
}

#[tokio::test]
async fn owners_topic_time_scope_and_params_are_refused_on_an_entry() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), ENTRY, &open_entry(), "entry", "open");
    let before = read(tmp.path(), ENTRY);

    for (name, patch) in [
        ("owners", json!({"owners": ["me"], "status": "done"})),
        ("topic", json!({"topic": "t"})),
        ("time_scope", json!({"time_scope": "2026-W46"})),
        ("params", json!({"params": {"a": 1}})),
    ] {
        let text = refusal(&ctx, &id, patch).await;
        assert!(text.contains("an entry has no"), "{name}: {text}");
        assert!(text.contains(&format!("gives {name};")), "{name}: {text}");
        assert_one_remedy(&text);
        assert_eq!(read(tmp.path(), ENTRY), before, "{name}: file untouched");
    }
    let text = refusal(
        &ctx,
        &id,
        json!({"owners": ["me"], "topic": "t", "time_scope": "x", "params": {}}),
    )
    .await;
    assert!(
        text.contains("owners, topic, time_scope, params"),
        "names every one given: {text}"
    );
    assert_eq!(row(&ctx, &id).status, "open");
}

/// `entry_kind` is fixed: a document that does not parse as an entry, and an entry whose kind the
/// readme does not declare, cannot be repaired by an update, and the refusal says so.
#[tokio::test]
async fn an_unparsed_entry_and_an_undeclared_kind_are_refused_as_not_repairable() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let unparsed = entry(
        "kind: entry\nentry_kind: decision\ntitle: Pick X\nstatus: open\ncreated: 2026-11-01\n",
    );
    let id = seed(&ctx, tmp.path(), ENTRY, &unparsed, "entry", "open");

    let text = refusal(&ctx, &id, json!({"status": "done"})).await;

    assert!(text.contains("status_since"), "{text}");
    assert!(text.contains("cannot repair"), "{text}");
    assert_one_remedy(&text);
    assert_eq!(read(tmp.path(), ENTRY), unparsed, "the file is untouched");

    let other = entry(&OPEN_FIELDS.replace("decision", "other"));
    let id = seed(&ctx, tmp.path(), ENTRY, &other, "entry", "open");
    let text = refusal(&ctx, &id, json!({"title": "Pick Y"})).await;
    assert!(text.contains("entry_kind \"other\""), "{text}");
    assert!(text.contains("an update cannot change it"), "{text}");
    assert_one_remedy(&text);
    assert_eq!(read(tmp.path(), ENTRY), other, "the file is untouched");

    let id = seed(&ctx, tmp.path(), ENTRY, &open_entry(), "entry", "open");
    let text = refusal(&ctx, &id, json!({"extra": {"entry_kind": "other"}})).await;
    assert!(text.contains("entry_kind is fixed"), "{text}");
    assert_one_remedy(&text);
}

/// The frontmatter change and the body edit land in one write.
#[tokio::test]
async fn a_body_edit_and_a_status_change_in_one_call_both_apply() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), ENTRY, &open_entry(), "entry", "open");

    update(
        &ctx,
        &id,
        json!({
            "status": "done",
            "body_edits": [{"heading": "## Decision", "action": "replace", "content": "We pick Y."}]
        }),
    )
    .await
    .unwrap();

    let text = read(tmp.path(), ENTRY);
    assert!(has_line(&text, "status: done"), "{text}");
    assert!(has_line(&text, &format!("closed: {}", today())), "{text}");
    assert!(text.contains("We pick Y."), "{text}");
    assert!(!text.contains("We pick X."), "{text}");
    let r = row(&ctx, &id);
    assert_eq!(r.status, "done");
    assert_eq!(
        r.file_sha256,
        crate::librarian::util::sha_of_bytes(text.as_bytes())
    );
}

/// P-R1: an archived entry is not reopened in place.
#[tokio::test]
async fn reopening_an_archived_entry_is_refused_and_names_the_move() {
    let archived = "docs/trackers/ledgers/d/archive/2026-11-01-pick-x.md";
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let done = entry(
        "kind: entry\nentry_kind: decision\ntitle: Pick X\nstatus: done\n\
         status_since: 2026-11-02\ncreated: 2026-11-01\nclosed: 2026-11-02\n",
    );
    let id = seed(&ctx, tmp.path(), archived, &done, "entry", "done");

    let text = refusal(&ctx, &id, json!({"status": "open"})).await;

    assert!(text.contains("archive"), "{text}");
    assert!(
        text.contains("doc(action=\"move\")"),
        "names the move: {text}"
    );
    assert_one_remedy(&text);
    assert_eq!(read(tmp.path(), archived), done, "the file is untouched");
    assert_eq!(row(&ctx, &id).status, "done");

    // A write that does not reopen it still goes through.
    update(&ctx, &id, json!({"title": "Pick X again"}))
        .await
        .unwrap();
    assert!(has_line(&read(tmp.path(), archived), "title: Pick X again"));
}

/// P-R7: the collection is loaded by its folder, so a collection under `<lifecycle>/archive/`
/// gives its schema to its entries.
#[tokio::test]
async fn an_entry_in_an_archived_collection_is_updated_through_its_dir() {
    let readme = "docs/trackers/ledgers/archive/d/readme.md";
    let rel = "docs/trackers/ledgers/archive/d/2026-11-01-pick-x.md";
    let tmp = TempDir::new().unwrap();
    write(tmp.path(), readme, LEDGER);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), rel, &open_entry(), "entry", "open");

    update(&ctx, &id, json!({"status": "done"})).await.unwrap();

    let text = read(tmp.path(), rel);
    assert!(has_line(&text, &format!("closed: {}", today())), "{text}");
    let text = refusal(&ctx, &id, json!({"status": "bogus"})).await;
    assert!(text.contains("\"bogus\" is not declared"), "{text}");
}

/// P-R4: `revisit_by` through `extra`, without a status, re-dates a blocked entry and keeps its
/// status and `status_since`.
#[tokio::test]
async fn revisit_by_through_extra_redates_a_blocked_entry() {
    let tmp = project(&[]);
    let ctx = mk_ctx(tmp.path());
    let soon = today().checked_add_days(Days::new(3)).unwrap();
    let later = today().checked_add_days(Days::new(10)).unwrap();
    let blocked = entry(&format!(
        "kind: entry\nentry_kind: decision\ntitle: Pick X\nstatus: waiting\n\
         status_since: 2026-11-01\ncreated: 2026-11-01\nrevisit_by: {soon}\n"
    ));
    let id = seed(&ctx, tmp.path(), ENTRY, &blocked, "entry", "waiting");

    update(
        &ctx,
        &id,
        json!({"extra": {"revisit_by": later.to_string()}}),
    )
    .await
    .unwrap();

    let text = read(tmp.path(), ENTRY);
    assert!(has_line(&text, &format!("revisit_by: {later}")), "{text}");
    assert!(has_line(&text, "status: waiting"), "{text}");
    assert!(has_line(&text, "status_since: 2026-11-01"), "{text}");
    assert_eq!(row(&ctx, &id).status, "waiting");

    let past = today().checked_sub_days(Days::new(1)).unwrap();
    let text = refusal(
        &ctx,
        &id,
        json!({"extra": {"revisit_by": past.to_string()}}),
    )
    .await;
    assert!(text.contains("is before today"), "{text}");
    assert_one_remedy(&text);
}

/// A collection readme, a file below a lifecycle folder that places with an error, and a legacy
/// flat tracker all take the generic path: each ends byte-identical to a twin outside
/// `docs/trackers/` given the same patch, a status no schema declares included.
#[tokio::test]
async fn a_non_entry_document_takes_todays_path_unchanged() {
    let text = "---\nkind: tracker\ntitle: T\nstatus: active\n---\n\n# T\n";
    let patch = json!({
        "status": "bogus", "title": "T2", "tags": [" a", "a"], "extra": {"note": "n"}
    });
    for (n, rel) in [
        README,
        "docs/trackers/ledgers/Bad_Key/x.md",
        "docs/trackers/legacy.md",
    ]
    .into_iter()
    .enumerate()
    {
        let tmp = project(&[]);
        let ctx = mk_ctx(tmp.path());
        let twin = format!("docs/notes/twin-{n}.md");
        let id = seed(&ctx, tmp.path(), rel, text, "tracker", "active");
        let twin_id = seed(&ctx, tmp.path(), &twin, text, "tracker", "active");

        update(&ctx, &id, patch.clone()).await.unwrap();
        update(&ctx, &twin_id, patch.clone()).await.unwrap();

        assert_eq!(read(tmp.path(), rel), read(tmp.path(), &twin), "{rel}");
        let (r, t) = (row(&ctx, &id), row(&ctx, &twin_id));
        assert_eq!(
            (r.status, r.title, r.tags),
            (t.status, t.title, t.tags),
            "{rel}"
        );
    }
}

/// Review focus 5: the schema comes from the worktree's checkout, not the main one. The main
/// readme declares `parked` and not `done`; the worktree readme the other way round.
#[tokio::test]
async fn an_update_in_a_worktree_reads_the_schema_from_the_worktree() {
    let (_tmp, main, worktree) = repo_with_worktree();
    let main_ledger = LEDGER.replace("done: terminal", "parked: terminal");
    write(&main, README, &main_ledger);
    write(&worktree, README, LEDGER);
    write(&worktree, ENTRY, &open_entry());
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
    // The row is the main checkout's, as a reindex there would make it; the update forks it.
    let id = seed(&ctx, &main, ENTRY, &open_entry(), "entry", "open");

    update(&ctx, &id, json!({"status": "done"})).await.unwrap();

    let text = read(&worktree, ENTRY);
    assert!(has_line(&text, "status: done"), "{text}");
    assert!(has_line(&text, &format!("closed: {}", today())), "{text}");
    assert_eq!(read(&main, ENTRY), open_entry(), "main is untouched");

    let text = refusal(&ctx, &id, json!({"status": "parked"})).await;
    assert!(text.contains("\"parked\" is not declared"), "{text}");
}
