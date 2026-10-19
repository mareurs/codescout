//! The read door's layout table: every librarian site that reads the CONTENT of a file a
//! repository or the catalog names, run over one set of layouts, with one verdict per layout.
//!
//! The invariant under test is stated once, in `crate::librarian::read_door`: the librarian hands
//! the bytes of a file that a repository or the catalog names (to a caller, to the chunk table or
//! to an embedder) only if the file, AS OPENED, lies inside the root `util::door_root` names for
//! its spelling and is not on the read deny-list; with no root to name, only if its spelling
//! crosses no symbolic link at all.
//!
//! How to read the table. Every layout plants the same marker in a target file (`x.md`, and a
//! `x.json` twin for the `config_value` site) and spells it from a project. Each SITE is an adapter
//! that drives one production read path and answers one question: did the marker come out (to a
//! caller, the chunk table or the embedder)? A layout's verdict is ADMIT (it must come out of every
//! site), REFUSE (it must come out of none) or NO BYTES (nothing to read). The table asserts:
//!
//! 1. every read site gives the layout's verdict, so the sites agree with each other;
//! 2. the verdict agrees with the WRITE door's containment answer for the same spelling
//!    (`refuse_catalogued_path_outside_repo`), on every layout marked comparable, so the
//!    librarian reads only where it would write;
//! 3. the read-modify-write site (`doc(update)`) reads only what both doors admit: the full write
//!    door (leaf rule included) and the read door. That is the one place a read verdict and a
//!    write verdict differ on purpose: an in-repo leaf link is readable and never written through.
//!
//! Sites that emit no content (a hash, a count, a boolean) cannot show a marker; they are held
//! to the door by the source gate in `read_door.rs` instead, and the module doc there lists them.
//!
//! Unix only: links. The deny-list rows need `$HOME`, which a test must not mutate
//! (`docs/conventions/test-env-isolation.md`), so they run in a child process with `HOME` set for
//! the child only (`deny_listed_target_inside_a_managed_root_reaches_no_read_site`).
#![cfg(unix)]

use super::{
    context, doctor, find, get, link_scan, refresh, update, TestToolContextBuilder, ToolContext,
};
use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
use crate::librarian::artifact_store::ArtifactVectorStore;
use crate::librarian::catalog::{artifact, augmentation, Catalog};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Lowercase letters only, so the `requirements` site (which echoes `[a-z-]+` values) can carry it.
const MARK: &str = "fakekeymarker";

/// The target's text. Every content-echoing site has something to echo: a frontmatter `id`
/// (doctor), an entry section (context's entry anchor), a nested heading (update's guard), a
/// declared tag value (requirements), a link target (link_scan) and an external id (doctor's
/// external authority).
fn doc_text() -> String {
    format!(
        "---\nid: {MARK}\nunverified: {MARK} caveat\n---\n\n# {MARK} title\n\n## T-1 — {MARK} entry\n\n{MARK} body line\n\n\
         - `cluster/{MARK}`\n\n[link]({MARK}.md) and {MARK}:T-1\n\nTC-1\n\n## A\n\n### {MARK} nested\n\ntext\n"
    )
}

fn json_text() -> String {
    format!("{{\"k\": \"{MARK}\"}}")
}

/// Write the two targets into `dir`.
fn target(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("x.md"), doc_text()).unwrap();
    std::fs::write(dir.join("x.json"), json_text()).unwrap();
}

fn git(dir: &Path) {
    std::fs::create_dir_all(dir.join(".git")).unwrap();
}

fn ln(target: impl AsRef<Path>, link: &Path) {
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(target, link).unwrap();
}

/// One built layout.
pub(super) struct Fx {
    /// The active project `(abs_path, git_root)`; `None` = no managed root at all.
    project: Option<(PathBuf, PathBuf)>,
    /// What the walk indexes.
    walk_root: PathBuf,
    /// The spelled paths of the two targets.
    md: PathBuf,
    json: PathBuf,
    /// A `force_include` pattern, written to `<walk_root>/.codescout/project.toml`, for a layout
    /// the main walk cannot reach (it never descends a directory link).
    force_include: Option<&'static str>,
}

impl Fx {
    fn base_dir(&self) -> PathBuf {
        self.project
            .as_ref()
            .map(|(p, _)| p.clone())
            .unwrap_or_else(|| self.walk_root.clone())
    }
    fn rel(&self, p: &Path) -> String {
        p.strip_prefix(self.base_dir())
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }
    fn roots(&self) -> Vec<PathBuf> {
        match &self.project {
            Some((p, g)) => {
                let mut v = vec![g.clone()];
                if p != g {
                    v.push(p.clone());
                }
                v
            }
            None => Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Verdict {
    Admit,
    Refuse,
    NoBytes,
}

pub(super) struct Layout {
    pub(super) name: &'static str,
    pub(super) verdict: Verdict,
    /// Does the write door's containment answer have to equal this read verdict? `false` only
    /// where the difference is the design (stated in `why_not_comparable`).
    pub(super) comparable: bool,
    pub(super) why_not_comparable: &'static str,
    /// Sites that cannot observe this layout, with the reason.
    pub(super) skip: &'static [(&'static str, &'static str)],
    pub(super) build: fn(&Path) -> Fx,
}

fn project(base: &Path) -> PathBuf {
    let p = base.join("p");
    git(&p);
    p
}

fn fx(p: &Path, md_dir: &Path, force_include: Option<&'static str>) -> Fx {
    Fx {
        project: Some((p.to_path_buf(), p.to_path_buf())),
        walk_root: p.to_path_buf(),
        md: md_dir.join("x.md"),
        json: md_dir.join("x.json"),
        force_include,
    }
}

const WALK_DUP: (&str, &str) = (
    "walk",
    "the walk skips a link that resolves inside the walk root as a second NAME for a file it \
     reaches by its real path (indexer `is_symlink` block), before the door is asked",
);

/// `gather_from` paths are joined to the ACTIVE project's root; with no project they are joined to
/// the server's working directory, so a no-project layout's file is never what they name. And
/// with no active project `link_scan` reports no finding carrying the probe's cross-repository
/// token (observed on R13, a plain file), so it cannot show the marker either way.
const NO_PROJECT: &[(&str, &str)] = &[
    (
        "gather_file",
        "no active project: gather paths resolve against the server's cwd",
    ),
    (
        "gather_grep",
        "no active project: gather paths resolve against the server's cwd",
    ),
    (
        "config_value",
        "no active project: gather paths resolve against the server's cwd",
    ),
    (
        "link_scan",
        "no active project: the probe's cross-repo token is not reported",
    ),
];

/// Sites that build a relative spelling from the layout (`Fx::rel`), which is lossy for a
/// non-UTF-8 name, so what they would read is not the layout's file; and the three sites that
/// return STORED text for a seeded row, which for a lossy stored path is judged by the reindex,
/// not at read time (review round 3, R3-6: refusing it at read time hid every ordinary file below
/// a non-UTF-8 name). Those three are the accepted window; the reindex cell is
/// `a_non_utf8_path_row_is_withheld_and_removed_without_copying_its_text`.
const LOSSY: &[(&str, &str)] = &[
    (
        "find_semantic",
        "stored text of a lossy path: judged by the reindex (R3-6 window)",
    ),
    (
        "find_title",
        "stored text of a lossy path: judged by the reindex (R3-6 window)",
    ),
    (
        "backfill_stored",
        "stored text of a lossy path: judged by the reindex (R3-6 window)",
    ),
    ("gather_file", "Fx::rel is lossy for a non-UTF-8 name"),
    ("gather_grep", "Fx::rel is lossy for a non-UTF-8 name"),
    ("config_value", "Fx::rel is lossy for a non-UTF-8 name"),
    ("requirements", "Fx::rel is lossy for a non-UTF-8 name"),
    (
        "external_authority",
        "Fx::rel is lossy for a non-UTF-8 name",
    ),
];

/// `gather_grep` walks with `follow_links(false)`, which reports a LEAF link (the root entry
/// included) by its own type, so a leaf-link spelling is never grepped, inside or out. The
/// directory-link rows (R4, R6, R8) exercise the site.
const GREP_LEAF: (&str, &str) = (
    "gather_grep",
    "the grep walk never follows a leaf link, so this spelling is never grepped",
);

pub(super) fn layouts() -> Vec<Layout> {
    vec![
        Layout {
            name: "R1 plain file in the project",
            verdict: Verdict::Admit,
            comparable: true,
            why_not_comparable: "",
            skip: &[],
            build: |b| {
                let p = project(b);
                target(&p.join("docs"));
                fx(&p, &p.join("docs"), None)
            },
        },
        Layout {
            name: "R2 leaf link, absolute target outside the repository",
            verdict: Verdict::Refuse,
            comparable: true,
            why_not_comparable: "",
            skip: &[],
            build: |b| {
                let p = project(b);
                let out = b.join("out");
                target(&out);
                ln(out.join("x.md"), &p.join("docs/x.md"));
                ln(out.join("x.json"), &p.join("docs/x.json"));
                fx(&p, &p.join("docs"), None)
            },
        },
        Layout {
            name: "R3 leaf link, relative target outside the repository",
            verdict: Verdict::Refuse,
            comparable: true,
            why_not_comparable: "",
            skip: &[],
            build: |b| {
                let p = project(b);
                target(&b.join("out"));
                ln("../../out/x.md", &p.join("docs/x.md"));
                ln("../../out/x.json", &p.join("docs/x.json"));
                fx(&p, &p.join("docs"), None)
            },
        },
        Layout {
            name: "R4 directory link out of the repository (force_include reaches it)",
            verdict: Verdict::Refuse,
            comparable: true,
            why_not_comparable: "",
            skip: &[],
            build: |b| {
                let p = project(b);
                let out = b.join("out");
                target(&out);
                ln(&out, &p.join("docs"));
                fx(&p, &p.join("docs"), Some("docs/**"))
            },
        },
        Layout {
            name: "R5 leaf link to a file inside the repository",
            verdict: Verdict::Admit,
            comparable: true,
            why_not_comparable: "",
            skip: &[WALK_DUP, GREP_LEAF],
            build: |b| {
                let p = project(b);
                target(&p.join("real"));
                ln("../real/x.md", &p.join("docs/x.md"));
                ln("../real/x.json", &p.join("docs/x.json"));
                fx(&p, &p.join("docs"), None)
            },
        },
        Layout {
            name: "R6 directory link that stays inside the repository",
            verdict: Verdict::Admit,
            comparable: true,
            why_not_comparable: "",
            skip: &[],
            build: |b| {
                let p = project(b);
                target(&p.join("real"));
                ln(p.join("real"), &p.join("docs"));
                fx(&p, &p.join("docs"), Some("docs/**"))
            },
        },
        Layout {
            name: "R7 monorepo: leaf link out of the project, inside its git root",
            verdict: Verdict::Admit,
            comparable: true,
            why_not_comparable: "",
            skip: &[GREP_LEAF],
            build: |b| {
                let g = b.join("g");
                git(&g);
                let p = g.join("pkg");
                target(&g.join("shared"));
                ln("../../shared/x.md", &p.join("docs/x.md"));
                ln("../../shared/x.json", &p.join("docs/x.json"));
                Fx {
                    project: Some((p.clone(), g.clone())),
                    walk_root: p.clone(),
                    md: p.join("docs/x.md"),
                    json: p.join("docs/x.json"),
                    force_include: None,
                }
            },
        },
        Layout {
            name: "R8 nested repository whose directory link points into its superproject (F2)",
            verdict: Verdict::Refuse,
            comparable: true,
            why_not_comparable: "",
            skip: &[],
            build: |b| {
                let p = project(b);
                let n = p.join("nested");
                git(&n);
                target(&p.join("other"));
                ln(p.join("other"), &n.join("docs"));
                fx(&p, &n.join("docs"), Some("nested/docs/**"))
            },
        },
        Layout {
            name: "R9 dangling leaf link",
            verdict: Verdict::NoBytes,
            comparable: false,
            why_not_comparable: "a dangling link has no target to contain; there are no bytes",
            skip: &[],
            build: |b| {
                let p = project(b);
                let out = b.join("out");
                std::fs::create_dir_all(&out).unwrap();
                ln(out.join("x.md"), &p.join("docs/x.md"));
                ln(out.join("x.json"), &p.join("docs/x.json"));
                fx(&p, &p.join("docs"), None)
            },
        },
        Layout {
            name: "R12 no managed root, no repository: leaf link out",
            verdict: Verdict::Refuse,
            comparable: false,
            why_not_comparable: "with no root to name, the write door applies only its leaf rule; \
                 the read door reads only a spelling that crosses no link",
            skip: NO_PROJECT,
            build: |b| {
                let p = b.join("p");
                let out = b.join("out");
                target(&out);
                ln(out.join("x.md"), &p.join("docs/x.md"));
                ln(out.join("x.json"), &p.join("docs/x.json"));
                Fx {
                    project: None,
                    walk_root: p.clone(),
                    md: p.join("docs/x.md"),
                    json: p.join("docs/x.json"),
                    force_include: None,
                }
            },
        },
        Layout {
            name: "R13 no managed root, no repository: plain file",
            verdict: Verdict::Admit,
            comparable: true,
            why_not_comparable: "",
            skip: NO_PROJECT,
            build: |b| {
                let p = b.join("p");
                target(&p.join("docs"));
                Fx {
                    project: None,
                    walk_root: p.clone(),
                    md: p.join("docs/x.md"),
                    json: p.join("docs/x.json"),
                    force_include: None,
                }
            },
        },
        Layout {
            name: "R15 non-UTF-8 directory name, leaf links out (review round 2, V10)",
            verdict: Verdict::Refuse,
            comparable: true,
            why_not_comparable: "",
            skip: LOSSY,
            build: |b| {
                use std::os::unix::ffi::OsStrExt;
                let p = project(b);
                let d = p.join(std::ffi::OsStr::from_bytes(b"d\xff"));
                let out = b.join("out");
                target(&out);
                ln(out.join("x.md"), &d.join("x.md"));
                ln(out.join("x.json"), &d.join("x.json"));
                fx(&p, &d, None)
            },
        },
        Layout {
            name: "R14 no managed root, no repository: directory link out",
            verdict: Verdict::Refuse,
            comparable: false,
            why_not_comparable: "with no root to name, the write door cannot judge a parent link; \
                 the read door reads only a spelling that crosses no link",
            skip: NO_PROJECT,
            build: |b| {
                let p = b.join("p");
                let out = b.join("out");
                target(&out);
                ln(&out, &p.join("docs"));
                Fx {
                    project: None,
                    walk_root: p.clone(),
                    md: p.join("docs/x.md"),
                    json: p.join("docs/x.json"),
                    force_include: Some("docs/**"),
                }
            },
        },
    ]
}

// ---------------------------------------------------------------------------------------------
// The sites.
// ---------------------------------------------------------------------------------------------

struct Cap {
    seen: Arc<parking_lot::Mutex<Vec<String>>>,
    dims: usize,
}

#[async_trait::async_trait]
impl codescout_embed::Embedder for Cap {
    fn dimensions(&self) -> usize {
        self.dims
    }
    async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
        let mut g = self.seen.lock();
        for t in texts {
            g.push(t.to_string());
        }
        Ok(texts.iter().map(|_| vec![0.5; self.dims]).collect())
    }
}

fn svc(
    dims: usize,
) -> (
    Arc<crate::librarian::embedding::EmbeddingService>,
    Arc<parking_lot::Mutex<Vec<String>>>,
) {
    let seen = Arc::new(parking_lot::Mutex::new(Vec::new()));
    (
        Arc::new(crate::librarian::embedding::EmbeddingService::new(
            Arc::new(Cap {
                seen: seen.clone(),
                dims,
            }),
        )),
        seen,
    )
}

fn rules() -> Vec<crate::librarian::classify::CompiledRule> {
    crate::librarian::classify::load_rules("[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n")
        .unwrap()
}

fn ctx_for(
    fx: &Fx,
    embedding: Option<Arc<crate::librarian::embedding::EmbeddingService>>,
) -> (ToolContext, Arc<InMemoryArtifactStore>) {
    let store = Arc::new(InMemoryArtifactStore::default());
    let mut b = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_rules(rules())
        .with_artifact_store(store.clone());
    if let Some((p, g)) = &fx.project {
        b = b.with_current_project(Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: p.clone(),
                git_root: g.clone(),
                main_root: None,
                umbrella: None,
            },
        ));
    }
    if let Some(e) = embedding {
        b = b.with_embedding(e);
    }
    (b.build(), store)
}

