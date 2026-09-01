use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::artifact::{self, ArtifactRow};
use crate::librarian::frontmatter::Frontmatter;

fn validate_rel_path(rel: &str) -> Result<()> {
    use std::path::{Component, Path};
    let p = Path::new(rel);
    if p.is_absolute() {
        return Err(RecoverableError::new(format!(
            "rel_path must be relative: {}",
            rel
        )));
    }
    for c in p.components() {
        match c {
            Component::ParentDir => {
                return Err(RecoverableError::new(format!(
                    "rel_path must not contain `..`: {}",
                    rel
                )))
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err(RecoverableError::new(format!(
                    "rel_path must be relative: {}",
                    rel
                )))
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// The augmentation an artifact is created with. Mirrors `artifact_augment`'s
/// caller-controlled fields so "created atomically with its augmentation" is true
/// rather than partly true.
///
/// `deny_unknown_fields` is load-bearing. Without it serde silently discards a
/// misspelled or unsupported key and the call still reports success — which is how
/// this struct's previous two-field shape turned a user typo into silent data loss
/// instead of an error. The sibling write surface (`update`'s `patch`) already
/// rejects unknown keys by naming the valid ones; this makes the two agree.
pub struct AugmentSpec {
    pub prompt: String,
    pub params: Option<Value>,
    pub render_template: Option<String>,
    /// Accepted as a JSON object and stored as text, matching `AugmentationRow`.
    pub params_schema: Option<Value>,
    pub entry_collection: Option<String>,
    #[serde(default)]
    pub append_mode: bool,
    pub history_cap: Option<i64>,
}

#[derive(Deserialize)]
pub struct Args {
    pub repo: Option<String>,
    pub rel_path: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub status: Option<String>,
    pub topic: Option<String>,
    pub time_scope: Option<String>,
    #[serde(default)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
    pub augment: Option<AugmentSpec>,
}
/// The six statuses a `kind: bug` file may carry, per
/// `get_guide("tracker-conventions")` § *Bug files*.
const BUG_STATUSES: &[&str] = &[
    "open",
    "investigating",
    "fixed",
    "mitigated",
    "wontfix",
    "zombie",
];

/// Which tool surface is validating `extra` — the *defect* is shared, the *remedy* is not.
///
/// `create` takes every reserved key but `id` as its own top-level parameter. `update`
/// takes its six settable ones inside `patch`, and takes `id` and `kind` nowhere at all.
/// One fixed hint cannot be true on both surfaces, and the original was written looking
/// at `create`: it told every `update` caller to "pass `kind=` as its own parameter", a
/// parameter `update` does not have. That costs a round-trip to discover, and still does
/// not reveal the actual constraint — that `kind` is unreachable there by design.
///
/// See `docs/issues/archive/2026-08-16-artifact-update-kind-hint-misroutes.md`.
#[derive(Clone, Copy)]
pub(crate) enum ExtraKeySurface {
    Create,
    Update,
}

impl ExtraKeySurface {
    /// Why this surface cannot set `key` through any channel, or `None` when it can.
    ///
    /// Per-key rather than a flat list: a caller told only "not settable" cannot tell
    /// whether that is a wrong-channel mistake or a permanent property of the field.
    fn unsettable_reason(self, key: &str) -> Option<&'static str> {
        match (self, key) {
            (_, "id") => Some("`id` is derived from the artifact's path, never caller-supplied"),
            (Self::Update, "kind") => Some(
                "`kind` is fixed once the artifact exists — it drives catalog \
                 classification, and `patch` does not accept it",
            ),
            _ => None,
        }
    }

    /// The channel a settable reserved key travels on here, rendered for a hint.
    fn channel(self, key: &str) -> String {
        match self {
            Self::Create => format!("`{key}=` as its own parameter"),
            Self::Update => format!("`patch={{{key}: …}}`"),
        }
    }
}

/// Refuse an `extra` map that names a field the frontmatter already models.
///
/// `extra` exists for keys the schema does not know. Passing a known one — most
/// plausibly `kind`, because bug files carry `kind: bug` and it is natural to
/// restate it — used to emit that key twice into one YAML mapping. A duplicate key
/// makes the block unparseable, and an unparseable block does not fail loudly: the
/// artifact falls back to glob classification and loses its kind, status, title,
/// owners and tags together. One live casualty, invisible for a working day.
///
/// Refused rather than repaired, even when the values agree. Repair-and-continue is
/// for input whose intent is unambiguous; here the caller has said the same thing
/// twice through two channels, and the right correction — drop it, or drop the
/// typed parameter — is a question about what they meant, not a typo to fix.
/// [`crate::librarian::frontmatter::write`] additionally drops these on the way out
/// so an internal caller cannot write an unreadable file either.
///
/// The hint is `surface`-dependent; see [`ExtraKeySurface`] for why it has to be.
///
/// See `docs/issues/archive/2026-08-08-artifact-extra-key-collision-unclassifies-silently.md`.
pub(crate) fn reject_reserved_extra_keys(
    extra: &std::collections::BTreeMap<String, serde_json::Value>,
    surface: ExtraKeySurface,
) -> Result<()> {
    let clashes = crate::librarian::frontmatter::reserved_keys_in_extra(extra);
    if clashes.is_empty() {
        return Ok(());
    }

    let (unsettable, settable): (Vec<&str>, Vec<&str>) = clashes
        .iter()
        .copied()
        .partition(|k| surface.unsettable_reason(k).is_some());

    let mut hint = String::new();
    if !settable.is_empty() {
        hint.push_str(&format!(
            "pass {} instead. ",
            settable
                .iter()
                .map(|k| surface.channel(k))
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }
    for key in &unsettable {
        // `unsettable` was partitioned on this returning Some, so the fallback is
        // unreachable — kept total rather than unwrapping on an invariant held elsewhere.
        let reason = surface
            .unsettable_reason(key)
            .unwrap_or("this field is not caller-settable here");
        hint.push_str(&format!("{reason}. "));
    }
    hint.push_str(&format!(
        "Reserved: {}. `extra` is for keys outside the schema (opened, closed, \
         severity, owner, related, …).",
        crate::librarian::frontmatter::RESERVED_KEYS.join(", ")
    ));

    Err(RecoverableError::with_hint(
        format!(
            "extra must not contain frontmatter field(s) the schema already models: {}",
            clashes.join(", ")
        ),
        hint,
    ))
}
/// Refuse a `body` that itself begins with a frontmatter block.
///
/// [`call`] composes the file as `frontmatter::write(&fm, "\n{body}\n")`, so a body
/// whose first line is `---` produces a **second** block below the catalog's. Unlike
/// the duplicate-key case in [`reject_reserved_extra_keys`], the result parses fine —
/// [`crate::librarian::frontmatter::parse`] reads the first block and hands the second
/// back as body text — which is precisely why nothing failed. Three harms, all silent,
/// measured on a real reproduction 2026-09-01:
///
/// 1. Keys the catalog block does not model (`opened`, `closed`, `severity`, `owner`,
///    `related` — the exact set `_TEMPLATE.md` carries) live **only** in the inert
///    block, so no query can read them.
/// 2. A key present in both blocks with **different** values resolves silently to the
///    catalog's. A probe passing `status: scratch` in the body block was reported as
///    `status: open` by `artifact(action="get")`, the caller's value simply gone.
/// 3. The inert block is served as the artifact's `preview.summary`, so the first thing
///    an agent reads about the artifact is a mangled YAML fragment.
///
/// **Not a rare caller mistake.** The trigger is the project's own documented workflow:
/// `docs/issues/_TEMPLATE.md` opens with `---`, and `get_guide("tracker-conventions")`
/// says to copy it. Measured across the committed corpus the same day: **9 files already
/// carry a doubled block**, 8 archived bug files and one spec.
///
/// **Refused, not repaired — the same reasoning as [`reject_reserved_extra_keys`]**, and
/// here the two channels disagree often enough to make that concrete: merging would have
/// to pick a winner for `status`, and picking is a question about what the caller meant.
///
/// **The predicate is [`crate::librarian::frontmatter::parse`]'s own**
/// (`starts_with("---\n")`), deliberately, rather than a new one: a guard recognising a
/// different set of bodies than the reader does would add a third grammar to a namespace
/// that already has two — the `IC-6` shape this repo has 27 instances of. **The escape,
/// since refusing a token owes one:** a body that genuinely opens with a `---` horizontal
/// rule keeps it by leading with a blank line, and the hint says so.
fn reject_body_leading_frontmatter(body: &str) -> Result<()> {
    if !(body.starts_with("---\n") || body.starts_with("---\r\n")) {
        return Ok(());
    }
    Err(RecoverableError::with_hint(
        "body begins with its own `---` frontmatter block, which would be written BELOW \
         the catalog's and silently ignored"
            .to_string(),
        "artifact(action=\"create\") writes the frontmatter itself from the typed \
         parameters. Drop the block from `body` and route its keys: kind / status / \
         title / owners / tags / topic / time_scope as their own parameters, everything \
         else (opened, closed, severity, owner, related, …) via `extra={...}`. If you \
         meant a literal `---` horizontal rule, start `body` with a blank line."
            .to_string(),
    ))
}

/// Resolve the `status` for a new artifact, defaulting **per kind**.
///
/// The two vocabularies are disjoint and the default used to ignore `kind`, so
/// every bug file created without an explicit status got `draft` — a value the bug
/// vocabulary does not contain. That is not cosmetic: the canonical triage query is
/// `find(kind="bug", status="open")`, so such a bug never appears in the answer to
/// "what's open?" and nothing anywhere notices. Seen live on a bug that was
/// written, committed, pushed and cited from three documents, and still absent from
/// the ledger.
///
/// An out-of-vocabulary status on a bug is refused rather than silently stored,
/// because the failure it causes is invisible by construction — the row simply does
/// not match a query someone else runs later.
///
/// Tracker statuses are deliberately NOT validated here: that vocabulary documents
/// `active | draft | archived | superseded` but the guide also states that
/// unrecognised values "appear as active", i.e. free-form is load-bearing there.
///
/// See `docs/issues/archive/2026-08-06-artifact-create-bug-defaults-to-invalid-draft-status.md`.
fn resolve_status(kind: &str, requested: Option<&str>) -> anyhow::Result<String> {
    match requested {
        Some(s) if kind == "bug" && !BUG_STATUSES.contains(&s) => {
            Err(RecoverableError::new(format!(
                "status {s:?} is not a bug status; use one of: {}",
                BUG_STATUSES.join(", ")
            )))
        }
        Some(s) => Ok(s.to_string()),
        // A new bug is open; a new tracker or design artifact is a draft.
        None if kind == "bug" => Ok("open".to_string()),
        None => Ok("draft".to_string()),
    }
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let mut a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(format!("artifact(action=\"create\") requires 'rel_path', 'kind', 'title' and 'body': {e}"), "e.g. artifact(action=\"create\", rel_path=\"docs/plans/my-plan.md\", kind=\"plan\", title=\"My plan\", body=\"# My plan\"). rel_path is relative to the repo root and does NOT include the repo name.")
    })?;

    // Resolve base directory: explicit repo arg looks up in workspace.roots
    // (legacy compatibility), otherwise derive from current_project.abs_path.
    let base_dir: std::path::PathBuf = match a.repo.as_deref() {
        Some(r) => {
            let root = ctx
                .workspace
                .roots
                .iter()
                .find(|root| root.name == r)
                .ok_or_else(|| {
                    let valid = ctx
                        .workspace
                        .roots
                        .iter()
                        .map(|root| root.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    RecoverableError::with_hint(
                        format!("unknown repo `{r}`"),
                        format!("Valid repo names: {valid}"),
                    )
                })?;
            root.path.clone()
        }
        None => ctx
            .current_project
            .as_ref()
            .map(|p| p.abs_path.clone())
            .ok_or_else(|| {
                RecoverableError::with_hint(
                    "no active project — cannot resolve rel_path",
                    "Pass repo=<name> or activate a project via workspace(action='activate', ...)",
                )
            })?,
    };

    // Prevention: refuse writing a temp-dir-rooted artifact into the real shared
    // catalog. See docs/issues/archive/2026-07-17-tmp-probe-artifacts-pollute-global-catalog.md.
    super::temp_write_guard::guard_temp_workspace_write(
        &base_dir,
        &ctx.catalog.lock().conn,
        &ctx.temp_guard,
    )?;

    validate_rel_path(&a.rel_path)?;
    a.rel_path = crate::librarian::util::normalize_rel_path(&a.rel_path);
    let full = base_dir.join(&a.rel_path);
    if full.exists() {
        return Err(RecoverableError::new(format!(
            "path exists: {}",
            full.display()
        )));
    }
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let id = crate::librarian::ids::artifact_id_from_abs(&full);
    reject_reserved_extra_keys(&a.extra, ExtraKeySurface::Create)?;
    reject_body_leading_frontmatter(&a.body)?;
    let status = resolve_status(&a.kind, a.status.as_deref())?;
    let fm = Frontmatter {
        id: Some(id.clone()),
        kind: Some(a.kind.clone()),
        status: Some(status.clone()),
        title: Some(a.title.clone()),
        owners: a.owners.clone(),
        tags: a.tags.clone(),
        topic: a.topic.clone(),
        time_scope: a.time_scope.clone(),
        extra: a.extra.clone(),
    };
    let content = crate::librarian::frontmatter::write(&fm, &format!("\n{}\n", a.body));
    let now = chrono::Utc::now().timestamp_millis();
    let row = ArtifactRow {
        id: id.clone(),
        abs_path: full.clone(),
        kind: a.kind.clone(),
        status: status.clone(),
        title: Some(a.title),
        owners: a.owners,
        tags: a.tags,
        topic: a.topic,
        time_scope: a.time_scope,
        source: Some("generated".into()),
        created_at: now,
        updated_at: now,
        file_mtime: now,
        file_sha256: crate::librarian::util::sha_of_bytes(content.as_bytes()),
        confidence: 1.0,
    };
    artifact::upsert_and_mint_slug(&ctx.catalog.lock(), &row)?;
    if let Some(aug_spec) = &a.augment {
        let params_str = aug_spec
            .params
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());
        let now_ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let cat = ctx.catalog.lock();
        crate::librarian::catalog::augmentation::upsert(
            &cat,
            &crate::librarian::catalog::augmentation::AugmentationRow {
                artifact_id: id.clone(),
                prompt: aug_spec.prompt.clone(),
                params: params_str,
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: now_ts.clone(),
                updated_at: now_ts,
                render_template: aug_spec.render_template.clone(),
                params_schema: aug_spec
                    .params_schema
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                append_mode: aug_spec.append_mode,
                history_cap: aug_spec.history_cap,
                entry_collection: aug_spec.entry_collection.clone(),
                // Server-computed provenance: never caller-supplied.
                refreshed_at_commit: None,
            },
        )?;
    }
    // Disk write last — the file is the user-visible side effect; the DB row
    // is the durable record. If a catalog upsert above fails, no orphan file
    // is left on disk to block a retry (BUG-058).
    std::fs::write(&full, &content)?;

    // A worktree-born artifact has no main-checkout twin to fork — just make
    // sure the worktree session itself is durably registered.
    if let Some(cp) = ctx.current_project.as_deref() {
        let cat = ctx.catalog.lock();
        super::worktree::ensure_registration(&cat, cp)?;
    }

    let mut result = json!({"id": id, "abs_path": row.abs_path.display().to_string()});
    if a.kind == "tracker" && a.augment.is_none() {
        result["tracker_hint"] = json!(
            "Tracker created without augmentation. \
             Call librarian(tracker_design) to pick an archetype \
             and attach a refresh prompt via artifact_augment."
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::Root;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: tmp_root,
            })
            .build()
    }

    #[tokio::test]
    async fn creates_file_and_row() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/specs/x.md",
                "kind": "spec", "title": "X", "body": "hello"
            }),
        )
        .await
        .unwrap();
        let path = tmp.path().join("docs/specs/x.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("title: X"));
        let id = v["id"].as_str().unwrap();
        assert!(artifact::get(&ctx.catalog.lock(), id).unwrap().is_some());
    }

    #[tokio::test]
    async fn create_mints_a_slug_for_the_new_artifact() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/specs/x.md",
                "kind": "spec", "title": "X Spec", "body": "hello"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap();
        let slug: Option<String> = ctx
            .catalog
            .lock()
            .conn
            .query_row("SELECT slug FROM artifact WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            slug.as_deref(),
            Some("x-spec"),
            "create() must mint a slug immediately, not leave it for a manual doctor sweep"
        );
    }

    /// A new bug must land in the answer to "what's open?". The two vocabularies are
    /// disjoint, and defaulting `kind: bug` to the tracker default `draft` put the row
    /// outside `find(kind="bug", status="open")` — so a bug written, committed and cited
    /// was still absent from the ledger, with nothing to notice it.
    #[tokio::test]
    async fn new_bug_defaults_to_open_and_a_new_tracker_still_defaults_to_draft() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let bug = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/issues/2026-01-01-x.md",
                "kind": "bug", "title": "B", "body": "b"
            }),
        )
        .await
        .unwrap();
        let bug_id = bug["id"].as_str().unwrap().to_string();

        // Read back through the catalog, which is what the triage query reads.
        let row = artifact::get(&ctx.catalog.lock(), &bug_id)
            .unwrap()
            .expect("bug row present");
        assert_eq!(
            row.status, "open",
            "a new bug must be `open`, not the tracker default"
        );

        // Over-match guard: the tracker default is unchanged. Without this the fix could
        // have been \"default everything to open\", which breaks the tracker vocabulary.
        let tracker = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/trackers/t.md",
                "kind": "tracker", "title": "T", "body": "t"
            }),
        )
        .await
        .unwrap();
        let trow = artifact::get(&ctx.catalog.lock(), tracker["id"].as_str().unwrap())
            .unwrap()
            .expect("tracker row present");
        assert_eq!(trow.status, "draft", "tracker default must not change");
    }

    /// An out-of-vocabulary bug status is refused rather than stored, because the harm it
    /// causes is invisible: the row simply fails to match a query someone runs later.
    #[tokio::test]
    async fn an_out_of_vocabulary_bug_status_is_refused() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let err = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/issues/2026-01-01-y.md",
                "kind": "bug", "title": "Y", "body": "y", "status": "draft"
            }),
        )
        .await
        .expect_err("`draft` is a tracker status, not a bug status");
        let msg = err.to_string();
        assert!(msg.contains("not a bug status"), "got: {msg}");
        // The message must name the alternatives, or the caller has to go read a guide.
        assert!(msg.contains("investigating"), "got: {msg}");

        // Over-match guard: every documented bug status is still accepted.
        for (i, s) in BUG_STATUSES.iter().enumerate() {
            call(
                &ctx,
                json!({
                    "repo": "r", "rel_path": format!("docs/issues/2026-01-01-ok-{i}.md"),
                    "kind": "bug", "title": "OK", "body": "ok", "status": s
                }),
            )
            .await
            .unwrap_or_else(|e| panic!("{s} should be accepted: {e}"));
        }
    }

    #[tokio::test]
    async fn refuses_if_file_exists() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/x.md"), "").unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/x.md",
                "kind": "doc", "title": "X", "body": "hi"
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("path exists"));
    }

    #[tokio::test]
    async fn create_does_not_leave_orphan_file_when_upsert_fails() {
        // BUG-058: if the artifact upsert fails after the file has been
        // written, future create calls bail with "path exists" even though
        // the artifact is not in the DB. Disk write must come AFTER all
        // catalog writes so a DB error leaves the disk untouched.
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // Force every artifact INSERT to abort, simulating the constraint
        // violation that BUG-058 reported under partial v6 migration state.
        ctx.catalog
            .lock()
            .conn
            .execute_batch(
                "CREATE TRIGGER fail_artifact BEFORE INSERT ON artifact \
                 BEGIN SELECT RAISE(ABORT, 'simulated upsert failure'); END;",
            )
            .unwrap();

        let result = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/orphan.md",
                "kind": "doc", "title": "X", "body": "hi"
            }),
        )
        .await;

        assert!(result.is_err(), "upsert must fail with abort trigger");
        let target = tmp.path().join("docs/orphan.md");
        assert!(
            !target.exists(),
            "no orphan file must remain after failed upsert: {}",
            target.display()
        );
    }

    #[tokio::test]
    async fn rejects_parent_dir_traversal() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "../escape.md",
                "kind": "doc", "title": "X", "body": "hi"
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains(".."), "got: {err}");
    }

    #[tokio::test]
    async fn rejects_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "/etc/passwd",
                "kind": "doc", "title": "X", "body": "hi"
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("relative"), "got: {err}");
    }

    #[tokio::test]
    async fn create_with_augment_writes_augmentation_row() {
        use crate::librarian::catalog::augmentation;
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let result = call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/my-tracker.md",
                "kind": "tracker",
                "title": "My Tracker",
                "body": "initial body",
                "status": "active",
                "augment": {
                    "prompt": "Keep this tracker up to date.",
                    "params": {"threshold": 5}
                }
            }),
        )
        .await
        .unwrap();

        let id = result["id"].as_str().unwrap().to_string();
        let cat = ctx.catalog.lock();
        let aug = augmentation::get(&cat, &id).unwrap();
        assert!(aug.is_some(), "augmentation row must be created");
        let aug = aug.unwrap();
        assert_eq!(aug.prompt, "Keep this tracker up to date.");
        let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
        assert_eq!(params["threshold"], 5);
    }

    /// BL-18. `augment` accepted only `prompt` + `params`; the other five
    /// augmentation fields were hardcoded to None/false at the insert, and serde
    /// silently dropped them on the way in. The call still returned success, so a
    /// tracker "created atomically with its augmentation" was half-configured with
    /// nothing saying so — a missing `render_template` means it contributes no
    /// `[LIVE]` block to `librarian(context)`, and a missing `entry_collection`
    /// makes `entry_filter` fail with a message about a different problem.
    ///
    /// Mutation caught: re-pinning any of these to a literal at the insert.
    #[tokio::test]
    async fn create_augment_accepts_the_full_augmentation_shape() {
        use crate::librarian::catalog::augmentation;
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let result = call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/full.md",
                "kind": "tracker",
                "title": "Full",
                "body": "b",
                "status": "active",
                "augment": {
                    "prompt": "p",
                    "params": {"rows": []},
                    "render_template": "| {{ rows|length }} |",
                    "params_schema": {"type": "object"},
                    "entry_collection": "rows",
                    "append_mode": true,
                    "history_cap": 12
                }
            }),
        )
        .await
        .unwrap();

        let id = result["id"].as_str().unwrap().to_string();
        let cat = ctx.catalog.lock();
        let aug = augmentation::get(&cat, &id)
            .unwrap()
            .expect("augmentation row");

        assert_eq!(
            aug.render_template.as_deref(),
            Some("| {{ rows|length }} |")
        );
        assert_eq!(aug.entry_collection.as_deref(), Some("rows"));
        assert!(aug.params_schema.is_some(), "params_schema must persist");
        assert!(aug.append_mode, "append_mode must persist");
        assert_eq!(aug.history_cap, Some(12));
    }

    /// The other half: a typo inside `augment` must fail loudly. `update`'s `patch`
    /// already rejects unknown keys by naming the valid ones; `create` silently
    /// discarded them, so the same user error was self-correcting on one write
    /// surface and silent data loss on its sibling.
    #[tokio::test]
    async fn create_augment_rejects_an_unknown_field() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let err = call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/typo.md",
                "kind": "tracker",
                "title": "Typo",
                "body": "b",
                "augment": { "prompt": "p", "render_tempalte": "oops" }
            }),
        )
        .await
        .expect_err("an unknown key inside augment must be rejected, not discarded");

        let msg = err.to_string();
        assert!(
            msg.contains("render_tempalte") || msg.contains("unknown field"),
            "the error must name the offending key, got: {msg}"
        );
    }

    #[tokio::test]
    async fn create_with_explicit_status_active() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/active.md",
                "kind": "tracker",
                "title": "Active",
                "body": "",
                "status": "active"
            }),
        )
        .await
        .unwrap();

        let cat = ctx.catalog.lock();
        let row = crate::librarian::catalog::artifact::get(
            &cat,
            &crate::librarian::ids::artifact_id_from_abs(&tmp.path().join("trackers/active.md")),
        )
        .unwrap()
        .unwrap();
        assert_eq!(row.status, "active");
    }

    #[tokio::test]
    async fn create_with_time_scope_persists_to_row_and_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/scoped.md",
                "kind": "tracker",
                "title": "Scoped",
                "body": "",
                "time_scope": "2026-W25"
            }),
        )
        .await
        .unwrap();

        let abs = tmp.path().join("trackers/scoped.md");
        let row = crate::librarian::catalog::artifact::get(
            &ctx.catalog.lock(),
            &crate::librarian::ids::artifact_id_from_abs(&abs),
        )
        .unwrap()
        .unwrap();
        assert_eq!(row.time_scope.as_deref(), Some("2026-W25"));

        // The value must also land in the YAML frontmatter, not just the catalog.
        let on_disk = std::fs::read_to_string(&abs).unwrap();
        let (fm, _) = crate::librarian::frontmatter::parse(&on_disk).unwrap();
        assert_eq!(fm.unwrap().time_scope.as_deref(), Some("2026-W25"));
    }
    #[tokio::test]
    async fn create_with_topic_persists_to_row_and_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/topical.md",
                "kind": "tracker",
                "title": "Topical",
                "body": "",
                "topic": "auth middleware"
            }),
        )
        .await
        .unwrap();

        let abs = tmp.path().join("trackers/topical.md");
        let row = crate::librarian::catalog::artifact::get(
            &ctx.catalog.lock(),
            &crate::librarian::ids::artifact_id_from_abs(&abs),
        )
        .unwrap()
        .unwrap();
        assert_eq!(row.topic.as_deref(), Some("auth middleware"));

        // The value must also land in the YAML frontmatter, not just the catalog.
        let on_disk = std::fs::read_to_string(&abs).unwrap();
        let (fm, _) = crate::librarian::frontmatter::parse(&on_disk).unwrap();
        assert_eq!(fm.unwrap().topic.as_deref(), Some("auth middleware"));
    }

    #[tokio::test]
    async fn create_with_extra_writes_custom_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/custom.md",
                "kind": "tracker",
                "title": "Custom",
                "body": "",
                "extra": {"origin_session_id": "abc123", "branch": "feature/x"}
            }),
        )
        .await
        .unwrap();

        let abs = tmp.path().join("trackers/custom.md");
        let on_disk = std::fs::read_to_string(&abs).unwrap();
        let (fm, _) = crate::librarian::frontmatter::parse(&on_disk).unwrap();
        let fm = fm.unwrap();
        assert_eq!(
            fm.extra.get("origin_session_id"),
            Some(&serde_json::json!("abc123"))
        );
        assert_eq!(
            fm.extra.get("branch"),
            Some(&serde_json::json!("feature/x"))
        );
    }

    #[tokio::test]
    async fn create_rejects_an_extra_key_that_names_a_frontmatter_field() {
        // `kind` is the one a caller reaches for by reflex — a bug file carries
        // `kind: bug`, so restating it in `extra` looks like belt-and-braces. It used to
        // write the key twice and cost the file its entire frontmatter.
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let err = call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "issues/x.md",
                "kind": "bug",
                "title": "X",
                "body": "",
                "extra": {"kind": "bug", "opened": "2026-08-08"}
            }),
        )
        .await
        .expect_err("a reserved key in `extra` must be refused, not written");

        let msg = err.to_string();
        assert!(msg.contains("kind"), "the error must name the clash: {msg}");
        assert!(
            msg.contains("own parameter"),
            "the hint must point at the right channel: {msg}"
        );

        // Refused means refused: no half-written file left behind for a later reindex
        // to classify from a glob.
        assert!(
            !tmp.path().join("issues/x.md").exists(),
            "the artifact must not be created"
        );
    }
    /// The documented workflow IS the reproduction: `docs/issues/_TEMPLATE.md` opens
    /// with `---` and `get_guide("tracker-conventions")` says to copy it, so the body
    /// below is what a caller following instructions actually passes.
    ///
    /// The fixture's load-bearing detail is `"status": "scratch"` INSIDE the body block
    /// while the call passes no `status=` parameter. That is what makes this the
    /// silent-conflict case rather than merely a cosmetic one: pre-fix, the catalog
    /// defaulted `status` to `open` for `kind: bug` and `artifact(action="get")`
    /// reported `open`, with the caller's `scratch` present on disk and readable by
    /// nothing. Change it to agree with the default and the test still passes while
    /// no longer demonstrating the harm.
    ///
    /// Mutation caught: deleting the `reject_body_leading_frontmatter` call in `call()`,
    /// or narrowing its predicate to require a closing `---`.
    #[tokio::test]
    async fn create_refuses_a_body_that_opens_its_own_frontmatter_block() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let err = call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "issues/x.md",
                "kind": "bug",
                "title": "X",
                "body": "---\nstatus: scratch\nseverity: low\n---\n\n# BUG: something\n"
            }),
        )
        .await
        .expect_err("a body opening its own frontmatter block must be refused");

        let msg = err.to_string();
        assert!(
            msg.contains("frontmatter block"),
            "the error must name what it found: {msg}"
        );
        assert!(
            msg.contains("extra="),
            "the hint must route the non-schema keys to the channel that accepts them: {msg}"
        );
        assert!(
            msg.contains("blank line"),
            "refusing a token owes an escape, and the hint is where a caller meets it: {msg}"
        );

        // Refused means refused — and this guard runs BEFORE the disk write, so there is
        // no orphan for a later reindex to classify from a glob.
        assert!(
            !tmp.path().join("issues/x.md").exists(),
            "the artifact must not be created"
        );
    }

    /// The twin, and the one that carries the weight. The refusal test above is an
    /// EXISTENCE assertion ("an error came back"), which is monotone under widening —
    /// a guard that refused every body whatsoever would satisfy it completely. This
    /// mutates in the direction that one cannot: bodies containing `---` that are not
    /// a leading block must still be created.
    ///
    /// Three shapes, because they fail for three different reasons: a leading blank
    /// line (the escape the refusal's own hint promises, so this is that promise under
    /// test), a horizontal rule further down, and a fenced YAML example — the last
    /// being how any document *about* frontmatter is written, including this repo's
    /// own guides.
    ///
    /// Mutation caught: widening the predicate to `body.contains("---")` or to a
    /// `trim_start()`-ed prefix check kills this and leaves the refusal test green.
    #[tokio::test]
    async fn create_accepts_a_body_whose_dashes_are_not_a_leading_block() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        for (n, body) in [
            "\n---\nstatus: scratch\n---\n\ntext\n",
            "# Title\n\nsome prose\n\n---\n\nmore prose\n",
            "# Guide\n\n```yaml\n---\nkind: bug\n---\n```\n",
        ]
        .iter()
        .enumerate()
        {
            let rel = format!("issues/ok-{n}.md");
            call(
                &ctx,
                json!({
                    "repo": "r", "rel_path": rel,
                    "kind": "bug", "title": "X", "body": body
                }),
            )
            .await
            .unwrap_or_else(|e| panic!("body {n} must be accepted, got: {e}"));
            assert!(tmp.path().join(&rel).exists(), "body {n} must be written");
        }
    }

    /// The hint is the whole point of this check — a caller who cannot act on it pays a
    /// wasted round-trip. The message used to be written for `create` and emitted on both
    /// surfaces, so it told every `update` caller to pass a `kind=` parameter that
    /// `update` does not have. Neither existing test could catch that: both assert only
    /// that the message names the clashing key, which the wrong hint also did.
    ///
    /// See `docs/issues/archive/2026-08-16-artifact-update-kind-hint-misroutes.md`.
    #[test]
    fn reserved_key_hint_names_a_remedy_that_exists_on_the_calling_surface() {
        let hint_for = |key: &str, surface: ExtraKeySurface| {
            let mut extra = std::collections::BTreeMap::new();
            extra.insert(key.to_string(), serde_json::json!("x"));
            reject_reserved_extra_keys(&extra, surface)
                .expect_err("a reserved key in `extra` is always refused")
                .to_string()
        };

        // create: every reserved key but `id` really is a top-level parameter.
        let create_kind = hint_for("kind", ExtraKeySurface::Create);
        assert!(
            create_kind.contains("own parameter"),
            "create must still route to the top-level parameter: {create_kind}"
        );

        // update: `kind` is reachable through no channel at all, so the hint must say
        // that rather than name a parameter the caller will go looking for and not find.
        let update_kind = hint_for("kind", ExtraKeySurface::Update);
        assert!(
            !update_kind.contains("own parameter"),
            "update has no top-level `kind=` parameter to route to: {update_kind}"
        );
        assert!(
            update_kind.contains("fixed once the artifact exists"),
            "update must explain that `kind` is not settable: {update_kind}"
        );

        // update: the six settable reserved keys live inside `patch`, not as bare
        // parameters — the old hint was wrong for these too, not only for `kind`.
        let update_status = hint_for("status", ExtraKeySurface::Update);
        assert!(
            update_status.contains("patch={status"),
            "update must route settable keys through `patch`: {update_status}"
        );

        // `id` is caller-supplied on neither surface.
        for surface in [ExtraKeySurface::Create, ExtraKeySurface::Update] {
            let h = hint_for("id", surface);
            assert!(
                h.contains("derived from the artifact's path"),
                "`id` is never caller-settable: {h}"
            );
        }
    }

    #[tokio::test]
    async fn tracker_without_augment_returns_hint() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "repo": "r",
                "rel_path": "docs/trackers/my-tracker.md",
                "kind": "tracker",
                "title": "My Tracker",
                "body": ""
            }),
        )
        .await
        .unwrap();
        assert!(
            result["tracker_hint"].is_string(),
            "tracker without augment must include tracker_hint"
        );
    }

    #[tokio::test]
    async fn tracker_with_augment_no_hint() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "repo": "r",
                "rel_path": "docs/trackers/augmented-tracker.md",
                "kind": "tracker",
                "title": "Augmented Tracker",
                "body": "",
                "augment": {"prompt": "track the state of X"}
            }),
        )
        .await
        .unwrap();
        assert!(
            result.get("tracker_hint").is_none(),
            "tracker with augment must not include tracker_hint"
        );
    }

    #[tokio::test]
    async fn non_tracker_kind_no_hint() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "repo": "r",
                "rel_path": "docs/plans/my-plan.md",
                "kind": "plan",
                "title": "My Plan",
                "body": ""
            }),
        )
        .await
        .unwrap();
        assert!(
            result.get("tracker_hint").is_none(),
            "non-tracker kind must not include tracker_hint"
        );
    }

    #[tokio::test]
    async fn creates_with_inferred_repo() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        let mut ctx = mk_ctx(path.clone());
        ctx.current_project = Some(Arc::new(CurrentProject {
            abs_path: path.clone(),
            git_root: path.clone(),
            main_root: None,
            umbrella: None,
        }));
        let result = call(
            &ctx,
            json!({
                "rel_path": "docs/inferred.md",
                "kind": "spec",
                "title": "Inferred",
                "body": "body"
            }),
        )
        .await
        .unwrap();
        let abs = result["abs_path"].as_str().unwrap();
        assert!(abs.ends_with("docs/inferred.md"), "got: {abs}");
        assert!(
            abs.starts_with(path.to_string_lossy().as_ref()),
            "got: {abs}"
        );
    }

    #[tokio::test]
    async fn creates_with_subdir_prepend() {
        let tmp = TempDir::new().unwrap();
        let root_path = tmp.path().to_path_buf();
        let proj_path = root_path.join("myproj");
        std::fs::create_dir_all(&proj_path).unwrap();
        let mut ctx = mk_ctx(root_path.clone());
        ctx.current_project = Some(Arc::new(CurrentProject {
            abs_path: proj_path.clone(),
            git_root: root_path.clone(),
            main_root: None,
            umbrella: None,
        }));
        let result = call(
            &ctx,
            json!({
                "rel_path": "docs/foo.md",
                "kind": "spec",
                "title": "Subdir",
                "body": "body"
            }),
        )
        .await
        .unwrap();
        let abs = result["abs_path"].as_str().unwrap();
        let expected = proj_path.join("docs/foo.md");
        assert_eq!(abs, expected.to_string_lossy());
    }

    #[tokio::test]
    async fn wrong_repo_error_lists_valid_names() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "repo": "no-such-repo",
                "rel_path": "docs/x.md",
                "kind": "spec",
                "title": "X",
                "body": ""
            }),
        )
        .await
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no-such-repo"), "should name the bad repo");
        assert!(
            msg.contains('"') || msg.contains('r'),
            "should list valid repos"
        );
    }

    #[tokio::test]
    async fn create_refuses_temp_workspace_into_real_catalog() {
        // The real pollution shape: catalog outside the guard's temp root, workspace under
        // it. Both are physically under the OS temp dir — `synthetic_temp` explains why
        // that is deliberate, and why deriving "outside" from `current_dir()` (what this
        // test used to do) inverts silently from a cwd under /tmp.
        let (_scratch, env, inside, outside) =
            crate::librarian::tools::temp_write_guard::synthetic_temp();
        let cat = Catalog::open(&outside.join("catalog.db")).unwrap();
        let ws = inside.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "r".into(),
                path: ws,
            })
            .with_temp_guard(env)
            .build();

        let err = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/specs/x.md",
                "kind": "spec", "title": "X", "body": "hi",
            }),
        )
        .await
        .expect_err("temp workspace + real (outside-temp) catalog must be refused");
        assert!(
            err.to_string().contains("temp dir"),
            "unexpected error: {err}"
        );
    }
}
