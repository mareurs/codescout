//! `doc(action="get")` on a lifecycle collection readme: the response gains the collection's
//! entry table, and with `entry_filter` the matching rows as flat JSON (see `get_table`).

use std::path::Path;

use serde_json::{json, Value};
use tempfile::TempDir;

use super::testfixture::repo_with_worktree;
use super::{TestToolContextBuilder, ToolContext};
use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};
use crate::librarian::catalog::Catalog;
use crate::librarian::current_project::CurrentProject;
use crate::librarian::workspace::Root;

const README: &str = "docs/trackers/ledgers/d/readme.md";
const HEADER: &str = "| Entry | Kind | Status | Created | Closed | Archived |";

/// A ledger `d`: one entry kind, an open, a blocked and a terminal word, and one declared field.
const LEDGER: &str = "---\ntitle: Decisions\nentry_kinds: [decision]\n\
    statuses: {open: open, waiting: blocked, done: terminal}\n\
    initial_status: open\nfields:\n  area: {type: scalar}\nreview_every: 30d\n---\n\n\
    # Decisions\n\nWhat we decided.\n";

/// An entry document of kind `decision`: `status`, `created` (also its `status_since`) and
/// `more` frontmatter lines.
fn entry(title: &str, status: &str, created: &str, more: &str) -> String {
    format!(
        "---\nentry_kind: decision\ntitle: {title}\nstatus: {status}\nstatus_since: {created}\n\
         created: {created}\n{more}---\n\n## Decision\n\nText.\n"
    )
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn mk_ctx(root: &Path) -> ToolContext {
    TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "r".into(),
            path: root.to_path_buf(),
        })
        .build()
}