/// Seed a catalog row for `path` WITHOUT reading it: the state an older catalog, or a file
/// replaced by a link after indexing, leaves behind.
fn seed(ctx: &ToolContext, path: &Path, title: Option<&str>) -> String {
    let id = crate::librarian::ids::artifact_id_from_abs(path);
    let mut b = artifact::TestArtifactRowBuilder::new(&id)
        .with_abs_path(path)
        .with_kind("doc")
        .with_status("draft");
    if let Some(t) = title {
        b = b.with_title(t);
    }
    artifact::upsert(&ctx.catalog.lock(), &b.build()).unwrap();
    id
}

fn aug_row(id: &str, params: serde_json::Value) -> augmentation::AugmentationRow {
    augmentation::AugmentationRow {
        artifact_id: id.to_string(),
        prompt: "p".to_string(),
        params: params.to_string(),
        last_refreshed_at: None,
        refresh_count: 0,
        created_at: "2026-01-01T00:00:00.000Z".to_string(),
        updated_at: "2026-01-01T00:00:00.000Z".to_string(),
        render_template: None,
        params_schema: None,
        append_mode: false,
        history_cap: None,
        entry_collection: None,
        refreshed_at_commit: None,
    }
}

/// A regular control document in the project, to carry a `gather_from` augmentation.
fn control_row(ctx: &ToolContext, fx: &Fx, params: serde_json::Value) -> String {
    let c = fx.base_dir().join("control.md");
    std::fs::write(&c, "# control\n").unwrap();
    let id = seed(ctx, &c, Some("control"));
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&id, params)).unwrap();
    id
}

fn seed_chunks(ctx: &ToolContext, id: &str) -> Vec<crate::librarian::indexer::EmbedQueueItem> {
    crate::librarian::indexer::embed_queue_items(
        &ctx.catalog.lock(),
        id,
        None,
        &doc_text(),
        crate::librarian::catalog::chunk::ChunkGrain::Chunk,
    )
    .unwrap()
}

fn has(v: &impl std::fmt::Display) -> bool {
    v.to_string().contains(MARK)
}

fn out<T: std::fmt::Display, E: std::fmt::Display>(r: Result<T, E>) -> bool {
    match r {
        Ok(v) => has(&v),
        Err(e) => has(&format!("{e:#}")),
    }
}

fn out_anyhow<T: std::fmt::Display>(r: anyhow::Result<T>) -> bool {
    match r {
        Ok(v) => has(&v),
        Err(e) => has(&format!("{e:#}")),
    }
}

pub(super) type SiteFn =
    for<'a> fn(&'a Fx) -> std::pin::Pin<Box<dyn std::future::Future<Output = bool> + 'a>>;

/// `(name, row of the Phase 1 path table it drives, adapter)`.
pub(super) fn read_sites() -> Vec<(&'static str, &'static str, SiteFn)> {
    vec![
        ("walk", "#1 indexer walk", |fx| Box::pin(site_walk(fx))),
        ("get", "#7 doc(get)", |fx| Box::pin(site_get(fx))),
        ("context", "#8 librarian(context) file anchor", |fx| {
            Box::pin(site_context(fx))
        }),
        (
            "context_entry",
            "#9 librarian(context) entry anchor",
            |fx| Box::pin(site_context_entry(fx)),
        ),
        ("gather_body", "#10 doc(gather) current_body", |fx| {
            Box::pin(site_gather_body(fx))
        }),
        ("gather_file", "#11 gather_from file", |fx| {
            Box::pin(site_gather_file(fx))
        }),
        ("gather_grep", "#12 gather_from grep", |fx| {
            Box::pin(site_gather_grep(fx))
        }),
        ("config_value", "#13 gather_from config_value", |fx| {
            Box::pin(site_config_value(fx))
        }),
        ("find_semantic", "#14 doc(find) semantic snippet", |fx| {
            Box::pin(site_find_semantic(fx))
        }),
        ("find_title", "#15 doc(find) stored title", |fx| {
            Box::pin(site_find_title(fx))
        }),
        ("backfill_disk", "#3 backfill, no stored chunks", |fx| {
            Box::pin(site_backfill_disk(fx))
        }),
        ("backfill_stored", "#4 backfill, stored chunks", |fx| {
            Box::pin(site_backfill_stored(fx))
        }),
        ("doctor", "#18 doctor scans", |fx| Box::pin(site_doctor(fx))),
        ("link_scan", "#19 link_scan", |fx| {
            Box::pin(site_link_scan(fx))
        }),
        ("requirements", "#20 requirements tag_values_from", |fx| {
            Box::pin(site_requirements(fx))
        }),
        (
            "external_authority",
            "#21 doctor external_prefix authority",
            |fx| Box::pin(site_external_authority(fx)),
        ),
    ]
}

async fn site_walk(fx: &Fx) -> bool {
    if let Some(pat) = fx.force_include {
        let cfg = fx.walk_root.join(".codescout");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("project.toml"),
            format!("[ignored_paths]\nforce_include = [\"{pat}\"]\n"),
        )
        .unwrap();
    }
    let (ctx, _) = ctx_for(fx, None);
    let id = crate::librarian::ids::artifact_id_from_abs(&fx.md);
    let (_, queue) = crate::librarian::indexer::index_repo_sync(
        &ctx.catalog.lock(),
        &rules(),
        &fx.walk_root,
        &globset::GlobSet::empty(),
        true,
        false,
        false,
    )
    .unwrap();
    let queued = queue
        .iter()
        .any(|q| q.artifact_id == id && q.text.contains(MARK));
    let stored: i64 = ctx
        .catalog
        .lock()
        .conn
        .query_row(
            "SELECT COUNT(*) FROM artifact_chunk WHERE artifact_id = ?1 AND instr(content, ?2) > 0",
            rusqlite::params![id, MARK],
            |r| r.get(0),
        )
        .unwrap();
    queued || stored > 0
}

async fn site_get(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = seed(&ctx, &fx.md, None);
    let full = get::call(&ctx, json!({"id": id, "full": true})).await;
    let lines = get::call(&ctx, json!({"id": id, "start_line": 1, "end_line": 40})).await;
    let section = get::call(&ctx, json!({"id": id, "heading": "## A"})).await;
    out_anyhow(full) || out_anyhow(lines) || out_anyhow(section)
}

async fn site_context(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = seed(&ctx, &fx.md, None);
    out_anyhow(context::call(&ctx, json!({"anchor_id": id})).await)
}

async fn site_context_entry(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = seed(&ctx, &fx.md, None);
    ctx.catalog
        .lock()
        .conn
        .execute(
            "UPDATE artifact SET slug = 'probe' WHERE id = ?1",
            rusqlite::params![id],
        )
        .unwrap();
    out_anyhow(context::call(&ctx, json!({"anchor_id": "probe:T-1"})).await)
}

async fn site_gather_body(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = seed(&ctx, &fx.md, None);
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&id, json!({}))).unwrap();
    out_anyhow(refresh::call(&ctx, json!({"id": id})).await)
}

async fn site_gather_file(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = control_row(
        &ctx,
        fx,
        json!({"gather_from": [{"source": "file", "path": fx.rel(&fx.md)}]}),
    );
    out_anyhow(refresh::call(&ctx, json!({"id": id})).await)
}

async fn site_gather_grep(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = control_row(
        &ctx,
        fx,
        // The pattern must NOT be the marker: `refresh` echoes its sources, so a pattern spelling
        // the marker would "come out" without the file being read.
        json!({"gather_from": [{"source": "grep", "pattern": "body line", "path": fx.rel(&fx.md)}]}),
    );
    out_anyhow(refresh::call(&ctx, json!({"id": id})).await)
}

async fn site_config_value(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = control_row(
        &ctx,
        fx,
        json!({"gather_from": [{"source": "config_value", "path": fx.rel(&fx.json), "key": "k"}]}),
    );
    out_anyhow(refresh::call(&ctx, json!({"id": id})).await)
}