/// A temp project holding `files`.
fn project(files: &[(&str, &str)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    for (rel, text) in files {
        write(tmp.path(), rel, text);
    }
    tmp
}

/// Give the file at `rel` under `root` a catalog row, as a reindex would. Returns its id.
fn seed(ctx: &ToolContext, root: &Path, rel: &str, kind: &str) -> String {
    let abs = root.join(rel);
    let id = crate::librarian::ids::artifact_id_from_abs(&abs);
    let row = TestArtifactRowBuilder::new(&id)
        .with_abs_path(abs)
        .with_kind(kind)
        .with_status("active")
        .with_title("Decisions")
        .build();
    artifact::upsert(&ctx.catalog.lock(), &row).unwrap();
    id
}

async fn get(ctx: &ToolContext, args: Value) -> anyhow::Result<Value> {
    super::get::call(ctx, args).await
}

/// The ledger with two live entries: `2026-11-02-b` open in `ui`, `2026-11-01-a` done in `db`.
fn two_entry_project() -> TempDir {
    project(&[
        (README, LEDGER),
        (
            "docs/trackers/ledgers/d/2026-11-01-a.md",
            &entry("A", "done", "2026-11-01", "closed: 2026-11-03\narea: db\n"),
        ),
        (
            "docs/trackers/ledgers/d/2026-11-02-b.md",
            &entry("B", "open", "2026-11-02", "area: ui\n"),
        ),
    ])
}

fn table(out: &Value) -> &str {
    out["table"]
        .as_str()
        .unwrap_or_else(|| panic!("no table in {out}"))
}

/// A text shown to an agent: one `<problem>; <remedy>` pair.
fn assert_one_remedy(text: &str) {
    assert_eq!(text.matches("; ").count(), 1, "one `; ` in: {text}");
}

#[tokio::test]
async fn a_readme_id_returns_the_table() {
    let tmp = two_entry_project();
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let out = get(&ctx, json!({"id": id})).await.unwrap();

    let lines: Vec<&str> = table(&out).lines().collect();
    assert_eq!(
        lines,
        [
            HEADER,
            "|---|---|---|---|---|---|",
            "| [[2026-11-02-b]] B | decision | open | 2026-11-02 |  |  |",
            "| [[2026-11-01-a]] A | decision | done | 2026-11-01 | 2026-11-03 |  |",
        ]
    );
    assert_eq!(out["id"], json!(id), "the rest of the response is today's");
}

#[tokio::test]
async fn entry_filter_filters_on_status_and_on_a_declared_field() {
    let tmp = two_entry_project();
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let out = get(
        &ctx,
        json!({"id": id, "entry_filter": {"status": {"eq": "open"}}}),
    )
    .await
    .unwrap();
    assert_eq!(out["entry_total"], json!(2));
    let entries = out["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{out}");
    let b = &entries[0];
    assert_eq!(b["key"], json!("2026-11-02-b"));
    assert_eq!(b["class"], json!("open"));
    assert_eq!(b["created"], json!("2026-11-02"));
    assert_eq!(b["archived"], json!(false));
    assert_eq!(
        b["area"],
        json!("ui"),
        "a declared field is a top-level key"
    );
    assert!(b.get("fields").is_none(), "{b}");
    assert!(out["table"].is_string(), "the table comes with the entries");

    let out = get(
        &ctx,
        json!({"id": id, "entry_filter": {"area": {"eq": "db"}}}),
    )
    .await
    .unwrap();
    let keys: Vec<&Value> = out["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["key"])
        .collect();
    assert_eq!(keys, [&json!("2026-11-01-a")]);
    assert_eq!(out["entry_total"], json!(2));
    assert!(out.get("filter_warnings").is_none(), "{out}");
}

const GLOSSARY: &str = "docs/trackers/ledgers/g/readme.md";

/// A glossary `g`: named keys, one entry kind, and a declared list field `aliases`.
const GLOSSARY_README: &str = "---\ntitle: Glossary\nentry_kinds: [term]\n\
    statuses: {open: open, gone: terminal}\n\
    initial_status: open\nkey_style: name\nfields:\n  aliases: {type: list}\n---\n\n\
    # Glossary\n\nWhat we call things.\n";

/// An open entry document of kind `term`, created 2026-11-01, plus `more` frontmatter lines.
fn term(title: &str, more: &str) -> String {
    format!(
        "---\nentry_kind: term\ntitle: {title}\nstatus: open\nstatus_since: 2026-11-01\n\
         created: 2026-11-01\n{more}---\n\n## Meaning\n\nText.\n"
    )
}

/// A list field is filterable by membership: `contains` and `in` reach the array branch of the
/// filter on a collection's flat rows, and an entry without the field matches neither.
#[tokio::test]
async fn a_list_field_is_filterable_by_membership() {
    let tmp = project(&[
        (GLOSSARY, GLOSSARY_README),
        (
            "docs/trackers/ledgers/g/weekly-status.md",
            &term("Weekly status", "aliases: [wsr, status-report]\n"),
        ),
        (
            "docs/trackers/ledgers/g/risk-log.md",
            &term("Risk log", "aliases: [rl]\n"),
        ),
        (
            "docs/trackers/ledgers/g/glossary-intro.md",
            &term("Glossary intro", ""),
        ),
    ]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), GLOSSARY, "collection");

    let keys_for = |filter: Value| {
        let ctx = &ctx;
        let id = &id;
        async move {
            let out = get(ctx, json!({"id": id, "entry_filter": filter}))
                .await
                .unwrap();
            let keys: Vec<String> = out["entries"]
                .as_array()
                .unwrap_or_else(|| panic!("no entries in {out}"))
                .iter()
                .map(|e| e["key"].as_str().unwrap().to_string())
                .collect();
            (keys, out["entry_total"].clone())
        }
    };

    let (keys, _) = keys_for(json!({"aliases": {"contains": "wsr"}})).await;
    assert_eq!(keys, ["weekly-status"]);

    let (keys, total) = keys_for(json!({"aliases": {"contains": "nope"}})).await;
    assert!(keys.is_empty(), "{keys:?}");
    assert_eq!(total, json!(3), "the filter ran over all three entries");

    let (keys, _) = keys_for(json!({"aliases": {"in": ["rl", "wsr"]}})).await;
    assert_eq!(keys, ["risk-log", "weekly-status"]);
    assert!(
        !keys.contains(&"glossary-intro".to_string()),
        "an entry with no aliases line matches neither operator"
    );
}

/// A stray key named like a column cannot shadow the column in a flat row.
#[tokio::test]
async fn a_rows_own_column_wins_over_a_stray_key_of_the_same_name() {
    let tmp = project(&[
        (README, LEDGER),
        (
            "docs/trackers/ledgers/d/2026-11-02-b.md",
            &entry(
                "B",
                "open",
                "2026-11-02",
                "class: bogus\narchived: maybe\nfields: x\n",
            ),
        ),
    ]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let out = get(
        &ctx,
        json!({"id": id, "entry_filter": {"class": {"eq": "open"}}}),
    )
    .await
    .unwrap();
    let entries = out["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{out}");
    assert_eq!(entries[0]["class"], json!("open"));
    assert_eq!(entries[0]["archived"], json!(false));
    assert!(entries[0].get("fields").is_none(), "{}", entries[0]);
}

#[tokio::test]
async fn entries_are_absent_without_entry_filter() {
    let tmp = two_entry_project();
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let out = get(&ctx, json!({"id": id})).await.unwrap();

    assert!(out["table"].is_string());
    assert!(out.get("entries").is_none(), "{out}");
    assert!(out.get("entry_total").is_none(), "{out}");
}

#[tokio::test]
async fn a_collection_with_no_entries_gives_the_header_and_zero() {
    let tmp = project(&[(README, LEDGER)]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let out = get(
        &ctx,
        json!({"id": id, "entry_filter": {"status": {"eq": "open"}}}),
    )
    .await
    .unwrap();

    assert_eq!(
        table(&out).lines().collect::<Vec<_>>(),
        [HEADER, "|---|---|---|---|---|---|"]
    );
    assert_eq!(out["entry_total"], json!(0));
    assert_eq!(out["entries"], json!([]));
}

/// P-R5: `doc(get)` lists the archive too, marked in the `Archived` column.
#[tokio::test]
async fn an_archived_entry_shows_with_the_archived_column() {
    let tmp = project(&[
        (README, LEDGER),
        (
            "docs/trackers/ledgers/d/2026-11-02-b.md",
            &entry("B", "open", "2026-11-02", ""),
        ),
        (
            "docs/trackers/ledgers/d/archive/2026-11-01-a.md",
            &entry("A", "done", "2026-11-01", "closed: 2026-11-03\n"),
        ),
    ]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let out = get(
        &ctx,
        json!({"id": id, "entry_filter": {"archived": {"eq": true}}}),
    )
    .await
    .unwrap();

    assert!(
        table(&out).lines().any(
            |l| l == "| [[2026-11-01-a]] A | decision | done | 2026-11-01 | 2026-11-03 | yes |"
        ),
        "{}",
        table(&out)
    );
    assert_eq!(
        out["entry_total"],
        json!(2),
        "the archived row is considered"
    );
    let entries = out["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1, "{out}");
    assert_eq!(entries[0]["key"], json!("2026-11-01-a"));
}

#[tokio::test]
async fn a_heading_selector_still_returns_prose() {
    let tmp = two_entry_project();
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let out = get(&ctx, json!({"id": id, "heading": "# Decisions"}))
        .await
        .unwrap();
    assert!(out.get("table").is_none(), "{out}");
    assert!(
        out.to_string().contains("What we decided."),
        "the section's prose: {out}"
    );

    for selector in [json!({"full": true}), json!({"headings": ["# Decisions"]})] {
        let mut args = json!({"id": id});
        args.as_object_mut()
            .unwrap()
            .extend(selector.as_object().unwrap().clone());
        let out = get(&ctx, args).await.unwrap();
        assert!(out.get("table").is_none(), "{selector}: {out}");
    }
}
/// A line range is a body selector: it returns the lines, builds no table and loads no
/// collection, so a readme whose collection does not load still answers it.
#[tokio::test]
async fn a_line_range_returns_lines_and_loads_no_collection() {
    let tmp = two_entry_project();
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");
    // The readme's body is file lines 11-13 (ten frontmatter lines).
    for range in [
        json!({"start_line": 11, "end_line": 13}),
        json!({"start_line": 13}),
        json!({"end_line": 13}),
    ] {
        let mut args = json!({"id": id});
        args.as_object_mut()
            .unwrap()
            .extend(range.as_object().unwrap().clone());
        let out = get(&ctx, args).await.unwrap();
        assert!(out.get("table").is_none(), "{range}: {out}");
    }

    let broken = LEDGER.replace("initial_status: open\n", "initial_status: nope\n");
    let tmp = project(&[(README, &broken)]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");
    let out = get(&ctx, json!({"id": id, "start_line": 11, "end_line": 13}))
        .await
        .unwrap();
    assert!(out.get("table").is_none(), "{out}");
    assert!(
        out.to_string().contains("What we decided."),
        "the lines: {out}"
    );
}

/// P-R7: a readme under `<lifecycle>/archive/` is loaded by its folder.
#[tokio::test]
async fn an_archived_collections_readme_gives_a_table() {
    let readme = "docs/trackers/ledgers/archive/old/readme.md";
    let tmp = project(&[
        (readme, LEDGER),
        (
            "docs/trackers/ledgers/archive/old/2026-11-01-a.md",
            &entry("A", "open", "2026-11-01", ""),
        ),
    ]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), readme, "collection");

    let out = get(&ctx, json!({"id": id})).await.unwrap();

    assert!(
        table(&out)
            .lines()
            .any(|l| l == "| [[2026-11-01-a]] A | decision | open | 2026-11-01 |  | yes |"),
        "{}",
        table(&out)
    );
}

/// Review focus 5 and P-R7: a worktree session reading a main-checkout readme row gets the
/// table of its own checkout. The entry `2026-11-02-wt` exists only in the worktree, and
/// `2026-11-01-main` only in the main checkout.
#[tokio::test]
async fn the_table_is_built_from_the_sessions_checkout() {
    let (_tmp, main, worktree) = repo_with_worktree();
    write(&main, README, LEDGER);
    write(
        &main,
        "docs/trackers/ledgers/d/2026-11-01-main.md",
        &entry("Main", "open", "2026-11-01", ""),
    );
    write(&worktree, README, LEDGER);
    write(
        &worktree,
        "docs/trackers/ledgers/d/2026-11-02-wt.md",
        &entry("Wt", "open", "2026-11-02", ""),
    );
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_current_project(std::sync::Arc::new(CurrentProject {
            abs_path: worktree.clone(),
            git_root: worktree.clone(),
            main_root: Some(main.clone()),
            umbrella: None,
        }))
        .build();
    // The row is the main checkout's, as a reindex there would make it. `main` is no managed
    // root of this session: only its `main_root` gives the row a repo-relative path.
    let id = seed(&ctx, &main, README, "collection");

    let out = get(&ctx, json!({"id": id})).await.unwrap();

    let t = table(&out);
    assert!(t.contains("[[2026-11-02-wt]] Wt"), "{t}");
    assert!(!t.contains("2026-11-01-main"), "{t}");
}
/// A main-checkout session reading the readme row of a collection in a NESTED worktree checkout
/// (`<main>/.worktrees/x/docs/trackers/...`) gets that worktree's table: the first managed root
/// (main) gives `.worktrees/x/docs/trackers/...`, which places as nothing, so the root is narrowed
/// to the worktree that holds the file.
#[tokio::test]
async fn a_main_session_gets_the_table_of_a_readme_in_a_nested_worktree() {
    let (_tmp, main, worktree) = repo_with_worktree();
    write(&worktree, README, LEDGER);
    write(
        &worktree,
        "docs/trackers/ledgers/d/2026-11-02-wt.md",
        &entry("Wt", "open", "2026-11-02", ""),
    );
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "r".into(),
            path: main.clone(),
        })
        .with_current_project(std::sync::Arc::new(CurrentProject {
            abs_path: main.clone(),
            git_root: main.clone(),
            main_root: None,
            umbrella: None,
        }))
        .build();
    let id = seed(&ctx, &worktree, README, "collection");

    let out = get(&ctx, json!({"id": id})).await.unwrap();

    let t = table(&out);
    assert!(t.contains("[[2026-11-02-wt]] Wt"), "{t}");
}

/// A readme whose schema fails gives the loader's readable problems, never a table.
#[tokio::test]
async fn a_broken_collection_returns_readable_problems_not_a_table() {
    let broken = LEDGER.replace("initial_status: open\n", "initial_status: nope\n");
    let tmp = project(&[
        (README, &broken),
        (
            "docs/trackers/ledgers/d/2026-11-02-b.md",
            &entry("B", "open", "2026-11-02", ""),
        ),
    ]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let text = format!("{:#}", get(&ctx, json!({"id": id})).await.unwrap_err());

    assert!(text.contains("cannot be loaded"), "{text}");
    assert!(text.contains(README), "names the readme: {text}");
    assert!(!text.contains("| Entry |"), "{text}");
    assert!(
        text.contains("full=true"),
        "names the way to read it: {text}"
    );
    assert_one_remedy(&text);

    // The way the refusal names works: the readme's text, to fix it from.
    let out = get(&ctx, json!({"id": id, "full": true})).await.unwrap();
    assert!(out.get("table").is_none(), "{out}");
    assert!(out.to_string().contains("What we decided."), "{out}");
}
/// A readme the session's checkout does not hold (a collection made on main after the worktree
/// branched, or a row whose file is gone) gives no table. The refusal names the checkout and the
/// call that still reads the catalogued file, and that call works.
#[tokio::test]
async fn a_readme_missing_from_the_sessions_checkout_names_full_true() {
    let (_tmp, main, worktree) = repo_with_worktree();
    write(&main, README, LEDGER);
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
    let id = seed(&ctx, &main, README, "collection");

    let text = format!("{:#}", get(&ctx, json!({"id": id})).await.unwrap_err());
    assert!(
        text.contains("has no readme in this session's checkout"),
        "{text}"
    );
    assert!(text.contains("pass full=true"), "{text}");
    assert_one_remedy(&text);

    let out = get(&ctx, json!({"id": id, "full": true})).await.unwrap();
    assert!(out.get("table").is_none(), "{out}");
    assert!(
        out.to_string().contains("What we decided."),
        "main's prose: {out}"
    );
}

/// A stale row: the readme was deleted by hand. The same refusal, and `full=true` still answers
/// with today's `body_error`.
#[tokio::test]
async fn a_readme_gone_from_disk_names_full_true() {
    let tmp = two_entry_project();
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");
    std::fs::remove_file(tmp.path().join(README)).unwrap();

    let text = format!("{:#}", get(&ctx, json!({"id": id})).await.unwrap_err());
    assert!(
        text.contains("has no readme in this session's checkout"),
        "{text}"
    );
    assert_one_remedy(&text);

    let out = get(&ctx, json!({"id": id, "full": true})).await.unwrap();
    assert!(out["body_error"].is_string(), "{out}");
}

/// An entry whose frontmatter does not parse is missing from the table and the rows; the
/// response names it, so the count is not taken for complete.
#[tokio::test]
async fn an_unparsed_entry_is_named_in_table_problems() {
    let bad = "docs/trackers/ledgers/d/2026-11-03-bad.md";
    let tmp = project(&[
        (README, LEDGER),
        (
            "docs/trackers/ledgers/d/2026-11-02-b.md",
            &entry("B", "open", "2026-11-02", ""),
        ),
        (
            bad,
            "---\nentry_kind: decision\ntitle: Bad\nstatus: open\n---\n\nNo dates.\n",
        ),
    ]);
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    let out = get(
        &ctx,
        json!({"id": id, "entry_filter": {"status": {"eq": "open"}}}),
    )
    .await
    .unwrap();

    assert_eq!(table(&out).lines().count(), 3, "{}", table(&out));
    assert_eq!(out["entry_total"], json!(1));
    let problems = out["table_problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{out}"));
    assert_eq!(problems.len(), 1, "{out}");
    let text = problems[0].as_str().unwrap();
    assert!(text.starts_with(bad), "{text}");
    assert_one_remedy(text);

    let out = get(&ctx, json!({"id": id, "heading": "# Decisions"}))
        .await
        .unwrap();
    assert!(
        out.get("table_problems").is_none(),
        "no table, no table problems: {out}"
    );

    let out = get(
        &ctx,
        json!({"id": id, "heading": "# Decisions", "entry_filter": {"status": {"eq": "open"}}}),
    )
    .await
    .unwrap();
    assert_eq!(
        out["table_problems"].as_array().map(Vec::len),
        Some(1),
        "the entries leave it out too: {out}"
    );
}

/// A clean collection carries no `table_problems` key.
#[tokio::test]
async fn a_clean_collection_has_no_table_problems() {
    let tmp = two_entry_project();
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");
    let out = get(&ctx, json!({"id": id})).await.unwrap();
    assert!(out.get("table_problems").is_none(), "{out}");
}

/// `entry_filter` with a body selector: the selected prose and the matching entries, no table,
/// and never the augmentation refusal.
#[tokio::test]
async fn entry_filter_with_a_heading_gives_the_section_and_the_entries() {
    let tmp = two_entry_project();
    let ctx = mk_ctx(tmp.path());
    let id = seed(&ctx, tmp.path(), README, "collection");

    for selector in [
        json!({"heading": "# Decisions"}),
        json!({"headings": ["# Decisions"]}),
        json!({"full": true}),
    ] {
        let mut args = json!({"id": id, "entry_filter": {"status": {"eq": "open"}}});
        args.as_object_mut()
            .unwrap()
            .extend(selector.as_object().unwrap().clone());
        let out = get(&ctx, args).await.unwrap();
        assert!(out.get("table").is_none(), "{selector}: {out}");
        assert!(
            out.to_string().contains("What we decided."),
            "{selector}: {out}"
        );
        assert_eq!(out["entry_total"], json!(2), "{selector}: {out}");
        let keys: Vec<&Value> = out["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| &e["key"])
            .collect();
        assert_eq!(keys, [&json!("2026-11-02-b")], "{selector}");
    }
}

/// A readme no managed root holds cannot be given a repo-relative path, so its collection
/// cannot be loaded: that is refused, never answered with prose in silence. An entry outside
/// every root keeps today's `get`.
#[tokio::test]
async fn a_readme_below_no_managed_root_is_refused_and_an_entry_is_not() {
    let elsewhere = two_entry_project();
    let home = TempDir::new().unwrap();
    let ctx = mk_ctx(home.path());
    let readme_id = seed(&ctx, elsewhere.path(), README, "collection");
    let entry_id = seed(
        &ctx,
        elsewhere.path(),
        "docs/trackers/ledgers/d/2026-11-02-b.md",
        "entry",
    );

    let text = format!(
        "{:#}",
        get(&ctx, json!({"id": readme_id})).await.unwrap_err()
    );
    assert!(text.contains("no managed root"), "{text}");
    assert_one_remedy(&text);

    let out = get(&ctx, json!({"id": entry_id})).await.unwrap();
    assert!(out.get("table").is_none(), "{out}");
}