async fn site_find_semantic(fx: &Fx) -> bool {
    let (e, _) = svc(4);
    let (ctx, store) = ctx_for(fx, Some(e));
    let id = seed(&ctx, &fx.md, None);
    let pid = fx
        .project
        .as_ref()
        .map(|(p, _)| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    for item in seed_chunks(&ctx, &id) {
        store
            .upsert(&pid, &item.chunk_id, &id, &[0.5, 0.5, 0.5, 0.5])
            .await
            .unwrap();
    }
    out_anyhow(find::call(&ctx, json!({"semantic": "anything", "limit": 20})).await)
}

async fn site_find_title(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    seed(&ctx, &fx.md, Some(&format!("{MARK} title")));
    out_anyhow(find::call(&ctx, json!({"limit": 50})).await)
}

async fn site_backfill_disk(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    seed(&ctx, &fx.md, None);
    let (e, seen) = svc(4);
    let store = InMemoryArtifactStore::default();
    let _ = crate::librarian::indexer::backfill_chunk_vectors(
        &ctx.catalog,
        &e,
        10,
        None,
        crate::librarian::indexer::BackfillTarget::Store {
            store: &store,
            project_id: "p",
        },
        &[],
    )
    .await;
    let embedded = seen.lock().iter().any(|t| t.contains(MARK));
    embedded
}

async fn site_backfill_stored(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = seed(&ctx, &fx.md, None);
    seed_chunks(&ctx, &id);
    let (e, seen) = svc(768);
    let _ = crate::librarian::indexer::backfill_chunk_vectors(
        &ctx.catalog,
        &e,
        10,
        None,
        crate::librarian::indexer::BackfillTarget::SqliteVec,
        &[],
    )
    .await;
    let embedded = seen.lock().iter().any(|t| t.contains(MARK));
    embedded
}

async fn site_doctor(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    seed(&ctx, &fx.md, None);
    // A terminal bug row, so `terminal_status_with_caveat` (a scan that reads through the scope's
    // door, `DoctorScope::read_text`) echoes the target's `unverified:` caveat; the frontmatter
    // `id:` is echoed by `check_frontmatter_id_matches_catalog` (a door passed in). Both must stay
    // silent for a refused layout.
    let id = crate::librarian::ids::artifact_id_from_abs(&fx.md);
    ctx.catalog
        .lock()
        .conn
        .execute(
            "UPDATE artifact SET kind = 'bug', status = 'fixed' WHERE id = ?1",
            rusqlite::params![id],
        )
        .unwrap();
    out_anyhow(doctor::call(&ctx, json!({})).await)
}

async fn site_link_scan(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    seed(&ctx, &fx.md, None);
    out_anyhow(link_scan::call(&ctx, json!({})).await)
}

async fn site_requirements(fx: &Fx) -> bool {
    let base = fx.base_dir();
    let cfg = base.join(".codescout");
    std::fs::create_dir_all(&cfg).unwrap();
    std::fs::write(
        cfg.join("requirements.toml"),
        format!(
            "[[require]]\nkind = \"bug\"\ntag_prefix = \"cluster/\"\ntag_values_from = [\"{}\"]\n\
             tag_declared_by = \"- \"\n",
            fx.rel(&fx.md)
        ),
    )
    .unwrap();
    out(crate::librarian::requirements::enforce_on_create(
        &base,
        "bug",
        &["cluster/nope".to_string()],
        "body",
    )
    .map(|_| ""))
}

async fn site_external_authority(fx: &Fx) -> bool {
    let base = fx.base_dir();
    let declaring = base.join("declaring.md");
    std::fs::write(&declaring, "# d\n").unwrap();
    // The site's output is a yes/no ("holds a `TC-<n>` id"), so "the bytes came out" here means
    // "the target was read": `None` is only possible after reading `TC-1` from it. The authority
    // is relative to the declaring file's git root (else its directory), as the site reads it.
    let anchor = fx
        .project
        .as_ref()
        .map(|(_, g)| g.clone())
        .unwrap_or_else(|| base.clone());
    let authority = fx
        .md
        .strip_prefix(&anchor)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let door = crate::librarian::read_door::ReadDoor::new(fx.roots());
    doctor::external_authority_problem(&door, "TC", &declaring, &authority).is_none()
}

/// The read-modify-write site: `doc(update)` with a `replace` whose guard lists the nested
/// headings it would wipe. That error is built from the content BEFORE the write door runs.
async fn site_update(fx: &Fx) -> bool {
    let (ctx, _) = ctx_for(fx, None);
    let id = seed(&ctx, &fx.md, None);
    out_anyhow(
        update::call(
            &ctx,
            json!({"id": id, "patch": {"body_edits": [
                {"heading": "## A", "action": "replace", "content": "new"}
            ]}}),
        )
        .await,
    )
}

fn write_containment_admits(fx: &Fx) -> bool {
    crate::librarian::util::refuse_catalogued_path_outside_repo(&fx.roots(), &fx.md, "write")
        .is_ok()
}

fn write_door_admits(fx: &Fx) -> bool {
    crate::librarian::util::refuse_write_through_symlink(&fx.md).is_ok()
        && write_containment_admits(fx)
}

/// Run one site over one freshly built layout (every site gets its own tempdir, so no site sees
/// another site's catalog or config).
pub(super) async fn run_site(layout: &Layout, site: SiteFn) -> bool {
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    let fx = (layout.build)(&base);
    site(&fx).await
}

#[tokio::test]
async fn read_door_site_table() {
    let mut failures: Vec<String> = Vec::new();
    let mut table: Vec<String> = Vec::new();
    for layout in layouts() {
        let mut cells: Vec<String> = Vec::new();
        for (site, row, f) in read_sites() {
            if let Some((_, why)) = layout.skip.iter().find(|(s, _)| *s == site) {
                cells.push(format!("{site}=n/a({why})"));
                continue;
            }
            let seen = run_site(&layout, f).await;
            let want = layout.verdict == Verdict::Admit;
            cells.push(format!("{site}={}", if seen { "OUT" } else { "-" }));
            if seen != want {
                failures.push(format!(
                    "{} / {site} ({row}): marker {} but the verdict is {:?}",
                    layout.name,
                    if seen { "CAME OUT" } else { "did not come out" },
                    layout.verdict
                ));
            }
        }
        // The read-modify-write site reads only what BOTH doors admit: the full write door (leaf
        // rule included) and the read door (which, with no root to name, is the stricter one).
        let tmp = tempfile::tempdir().unwrap();
        let base = std::fs::canonicalize(tmp.path()).unwrap();
        let fx = (layout.build)(&base);
        let write_full = write_door_admits(&fx);
        let write_contained = write_containment_admits(&fx);
        let upd = site_update(&fx).await;
        let upd_want = write_full && layout.verdict == Verdict::Admit;
        cells.push(format!("update={}", if upd { "OUT" } else { "-" }));
        if upd != upd_want {
            failures.push(format!(
                "{} / update (#17 read before the write door): marker {} but the write door {}",
                layout.name,
                if upd { "CAME OUT" } else { "did not come out" },
                if write_full { "admits" } else { "refuses" }
            ));
        }
        if layout.comparable && write_contained != (layout.verdict == Verdict::Admit) {
            failures.push(format!(
                "{}: the write door's containment answer ({}) disagrees with the read verdict {:?}",
                layout.name,
                if write_contained { "admit" } else { "refuse" },
                layout.verdict
            ));
        }
        if !layout.comparable {
            assert!(
                !layout.why_not_comparable.is_empty(),
                "{}: a non-comparable row must say why",
                layout.name
            );
        }
        table.push(format!(
            "{:?} {} [write door: containment {}, full {}] {}",
            layout.verdict,
            layout.name,
            write_contained,
            write_full,
            cells.join(" ")
        ));
    }
    for line in &table {
        println!("TABLE {line}");
    }
    assert!(
        failures.is_empty(),
        "{} cell(s) break the read invariant:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------------------------------
// The deny-list layer: a target INSIDE the judged root that the read deny-list names.
// ---------------------------------------------------------------------------------------------

/// The deny-list rows. `$HOME` decides the deny list and a test must not mutate the process
/// environment, so this re-executes its own binary with `HOME` set for the child only. The child
/// builds the layout INSIDE that fake home (a repository whose `.ssh/` holds the target, the
/// active project being the home itself), so containment ADMITS and only the deny layer can
/// refuse. It exits non-zero on any failed assertion.
#[test]
fn deny_listed_target_inside_a_managed_root_reaches_no_read_site() {
    let fake = tempfile::tempdir().unwrap();
    let home = std::fs::canonicalize(fake.path()).unwrap().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "librarian::tools::read_door_tests::read_door_deny_child",
            "--ignored",
            "--nocapture",
        ])
        .env("HOME", &home)
        .env("CS_READ_DOOR_HOME", &home)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("DENY-CHILD-RAN"),
        "the deny-list child failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[tokio::test]
#[ignore = "child of deny_listed_target_inside_a_managed_root_reaches_no_read_site"]
async fn read_door_deny_child() {
    let home = PathBuf::from(std::env::var("CS_READ_DOOR_HOME").expect("run by the parent test"));
    assert_eq!(
        std::env::var("HOME").ok().map(PathBuf::from),
        Some(home.clone()),
        "refusing to run unless HOME is the throwaway directory the parent created"
    );
    git(&home);
    target(&home.join(".ssh"));
    std::fs::create_dir_all(home.join(".docker")).unwrap();
    std::fs::write(
        home.join(".docker/config.json"),
        format!("{{\"auths\": {{\"x\": {{\"auth\": \"{MARK}\"}}}}}}"),
    )
    .unwrap();
    let fx = Fx {
        project: Some((home.clone(), home.clone())),
        walk_root: home.clone(),
        md: home.join(".ssh/x.md"),
        json: home.join(".ssh/x.json"),
        force_include: Some(".ssh/**"),
    };
    assert!(
        write_containment_admits(&fx),
        "precondition: containment admits the target, so only the deny layer can refuse it"
    );
    let mut failures = Vec::new();
    for (site, row, f) in read_sites() {
        if f(&fx).await {
            failures.push(format!(
                "{site} ({row}) returned a deny-listed file's bytes"
            ));
        }
    }
    if site_update(&fx).await {
        failures.push("update (#17) read a deny-listed file before refusing".to_string());
    }
    // `params_path` (#16): an agent-named path gets the deny list only (decision D5).
    let (ctx, _) = ctx_for(&fx, None);
    let id = control_row(&ctx, &fx, json!({}));
    let r = super::augment::call(
        &ctx,
        json!({"id": id, "prompt": "p",
               "params_path": home.join(".docker/config.json").to_string_lossy()}),
    )
    .await;
    if r.is_ok() || out_anyhow(get::call(&ctx, json!({"id": id})).await) {
        failures.push("params_path (#16) read a deny-listed file".to_string());
    }
    println!("DENY-CHILD-RAN");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Decision D5: `params_path` is an agent-named path and gets `read_file`'s deny list, NOT a
/// containment check, because its documented use is a JSON file the agent wrote to a scratch or
/// temp directory. A pin: green before the fix, and it must stay green after it.
#[tokio::test]
async fn params_path_still_reads_a_json_file_outside_the_repository() {
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    let fx = (layouts()[0].build)(&base);
    let scratch = base.join("scratch");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(scratch.join("params.json"), "{\"rows\": [1, 2, 3]}").unwrap();
    let (ctx, _) = ctx_for(&fx, None);
    let id = control_row(&ctx, &fx, json!({}));
    super::augment::call(
        &ctx,
        json!({"id": id, "prompt": "p",
               "params_path": scratch.join("params.json").to_string_lossy()}),
    )
    .await
    .expect("a params file outside the repository and off the deny list is accepted");
    let got = get::call(&ctx, json!({"id": id})).await.unwrap();
    assert_eq!(got["augmentation"]["params"]["rows"], json!([1, 2, 3]));
}

// ---------------------------------------------------------------------------------------------
// Repository configuration files (#25): fixed names under the project root.
// ---------------------------------------------------------------------------------------------

/// Each loader of a repository config file, over three spellings of the file: a plain file, an
/// in-repo link (ADMIT: the bytes are the repository's own) and a link out of the repository
/// (REFUSE). Each loader's observation is whether the config's content TOOK EFFECT.
#[test]
fn repository_config_files_are_read_only_through_the_door() {
    type Loader = fn(&Path) -> bool;
    let wanted_backend = if cfg!(feature = "server-stack") {
        "sqlite-vec"
    } else {
        "qdrant"
    };
    let backend_toml = format!("[librarian]\nvector_backend = \"{wanted_backend}\"\n");
    let loaders: Vec<(&str, String, Loader)> = vec![
        (
            ".codescout/librarian.toml",
            "[[rule]]\nglob = \"**/*.md\"\nkind = \"fromconfig\"\n".to_string(),
            |p| {
                crate::librarian::classify::load_project_rules(p)
                    .map(|r| !r.is_empty())
                    .unwrap_or(false)
            },
        ),
        (
            ".codescout/project.toml",
            "[librarian]\nchunk_grain = false\n".to_string(),
            |p| {
                crate::librarian::catalog::chunk::ChunkGrain::resolve(p.to_str())
                    == crate::librarian::catalog::chunk::ChunkGrain::Artifact
            },
        ),
        (".codescout/project.toml", backend_toml, |p| {
            let got =
                crate::librarian::artifact_store::ArtifactBackend::resolve_with(None, p.to_str());
            let default = crate::librarian::artifact_store::ArtifactBackend::resolve_with(
                None,
                Some("/nonexistent-codescout-probe"),
            );
            got != default
        }),
        (
            ".codescout/project.toml",
            "[ignored_paths]\nforce_include = [\"hidden/**\"]\n".to_string(),
            |p| {
                std::fs::write(p.join(".gitignore"), "hidden/\n").unwrap();
                std::fs::create_dir_all(p.join("hidden")).unwrap();
                std::fs::write(p.join("hidden/y.md"), "# y\n").unwrap();
                let cat = Catalog::open_in_memory().unwrap();
                crate::librarian::indexer::index_repo_sync(
                    &cat,
                    &rules(),
                    p,
                    &globset::GlobSet::empty(),
                    false,
                    false,
                    false,
                )
                .unwrap();
                let id = crate::librarian::ids::artifact_id_from_abs(&p.join("hidden/y.md"));
                artifact::get(&cat, &id).unwrap().is_some()
            },
        ),
        (
            ".codescout/requirements.toml",
            "[[require]]\nkind = \"bug\"\nsections = [\"Zzz configured section\"]\n".to_string(),
            |p| {
                crate::librarian::requirements::enforce_on_create(p, "bug", &[], "body")
                    .err()
                    .is_some_and(|e| format!("{e:#}").contains("Zzz configured section"))
            },
        ),
        (
            ".codescout/workspace.toml",
            "[[umbrella]]\nname = \"fromconfig\"\nmembers = []\n".to_string(),
            |p| !crate::librarian::current_project::load_project_umbrellas(p).is_empty(),
        ),
    ];
    let mut failures = Vec::new();
    for (rel, text, load) in &loaders {
        for (shape, want) in [
            ("plain", true),
            ("in-repo link", true),
            ("outward link", false),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let base = std::fs::canonicalize(tmp.path()).unwrap();
            let p = project(&base);
            let cfg = p.join(rel);
            std::fs::create_dir_all(cfg.parent().unwrap()).unwrap();
            match shape {
                "plain" => std::fs::write(&cfg, text).unwrap(),
                "in-repo link" => {
                    std::fs::write(p.join("cfg-real.toml"), text).unwrap();
                    ln(p.join("cfg-real.toml"), &cfg);
                }
                _ => {
                    let out = base.join("out");
                    std::fs::create_dir_all(&out).unwrap();
                    std::fs::write(out.join("cfg.toml"), text).unwrap();
                    ln(out.join("cfg.toml"), &cfg);
                }
            }
            let took_effect = load(&p);
            if took_effect != want {
                failures.push(format!(
                    "{rel} as {shape}: content {} effect",
                    if took_effect { "TOOK" } else { "did not take" }
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---------------------------------------------------------------------------------------------
// Rows an older walk catalogued (#2): reclaimed, or kept and cleared (decision D4).
// ---------------------------------------------------------------------------------------------

/// A project with a regular `docs/x.md`, indexed; then `docs/x.md` replaced by a link to a file
/// outside the repository (what a pulled commit can do). Returns the context, the id, the link and
/// the dirs.
/// The REAL pre-fix state (review round 1, F6): the old walk read the TARGET, so the row's title,
/// fields, chunk text and vectors are the target's (`doc_text`, which carries `MARK`). Built by
/// indexing a regular file holding the target's bytes, then replacing it (`dir == false`: the leaf;
/// `dir == true`: its directory `docs/sub`) by a link out of the repository. The project opts in to
/// the audit shard export (`.gitattributes`) only AFTER that first index: shard lines a pre-fix
/// catalog already exported are not recallable by any fix, and are out of what the test asserts.
async fn prefix_state(
    dir: bool,
    with_history: bool,
) -> (
    tempfile::TempDir,
    ToolContext,
    Arc<InMemoryArtifactStore>,
    String,
    PathBuf,
    PathBuf,
) {
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&base);
    let docs = if dir {
        p.join("docs/sub")
    } else {
        p.join("docs")
    };
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("x.md"), doc_text()).unwrap();
    let fx = fx(&p, &docs, None);
    let (e, _) = svc(4);
    let (ctx, store) = ctx_for(&fx, Some(e));
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let id = crate::librarian::ids::artifact_id_from_abs(&docs.join("x.md"));
    let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
    assert!(
        row.title.as_deref().is_some_and(|t| t.contains(MARK))
            && store.chunks_under(&id) > 0
            && !catalog_hits(&ctx, None).is_empty(),
        "precondition: the target's text is in the catalog (title {:?}), or every \"nothing \
         remains\" assertion below is vacuous",
        row.title
    );
    if with_history {
        augmentation::upsert(&ctx.catalog.lock(), &aug_row(&id, json!({"note": "mine"}))).unwrap();
    }
    std::fs::write(
        p.join(".gitattributes"),
        ".codescout/audit/*.jsonl merge=union\n",
    )
    .unwrap();
    target(&base.join("out"));
    if dir {
        std::fs::remove_dir_all(&docs).unwrap();
        ln(base.join("out"), &docs);
    } else {
        std::fs::remove_file(docs.join("x.md")).unwrap();
        ln(base.join("out/x.md"), &docs.join("x.md"));
    }
    (tmp, ctx, store, id, p, docs.join("x.md"))
}

/// Every text value in every table of the catalog that carries `MARK`, as `table.column`, with
/// two exceptions, both the open decision "Deviation 8" of the issue: the `slug` of `kept` (an
/// escaping row kept for its history keeps its slug), and the audit `row_id` of a cascaded
/// `entry_cite` delete (`slug:local→dst_ref`, the routing key). The vec0 tables hold vectors, not
/// text, and are skipped.
fn catalog_hits(ctx: &ToolContext, kept: Option<&str>) -> Vec<String> {
    let sha = crate::librarian::util::sha_of_bytes(doc_text().as_bytes());
    let cat = ctx.catalog.lock();
    let tables: Vec<String> = cat
        .conn
        .prepare(
            "SELECT name FROM sqlite_master WHERE type = 'table' \
             AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'artifact_vec%'",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let mut hits = Vec::new();
    for t in tables {
        let mut st = cat.conn.prepare(&format!("SELECT * FROM \"{t}\"")).unwrap();
        let cols: Vec<String> = st.column_names().iter().map(|c| c.to_string()).collect();
        let mut rows = st.query([]).unwrap();
        while let Some(r) = rows.next().unwrap() {
            let row_id: Option<String> = cols
                .iter()
                .position(|c| c == "id")
                .and_then(|i| r.get::<_, String>(i).ok());
            let audited_tbl: Option<String> = cols
                .iter()
                .position(|c| c == "tbl")
                .and_then(|i| r.get::<_, String>(i).ok());
            for (i, c) in cols.iter().enumerate() {
                let v: rusqlite::types::Value = r.get(i).unwrap();
                let text = match v {
                    rusqlite::types::Value::Text(s) => s,
                    rusqlite::types::Value::Blob(b) => String::from_utf8_lossy(&b).into_owned(),
                    _ => continue,
                };
                // The marker, and the target's content hash (a digest of the outside file is
                // derived from it as surely as its title is).
                if !text.contains(MARK) && !text.contains(&sha) {
                    continue;
                }
                if t == "artifact" && c == "slug" && row_id.as_deref() == kept && kept.is_some() {
                    continue;
                }
                // A cascaded citation's audit `row_id` (`slug:local→dst_ref`) is the trail's
                // routing key and keeps the slug: Deviation 8, open. Its PAYLOAD is checked.
                if t == "catalog_audit"
                    && c == "row_id"
                    && audited_tbl.as_deref() == Some("entry_cite")
                {
                    continue;
                }
                let head: String = text.chars().take(160).collect();
                hits.push(format!("{t}.{c}: {head}"));
            }
        }
    }
    hits
}

/// Lines of the exported audit shards under `<project>/.codescout/audit/` that carry `MARK`.
fn shard_hits(p: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(p.join(".codescout/audit")) {
        for e in rd.flatten() {
            let s = std::fs::read_to_string(e.path()).unwrap_or_default();
            for l in s.lines().filter(|l| l.contains(MARK)) {
                out.push(l.chars().take(200).collect());
            }
        }
    }
    out
}

/// Review round 1, F1: after the reindex that reclaims or clears an escaping row, nothing the
/// target supplied is anywhere in the catalog (every table, `catalog_audit` included) or in an
/// exported shard. Also requires that the export DID run, so an empty shard directory cannot pass
/// for a clean one.
fn assert_nothing_of_the_target_remains(
    ctx: &ToolContext,
    p: &Path,
    kept: Option<&str>,
    v: &serde_json::Value,
) {
    let hits = catalog_hits(ctx, kept);
    assert!(
        hits.is_empty(),
        "the target's text survives in the catalog:\n{}",
        hits.join("\n")
    );
    let shards = shard_hits(p);
    assert!(
        shards.is_empty(),
        "the target's text was exported to a shard:\n{}",
        shards.join("\n")
    );
    assert!(
        v["audit_export"]["exported"].as_u64().unwrap_or(0) > 0,
        "the audit export must have run for the shard check to mean anything: {}",
        v["audit_export"]
    );
}

fn chunk_count(ctx: &ToolContext, id: &str) -> i64 {
    ctx.catalog
        .lock()
        .conn
        .query_row(
            "SELECT COUNT(*) FROM artifact_chunk WHERE artifact_id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap()
}

#[tokio::test]
async fn a_file_replaced_by_an_outward_link_is_reclaimed_with_its_stored_text() {
    let (_tmp, ctx, store, id, p, link) = prefix_state(false, false).await;
    let v = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    assert!(
        artifact::get(&ctx.catalog.lock(), &id).unwrap().is_none(),
        "the row must be reclaimed: {v}"
    );
    assert_eq!(
        chunk_count(&ctx, &id),
        0,
        "no stored chunk text may survive"
    );
    assert_eq!(store.chunks_under(&id), 0, "no vector may survive");
    let link_s = link.to_string_lossy().into_owned();
    assert!(
        v["reclaimed_escaping_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(id)),
        "{v}"
    );
    assert!(
        v["reclaimed_escaping_paths"]
            .as_array()
            .unwrap()
            .contains(&json!(link_s)),
        "{v}"
    );
    assert!(
        v["escaping_skipped_paths"]
            .as_array()
            .unwrap()
            .contains(&json!(link_s)),
        "{v}"
    );
    assert!(
        !has(&v),
        "the response must carry nothing of the target: {v}"
    );
    assert_nothing_of_the_target_remains(&ctx, &p, None, &v);
}

#[tokio::test]
async fn an_outward_row_with_history_is_kept_and_cleared_of_everything_its_target_supplied() {
    let (_tmp, ctx, store, id, p, link) = prefix_state(false, true).await;
    let v = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let row = artifact::get(&ctx.catalog.lock(), &id)
        .unwrap()
        .expect("a row with history is KEPT (round 3's keep-forever decision)");
    let link_s = link.to_string_lossy().into_owned();
    assert!(
        v["escaping_kept_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(id)),
        "{v}"
    );
    assert!(
        v["escaping_kept_paths"]
            .as_array()
            .unwrap()
            .contains(&json!(link_s)),
        "{v}"
    );
    assert_eq!(chunk_count(&ctx, &id), 0, "stored chunk text is dropped");
    assert_eq!(store.chunks_under(&id), 0, "vectors are dropped");
    assert_eq!(row.title, None, "the title is cleared");
    assert_eq!(row.kind, "unknown");
    assert_eq!(row.status, "unknown");
    assert!(row.tags.is_empty() && row.owners.is_empty());
    assert_eq!(row.topic, None);
    assert_eq!(row.time_scope, None);
    assert_eq!(
        row.file_sha256, "",
        "a hash of the target is derived from it"
    );
    let embedded: Option<String> = ctx
        .catalog
        .lock()
        .conn
        .query_row(
            "SELECT embedded_sha256 FROM artifact WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(embedded, None);
    let aug = augmentation::get(&ctx.catalog.lock(), &id).unwrap();
    assert!(
        aug.is_some_and(|a| a.params.contains("mine")),
        "history is kept"
    );
    let got = get::call(&ctx, json!({"id": id, "full": true}))
        .await
        .unwrap();
    assert!(!has(&got), "a kept row shows nothing of its target: {got}");
    assert_nothing_of_the_target_remains(&ctx, &p, Some(&id), &v);
    // Idempotent: a second reindex keeps the row, writes nothing new, and still leaves nothing.
    let v2 = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    assert!(
        v2["escaping_kept_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(id)),
        "{v2}"
    );
    assert!(catalog_hits(&ctx, Some(&id)).is_empty());
    assert!(shard_hits(&p).is_empty());
}

/// The swapped-DIRECTORY form: `docs/sub/` replaced by a link out after indexing. The walk never
/// descends a directory link, so the row goes unseen and reclamation judges it.
#[tokio::test]
async fn a_row_below_a_directory_swapped_for_an_outward_link_is_kept_and_cleared() {
    let (_tmp, ctx, store, id, p, _link) = prefix_state(true, true).await;
    let v = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let row = artifact::get(&ctx.catalog.lock(), &id)
        .unwrap()
        .expect("kept");
    assert!(
        v["escaping_kept_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(id)),
        "{v}"
    );
    assert_eq!(chunk_count(&ctx, &id), 0);
    assert_eq!(store.chunks_under(&id), 0);
    assert_eq!(row.title, None);
    let got = get::call(&ctx, json!({"id": id, "full": true}))
        .await
        .unwrap();
    assert!(!has(&got), "{got}");
    assert_nothing_of_the_target_remains(&ctx, &p, Some(&id), &v);
}

// ---------------------------------------------------------------------------------------------
// Review round 2.
// ---------------------------------------------------------------------------------------------

/// R2-1a: the pre-fix row's link now DANGLES (its target was removed). The row is gone from the
/// filesystem's point of view, but its path is still a link, so its removal must elide like an
/// escaping row's.
#[tokio::test]
async fn a_dangling_outward_link_row_is_removed_without_copying_its_text() {
    let (_tmp, ctx, _store, id, p, _link) = prefix_state(false, false).await;
    std::fs::remove_file(p.parent().unwrap().join("out/x.md")).unwrap();
    cite_from(&ctx, &id);
    let v = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    assert!(
        artifact::get(&ctx.catalog.lock(), &id).unwrap().is_none(),
        "{v}"
    );
    assert_nothing_of_the_target_remains(&ctx, &p, None, &v);
}

/// R2-1b, a KNOWN GAP, kept as an expected failure: the advised remedy (remove the link) turns the
/// row into an ordinary vanished row, and nothing on the filesystem says any more that it was
/// ever reached through a link out. The removal then copies its stored values into the trail and
/// the export. The operator decides the remedy (issue, Review round 2); until then this test
/// PASSES by panicking with the leak, and must be flipped when a fix lands. Remedy order for a
/// user meanwhile: reindex FIRST (the door clears or reclaims the row and elides its trail), then
/// remove the link.
#[tokio::test]
#[should_panic(expected = "the target's text survives in the catalog")]
async fn known_gap_a_removed_outward_link_row_leaves_its_text_in_the_trail() {
    let (_tmp, ctx, _store, _id, p, link) = prefix_state(false, false).await;
    std::fs::remove_file(&link).unwrap();
    let v = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    assert_nothing_of_the_target_remains(&ctx, &p, None, &v);
}

/// R2-2: a non-UTF-8 directory name. The catalog stores a LOSSY path (U+FFFD), which names no
/// file. From the pre-fix state, ONE post-fix reindex removes the row and elides like an escaping
/// row's, its cascaded citation included (round 3, R3-10). Before that reindex the row is in the
/// accepted window (round 3: a lossy path is not refused at read time, R3-6).
#[tokio::test]
async fn a_non_utf8_path_row_is_withheld_and_removed_without_copying_its_text() {
    use std::os::unix::ffi::OsStrExt;
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&base);
    let docs = p.join(std::ffi::OsStr::from_bytes(b"d\xff"));
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("x.md"), doc_text()).unwrap();
    let fxv = fx(&p, &docs, None);
    let (e, _) = svc(4);
    let (ctx, store) = ctx_for(&fxv, Some(e));
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let id = crate::librarian::ids::artifact_id_from_abs(&docs.join("x.md"));
    assert!(
        store.chunks_under(&id) > 0 && !catalog_hits(&ctx, None).is_empty(),
        "precondition: the target's text is in the catalog"
    );
    std::fs::write(
        p.join(".gitattributes"),
        ".codescout/audit/*.jsonl merge=union\n",
    )
    .unwrap();
    target(&base.join("out"));
    std::fs::remove_file(docs.join("x.md")).unwrap();
    ln(base.join("out/x.md"), &docs.join("x.md"));
    cite_from(&ctx, &id);
    let v = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    assert!(
        artifact::get(&ctx.catalog.lock(), &id).unwrap().is_none(),
        "{v}"
    );
    assert_nothing_of_the_target_remains(&ctx, &p, None, &v);
    assert_no_surface_prints_the_mark(&ctx, &id).await;
}

/// R2-5: the reclaimed row's cascaded `entry_cite` delete. Its PAYLOAD must not carry the slug or
/// the cited tokens; its `row_id` (the routing key, `slug:local→dst_ref`) does, and is recorded
/// in the issue under Deviation 8.
#[tokio::test]
async fn a_reclaimed_rows_cascaded_citation_delete_carries_no_slug_in_its_payload() {
    let (_tmp, ctx, _store, id, _p, _link) = prefix_state(false, false).await;
    {
        let cat = ctx.catalog.lock();
        let slug: String = cat
            .conn
            .query_row("SELECT slug FROM artifact WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(
            slug.contains(MARK),
            "precondition: the slug derives from the target: {slug}"
        );
        cat.conn
            .execute(
                "INSERT INTO entry_cite(src_slug, src_local, dst_ref, rel, origin, created_at) \
                 VALUES (?1, 'T-1', 'other:T-9', 'cites', 'write', 0)",
                [&slug],
            )
            .unwrap();
    }
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let cat = ctx.catalog.lock();
    let deletes: i64 = cat
        .conn
        .query_row(
            "SELECT COUNT(*) FROM catalog_audit WHERE tbl = 'entry_cite' AND op = 'delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(deletes, 1, "precondition: the cascade wrote its delete row");
    let leaking: Vec<String> = cat
        .conn
        .prepare(
            "SELECT payload FROM catalog_audit WHERE tbl = 'entry_cite' \
             AND instr(payload, ?1) > 0",
        )
        .unwrap()
        .query_map([MARK], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(leaking.is_empty(), "{leaking:?}");
}

/// A refused row (the link out was made after indexing, no reindex yet) whose stored fields are
/// the target's. Returns the context, its id, and the path.
fn refused_row_with_stored_fields() -> (tempfile::TempDir, ToolContext, String, Fx) {
    let tmp = tempfile::tempdir().unwrap();
    let b = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&b);
    target(&b.join("out"));
    ln(b.join("out/x.md"), &p.join("docs/x.md"));
    let fxv = fx(&p, &p.join("docs"), None);
    let (ctx, _) = ctx_for(&fxv, None);
    let id = seed(&ctx, &fxv.md, Some(&format!("{MARK} title")));
    ctx.catalog
        .lock()
        .conn
        .execute(
            "UPDATE artifact SET kind = ?2, status = ?3, topic = ?4, tags = ?5, owners = ?6, \
             time_scope = ?7 WHERE id = ?1",
            rusqlite::params![
                id,
                format!("{MARK}kind"),
                format!("{MARK}status"),
                format!("{MARK}topic"),
                format!("[\"{MARK}tag\"]"),
                format!("[\"{MARK}owner\"]"),
                format!("{MARK}timescope")
            ],
        )
        .unwrap();
    (tmp, ctx, id, fxv)
}

/// R2-3: every surface that prints a row's stored fields, for a refused row in the WINDOW before
/// the first post-fix reindex. `doc(find)` drops such a row from its page and `gather` from its
/// list, `doc(get)` and `doc(state_at)` print it with the fields withheld (time scope included,
/// round 3, R3-10).
#[tokio::test]
async fn no_surface_prints_the_stored_fields_of_a_refused_row() {
    let (_tmp, ctx, id, fxv) = refused_row_with_stored_fields();
    let mut leaks: Vec<String> = Vec::new();
    fn check(leaks: &mut Vec<String>, name: &str, v: String) {
        if v.contains(MARK) {
            leaks.push(format!("{name}: {}", &v[..v.len().min(400)]));
        }
    }
    check(
        &mut leaks,
        "get",
        format!("{}", get::call(&ctx, json!({"id": id})).await.unwrap()),
    );
    // The PAGE (the items). In the window a cold call's `catalog.by_kind` and its hints still
    // count the row under its stored kind (round 3, R3-8: no catalog scan); the first post-fix
    // reindex closes that, `no_oracle_on_a_pre_fix_row_after_one_reindex`.
    check(
        &mut leaks,
        "find",
        format!(
            "{}",
            find::call(&ctx, json!({"limit": 50})).await.unwrap()["items"]
        ),
    );
    // The count-and-hints oracle (a filter on a withheld field answering through `count` or
    // `more_in_scope`) is NOT closed in this window since round 3 (R3-8): it is closed by the
    // first post-fix reindex, `no_oracle_on_a_pre_fix_row_after_one_reindex`.
    check(
        &mut leaks,
        "state_at",
        format!(
            "{}",
            super::state_at::call(&ctx, json!({"id": id, "timestamp": 9_999_999_999_999i64}))
                .await
                .unwrap()
        ),
    );
    check(
        &mut leaks,
        "workspace_state_at",
        format!(
            "{}",
            super::workspace_state_at::call(&ctx, json!({"timestamp": 9_999_999_999_999i64}))
                .await
                .unwrap()
        ),
    );
    let c = control_row(
        &ctx,
        &fxv,
        json!({"gather_from": [{"source": "artifacts", "limit": 50}]}),
    );
    check(
        &mut leaks,
        "gather artifacts",
        format!("{}", refresh::call(&ctx, json!({"id": c})).await.unwrap()),
    );
    // Under a filter on a withheld field gather lists nothing: a masked item would still answer.
    let cf = control_row(
        &ctx,
        &fxv,
        json!({"gather_from": [{"source": "artifacts", "limit": 50,
                                "filter": {"title": {"contains": MARK}}}]}),
    );
    let gathered = format!("{}", refresh::call(&ctx, json!({"id": cf})).await.unwrap());
    if gathered.contains(&id) {
        leaks.push(format!(
            "gather title-contains oracle listed the row: {gathered}"
        ));
    }
    assert!(leaks.is_empty(), "{}", leaks.join("\n"));
}
/// R2-3: a semantic `doc(find)` with no filter is not scanned for withheld rows; its page drops a
/// refused row that still holds file values (a masked item would still say it ranked).
#[tokio::test]
async fn a_semantic_find_drops_a_refused_row_that_holds_file_values() {
    let tmp = tempfile::tempdir().unwrap();
    let b = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&b);
    target(&b.join("out"));
    ln(b.join("out/x.md"), &p.join("docs/x.md"));
    let fxv = fx(&p, &p.join("docs"), None);
    let (e, _) = svc(4);
    let (ctx, store) = ctx_for(&fxv, Some(e));
    let id = seed(&ctx, &fxv.md, Some(&format!("{MARK} title")));
    let pid = p.to_string_lossy().into_owned();
    for item in seed_chunks(&ctx, &id) {
        store
            .upsert(&pid, &item.chunk_id, &id, &[0.5, 0.5, 0.5, 0.5])
            .await
            .unwrap();
    }
    let out = find::call(&ctx, json!({"semantic": "anything", "limit": 20}))
        .await
        .unwrap();
    let text = format!("{out}");
    assert!(
        !text.contains(&id) && !text.contains(MARK),
        "the refused row ranked on the page: {text}"
    );
}

/// R2-4: a pre-fix row whose outward link now DANGLES. `doc(get)` withholds its stored title as
/// `doc(find)` does (the door's predicate refuses a dangling link); it used to return the title
/// because the read failed with "not found" rather than a refusal.
#[tokio::test]
async fn get_withholds_the_stored_title_of_a_dangling_link_row() {
    let tmp = tempfile::tempdir().unwrap();
    let b = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&b);
    std::fs::create_dir_all(b.join("out")).unwrap();
    ln(b.join("out/x.md"), &p.join("docs/x.md"));
    let fxv = fx(&p, &p.join("docs"), None);
    let (ctx, _) = ctx_for(&fxv, None);
    let id = seed(&ctx, &fxv.md, Some(&format!("{MARK} title")));
    let g = get::call(&ctx, json!({"id": id})).await.unwrap();
    assert!(!has(&g), "{g}");
    assert!(g["body_error"].is_string(), "the refusal is said: {g}");
}

/// The control for the withholding: a plain row's stored title IS returned by `doc(get)`.
#[tokio::test]
async fn get_still_returns_the_title_of_a_plain_row() {
    let tmp = tempfile::tempdir().unwrap();
    let b = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&b);
    target(&p.join("docs"));
    let fxv = fx(&p, &p.join("docs"), None);
    let (ctx, _) = ctx_for(&fxv, None);
    let id = seed(&ctx, &fxv.md, Some(&format!("{MARK} title")));
    let g = get::call(&ctx, json!({"id": id})).await.unwrap();
    assert_eq!(g["title"], json!(format!("{MARK} title")), "{g}");
    let f = find::call(&ctx, json!({"filter": {"title": {"contains": MARK}}}))
        .await
        .unwrap();
    assert_eq!(f["count"].as_u64(), Some(1), "{f}");
}

/// R2-9: `doc(update)` on an outward leaf link names the spelling and never the target.
#[tokio::test]
async fn a_refused_update_names_the_spelling_and_never_the_outside_target() {
    let tmp = tempfile::tempdir().unwrap();
    let b = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&b);
    target(&b.join("out-secret-place"));
    ln(b.join("out-secret-place/x.md"), &p.join("docs/x.md"));
    let fxv = fx(&p, &p.join("docs"), None);
    let (ctx, _) = ctx_for(&fxv, None);
    let id = seed(&ctx, &fxv.md, None);
    let err = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("refused");
    let msg = format!("{err:#}");
    assert!(msg.contains("x.md"), "{msg}");
    assert!(
        !msg.contains("out-secret-place"),
        "the outside target is named: {msg}"
    );
}

// ---------------------------------------------------------------------------------------------
// Review round 3. The premise: a stored row's text is tainted by WHEN it was ingested. A post-fix
// walk reads only through the door, so only a pre-fix row can hold text from outside, and the
// reindex judges every stored row of its root on every run (the walk through the door, the
// reclamation through the door and `stored_path_is_risky`). So the first post-fix reindex of a
// root is the migration: each test below builds the REAL pre-fix state, runs ONE reindex, and
// asserts the surface. Before that reindex is the accepted window (issue, Review round 3).
// ---------------------------------------------------------------------------------------------

/// A citation FROM row `id` (its slug, which a pre-fix walk minted from the target's title), so
/// the removal's cascaded `entry_cite` delete has something to carry.
fn cite_from(ctx: &ToolContext, id: &str) {
    let cat = ctx.catalog.lock();
    let slug: String = cat
        .conn
        .query_row("SELECT slug FROM artifact WHERE id = ?1", [id], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(
        slug.contains(MARK),
        "precondition: the slug derives from the target: {slug}"
    );
    cat.conn
        .execute(
            "INSERT INTO entry_cite(src_slug, src_local, dst_ref, rel, origin, created_at) \
             VALUES (?1, 'T-1', 'other:T-9', 'cites', 'write', 0)",
            [&slug],
        )
        .unwrap();
}

/// `doc(get)` of `id`, a cold `doc(find)` and `librarian(context)` on the marker print nothing of
/// it.
async fn assert_no_surface_prints_the_mark(ctx: &ToolContext, id: &str) {
    let g = match get::call(ctx, json!({"id": id})).await {
        Ok(v) => v.to_string(),
        Err(e) => format!("{e:#}"),
    };
    let f = find::call(ctx, json!({"limit": 50})).await.unwrap();
    let c = context::call(ctx, json!({"topic": MARK})).await.unwrap();
    for (name, out) in [
        ("get", g),
        ("find", f.to_string()),
        ("context", c.to_string()),
    ] {
        assert!(
            !out.contains(MARK),
            "{name} prints the target's text: {out}"
        );
    }
}

/// The pre-fix state of a row KEPT for its history (an augmentation): the walk read the target, so
/// its title, chunks and slug are the target's, and its frontmatter fields are set as a pre-fix
/// walk would have stored them from the target. Then ONE post-fix reindex.
async fn kept_after_one_reindex() -> (
    tempfile::TempDir,
    ToolContext,
    String,
    PathBuf,
    serde_json::Value,
) {
    let (tmp, ctx, _store, id, p, _link) = prefix_state(false, true).await;
    ctx.catalog
        .lock()
        .conn
        .execute(
            "UPDATE artifact SET kind = ?2, status = ?3, topic = ?4, tags = ?5, owners = ?6, \
             time_scope = ?7 WHERE id = ?1",
            rusqlite::params![
                id,
                format!("{MARK}kind"),
                format!("{MARK}status"),
                format!("{MARK}topic"),
                format!("[\"{MARK}tag\"]"),
                format!("[\"{MARK}owner\"]"),
                format!("{MARK}timescope")
            ],
        )
        .unwrap();
    let v = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
    assert!(
        row.title.is_none() && row.topic.is_none() && row.status == "unknown",
        "precondition: the reindex kept and cleared the row: {row:?}"
    );
    (tmp, ctx, id, p, v)
}

/// R3-1: a pre-fix row below a DIRECTORY link out whose target directory was then removed (the
/// directory link dangles). ONE reindex removes it and elides its trail.
#[tokio::test]
async fn a_row_below_a_dangling_directory_link_is_removed_without_copying_its_text() {
    let (_tmp, ctx, _store, id, p, _leaf) = prefix_state(true, false).await;
    std::fs::remove_dir_all(p.parent().unwrap().join("out")).unwrap();
    cite_from(&ctx, &id);
    let v = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    assert!(
        artifact::get(&ctx.catalog.lock(), &id).unwrap().is_none(),
        "{v}"
    );
    assert_nothing_of_the_target_remains(&ctx, &p, None, &v);
    assert_no_surface_prints_the_mark(&ctx, &id).await;
}

/// R2-3/R2-4 after round 3: once the first post-fix reindex has run, a pre-fix row has null file
/// fields, and a filter on any of them answers as a filter matching nothing does (count AND hints).
#[tokio::test]
async fn no_oracle_on_a_pre_fix_row_after_one_reindex() {
    let (_tmp, ctx, id, p, v) = kept_after_one_reindex().await;
    assert_nothing_of_the_target_remains(&ctx, &p, Some(&id), &v);
    for field in [
        "title",
        "topic",
        "status",
        "kind",
        "time_scope",
        "tags",
        "owners",
    ] {
        let by = |needle: &str| json!({"filter": {field: {"contains": needle}}});
        let hit = find::call(&ctx, by(MARK)).await.unwrap();
        let miss = find::call(&ctx, by("no-row-has-this-zq")).await.unwrap();
        assert_eq!(
            (&hit["count"], &hit["hints"]),
            (&miss["count"], &miss["hints"]),
            "a `{field} contains` filter answers about the pre-fix row: {hit}"
        );
    }
    assert_no_surface_prints_the_mark(&ctx, &id).await;
    // The kept row is listed, cleared, and says why its fields are absent (reviewer's M4).
    let f = find::call(&ctx, json!({"limit": 50})).await.unwrap();
    let item = f["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == json!(id))
        .cloned()
        .unwrap_or_else(|| panic!("the kept row is listed: {f}"));
    assert!(
        item["withheld"]
            .as_str()
            .is_some_and(|w| w.contains("outside the repository root")),
        "{item}"
    );
    for (name, out) in [
        (
            "state_at",
            super::state_at::call(&ctx, json!({"id": id, "timestamp": 9_999_999_999_999i64}))
                .await
                .unwrap(),
        ),
        (
            "workspace_state_at",
            super::workspace_state_at::call(&ctx, json!({"timestamp": 9_999_999_999_999i64}))
                .await
                .unwrap(),
        ),
    ] {
        assert!(!has(&out), "{name} prints the target's text: {out}");
    }
}

/// R3-2: `librarian(context, topic=X)` selects candidates by stored title and topic. After ONE
/// reindex it answers about the pre-fix row as about no row at all.
#[tokio::test]
async fn context_counts_nothing_of_a_pre_fix_row_after_one_reindex() {
    let (_tmp, ctx, _id, _p, _v) = kept_after_one_reindex().await;
    let hit = context::call(&ctx, json!({"topic": MARK})).await.unwrap();
    let miss = context::call(&ctx, json!({"topic": "no-row-has-this-zq"}))
        .await
        .unwrap();
    assert_eq!(hit["overflow"], miss["overflow"], "{hit}");
    assert!(!has(&hit), "{hit}");
}

/// R3-4: `doc(graft)`'s dry run prints both rows' titles. After ONE reindex the pre-fix row's is
/// null.
#[tokio::test]
async fn graft_preview_prints_nothing_of_a_pre_fix_row_after_one_reindex() {
    let (_tmp, ctx, id, p, _v) = kept_after_one_reindex().await;
    let c = p.join("docs/control.md");
    std::fs::write(&c, "# control\n").unwrap();
    let cid = seed(&ctx, &c, Some("control"));
    let v = super::graft::call(&ctx, json!({"from_id": id, "into_id": cid}))
        .await
        .unwrap();
    assert!(v["would_delete_row"]["title"].is_null(), "{v}");
    assert!(!has(&v), "{v}");
}

/// R3-5: `doc(delete, force)` copies the whole row into the trail. After ONE reindex the row holds
/// nothing of the target. The one value it keeps is the slug (Deviation 8, kept rows keep their
/// slug, operator decision open), so the delete image's `slug` is set aside here.
#[tokio::test]
async fn delete_of_a_pre_fix_row_after_one_reindex_copies_nothing_of_it() {
    let (_tmp, ctx, id, _p, _v) = kept_after_one_reindex().await;
    super::delete::call(&ctx, json!({"id": id, "force": true}))
        .await
        .unwrap();
    let cat = ctx.catalog.lock();
    let rows: Vec<(String, String)> = cat
        .conn
        .prepare(
            "SELECT op, payload FROM catalog_audit WHERE tbl = 'artifact' AND row_id = ?1 \
             AND payload IS NOT NULL",
        )
        .unwrap()
        .query_map([&id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(
        rows.iter().any(|(op, _)| op == "delete"),
        "precondition: the delete wrote its image: {rows:?}"
    );
    for (op, payload) in &rows {
        let mut v: serde_json::Value = serde_json::from_str(payload).unwrap();
        if op == "delete" {
            v.as_object_mut().unwrap().remove("slug");
        }
        assert!(!has(&v), "{op} {v}");
    }
}

/// R3-3: a worktree session's write to a MAIN row forks it: a shadow row seeded from the main
/// row, and a `worktree_fork` event carrying its fields (events are never elided). After ONE
/// reindex of the main checkout the main row is cleared, so the fork copies nothing of the target.
#[tokio::test]
async fn a_worktree_fork_of_a_pre_fix_row_after_one_reindex_copies_nothing_of_it() {
    let tmp = tempfile::tempdir().unwrap();
    let b = std::fs::canonicalize(tmp.path()).unwrap();
    let m = project(&b);
    target(&b.join("out"));
    ln(b.join("out/x.md"), &m.join("docs/x.md"));
    let w = m.join(".worktrees/feat");
    std::fs::create_dir_all(w.join("docs")).unwrap();
    std::fs::write(w.join(".git"), "gitdir: ../../.git/worktrees/feat\n").unwrap();
    ln(b.join("out/x.md"), &w.join("docs/x.md"));
    let md = m.join("docs/x.md");
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_rules(rules())
        .with_artifact_store(Arc::new(InMemoryArtifactStore::default()))
        .with_current_project(Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: w.clone(),
                git_root: w.clone(),
                main_root: Some(m.clone()),
                umbrella: None,
            },
        ))
        .build();
    // The pre-fix row: what a pre-fix walk stored from the target, with history so it is kept.
    let id = seed(&ctx, &md, Some(&format!("{MARK} title")));
    ctx.catalog
        .lock()
        .conn
        .execute(
            "UPDATE artifact SET topic = ?2, tags = ?3, owners = ?4, time_scope = ?5 WHERE id = ?1",
            rusqlite::params![
                id,
                format!("{MARK}topic"),
                format!("[\"{MARK}tag\"]"),
                format!("[\"{MARK}owner\"]"),
                format!("{MARK}timescope")
            ],
        )
        .unwrap();
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&id, json!({"note": "mine"}))).unwrap();
    // ONE post-fix reindex of the main checkout (the worktree session's own reindex skips a
    // linked worktree, so the main root is walked directly).
    crate::librarian::indexer::index_repo_sync(
        &ctx.catalog.lock(),
        &rules(),
        &m,
        &globset::GlobSet::empty(),
        false,
        false,
        false,
    )
    .unwrap();
    super::event_create::call(
        &ctx,
        json!({"artifact_id": id, "kind": "note", "payload": {"text": "n"}}),
    )
    .await
    .unwrap();
    let cat = ctx.catalog.lock();
    let forks: Vec<String> = cat
        .conn
        .prepare("SELECT payload FROM events WHERE kind = 'worktree_fork'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        forks.len(),
        1,
        "precondition: the write forked the main row"
    );
    assert!(!forks[0].contains(MARK), "{}", forks[0]);
    let shadow: Vec<String> = cat
        .conn
        .prepare("SELECT COALESCE(title,'') || COALESCE(topic,'') || tags || owners FROM artifact")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(shadow.iter().all(|s| !s.contains(MARK)), "{shadow:?}");
}

/// R3-6: an ORDINARY repository file whose directory name is not valid UTF-8 (no link anywhere)
/// is found, and its title returned. Round 2 refused every such stored path at read time.
#[tokio::test]
async fn an_ordinary_file_below_a_non_utf8_name_is_found_and_kept() {
    use std::os::unix::ffi::OsStrExt;
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&base);
    let docs = p.join(std::ffi::OsStr::from_bytes(b"d\xff"));
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("x.md"), "# plainnonutf title\n\nbody\n").unwrap();
    let fxv = fx(&p, &docs, None);
    let (ctx, _) = ctx_for(&fxv, None);
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let id = crate::librarian::ids::artifact_id_from_abs(&docs.join("x.md"));
    let ff = find::call(
        &ctx,
        json!({"filter": {"title": {"contains": "plainnonutf"}}}),
    )
    .await
    .unwrap();
    assert_eq!(ff["count"].as_u64(), Some(1), "{ff}");
    let g = get::call(&ctx, json!({"id": id})).await.unwrap();
    assert_eq!(g["title"].as_str(), Some("plainnonutf title"), "{g}");
    let r2 = super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    assert_eq!(r2["removed"].as_u64(), Some(0), "{r2}");
}

/// R3-7: a slug is unique among LIVE rows only, so a freed slug is minted again for another row.
/// Removing that row with elision must leave the EARLIER holder's citation history alone.
#[tokio::test]
async fn a_reused_slug_leaves_the_earlier_holders_citation_history_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&base);
    let docs = p.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("a.md"), "# Same Title\n\nfirst document\n").unwrap();
    let fxv = fx(&p, &docs, None);
    let (ctx, _) = ctx_for(&fxv, None);
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let id_a = crate::librarian::ids::artifact_id_from_abs(&docs.join("a.md"));
    let slug_a =
        crate::librarian::catalog::artifact::ensure_slug(&ctx.catalog.lock().conn, &id_a).unwrap();
    ctx.catalog
        .lock()
        .conn
        .execute(
            "INSERT INTO entry_cite(src_slug, src_local, dst_ref, rel, origin, created_at) \
             VALUES (?1, 'A-1', 'other:X-1', 'cites', 'write', 0)",
            [&slug_a],
        )
        .unwrap();
    // A goes as an ordinary vanished row: no elision, its cascaded delete image is kept.
    std::fs::remove_file(docs.join("a.md")).unwrap();
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let needle = format!("\"src_slug\":\"{slug_a}\"");
    let count = |ctx: &ToolContext| -> i64 {
        ctx.catalog
            .lock()
            .conn
            .query_row(
                "SELECT COUNT(*) FROM catalog_audit WHERE tbl = 'entry_cite' \
                 AND instr(payload, ?1) > 0",
                [&needle],
                |r| r.get(0),
            )
            .unwrap()
    };
    let before = count(&ctx);
    // B: same title, so the freed slug is minted again, B cites under the SAME key as A did; then
    // B's file becomes a dangling link inside the repository and B is removed with elision.
    std::fs::write(docs.join("b.md"), "# Same Title\n\nsecond document\n").unwrap();
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let id_b = crate::librarian::ids::artifact_id_from_abs(&docs.join("b.md"));
    let slug_b =
        crate::librarian::catalog::artifact::ensure_slug(&ctx.catalog.lock().conn, &id_b).unwrap();
    ctx.catalog
        .lock()
        .conn
        .execute(
            "INSERT INTO entry_cite(src_slug, src_local, dst_ref, rel, origin, created_at) \
             VALUES (?1, 'A-1', 'other:X-1', 'cites', 'write', 0)",
            [&slug_b],
        )
        .unwrap();
    std::fs::remove_file(docs.join("b.md")).unwrap();
    std::os::unix::fs::symlink(docs.join("nowhere.md"), docs.join("b.md")).unwrap();
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    assert_eq!(slug_a, slug_b, "precondition: the slug was reused");
    assert!(
        before >= 1,
        "precondition: A's citation images hold the slug"
    );
    assert_eq!(
        before,
        count(&ctx),
        "B's removal rewrote A's citation history"
    );
    // B's own cascaded citation WAS elided: the bound is not a no-op.
    let b_elided: i64 = ctx
        .catalog
        .lock()
        .conn
        .query_row(
            "SELECT COUNT(*) FROM catalog_audit WHERE tbl = 'entry_cite' AND op = 'delete' \
             AND instr(payload, '\"src_slug\":{\"elided\":\"read-door\"}') > 0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(b_elided, 1, "B's own citation delete must be elided");
}

/// R3-10: `gather` judges `truncated` AFTER it drops a refused row, so the dropped row does not
/// say "there is more" (window behaviour, before the first post-fix reindex). Two rows match
/// (the refused one and an ordinary one) and the limit is 1: the dropped row is not "more".
#[tokio::test]
async fn gather_judges_truncated_after_dropping_a_refused_row() {
    let (_tmp, ctx, _id, fxv) = refused_row_with_stored_fields();
    let plain = fxv.base_dir().join("plain.md");
    std::fs::write(&plain, "# plain\n").unwrap();
    seed(&ctx, &plain, Some("plain"));
    let sources: Vec<super::gather::GatherSource> =
        vec![serde_json::from_value(json!({"source": "artifacts", "limit": 1})).unwrap()];
    let (results, warnings) = super::gather::gather_all(&sources, &ctx, None)
        .await
        .unwrap();
    let data = results[0].data.to_string();
    assert!(!data.contains(MARK), "{data}");
    assert!(
        data.contains("plain"),
        "precondition: the ordinary row is listed: {data}"
    );
    assert!(
        !warnings.iter().any(|w| w.contains("truncated")),
        "the dropped row was counted as more: {warnings:?}"
    );
}

/// R3-10 (reviewer's M1): the citation prefix is `slug:` WITH its colon. A sibling slug minted by
/// the dedup (`same-title-2`) starts with the removed row's slug, and its citation history is that
/// row's own.
#[tokio::test]
async fn a_sibling_slugs_citation_history_is_left_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let base = std::fs::canonicalize(tmp.path()).unwrap();
    let p = project(&base);
    let docs = p.join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("a.md"), "# Same Title\n\nfirst\n").unwrap();
    let fxv = fx(&p, &docs, None);
    let (ctx, _) = ctx_for(&fxv, None);
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    std::fs::write(docs.join("b.md"), "# Same Title\n\nsecond\n").unwrap();
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let slug_of = |name: &str| -> String {
        let id = crate::librarian::ids::artifact_id_from_abs(&docs.join(name));
        crate::librarian::catalog::artifact::ensure_slug(&ctx.catalog.lock().conn, &id).unwrap()
    };
    let (slug_a, slug_b) = (slug_of("a.md"), slug_of("b.md"));
    assert!(
        slug_b.starts_with(&slug_a) && slug_b != slug_a,
        "precondition: a sibling slug ({slug_a}, {slug_b})"
    );
    // B cites and drops the citation AFTER A acquired its slug: a delete image under `slug_b:`.
    {
        let cat = ctx.catalog.lock();
        cat.conn
            .execute(
                "INSERT INTO entry_cite(src_slug, src_local, dst_ref, rel, origin, created_at) \
                 VALUES (?1, 'B-1', 'other:X-1', 'cites', 'write', 0)",
                [&slug_b],
            )
            .unwrap();
        cat.conn
            .execute("DELETE FROM entry_cite WHERE src_slug = ?1", [&slug_b])
            .unwrap();
    }
    // A's file becomes a dangling link: A is removed with elision.
    std::fs::remove_file(docs.join("a.md")).unwrap();
    std::os::unix::fs::symlink(docs.join("nowhere.md"), docs.join("a.md")).unwrap();
    super::reindex::call(&ctx, json!({"scope": "project"}))
        .await
        .unwrap();
    let kept: i64 = ctx
        .catalog
        .lock()
        .conn
        .query_row(
            "SELECT COUNT(*) FROM catalog_audit WHERE tbl = 'entry_cite' AND op = 'delete' \
             AND instr(payload, ?1) > 0",
            [format!("\"src_slug\":\"{slug_b}\"")],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        kept, 1,
        "A's removal rewrote its sibling {slug_b}'s citation history"
    );
}

/// Round 1's F1 invariant for a KEPT row: its clearing and its elision commit together. With the
/// elision made to fail, the reindex errors and the row still holds what it held, so no audit row
/// copied the clear's diff without its elision.
#[tokio::test]
async fn a_kept_rows_clear_and_elision_commit_together() {
    let (_tmp, ctx, _store, id, _p, _link) = prefix_state(false, true).await;
    ctx.catalog
        .lock()
        .conn
        .execute_batch(
            // `OF payload`: the elision's rewrite, not the per-connection `audit_stamp` trigger
            // (which sets `actor`/`verb` after every insert, and would fail the clear itself).
            "CREATE TEMP TRIGGER no_audit_rewrite BEFORE UPDATE OF payload ON catalog_audit \
             BEGIN SELECT RAISE(ABORT, 'audit rewrite refused by the test'); END;",
        )
        .unwrap();
    let r = super::reindex::call(&ctx, json!({"scope": "project"})).await;
    assert!(r.is_err(), "precondition: the elision failed: {r:?}");
    let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
    assert!(
        row.title.as_deref().is_some_and(|t| t.contains(MARK)),
        "the clear committed without its elision: {row:?}"
    );
}
