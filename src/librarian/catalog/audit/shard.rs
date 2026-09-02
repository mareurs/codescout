//! Committed audit shards: the export half.
//!
//! The local WAL cannot live in git — its in-transaction guarantee exists only
//! at mutation time on a gitignored database — so what git carries is a
//! REPLICA, and every surface must say so. See the spec's § Phase 2.
//!
//! Task 4 gave this file its real (non-test) callers: `export` is called
//! from `audit_log::call`'s `export=true` branch and from `reindex`'s
//! best-effort fold-in; `read_shards` is called from `audit_log::call`'s
//! query path to merge other hosts' committed rows into the local result.
//! Same `#[cfg_attr(not(test), expect(dead_code, reason = "..."))]` pattern
//! as `host.rs` — those attributes guarded every item here while only this
//! file's own `#[cfg(test)] mod tests` reached them, and were DELETED, not
//! widened, once `unfulfilled_lint_expectations` confirmed each had a live
//! non-test caller; none remain in this file today.

use super::host::{self};
use crate::librarian::catalog::gc;
use anyhow::{Context, Result};
use fs4::fs_std::FileExt;
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

pub(crate) const WATERMARK_KEY: &str = "audit_exported_through_seq";

/// The write-dedup / doctor-count cursor — DISTINCT from `WATERMARK_KEY`.
///
/// Ruling 16 (task-6 round-3 review): the two are different quantities and
/// conflating them into one clamped cursor was the round-3 Critical. The
/// `WATERMARK_KEY` cursor is a RECOVERABILITY cursor — it must retry an
/// unattributed row forever, so it is clamped below the earliest one seen
/// and can sit still indefinitely. If the SAME cursor also gated writing,
/// then the moment it sticks (a `d7bbeba8a9f23dd8`-shaped row: an artifact
/// insert whose artifact was since deleted, so `by_artifact_id` cannot
/// resolve it — this is not theoretical, it is `seq = 1` on the live
/// catalog), every later row that resolves cleanly would be re-selected
/// AND re-appended on every subsequent export, forever: the clamp holds
/// the CURSOR back correctly, but does nothing to stop the WRITE, because
/// the write happens during the same loop that computes the clamp, before
/// the clamp is known.
///
/// This cursor answers a different question — "what is already durably on
/// disk for this repo" — and is intentionally UNCLAMPED: it advances at
/// every row this export call could dispose of one way or another (a
/// commit/churn skip, a foreign attribution, or an export write), and only
/// stalls at a genuinely unattributed row (mirrors the OLD, pre-Ruling-16
/// running-max exactly, at the same three call sites — see `export`). A
/// row is appended to a shard file only when its `seq` is strictly greater
/// than this cursor's value as read at the START of the call, which is
/// what makes a repeat export with the same stuck unattributed row a
/// no-op on the file instead of doubling it (task-6 round-3 required
/// test: `a_repeat_export_with_the_row_still_unattributed_does_not_regrow_the_file`).
pub(crate) const WRITTEN_KEY: &str = "audit_written_through_seq";

/// Changed-key sets that are pure reindex bookkeeping. An `update` whose keys
/// are a SUBSET of this is dropped from the export; one that also carries any
/// other key is real history and is kept.
const CHURN_KEYS: &[&str] = &["file_mtime", "file_sha256", "updated_at", "missing_since"];

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ShardLine {
    // `#[serde(default)]` on every field below except `host` and `seq`: this
    // file's entire purpose is being read by binaries of OTHER vintages (a
    // shard is a git-merged file, committed by whichever version wrote it,
    // read back by whatever version is running now). Without a default,
    // adding any new required field on a future version makes every
    // already-committed line from an older version fail to parse —
    // `read_shards` would count each one as `malformed`, not "missing this
    // field", the day that version ships (Important, task-6 round-3 review).
    // `host` and `seq` are deliberately excluded, and for the SAME reason:
    // together they are the dedup key in `read_shards`' `(host, seq)` `seen`
    // set, and a defaulted `""`/`0` on either half would silently collide two
    // genuinely distinct rows into one `seen` entry rather than surfacing a
    // real parse failure (task-6 final review, Minor — `seq` alone was
    // excluded pre-final-review, which applied the argument to only one half
    // of a composite key) — worse than counting the line `malformed`, which
    // at least says so.
    pub host: String,
    pub seq: i64,
    #[serde(default)]
    pub at_ms: i64,
    #[serde(default)]
    pub tbl: String,
    #[serde(default)]
    pub op: String,
    #[serde(default)]
    pub row_id: String,
    #[serde(default)]
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Default, serde::Serialize)]
pub(crate) struct ExportReport {
    pub exported: usize,
    pub skipped_commits: usize,
    pub skipped_churn: usize,
    /// Rows whose owning repo could not be determined (the artifact/event
    /// they hang off is already gone and the payload does not carry enough
    /// to say). Counted so the number is visible, never exported, and —
    /// unlike `skipped_commits`/`skipped_churn` — never advances the
    /// watermark past itself: see the comment in `export`'s loop for the
    /// mechanism (a tracked minimum, not the single skipped iteration).
    pub unattributed: usize,
    /// Rows confidently attributed to a repo OTHER than `repo_root`. Correct
    /// and silent when that other repo eventually runs its own export (its
    /// own watermark governs it, and never consults ours) — but silent
    /// forever for a repo that never will, e.g. a linked worktree created
    /// with `git worktree add <path-outside-the-main-checkout>`: such a
    /// worktree's own rows attribute to a path under ITS root, while every
    /// export call for that session resolves `repo_root` to the MAIN
    /// checkout (see `project_root()` in audit_log.rs/reindex.rs) — so those
    /// rows are always "foreign" here and are never claimed by any export
    /// this codebase issues. Counting them at least makes the drop visible;
    /// it does not fix it. See the doc comment on `project_root()`.
    pub foreign: usize,
    pub files: Vec<String>,
    pub through_seq: i64,
    /// Absolute path of the directory `export` wrote into (or would write
    /// into, on a no-op call) — `host::audit_dir(repo_root)`. Ruling 19
    /// (task-6 round-3 review): a session running from a linked worktree
    /// resolves `repo_root` to the MAIN checkout (see `project_root()`), so
    /// the write lands OUTSIDE the tree that session's own `git status`
    /// looks at. The destination was always correct; it was invisible.
    /// Naming it here is the fix — "correct code in the wrong tree defeats
    /// every gate" (this repo's own SDD lesson) applies to a human checking
    /// their worktree for new files exactly as much as it does to a CI gate.
    pub dest: String,
}

fn is_pure_churn(op: &str, payload: Option<&str>) -> bool {
    if op != "update" {
        return false;
    }
    let Some(p) = payload else { return false };
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(p) else {
        return false;
    };
    !map.is_empty() && map.keys().all(|k| CHURN_KEYS.contains(&k.as_str()))
}
fn watermark_key(repo_root: &Path) -> String {
    // `RepoPath` gives a stable, forward-slash-normalized string so the same
    // repo keys the same way on every platform; two repos never collide
    // because a full path (not just its final component) is the key body.
    format!(
        "{WATERMARK_KEY}:{}",
        crate::util::fs::RepoPath::from_path(repo_root)
    )
}

/// Per-repo watermark. A pre-existing UNKEYED `audit_exported_through_seq`
/// value (written before this task, when the export was machine-wide) is
/// deliberately never read here — every repo starts at 0 and re-derives its
/// own true position from scratch. See the module-level note on why the old
/// key is left inert rather than deleted.
fn watermark(conn: &Connection, repo_root: &Path) -> Result<i64> {
    Ok(gc::get_meta(conn, &watermark_key(repo_root))?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0))
}

fn written_key(repo_root: &Path) -> String {
    format!(
        "{WRITTEN_KEY}:{}",
        crate::util::fs::RepoPath::from_path(repo_root)
    )
}

/// Per-repo write cursor — see `WRITTEN_KEY`'s doc comment for why this is a
/// separate quantity from `watermark()`. Same "starts at 0, never reads a
/// pre-existing unkeyed value" shape as `watermark()`, for the same reason.
fn written_through(conn: &Connection, repo_root: &Path) -> Result<i64> {
    Ok(gc::get_meta(conn, &written_key(repo_root))?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0))
}

/// Per-repo GC meta key holding the exact set of seqs currently sitting
/// BELOW `written_through` that are still unattributed — i.e. the specific
/// rows `written_through`'s unconditional advance "jumped over".
///
/// Why this exists, on top of the two scalar cursors: `written_through`
/// must advance unconditionally (including past a still-open gap) to fix
/// the Critical this round is about — a PERMANENTLY unattributable row
/// must not gate every later row's write-dedup forever. But that same
/// unconditional advance means a row's own `seq <= written_through` no
/// longer implies "already written" once a gap sits below it: the gap
/// itself has `seq <= written_through` too, the moment a LATER row's
/// resolution drags the cursor past it, and it was never written. Without
/// this set, the write-dedup guard cannot tell those two cases apart and
/// picks one side wrong regardless of which way it defaults — silently
/// dropping the gap row's eventual write forever (breaking Ruling 14's
/// "stays recoverable" property this round is required to preserve), or
/// re-duplicating every row above it on every single retry (reintroducing
/// the unbounded-regrowth bug this round exists to fix).
///
/// Bounded in practice: entries are added only while a row stays
/// unattributed, and removed the moment it resolves (permanently, for
/// most real rows — see `WRITTEN_KEY`'s `d7bbeba8a9f23dd8` example — this
/// set gains one entry and then never shrinks for that row, which is
/// exactly correct: it costs one `HashSet` lookup per export, forever,
/// not a re-scan of the file).
const GAPS_KEY: &str = "audit_open_gaps";

fn gaps_key(repo_root: &Path) -> String {
    format!(
        "{GAPS_KEY}:{}",
        crate::util::fs::RepoPath::from_path(repo_root)
    )
}

fn open_gaps(conn: &Connection, repo_root: &Path) -> Result<BTreeSet<i64>> {
    // Same root cause, same fix family, as the atomicity note on `export`'s
    // tail (task-6 final review, Important): a malformed persisted gap set is
    // the ONE piece of state whose loss is unrecoverable, so a parse failure
    // here must surface as an error, not silently collapse to an empty set.
    // The old `.ok()` -> `unwrap_or_default()` chain reached the identical
    // loss path from a different direction — corrupt JSON (truncated write,
    // manual edit, a future format change) would read back as "no open
    // gaps", `export` would then treat every genuinely-open gap below
    // `written_start` as "already written", and each would be dropped for
    // good with no counter or warning, exactly as an un-persisted gap would.
    match gc::get_meta(conn, &gaps_key(repo_root))? {
        None => Ok(BTreeSet::new()),
        Some(v) => {
            let parsed: Vec<i64> = serde_json::from_str(&v).with_context(|| {
                format!(
                    "corrupt {} value in catalog_meta for {}: {v:?}. Export for this \
                     repo stays blocked until it is repaired. Fix the value to a JSON \
                     array of integers, or delete it TOGETHER WITH {} — never this key \
                     alone: `export` skips any `seq <= written_through` that is not in \
                     this set, so clearing the set while the write cursor stands strands \
                     every gap that has since become attributable, which is the loss \
                     this key exists to prevent. Dropping both re-exports from the \
                     recoverability watermark instead; duplicates dedupe on read by \
                     (host, seq).",
                    gaps_key(repo_root),
                    repo_root.display(),
                    written_key(repo_root),
                )
            })?;
            Ok(parsed.into_iter().collect())
        }
    }
}

/// Resolve which repo an audited row belongs to, by walking from the row
/// back to an `artifact.abs_path` (or, for `worktree_registration`, straight
/// from the row id, which IS the worktree root). Returns `None` when the
/// row cannot be traced to any repo — the underlying artifact/event is
/// already gone and the payload does not carry enough to say.
///
/// Only the `artifact` table's `delete` arm avoids a live join entirely: its
/// payload carries `abs_path` directly, so there is nothing left to look up.
/// Every OTHER table's `delete` arm still ends in a live `SELECT` against
/// `artifact` (`by_artifact_id`, or a slug lookup for `entry_cite`) — even
/// when the id going into that join was itself read from the payload (as in
/// `events`). `artifact_augmentation`, `events`, `artifact_link` and
/// `entry_cite` are all `ON DELETE CASCADE` off `artifact`, so a single
/// artifact delete fires cascade-delete audit rows for all of these in the
/// SAME BATCH, and by the time each one's live join runs, the artifact row
/// it needs is already gone — which is exactly the shape that makes those
/// deletes land as `unattributed`, not the payload-routing this comment used
/// to claim for all of them.
///
/// Routing table (payload keys/columns confirmed against schema.sql,
/// catalog/mod.rs and audit/mod.rs's `AUDITED_TABLES` row-id formats):
/// - `artifact`: row_id IS the artifact id. `delete` reads `abs_path` out of
///   the payload; insert/update join `artifact.id`.
/// - `artifact_augmentation`: row_id IS the artifact id for every op —
///   always a live join, even on delete.
/// - `events`: `delete` reads `artifact_id` out of the payload, then still
///   joins `artifact` on it live; insert/update join `events.id` to get
///   `artifact_id` first, then resolve that the same way.
/// - `artifact_link`: row_id is `"{src_id}→{dst_id}:{rel}"` (no surrounding
///   whitespace) — attribute via `src_id`, always a live join.
/// - `entry_cite`: row_id is `"{src_slug}:{src_local}→{dst_ref}"` — attribute
///   via `src_slug`, joined against `artifact.slug` (always a live join).
///   `artifact.slug` is NULLable (schema.sql) and backfilled lazily by
///   `librarian doctor fix=mint_slugs`; an `entry_cite` row whose artifact
///   has a NULL slug is unattributable by construction, delete or not.
///   Also note: this split assumes `src_slug` itself contains no `:` — no
///   escape and no disambiguator if it ever did (CLAUDE.md § Parsers Over a
///   Namespace). Slugs are server-minted today, so the corpus does not
///   contain one, but nothing here would refuse it if it did.
/// - `worktree_registration`: row_id IS the worktree root itself (the PK),
///   identical across insert/update/delete — no join needed at all.
fn attribute(
    conn: &Connection,
    tbl: &str,
    op: &str,
    row_id: &str,
    payload: Option<&str>,
) -> Option<PathBuf> {
    fn by_artifact_id(conn: &Connection, id: &str) -> Option<PathBuf> {
        conn.query_row("SELECT abs_path FROM artifact WHERE id = ?1", [id], |r| {
            r.get::<_, String>(0)
        })
        .ok()
        .map(PathBuf::from)
    }
    fn payload_str(payload: Option<&str>, key: &str) -> Option<String> {
        let v: serde_json::Value = serde_json::from_str(payload?).ok()?;
        v.get(key)?.as_str().map(str::to_string)
    }

    match tbl {
        "artifact" => {
            if op == "delete" {
                payload_str(payload, "abs_path").map(PathBuf::from)
            } else {
                by_artifact_id(conn, row_id)
            }
        }
        "artifact_augmentation" => by_artifact_id(conn, row_id),
        "events" => {
            let artifact_id = if op == "delete" {
                payload_str(payload, "artifact_id")?
            } else {
                conn.query_row(
                    "SELECT artifact_id FROM events WHERE id = ?1",
                    [row_id],
                    |r| r.get::<_, String>(0),
                )
                .ok()?
            };
            by_artifact_id(conn, &artifact_id)
        }
        "artifact_link" => {
            // No delimiter present would just return the whole string via
            // `.next()`, never `None` — a malformed row_id fails the
            // downstream artifact lookup instead of panicking here.
            let src_id = row_id.split('→').next()?;
            by_artifact_id(conn, src_id)
        }
        "entry_cite" => {
            let before_dst = row_id.split('→').next()?;
            let src_slug = before_dst.split(':').next()?;
            conn.query_row(
                "SELECT abs_path FROM artifact WHERE slug = ?1",
                [src_slug],
                |r| r.get::<_, String>(0),
            )
            .ok()
            .map(PathBuf::from)
        }
        "worktree_registration" => Some(PathBuf::from(row_id)),
        _ => None,
    }
}

/// Rows past the WRITE cursor that export would ACTUALLY consume for this
/// repo — the same population `export` counts as `exported +
/// skipped_commits + skipped_churn` (never `unattributed`, and never a row
/// resolved to some other repo — see `export`'s loop for why both of those
/// are excluded here). Doctor reports this, so it must not describe a
/// different set than the verb does; a delta that counts rows export will
/// never write for this repo reads as a permanent backlog.
///
/// Floors on `written_through`, NOT `watermark` (task-6 round-3 review,
/// Ruling 16): `watermark` is the recoverability cursor and can sit
/// permanently below an unattributed row — flooring THIS scan on it would
/// re-count every already-written row past that point on every single
/// `doctor` call, forever, which is exactly the "delta that can never reach
/// zero" the round-3 review measured on the live catalog (`seq = 1` never
/// resolves, so the old `watermark`-floored count never dropped below the
/// full trail's length). `written_through` is unclamped and advances past
/// everything but a genuinely unattributed row, so it tracks what is
/// actually still outstanding.
///
/// Not measured, flagged cheap (Minor, task-6 review): `attribute()` runs
/// once per row here via `conn.query_row`, which re-prepares its SQL every
/// call rather than reusing a prepared statement — and `doctor` calls this
/// unconditionally, so a repo that has never exported gets its whole trail
/// rescanned, re-preparing the same handful of queries per row, on every
/// `doctor` invocation. Worth a prepared-statement pass if this ever shows
/// up in a profile; not done here.
pub(crate) fn unexported_count(conn: &Connection, repo_root: &Path) -> Result<i64> {
    let w = written_through(conn, repo_root)?;
    let mut stmt = conn.prepare(
        "SELECT tbl, op, row_id, payload FROM catalog_audit WHERE seq > ?1 ORDER BY seq",
    )?;
    let rows = stmt
        .query_map([w], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut n = 0i64;
    for (tbl, op, row_id, payload) in rows {
        if tbl == "commits" || is_pure_churn(&op, payload.as_deref()) {
            n += 1;
            continue;
        }
        if matches!(attribute(conn, &tbl, &op, &row_id, payload.as_deref()), Some(owner) if owner.starts_with(repo_root))
        {
            n += 1;
        }
    }
    Ok(n)
}

/// Ruling 18 (task-6 round-3 review): the automatic reindex fold-in export
/// must never write into a repo that has not opted in to merging shard files
/// — writing `.codescout/audit/*.jsonl` into a checkout whose `.gitattributes`
/// lacks `merge=union` for that path means a future branch merge treats two
/// hosts' independently-appended lines as a plain-text conflict instead of a
/// clean union, the exact hazard the attribute exists to prevent. A manual
/// `export=true` call (`audit_log.rs`) is a deliberate, individually-reviewed
/// action and stays UNGATED — this check exists only for the automatic,
/// every-reindex path where nobody is looking at each write.
///
/// Deliberately tolerant of exact spelling variance: matches a line whose
/// whitespace-trimmed form is at least the two tokens `.codescout/audit/*.jsonl`
/// and `merge=union`, in either order, ignoring any other attributes on the
/// same line (e.g. `.codescout/audit/*.jsonl merge=union -diff` still counts).
/// A missing or unreadable `.gitattributes` is treated as "not opted in", not
/// as an error — most repos simply have not adopted `.codescout/audit/` yet.
pub(crate) fn gitattributes_declares_shard_union(repo_root: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(repo_root.join(".gitattributes")) else {
        return false;
    };
    text.lines().any(|line| {
        let line = line.trim();
        if line.starts_with('#') {
            return false;
        }
        let mut tokens = line.split_whitespace();
        let Some(pattern) = tokens.next() else {
            return false;
        };
        pattern == ".codescout/audit/*.jsonl" && tokens.any(|t| t == "merge=union")
    })
}

pub(crate) fn export(conn: &Connection, repo_root: &Path) -> Result<ExportReport> {
    let host_id = host::resolve_host_id(conn)?;
    let from = watermark(conn, repo_root)?;
    let written_start = written_through(conn, repo_root)?;
    // Snapshot of which seqs BELOW `written_start` are known gaps — loaded
    // BEFORE the loop so the write-gate can tell "already written" (not in
    // this set) from "jumped over by a later row's advance, never written"
    // (in this set). See `GAPS_KEY`'s doc comment for why a scalar cursor
    // alone cannot make this distinction.
    let gaps_start = open_gaps(conn, repo_root)?;
    let dir = host::audit_dir(repo_root);

    let mut stmt = conn.prepare(
        "SELECT seq, at_ms, tbl, op, row_id, actor, verb, payload
             FROM catalog_audit WHERE seq > ?1 ORDER BY seq",
    )?;
    let rows = stmt
        .query_map([from], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut report = ExportReport {
        through_seq: from,
        dest: dir.display().to_string(),
        ..Default::default()
    };
    // Keyed by month string; value is (a representative at_ms for that month,
    // used only to derive the shard filename, and the serialized lines).
    let mut by_month: BTreeMap<String, (i64, Vec<String>)> = BTreeMap::new();
    // Tracks the smallest seq of any row this batch could not attribute.
    // `through_seq` is clamped against it ONCE, after the whole batch is
    // scanned (see the clamp below) — not skipped only at its own iteration.
    // Without this, a later, higher-seq ROW THAT DOES resolve (e.g. an
    // artifact's own delete row, attributable via its payload) still runs
    // the loop's running `.max()` and drags `through_seq` past the earlier
    // unattributed row, permanently stranding it: `ON DELETE CASCADE` on
    // `events`, `artifact_link`, `entry_cite`, `artifact_augmentation` means
    // a single artifact delete produces exactly this shape in ONE batch —
    // the cascade-deleted children (unattributable, their live join fails)
    // alongside the parent's own delete row (attributable via payload).
    // (task-6 review, Critical 1.) `.get_or_insert` is correct only because
    // rows are visited in ascending `seq` order (`ORDER BY seq` above), so
    // the first unattributed hit is always the minimum.
    let mut min_unattributed: Option<i64> = None;
    // The UNCLAMPED write cursor for THIS call — see `WRITTEN_KEY`'s doc
    // comment. Advances at every row this loop can fully dispose of; only an
    // unattributed row leaves it where it was (task-6 round-3, Ruling 16).
    let mut written_max = written_start;
    // Every seq this call still cannot attribute — becomes the NEXT persisted
    // `GAPS_KEY` set wholesale (see `open_gaps`'s doc comment: a persisted
    // gap that resolves this call simply does not get re-added here, which is
    // exactly the removal rule, since every previously-open gap has a seq
    // strictly greater than `from` and is therefore always rescanned above).
    let mut gaps_this_call: BTreeSet<i64> = BTreeSet::new();

    for (seq, at_ms, tbl, op, row_id, actor, verb, payload) in rows {
        // commits/churn need no attribution at all — they are skip-only for
        // EVERY repo's export, so advancing past them here is always safe
        // regardless of which repo owns them (they don't have one).
        if tbl == "commits" {
            report.through_seq = report.through_seq.max(seq);
            written_max = written_max.max(seq);
            report.skipped_commits += 1;
            continue;
        }
        if is_pure_churn(&op, payload.as_deref()) {
            report.through_seq = report.through_seq.max(seq);
            written_max = written_max.max(seq);
            report.skipped_churn += 1;
            continue;
        }

        // Attribute BEFORE touching the watermark. A row that cannot be
        // traced to any repo must not be treated as "past" THIS repo's
        // cursor: it might in fact belong to a repo that has not exported
        // yet, and advancing here would make it permanently unrecoverable —
        // no other repo's independently-keyed watermark would ever revisit
        // a seq value below its own cursor. Counted so the number is
        // visible; never exported.
        let Some(owner) = attribute(conn, &tbl, &op, &row_id, payload.as_deref()) else {
            report.unattributed += 1;
            min_unattributed.get_or_insert(seq);
            gaps_this_call.insert(seq);
            continue;
        };

        // A row confidently attributed to a DIFFERENT repo is a stable,
        // resolved fact, not an open question — safe to advance past it.
        // That other repo's own watermark governs whether it re-exports the
        // row, and it never consults ours.
        report.through_seq = report.through_seq.max(seq);
        written_max = written_max.max(seq);
        if !owner.starts_with(repo_root) {
            report.foreign += 1;
            continue;
        }

        // Write-dedup (Ruling 16, task-6 round-3): this row resolves to US,
        // but it may already be sitting on disk from a PRIOR export call —
        // selection always rescans from the (possibly stuck) recoverability
        // watermark, which can sit well below `written_start` for as long as
        // some earlier row stays unattributed. Only a row past our own last
        // write (OR one this exact call still owes, per `gaps_start`)
        // actually needs writing; re-appending one we already wrote would
        // duplicate it on every single retry of the stuck row. A row in
        // `gaps_start` is the one case where `seq <= written_start` does NOT
        // mean "already on disk" — it means "jumped over while stuck, and
        // resolving right now" — see `GAPS_KEY`'s doc comment.
        if seq <= written_start && !gaps_start.contains(&seq) {
            continue;
        }

        let line = ShardLine {
            host: host_id.clone(),
            seq,
            at_ms,
            tbl,
            op,
            row_id,
            actor,
            verb,
            // A payload that fails to parse as JSON is still audit data — do
            // not let it vanish silently. `skip_serializing_if` only omits a
            // genuine `None`, so falling back to a string preserves the raw
            // bytes in the shard line instead of dropping the field with no
            // error and no counter (small fix 8, task-2 review).
            payload: payload.as_deref().map(|p| {
                serde_json::from_str(p).unwrap_or_else(|_| serde_json::Value::String(p.to_string()))
            }),
        };
        by_month
            .entry(host::month_key(at_ms))
            .or_insert_with(|| (at_ms, Vec::new()))
            .1
            .push(serde_json::to_string(&line)?);
        report.exported += 1;
    }

    // Clamp once, after the whole batch is scanned: no row at or after the
    // earliest unattributed seq is allowed to count as "exported past" for
    // this repo's watermark, no matter how many later rows in the same
    // batch resolved cleanly (task-6 review, Critical 1).
    if let Some(min_seq) = min_unattributed {
        report.through_seq = report.through_seq.min(min_seq - 1);
    }

    if !by_month.is_empty() {
        // Only created when there is something to write — a no-op export
        // (everything skipped, or nothing past the watermark) must not touch
        // the filesystem at all (small fix 10, task-2 review). The test
        // helper `lines()` was updated to tolerate a missing directory so it
        // no longer forces this to run unconditionally.
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        for (representative_at_ms, lines) in by_month.values() {
            // Ruling 4: the filename convention has exactly one definition —
            // host::shard_file_name — so Task 1's shard_names_round_trip test
            // covers the bytes this writer actually produces. Building the
            // name inline here would let the two drift with no error anywhere.
            let name = host::shard_file_name(&host_id, *representative_at_ms);
            let path = dir.join(&name);
            // One exclusive lock per file: two sessions reindexing at once must
            // not interleave partial lines into a file that is about to be
            // committed. Same primitive as src/retrieval/index_lock.rs. Live
            // callers are audit_log.rs (manual `export=true`) and reindex.rs
            // (the automatic fold-in), wired by Task 4 — real contention is
            // reachable now. No test exercises it: two concurrent reindexes
            // cannot be staged within one test process.
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                // `.read(true)` is NOT redundant, and is load-bearing on Windows only.
                //
                // The next line locks this handle, and `fs4`'s Windows path is
                // `LockFileEx`, which REQUIRES the handle to carry `GENERIC_READ` or
                // `GENERIC_WRITE`. std maps an append-only open to
                // `FILE_GENERIC_WRITE & !FILE_WRITE_DATA` (`library/std/src/sys/fs/
                // windows.rs`, `get_access_mode`, the `(false, _, true, None)` arm) —
                // which is neither, so `LockFileEx` returns `ERROR_ACCESS_DENIED` and
                // every export fails with a bare `Access is denied. (os error 5)`.
                // Adding read selects the `(true, _, true, None)` arm, which ORs
                // `GENERIC_READ` back in. Append semantics are unchanged.
                //
                // Invisible on Unix, where `flock(2)` ignores the descriptor's access
                // mode entirely: this cost 21 tests on every `windows-latest` lane while
                // Linux and macOS stayed green. Do not "simplify" it away.
                // docs/issues/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md
                .read(true)
                .append(true)
                .open(&path)
                .with_context(|| format!("opening {}", path.display()))?;
            FileExt::lock_exclusive(&f).with_context(|| format!("locking {}", path.display()))?;
            // A single `write_all` over one already-newline-terminated buffer,
            // not `writeln!`, which lowers to two separate `write_all` calls
            // (body, then "\n") — a kill between them leaves a torn line with
            // no trailing newline, and the next export's append then
            // concatenates its first JSON object onto that unterminated line
            // (small fix 3, task-2 review). One write_all is one syscall; a
            // partial write of it still can't interleave a second object onto
            // the same physical line the way two calls can.
            let r = f
                .write_all(format!("{}\n", lines.join("\n")).as_bytes())
                .and_then(|_| f.sync_all());
            let _ = FileExt::unlock(&f);
            r.with_context(|| format!("appending to {}", path.display()))?;
            report.files.push(name);
        }
        // Best-effort: fsync the directory too, so the dirent linking a
        // newly-created shard file is durable. Without this, a power loss
        // can durably advance the watermark (SQLite fsyncs its own commit)
        // while the shard file's directory entry never lands — the exact
        // silent-loss mode the append-then-watermark ordering exists to
        // prevent (small fix 4, task-2 review). Best-effort and ignored on
        // platforms/filesystems that refuse to open or sync a directory.
        if let Ok(dir_handle) = std::fs::File::open(&dir) {
            let _ = dir_handle.sync_all();
        }
    }

    // Only after every append (and, best-effort, its directory entry) is
    // durable — see the module-level ordering note on this function, and
    // a_failed_append_never_advances_the_watermark/a_crash_between_append_and_
    // watermark_duplicates_rather_than_loses below, which pin it from both
    // directions.
    //
    // All three cursors below are persisted in ONE transaction, gaps FIRST
    // (task-6 final review, Important). The three are not interchangeable on
    // partial failure: a stale `watermark` just re-scans more rows next call,
    // and a stale `written_through` just re-writes (dedup-safe, "duplicate,
    // never lose") rows already on disk — both fail safe. A stale (absent)
    // `gaps` set does not: if `written_through` had already committed while
    // `gaps` had not, the next call hits `seq <= written_start &&
    // !gaps_start.contains(&seq)`, skips the still-open gap as "already
    // written", and — because a skipped row is never re-added to
    // `gaps_this_call` — the gap is dropped from the set for good, with no
    // counter or warning. `gc::set_meta` is a bare `conn.execute`, so without
    // a transaction a crash or SQLITE_BUSY between calls (unremarkable on a
    // machine-wide shared catalog) can commit one and lose another. Wrapping
    // all three in one `unchecked_transaction()` closes that window; ordering
    // gaps first inside it is belt-and-braces, not load-bearing once the
    // transaction exists — do not "tidy" the order back to
    // watermark/written/gaps, since that silently removes the
    // fails-safe-if-still-partial property this comment documents.
    let tx = conn.unchecked_transaction()?;
    gc::set_meta(
        &tx,
        &gaps_key(repo_root),
        &serde_json::to_string(&gaps_this_call.into_iter().collect::<Vec<i64>>())?,
    )?;
    gc::set_meta(
        &tx,
        &watermark_key(repo_root),
        &report.through_seq.to_string(),
    )?;
    // The write cursor is persisted unconditionally, even on a call that
    // wrote nothing to disk — it still needs to remember that commits/churn/
    // foreign rows up to `written_max` were fully dispositioned, or
    // `unexported_count` (which now floors its scan on this cursor, not the
    // recoverability one) would re-scan and re-count them on every call
    // forever (Ruling 16, task-6 round-3 review).
    gc::set_meta(&tx, &written_key(repo_root), &written_max.to_string())?;
    tx.commit()?;
    Ok(report)
}

#[derive(Debug, Default)]
pub(crate) struct ShardRead {
    pub rows: Vec<ShardLine>,
    pub malformed: usize,
    /// host → (min seq, max seq) present, **scoped to the files that survived
    /// window pruning** — not the full committed shard set for that host. A
    /// file skipped by `month_in_window` never has its lines parsed, so a host
    /// whose only shard falls outside the query window is simply absent from
    /// this map rather than reporting a window that was never opened. This is
    /// DERIVED from the rows rather than declared in a header: a header line
    /// would be duplicated by `merge=union` on every same-host branch merge,
    /// and a declared window that disagrees with the rows is worse than none.
    pub hosts: BTreeMap<String, (i64, i64)>,
    /// Files whose lines were parsed into `rows`/`hosts`. Task 4 deferred
    /// decision 3: this does NOT sum with `files_skipped_by_window` +
    /// `unreadable_files` to the on-disk file count in the audit directory —
    /// `self_host`'s own shard file is excluded before any of these three
    /// counters is touched, so the delta between the sum and the directory
    /// listing is exactly one file (this host's own) on any host that has
    /// ever exported, and zero on a host that never has.
    pub files_read: usize,
    pub files_skipped_by_window: usize,
    /// Shard files that parsed as a shard name, survived the window, but
    /// could not be read (permissions, race with a concurrent writer, a
    /// symlink to nowhere). Counted separately from `malformed` — malformed
    /// is a bad *line* inside a file we did read; this is a whole file we
    /// never got to look at, and conflating them would hide which one it is.
    pub unreadable_files: usize,
}

/// Mirrors `filter_where` (`mod.rs`) field-for-field — the SQL predicate for
/// local rows and this predicate for shard rows must never drift, since
/// Task 4 sums a SQL-filtered count and a `matches()`-filtered count into one
/// `filtered_total`. The exhaustive destructure below is the parity guard: a
/// new `AuditFilter` field added to `filter_where` and forgotten here fails
/// the build instead of silently producing a wrong total.
fn matches(l: &ShardLine, f: &super::AuditFilter) -> bool {
    let super::AuditFilter {
        tbl,
        row_id,
        actor,
        op,
        since,
        until,
    } = f;
    tbl.as_ref().is_none_or(|v| *v == l.tbl)
        && row_id.as_ref().is_none_or(|v| *v == l.row_id)
        && actor.as_ref().is_none_or(|v| *v == l.actor)
        && op.as_ref().is_none_or(|v| *v == l.op)
        && since.is_none_or(|v| l.at_ms >= v)
        && until.is_none_or(|v| l.at_ms <= v)
}

/// `self_host`'s own shard is skipped: those rows are already in the local
/// table, and counting them twice would produce a wrong `filtered_total` —
/// a plausible number rather than an error, which nothing downstream catches.
pub(crate) fn read_shards(
    repo_root: &Path,
    f: &super::AuditFilter,
    self_host: &str,
) -> Result<ShardRead> {
    let dir = host::audit_dir(repo_root);
    let mut out = ShardRead::default();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out); // never exported, or a fresh clone: an empty read.
    };
    let mut seen: std::collections::HashSet<(String, i64)> = Default::default();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some((file_host, month)) = host::parse_shard_file_name(&name) else {
            continue; // a README or stray file: not a shard, not malformed.
        };
        if file_host == self_host {
            continue;
        }
        if !month_in_window(&month, f) {
            out.files_skipped_by_window += 1;
            continue;
        }
        let Ok(body) = std::fs::read_to_string(e.path()) else {
            // A file whose NAME parsed as a valid shard but whose CONTENTS we
            // could not read (permissions, a race with a concurrent writer, a
            // dangling symlink) must not vanish silently — that would under-count
            // `files_read` against what the directory listing promised with no
            // signal anywhere that a file was dropped.
            out.unreadable_files += 1;
            continue;
        };
        out.files_read += 1;
        for raw in body.lines() {
            if raw.trim().is_empty() {
                continue;
            }
            let Ok(line) = serde_json::from_str::<ShardLine>(raw) else {
                out.malformed += 1;
                continue;
            };
            // Defense in depth: the file name already excludes self_host, but a
            // line's own `host` field is attacker/bug-reachable independently of
            // the file it lives in (a manual edit, a future writer bug that puts
            // the wrong host in the payload). Checking the field too costs one
            // string comparison and closes that gap rather than trusting the
            // filename alone to carry the invariant.
            if line.host == self_host {
                continue;
            }
            let key = (line.host.clone(), line.seq);
            let span = out
                .hosts
                .entry(line.host.clone())
                .or_insert((line.seq, line.seq));
            span.0 = span.0.min(line.seq);
            span.1 = span.1.max(line.seq);
            if !seen.insert(key) || !matches(&line, f) {
                continue;
            }
            out.rows.push(line);
        }
    }
    // Sort key is `at_ms`, never `seq`: `seq` is a per-host autoincrement, so
    // two different hosts' rows can carry the identical seq from 1 upward —
    // it has no meaning as a CROSS-host ordering key. `at_ms` is a real wall-clock
    // timestamp and is the only field here that is comparable across hosts.
    out.rows.sort_by(|a, b| {
        b.at_ms
            .cmp(&a.at_ms)
            .then_with(|| b.seq.cmp(&a.seq))
            .then_with(|| a.host.cmp(&b.host))
    });
    Ok(out)
}

/// Whole-file pruning from the filename's `YYYYMM`. Inclusive at both ends and
/// deliberately coarse — a month that straddles the boundary is opened and its
/// rows filtered per-line. Being generous here is the safe direction: a file
/// wrongly skipped is a silently missing row.
fn month_in_window(month: &str, f: &super::AuditFilter) -> bool {
    let in_bound = |ms: i64, keep_if_ge: bool| -> bool {
        let bound = host::month_key(ms);
        if keep_if_ge {
            month.as_bytes() >= bound.as_bytes()
        } else {
            month.as_bytes() <= bound.as_bytes()
        }
    };
    f.since.is_none_or(|v| in_bound(v, true)) && f.until.is_none_or(|v| in_bound(v, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact, Catalog};

    // Places the seeded artifact UNDER `root` — load-bearing for every test
    // that exports against `tmp.path()` as the repo root: Task 6's `export`
    // only writes a row into its own shard when the row's artifact resolves
    // under that same root (component-boundary `starts_with`, not a global
    // free-for-all). A `seed` that planted rows outside `root` would make
    // every existing single-repo test fail closed (0 exported) under the
    // new attribution step rather than testing what it says it tests.
    fn seed(cat: &Catalog, root: &std::path::Path, id: &str) {
        let row = artifact::TestArtifactRowBuilder::new(id)
            .with_abs_path(root.join(format!("{id}.md")))
            .with_status("draft")
            .build();
        artifact::upsert(cat, &row).unwrap();
    }

    fn lines(dir: &std::path::Path) -> Vec<ShardLine> {
        let mut out = Vec::new();
        // export() now only creates the audit directory when it actually has
        // something to write (small fix 10, task-2 review) — a no-op export,
        // or a test that never calls export at all, leaves it absent. That is
        // a valid "nothing exported yet" state, not an error, so tolerate it
        // here rather than forcing export to create the directory unconditionally.
        let entries = match std::fs::read_dir(super::super::host::audit_dir(dir)) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
            Err(e) => panic!("reading audit dir: {e}"),
        };
        for e in entries {
            let p = e.unwrap().path();
            for l in std::fs::read_to_string(&p).unwrap().lines() {
                out.push(serde_json::from_str(l).unwrap());
            }
        }
        out
    }

    /// Build an `artifact`/`delete` audit payload the way the catalog's own triggers do
    /// — through a serializer, never by interpolation.
    ///
    /// **A fixture helper that exists because its absence cost 2 CI failures.** Two tests
    /// wrote `format!("'{{\"abs_path\":\"{p}\"}}'")` with `p` straight from
    /// `tempdir().join(..)`. On Unix that is `/tmp/.tmpAbC/gone.md` and the JSON is valid;
    /// on Windows it is `C:\Users\RUNNER~1\...` and `\U`, `\A`, `\T` are not legal JSON
    /// escapes, so `serde_json::from_str` in [`attribute`] fails, the row resolves to
    /// `None`, and export reports `unattributed: 1`. Green on every developer machine,
    /// red on all four Windows lanes.
    ///
    /// Production was never affected: the audit triggers build payloads with SQLite
    /// expressions and `json_object()` over bound parameters (`audit/mod.rs`), so only
    /// hand-written fixtures could reach this.
    ///
    /// See `docs/issues/2026-09-02-a-test-fixture-interpolates-a-path-into-json.md`.
    fn delete_payload(abs_path: &str) -> String {
        serde_json::json!({ "abs_path": abs_path }).to_string()
    }

    /// A path containing backslashes must survive the fixture round-trip.
    ///
    /// **This is the guard the sibling Windows bug could not have.** `LockFileEx`'s
    /// access-mode defect is unobservable from Linux because `flock(2)` ignores access
    /// mode — nothing local can express it. This one is different in kind: the trigger is
    /// the *content* of the path, not the platform, so a Windows-shaped string exercises
    /// it fully on any OS. Reverting [`delete_payload`] to string interpolation fails this
    /// test on Linux, which is precisely what the two original fixtures could not do.
    ///
    /// Mutation caught: `format!("{{\"abs_path\":\"{abs_path}\"}}")` in place of the
    /// serializer.
    #[test]
    fn a_backslash_path_survives_the_delete_payload_round_trip() {
        let windows_shaped = r"C:\Users\RUNNER~1\AppData\Local\Temp\.tmpAbC\gone.md";
        let payload = delete_payload(windows_shaped);

        let parsed: serde_json::Value = serde_json::from_str(&payload).unwrap_or_else(|e| {
            panic!(
                "fixture payload is not valid JSON ({e}): {payload} — a path was \
                    interpolated instead of serialized"
            )
        });
        assert_eq!(
            parsed.get("abs_path").and_then(|v| v.as_str()),
            Some(windows_shaped),
            "the path must round-trip byte-for-byte, backslashes included"
        );

        // And end-to-end through the real resolver, which is where the failure surfaced:
        // `attribute` parses this payload and must recover the path rather than `None`.
        let conn = Catalog::open_in_memory().unwrap().conn;
        assert_eq!(
            attribute(&conn, "artifact", "delete", "gone", Some(&payload)),
            Some(PathBuf::from(windows_shaped)),
            "attribute() must resolve a delete row whose payload holds a backslash path"
        );
    }

    #[test]
    fn export_writes_rows_past_the_watermark_and_advances_it() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        let host_id = host::resolve_host_id(&cat.conn).unwrap();
        // Pin the full line shape against the actual source row, not just its
        // presence — a mutation probe found `at_ms`/`actor`/`verb`/`payload`
        // all survived being zeroed at the struct literal because nothing read
        // them back (Important 1, task-2 review).
        //
        // The insert row alone cannot cover `verb`/`payload`: per
        // audit/mod.rs's own insert_update_delete_on_artifact_each_leave_an_audit_row,
        // an insert carries no payload, and `verb` stays `None` unless something
        // stamps it first — so both would be `None == None` under a broken
        // implementation too, and the assertion could not see a real value
        // collapsing to `None` (re-review, task-2 round 2). Stamp a verb and
        // perform a column-changing UPDATE (not a pure-churn one) to get a row
        // with both fields genuinely populated, and assert against THAT row.
        cat.set_audit_verb("artifact.update").unwrap();
        cat.conn
            .execute("UPDATE artifact SET status='archived' WHERE id='a1'", [])
            .unwrap();
        let (src_at_ms, src_actor, src_verb, src_payload): (
            i64,
            String,
            Option<String>,
            Option<String>,
        ) = cat
            .conn
            .query_row(
                "SELECT at_ms, actor, verb, payload FROM catalog_audit
             WHERE tbl='artifact' AND row_id='a1' AND op='update'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert!(
            src_verb.is_some(),
            "fixture must stamp a real verb, not None"
        );
        assert!(
            src_payload.is_some(),
            "fixture's UPDATE must change a real column, not be pure churn"
        );
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert!(r.exported >= 1, "{r:?}");
        assert!(r.through_seq > 0);
        let got = lines(tmp.path());
        let line = got
            .iter()
            .find(|l| l.row_id == "a1" && l.tbl == "artifact" && l.op == "update")
            .expect("the seeded artifact's update row must be exported");
        assert_eq!(
            line.host, host_id,
            "the line's host must match the exporting host's own id, not just be non-empty"
        );
        assert_eq!(line.at_ms, src_at_ms);
        assert_eq!(line.actor, src_actor);
        assert_eq!(
            line.verb, src_verb,
            "verb must survive to the exported line"
        );
        assert_eq!(
            line.payload,
            src_payload
                .as_deref()
                .and_then(|p| serde_json::from_str(p).ok()),
            "payload must survive to the exported line"
        );
        assert!(
            got.iter().all(|l| !l.host.is_empty()),
            "every line names its host"
        );
    }

    #[test]
    fn a_second_export_with_no_new_rows_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        export(&cat.conn, tmp.path()).unwrap();
        let before = lines(tmp.path()).len();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.exported, 0, "the watermark must hold");
        assert_eq!(
            lines(tmp.path()).len(),
            before,
            "and nothing may be appended"
        );
    }

    #[test]
    fn commits_rows_are_never_exported() {
        // Git already records commits; auditing them INTO git is circular.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        cat.conn
            .execute(
                "INSERT INTO commits(hash, git_root, authored_at, subject)
                 VALUES('deadbeef','/r',1,'s')",
                [],
            )
            .unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.skipped_commits, 1, "{r:?}");
        assert!(lines(tmp.path()).iter().all(|l| l.tbl != "commits"));
        // The watermark must advance past a skip-only row too, or it sits
        // behind the row forever and doctor reports a permanent phantom
        // backlog (Important 2, task-2 review) — moving the `through_seq`
        // update below the `continue`s left every other assertion in this
        // suite green.
        assert_eq!(unexported_count(&cat.conn, tmp.path()).unwrap(), 0);
    }

    #[test]
    fn reindex_churn_updates_are_never_exported() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        export(&cat.conn, tmp.path()).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET file_mtime=99, file_sha256='z', updated_at=99 WHERE id='a1'",
                [],
            )
            .unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.skipped_churn, 1, "{r:?}");
        assert_eq!(r.exported, 0);
        // Same phantom-backlog guard as commits_rows_are_never_exported: a
        // churn-only row must not sit past the watermark forever.
        assert_eq!(unexported_count(&cat.conn, tmp.path()).unwrap(), 0);
    }

    #[test]
    fn a_semantic_update_that_also_touches_mtime_is_still_exported() {
        // Pair of the above: the churn filter is a SUBSET test, not an
        // intersection test. An update carrying `status` alongside the mtime
        // trio is real history, and dropping it would lose exactly the rows the
        // trail exists for.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        export(&cat.conn, tmp.path()).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET status='active', file_mtime=99, updated_at=99 WHERE id='a1'",
                [],
            )
            .unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.exported, 1, "{r:?}");
        assert_eq!(r.skipped_churn, 0);
    }

    #[test]
    fn rows_from_different_months_land_in_different_files() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        // Delete rows route from the payload's captured OLD-row image, not a
        // live join (Task 2/6) — supply one that resolves under `tmp.path()`
        // or these rows land as `unattributed` instead of exported, and the
        // test would stop expressing what it says it tests.
        let old_path = tmp.path().join("old.md").to_string_lossy().to_string();
        let new_path = tmp.path().join("new.md").to_string_lossy().to_string();
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor,payload)
                 VALUES(1751328000000,'artifact','delete','old','unknown',?1),
                       (1788220800000,'artifact','delete','new','unknown',?2)",
                rusqlite::params![delete_payload(&old_path), delete_payload(&new_path)],
            )
            .unwrap();
        export(&cat.conn, tmp.path()).unwrap();
        let dir = super::super::host::audit_dir(tmp.path());
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names.len(), 2, "one file per month, got {names:?}");
        assert!(names[0].ends_with("-202507.jsonl"), "{names:?}");
        assert!(names[1].ends_with("-202609.jsonl"), "{names:?}");
    }

    #[test]
    fn a_crash_between_append_and_watermark_duplicates_rather_than_loses() {
        // The ordering is load-bearing and this test is what pins it. Append
        // first, advance the cursors second: a crash in between re-exports
        // rows already on disk, and readers dedupe on (host, seq). The inverse
        // order would LOSE rows with no signal anywhere.
        //
        // Two artifacts are seeded, not one: with a single row, `n == 1` and
        // `seqs.len() == n` is `1 == 1` for ANY value of `seq` — replacing
        // `seq` with a constant at the struct literal leaves this green
        // (Important 1, task-2 review). With two distinct seqs the set only
        // has cardinality `n` if both are preserved and distinct.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        seed(&cat, tmp.path(), "a2");
        export(&cat.conn, tmp.path()).unwrap();
        let n = lines(tmp.path()).len();
        assert!(n >= 2, "expected at least the two seeded rows, got {n}");
        // Simulate the crash: the file is written, NEITHER cursor advanced.
        // Task-6 round-3 (Ruling 16) split the single watermark into two —
        // the recoverability cursor and the write-dedup cursor — but both are
        // still persisted together at the tail of the same `export` call, so
        // a crash between the file append and that persist leaves both stale.
        // Resetting only the old watermark would leave the NEW write cursor
        // correctly advanced from the first call, and the write-dedup guard
        // would then (correctly, by its own design) suppress the resulting
        // re-export — silently defeating what this test means to exercise.
        gc::set_meta(&cat.conn, &watermark_key(tmp.path()), "0").unwrap();
        gc::set_meta(&cat.conn, &written_key(tmp.path()), "0").unwrap();
        export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(
            lines(tmp.path()).len(),
            n * 2,
            "re-export duplicates, by design"
        );
        let seqs: std::collections::HashSet<i64> =
            lines(tmp.path()).iter().map(|l| l.seq).collect();
        assert_eq!(seqs.len(), n, "and every duplicate shares its (host, seq)");
    }

    /// The test above proves the SYSTEM tolerates a stale watermark (it
    /// re-exports and duplicates rather than losing rows) but it calls
    /// `export` to completion twice — it never observes a crash occurring
    /// *between* the two steps `export` performs, so it cannot actually
    /// tell the two orders apart: inverting the order inside `export`
    /// still leaves this test green, because both calls always finish
    /// both steps. Confirmed empirically before writing this test: with
    /// the watermark-set line moved to run before the append loop, `cargo
    /// test a_crash_between_append_and_watermark_duplicates_rather_than_loses`
    /// still reported `1 passed`.
    ///
    /// This test instead forces the append itself to fail (the shard
    /// directory is made unwritable) and checks the ordering claim
    /// directly: the watermark must be exactly what it was before the
    /// call, because `export` returns its error via `?` from inside the
    /// append loop, which runs strictly before the `gc::set_meta` line.
    /// Were the order inverted, this call would advance the watermark
    /// and THEN fail to append — silently losing the row the watermark
    /// now claims was exported, which is exactly the failure mode the
    /// ordering exists to prevent.
    #[cfg(unix)]
    #[test]
    fn a_failed_append_never_advances_the_watermark() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        let audit_dir = host::audit_dir(tmp.path());
        std::fs::create_dir_all(&audit_dir).unwrap();
        std::fs::set_permissions(&audit_dir, std::fs::Permissions::from_mode(0o500)).unwrap();

        // Root (and some filesystems) ignore the write bit — probe rather
        // than assume, and degrade to a no-op if the precondition never
        // triggers (same pattern as
        // rendezvous::tests::publish_degrades_to_none_on_filesystem_failure).
        // This is the SOLE guard of a load-bearing ordering invariant, so the
        // skip must be loud: silent degradation here would read identically
        // to a real pass on a CI runner that happens to run as root (small
        // fix 5, task-2 review).
        let probe_ok = std::fs::File::create(audit_dir.join("probe")).is_ok();
        if probe_ok {
            eprintln!(
                "a_failed_append_never_advances_the_watermark: SKIPPED — this \
                 environment's filesystem does not enforce 0o500 (likely running \
                 as root), so the ordering invariant was NOT exercised by this run"
            );
            let _ = std::fs::remove_file(audit_dir.join("probe"));
            let _ = std::fs::set_permissions(&audit_dir, std::fs::Permissions::from_mode(0o700));
            return;
        }

        let before = gc::get_meta(&cat.conn, &watermark_key(tmp.path())).unwrap();
        let result = export(&cat.conn, tmp.path());
        let _ = std::fs::set_permissions(&audit_dir, std::fs::Permissions::from_mode(0o700));

        let err =
            result.expect_err("append into an unwritable directory must fail, not silently no-op");
        // Pin WHICH failure this is, not just that some error occurred — an
        // earlier failure (e.g. at create_dir_all) would also make `result`
        // an `Err` without ever reaching the append step this test targets,
        // which would pass without exercising the ordering claim at all
        // (small fix 6, task-2 review).
        let chain = format!("{err:#}");
        assert!(
            chain.contains("opening "),
            "expected the failure to originate at export's file-open step \
             (context \"opening <path>\"), got: {chain}"
        );
        let after = gc::get_meta(&cat.conn, &watermark_key(tmp.path())).unwrap();
        assert_eq!(
            before, after,
            "a failed append must never advance the watermark — advancing it \
                 here would claim row a1 was exported when it never touched disk"
        );
    }

    // Parity claim is conditional (task-6 final review, Minor): it holds only
    // when this call resolves no PRE-EXISTING gap. `unexported_count` floors
    // its scan on `written_through` (`WHERE seq > written_through`), so a
    // resolving gap — seq <= written_through, present in `gaps_start` — is
    // invisible to it; `export`'s loop writes that row anyway via the
    // `!gaps_start.contains(&seq)` exception. `seed` below opens no gap, so
    // the two counts agree here; with one present, `unexported_count` would
    // under-report by the number of resolving gaps (doctor is reporting-only,
    // so this does not lose data — `unexported_count` reaching 0 afterward
    // still holds — but the parity this test asserts is not unconditional).
    #[test]
    fn unexported_count_matches_what_the_next_export_would_write() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        let pending = unexported_count(&cat.conn, tmp.path()).unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(
            pending as usize,
            r.exported + r.skipped_commits + r.skipped_churn,
            "doctor's delta must describe the same population export consumes"
        );
        assert_eq!(unexported_count(&cat.conn, tmp.path()).unwrap(), 0);
    }

    // Ruling 18 (task-6 round-3 review): unit coverage for the parser itself,
    // independent of the reindex-level gate tests in reindex.rs — covers the
    // tolerance rules the doc comment promises (extra attributes on the same
    // line, comment lines, a missing file) so a future edit to the matching
    // logic cannot silently narrow or widen them without a test noticing.
    #[test]
    fn gitattributes_union_declaration_matches_are_tolerant_but_specific() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            !gitattributes_declares_shard_union(tmp.path()),
            "a missing .gitattributes must read as not-opted-in, not an error"
        );

        std::fs::write(
            tmp.path().join(".gitattributes"),
            "# comment mentioning .codescout/audit/*.jsonl merge=union should not count\n\
             *.png binary\n",
        )
        .unwrap();
        assert!(
            !gitattributes_declares_shard_union(tmp.path()),
            "a commented-out line must not count"
        );

        std::fs::write(
            tmp.path().join(".gitattributes"),
            ".codescout/audit/*.jsonl merge=union -diff\n",
        )
        .unwrap();
        assert!(
            gitattributes_declares_shard_union(tmp.path()),
            "extra attributes on the same line must not defeat the match"
        );

        std::fs::write(
            tmp.path().join(".gitattributes"),
            ".codescout/audit/*.jsonl text\n",
        )
        .unwrap();
        assert!(
            !gitattributes_declares_shard_union(tmp.path()),
            "the right path without merge=union must not count"
        );
    }

    // Task 6, required test 1/6: export scoped to repo A must not leak repo
    // B's rows into A's shard. Mutation this catches: dropping the
    // `owner.starts_with(repo_root)` gate in `export` (or replacing it with
    // an always-true check) would make B's row attribute successfully and
    // then export into A's shard anyway — this test would see `b1` show up
    // in `lines(tmp_a.path())` and fail.
    #[test]
    fn export_scoped_to_one_repo_excludes_another_repos_rows() {
        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp_a.path(), "a1");
        seed(&cat, tmp_b.path(), "b1");

        let r = export(&cat.conn, tmp_a.path()).unwrap();
        assert_eq!(r.exported, 1, "{r:?}");

        let got_a = lines(tmp_a.path());
        assert!(
            got_a.iter().any(|l| l.row_id == "a1"),
            "repo A's own row must be exported into its shard"
        );
        assert!(
            !got_a.iter().any(|l| l.row_id == "b1"),
            "repo B's row must never land in repo A's shard"
        );
        // B's shard was never touched by A's export at all.
        assert!(lines(tmp_b.path()).is_empty());
    }

    // Task 6, required test 2/6: this is the headline defect the whole task
    // exists to fix. Mutation this catches: reverting `watermark`/`export`'s
    // per-repo key back to the single global `WATERMARK_KEY` — exporting A
    // first would then advance the ONE shared cursor past B's row's `seq`,
    // and B's own export would see nothing past its (shared) watermark and
    // export zero rows instead of one.
    #[test]
    fn watermarks_are_independent_per_repo() {
        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp_a.path(), "a1");
        seed(&cat, tmp_b.path(), "b1");

        let r_a = export(&cat.conn, tmp_a.path()).unwrap();
        assert_eq!(r_a.exported, 1, "{r_a:?}");

        let r_b = export(&cat.conn, tmp_b.path()).unwrap();
        assert_eq!(
            r_b.exported, 1,
            "B's export must not be starved by A's watermark having already \
             advanced past B's row: {r_b:?}"
        );
        assert!(
            lines(tmp_b.path()).iter().any(|l| l.row_id == "b1"),
            "b1 must actually be written to B's own shard"
        );
    }

    // Task 6, required test 3/6: a `delete` row's owning repo must be read
    // from the payload's captured OLD-row image, never from a live join —
    // the artifact row is already gone by the time export runs. Mutation
    // this catches: routing `artifact`/`delete` through `by_artifact_id`
    // (a live join) instead of `payload_str(payload, "abs_path")` — the join
    // would find nothing (no such artifact exists), `attribute` would return
    // `None`, and the row would come out `unattributed` instead of exported.
    #[test]
    fn delete_row_is_attributed_from_its_payload_not_a_live_join() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let gone_path = tmp.path().join("gone.md").to_string_lossy().to_string();
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor,payload)
                 VALUES(1751328000000,'artifact','delete','gone','unknown',?1)",
                rusqlite::params![delete_payload(&gone_path)],
            )
            .unwrap();

        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(
            r.unattributed, 0,
            "a delete row with a usable payload must not fall through to unattributed: {r:?}"
        );
        assert_eq!(r.exported, 1, "{r:?}");
        assert!(lines(tmp.path()).iter().any(|l| l.row_id == "gone"));
    }

    // Important 3 (task-6 review): only `artifact` insert/delete had coverage
    // (2 of 7 attribution routes in `attribute`'s match). The five below cover
    // the rest, using the REAL audited tables + their install()-installed
    // triggers (not hand-built `catalog_audit` rows) so each test exercises
    // the actual trigger-produced `row_id` shape, not a guess at it.

    #[test]
    fn artifact_augmentation_insert_is_attributed_via_its_artifact_id() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        cat.conn
            .execute(
                "INSERT INTO artifact_augmentation(artifact_id,prompt) VALUES('a1','p')",
                [],
            )
            .unwrap();

        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.unattributed, 0, "{r:?}");
        assert!(
            lines(tmp.path())
                .iter()
                .any(|l| l.tbl == "artifact_augmentation" && l.row_id == "a1"),
            "{:?}",
            lines(tmp.path())
        );
    }

    #[test]
    fn events_insert_is_attributed_via_its_artifact_id() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        crate::librarian::catalog::events::insert(
            &cat,
            &crate::librarian::catalog::events::TestEventRowBuilder::new("a1", "note")
                .with_id("e1")
                .build(),
        )
        .unwrap();

        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.unattributed, 0, "{r:?}");
        assert!(
            lines(tmp.path())
                .iter()
                .any(|l| l.tbl == "events" && l.row_id == "e1"),
            "{:?}",
            lines(tmp.path())
        );
    }

    #[test]
    fn artifact_link_insert_is_attributed_via_its_src_id() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "src");
        seed(&cat, tmp.path(), "dst");
        cat.conn
            .execute(
                "INSERT INTO artifact_link(src_id,dst_id,rel,created_at) VALUES('src','dst','cites',1)",
                [],
            )
            .unwrap();

        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.unattributed, 0, "{r:?}");
        assert!(
            lines(tmp.path())
                .iter()
                .any(|l| l.tbl == "artifact_link" && l.row_id == "src→dst:cites"),
            "{:?}",
            lines(tmp.path())
        );
    }

    #[test]
    fn entry_cite_insert_is_attributed_via_its_src_slug() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, tmp.path(), "a1");
        cat.conn
            .execute("UPDATE artifact SET slug='a-one' WHERE id='a1'", [])
            .unwrap();
        cat.conn
            .execute(
                "INSERT INTO entry_cite(src_slug,src_local,dst_ref,rel,created_at)
                 VALUES('a-one','F-1','x','cites',1)",
                [],
            )
            .unwrap();

        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.unattributed, 0, "{r:?}");
        assert!(
            lines(tmp.path())
                .iter()
                .any(|l| l.tbl == "entry_cite" && l.row_id == "a-one:F-1→x"),
            "{:?}",
            lines(tmp.path())
        );
    }

    // Important 3 (task-6 review), NULL-slug case: `artifact.slug` is NULLable
    // (schema.sql) and only backfilled lazily by `librarian doctor
    // fix=mint_slugs`. `entry_cite.src_slug` is itself NOT NULL and
    // FK-enforced against `artifact.slug` (schema.sql / catalog/mod.rs), so a
    // real `entry_cite` row can only ever be written once its citing
    // artifact already has a non-NULL slug — there is no way to construct an
    // FK-valid row that reproduces the gap. What actually happens instead:
    // the target artifact's slug is NULL *at export time*, and the audited
    // row (however it got there — e.g. hand-repaired data, or a catalog
    // opened with `foreign_keys=OFF` at some point in its history) captures a
    // `src_slug` that currently matches no row. `attribute`'s live join
    // (`SELECT abs_path FROM artifact WHERE slug = ?1`) then finds nothing —
    // same observable outcome as the FK-blocked case, reached via a raw
    // `catalog_audit` row rather than a real `entry_cite` insert, mirroring
    // `delete_row_is_attributed_from_its_payload_not_a_live_join`'s approach
    // for the `artifact`/delete arm above.
    #[test]
    fn entry_cite_row_referencing_a_still_null_slug_is_unattributed() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        // "a1" exists but has never had mint_slugs run — slug stays NULL.
        seed(&cat, tmp.path(), "a1");
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor)
                 VALUES(1751328000000,'entry_cite','insert','a-one:F-1→x','unknown')",
                [],
            )
            .unwrap();

        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(
            r.unattributed, 1,
            "no artifact has slug='a-one' (it is still NULL), so the live join \
             must fail and the row must be counted unattributed, not silently \
             dropped or wrongly attributed to some other artifact: {r:?}"
        );
        // 1, not 0: `seed`'s own artifact-insert audit row (attributable via
        // a live join on `artifact.id`) is exported normally — only the
        // entry_cite row referencing the still-NULL slug is unattributed.
        assert_eq!(r.exported, 1, "{r:?}");
    }

    #[test]
    fn worktree_registration_insert_is_attributed_via_its_own_root() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let root = tmp.path().to_string_lossy().to_string();
        cat.conn
            .execute(
                &format!(
                    "INSERT INTO worktree_registration(worktree_root,main_root,created_at)
                     VALUES('{root}','{root}',1)"
                ),
                [],
            )
            .unwrap();

        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.unattributed, 0, "{r:?}");
        assert!(
            lines(tmp.path())
                .iter()
                .any(|l| l.tbl == "worktree_registration" && l.row_id == root),
            "{:?}",
            lines(tmp.path())
        );
    }

    // Task 6, required test 4/6: a row that cannot be traced to any repo is
    // counted but neither exported nor allowed to advance the watermark past
    // its own `seq` — advancing would make it permanently unrecoverable once
    // the row later becomes attributable (e.g. a delayed insert resolves the
    // artifact it references). Mutation this catches: advancing
    // `report.through_seq` in the `None` branch of `export`'s attribution
    // check (matching the commits/churn branches above it) — the second
    // `export` call below would then never re-scan the row's `seq`, and
    // `ghost` would stay unexported forever even after the artifact exists.
    //
    // Critical 2 (task-6 review): the ORIGINAL version of this test used a
    // single raw audit row, so `through_seq` trivially stayed at 0 regardless
    // of whether the watermark-advance logic tracked a minimum unattributed
    // `seq` or just skipped the `.max()` call at the unattributed row's own
    // iteration — the running-max defect (Critical 1: a LATER, resolvable row
    // in the SAME batch re-advances `through_seq` past an earlier unattributed
    // one via `.max()`) was never exercised. This version adds `other` — a
    // second, HIGHER-seq, attributable row in the SAME first `export` call —
    // so the running-max path is actually entered, and asserts the watermark
    // still stays clamped below `ghost`'s seq even though `other` resolved.
    #[test]
    fn unattributed_row_is_counted_not_exported_and_stays_recoverable() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        // No artifact named "ghost" exists yet, and this is not a delete (no
        // payload to fall back on) — `attribute` must return `None`. This is
        // the FIRST row ever inserted into `catalog_audit` in this test, so it
        // is seq 1.
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor)
                   VALUES(1751328000000,'artifact','update','ghost','unknown')",
                [],
            )
            .unwrap();
        // A second, ATTRIBUTABLE row at a HIGHER seq (2, via `seed`'s own
        // artifact-insert audit trigger) in the SAME batch the first export
        // below will scan. This is the fixture Critical 1 needs: without it,
        // `through_seq` never leaves 0 regardless of whether the running-max
        // defect is present, and the test cannot tell the fixed code from the
        // broken code.
        seed(&cat, tmp.path(), "other");

        let r1 = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r1.unattributed, 1, "{r1:?}");
        // "other" (seq 2) is attributable and under this repo root — it must
        // still be exported. The bug this test guards against is the
        // WATERMARK wrongly advancing past "ghost" because "other" resolved,
        // not "other" being wrongly withheld.
        assert_eq!(r1.exported, 1, "{r1:?}");
        assert_eq!(
            watermark(&cat.conn, tmp.path()).unwrap(),
            0,
            "the watermark must stay clamped below the unattributed row's seq \
               (1) even though a LATER row in the same batch (seq 2, \"other\") \
               resolved fine and would otherwise drag `through_seq` past it via \
               the loop's running `.max()` — this is exactly what Critical 1 \
               (task-6 review) named: an unattributed row survives its OWN \
               iteration but is stranded by a later row's advance unless \
               `through_seq` is clamped post-loop against the minimum \
               unattributed seq. r1={r1:?}"
        );
        assert!(
            lines(tmp.path()).iter().all(|l| l.row_id != "ghost"),
            "ghost must not appear in the shard yet: {:?}",
            lines(tmp.path())
        );

        // The artifact now exists under this same repo root — the row becomes
        // attributable. Because the watermark never advanced past it, a second
        // export must still see and export it.
        seed(&cat, tmp.path(), "ghost");
        let r2 = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r2.unattributed, 0, "{r2:?}");
        // 2, not 3 (task-6 round-3, Ruling 16): the second export still
        // RE-SCANS from watermark 0 — that part is unchanged, and is what
        // makes "ghost" (seq 1) recoverable at all — but "other"'s insert row
        // (seq 2) was already written to disk by the FIRST export call, and
        // the write-dedup cursor (`written_through`, separate from the
        // recoverability watermark) remembers that. Only rows past it are
        // actually written: the originally-stuck "ghost" `update` row (seq 1)
        // and `seed`'s own fresh "ghost" `insert` row (seq 3). Re-exporting
        // "other" a second time here is exactly the unbounded-regrowth defect
        // Ruling 16 fixed — the live-catalog scenario that motivated the fix
        // had a permanently-unattributable population (min unattributable seq
        // = 1) re-dragging every already-written row above it back onto disk
        // on every single export/reindex. A precise headcount is deliberately
        // not cited here: an earlier draft of this comment cited 87, and an
        // independent re-derivation (replicating `attribute`'s seven routes
        // directly in SQL against the live catalog) got 72 with the churn
        // filter applied and 102 without it, matching neither 87 nor each
        // other (task-6 final review) — and the count grows over time
        // regardless, so any fixed number here would rot the same way. If a
        // fresh figure is wanted, re-run that SQL and state which counting
        // rule (churn filtered or not) produced it.
        assert_eq!(
            r2.exported, 2,
            "the originally-unattributed row must still be recoverable, but a \
               row already written to disk by the first call must not be \
               re-appended: {r2:?}"
        );
        assert_eq!(
            lines(tmp.path())
                .iter()
                .filter(|l| l.row_id == "ghost")
                .count(),
            2,
            "both the original stuck update row and seed's own insert row must be exported"
        );
        assert_eq!(
            lines(tmp.path())
                .iter()
                .filter(|l| l.row_id == "other")
                .count(),
            1,
            "\"other\" must appear exactly once across both exports — the \
               write-dedup cursor is what stops the second export from \
               re-appending it just because the recoverability watermark had to \
               stay behind for \"ghost\"'s sake"
        );
        assert_eq!(
            watermark(&cat.conn, tmp.path()).unwrap(),
            3,
            "with no unattributed rows left in this batch, the watermark must \
               advance all the way to the highest seq scanned"
        );
        assert_eq!(
            written_through(&cat.conn, tmp.path()).unwrap(),
            3,
            "the write cursor also reaches the highest seq actually written, \
               and the two cursors converge once nothing is left stuck"
        );
    }

    // Critical (task-6 round-3 review): a row that stays PERMANENTLY
    // unattributable (the live catalog has at least one of these — e.g. seq 1
    // is an `artifact|update` row whose artifact no longer exists — a total
    // count is deliberately not cited: two independent re-derivations already
    // disagreed, 87 vs. 72 vs. 102 depending on the churn-filter rule, and
    // the population grows, so any fixed number here rots the same way; see
    // the comment above `unattributed_row_is_counted_not_exported_and_stays_
    // recoverable`'s `r2.exported` assertion for how to re-derive it) must
    // not force
    // every already-written row above it to be re-appended on every single
    // future export/reindex call. This is the regression the two-cursor split
    // (Ruling 16) plus the `GAPS_KEY` gap-set fix this round exist to close.
    //
    // This test intentionally FAILS against the round-2 code (a single
    // watermark, clamped and never separated from the write cursor): with
    // only `watermark`, the recoverability cursor stays pinned at 0 forever
    // because "ghost" never resolves, so a naive re-implementation using
    // `watermark` as the write-dedup floor re-scans "other" from seq 0 on
    // EVERY call and re-appends it every time — this test's second and third
    // `export()` calls would each add another copy of "other" to the file
    // instead of leaving it at exactly one.
    #[test]
    fn a_repeat_export_with_the_row_still_unattributed_does_not_regrow_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        // Permanently unattributable: no artifact named "ghost" ever gets
        // created in this test, modeling the live catalog's seq-1 row.
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor)
                   VALUES(1751328000000,'artifact','update','ghost','unknown')",
                [],
            )
            .unwrap();
        seed(&cat, tmp.path(), "other");

        let r1 = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r1.exported, 1, "{r1:?}");
        assert_eq!(r1.unattributed, 1, "{r1:?}");
        assert_eq!(
            watermark(&cat.conn, tmp.path()).unwrap(),
            0,
            "stuck below ghost's seq, same as the sibling test above"
        );

        // Call export again with NOTHING new — "ghost" is still unattributed,
        // so the recoverability watermark cannot move. The write cursor must
        // be what stops "other" from being re-appended.
        let r2 = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(
            r2.exported, 0,
            "a row already on disk must not be re-exported just because the \
             recoverability watermark is stuck behind a permanently-unattributable \
             row: {r2:?}"
        );
        assert_eq!(r2.unattributed, 1, "{r2:?}");

        // A third call, for good measure — the file must not grow AT ALL
        // across repeated no-progress retries of the same stuck row.
        let r3 = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r3.exported, 0, "{r3:?}");
        assert_eq!(
            lines(tmp.path())
                .iter()
                .filter(|l| l.row_id == "other")
                .count(),
            1,
            "\"other\" must appear exactly once no matter how many times export \
             is retried while \"ghost\" stays stuck"
        );
        assert!(
            lines(tmp.path()).iter().all(|l| l.row_id != "ghost"),
            "ghost itself must never appear — it never resolved in this test"
        );
    }

    // Closes the loss path named in the task-6 final review (Important): the
    // three cursor writes at the end of `export` (gaps, watermark,
    // written_through) used to be three independent bare `gc::set_meta`
    // calls. If a crash or SQLITE_BUSY landed between them — unremarkable on
    // a machine-wide shared catalog — `written_through` could commit while
    // `gaps` did not, and the still-open gap would be silently dropped for
    // good on the next call (see `open_gaps`'s doc comment). This test proves
    // the fix by forcing a failure BETWEEN two of the three writes and
    // asserting the three cursors are either ALL updated or NONE are; a
    // partial update (the pre-fix defect) is the one outcome the transaction
    // makes unreachable.
    //
    // Mutation this catches: reverting the three `gc::set_meta(&tx, ...)`
    // calls to bare `gc::set_meta(conn, ...)` (dropping the transaction) lets
    // the forced failure below land AFTER `gaps` has already durably
    // committed on its own, so the post-call gap set would gain the new gap
    // even though the whole call reported an error — this test's `gaps`
    // equality assertion is what catches exactly that (verified by
    // temporarily applying that mutation locally: this test fails with the
    // mutation in place and passes without it).
    #[test]
    fn a_failure_during_cursor_persistence_strands_no_partial_state() {
        // A trigger, not a competing connection: a lock held by a second
        // connection would block export's FIRST write before anything runs,
        // which cannot distinguish this fix from the pre-fix bug (three bare,
        // individually-autocommitted `gc::set_meta` calls) — both fail
        // identically before any of the three commits. What discriminates
        // them is a failure that lands AFTER the first write and BEFORE the
        // last, mirroring `apply_rehome_rolls_back_atomically_on_mid_batch_
        // failure`'s "force a real failure mid-batch, assert full rollback"
        // shape: a `BEFORE UPDATE` trigger rejects only the `written_through`
        // key (persisted last, after gaps and watermark, in the fixed
        // order), so `gaps` and `watermark` have already executed within the
        // same transaction when the failure hits. Pre-fix, `gaps` (bare
        // `conn.execute`, autocommit) would already be durably committed by
        // that point; post-fix, it is only provisional inside `tx` and must
        // roll back with everything else.
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("cat.sqlite");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();

        let cat = Catalog::open(&db_path).unwrap();

        // Seed one unattributable row (opens a gap) and one attributable row,
        // then run one clean export to establish a known-good baseline for
        // all three cursors — including a real value for `written_key`, so
        // the second call below hits the trigger's `BEFORE UPDATE` (not
        // `BEFORE INSERT`, which the trigger does not guard).
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor)
                     VALUES(1751328000000,'artifact','update','ghost','unknown')",
                [],
            )
            .unwrap();
        seed(&cat, &repo, "baseline");
        let r0 = export(&cat.conn, &repo).unwrap();
        assert_eq!(r0.exported, 1, "{r0:?}");
        assert_eq!(r0.unattributed, 1, "{r0:?}");

        let watermark_before = watermark(&cat.conn, &repo).unwrap();
        let written_before = written_through(&cat.conn, &repo).unwrap();
        let gaps_before = open_gaps(&cat.conn, &repo).unwrap();
        assert!(
            !gaps_before.is_empty(),
            "baseline must leave an open gap (ghost) for this test to mean anything: \
             {gaps_before:?}"
        );

        // A SECOND unattributable row, plus an attributable one, so the next
        // export call's gap set actually DIFFERS from `gaps_before` (ghost
        // alone never resolves, so re-scanning it every call would otherwise
        // write back the same value regardless of atomicity — this second
        // ghost is what makes the persisted gap set observably change, and
        // is what the equality assertion below needs to have any teeth).
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor)
                     VALUES(1751328000001,'artifact','update','ghost2','unknown')",
                [],
            )
            .unwrap();
        seed(&cat, &repo, "second");

        // Reject the LAST write in the fixed order (gaps, then watermark,
        // then written_through) so the first two have already executed —
        // committed pre-fix, merely provisional post-fix — when the failure
        // hits.
        let written_key_literal = written_key(&repo).replace('\'', "''");
        cat.conn
            .execute_batch(&format!(
                "CREATE TRIGGER reject_written_write BEFORE UPDATE ON catalog_meta
                 WHEN NEW.key = '{written_key_literal}'
                 BEGIN SELECT RAISE(ABORT, 'forced failure for atomicity test'); END;"
            ))
            .unwrap();

        let result = export(&cat.conn, &repo);
        assert!(
            result.is_err(),
            "export must surface the trigger's forced failure, not silently swallow it: \
             {result:?}"
        );

        cat.conn
            .execute_batch("DROP TRIGGER reject_written_write;")
            .unwrap();

        // All three cursors must be EXACTLY as they were before the failed
        // call — not one, not two, all three. Pre-fix, `gaps` (persisted
        // first, autocommitted individually) would have already stuck at its
        // NEW value (ghost + ghost2) even though `written_through` then
        // failed — reproducing the exact loss path the final review named,
        // just inverted: here the newly-opened `ghost2` gap would have been
        // recorded despite the call failing, which is the wrong direction of
        // the same bug (a partial, inconsistent write winning over "nothing
        // happened").
        assert_eq!(
            watermark(&cat.conn, &repo).unwrap(),
            watermark_before,
            "watermark must not move on a call whose persistence step failed"
        );
        assert_eq!(
            written_through(&cat.conn, &repo).unwrap(),
            written_before,
            "written_through must not move on a call whose persistence step failed"
        );
        assert_eq!(
            open_gaps(&cat.conn, &repo).unwrap(),
            gaps_before,
            "the gap set is the unrecoverable one — a call that failed must leave it \
             untouched, not partially advanced to include a gap opened during that \
             same failed call"
        );

        // And once the trigger is gone, a clean retry must succeed and make
        // real progress — the failure above was transient, not a permanent
        // wedge.
        let r1 = export(&cat.conn, &repo).unwrap();
        assert_eq!(r1.exported, 1, "{r1:?}");
        assert!(
            lines(&repo).iter().any(|l| l.row_id == "second"),
            "a clean retry after the trigger is removed must actually write the row \
             that the failed call could not: {:?}",
            lines(&repo)
        );
        assert_eq!(
            open_gaps(&cat.conn, &repo).unwrap().len(),
            2,
            "the clean retry must now record BOTH ghosts as open gaps — proving the \
             earlier failed call really did not persist ghost2 as a gap, rather than \
             this test passing by coincidence"
        );
    }

    // Task 6, required test 6/6: the repo-ownership check must be
    // component-boundary correct, not a naive string prefix. Mutation this
    // catches: replacing `owner.starts_with(repo_root)` (`std::path::Path`'s
    // component-aware `starts_with`) with a raw string comparison such as
    // `owner.to_string_lossy().starts_with(&repo_root.to_string_lossy())` —
    // `"/tmp/x/repo-backup"` textually starts with `"/tmp/x/repo"`, so the
    // naive form would wrongly treat `repo-backup`'s row as belonging to
    // `repo` and export it there.
    #[test]
    fn sibling_directory_sharing_a_string_prefix_is_not_treated_as_the_same_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let repo_backup = tmp.path().join("repo-backup");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&repo_backup).unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, &repo_backup, "b1");

        let r = export(&cat.conn, &repo).unwrap();
        assert_eq!(
            r.exported, 0,
            "repo-backup's row must not export into repo's shard: {r:?}"
        );
        assert_eq!(
            r.unattributed, 0,
            "the row IS attributable — just to a different repo, not to no repo"
        );
        assert!(lines(&repo).is_empty());
    }

    fn write_shard(root: &std::path::Path, name: &str, lines: &[&str]) {
        let dir = super::super::host::audit_dir(root);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), lines.join("\n")).unwrap();
    }

    fn foreign_line(seq: i64, at_ms: i64, row_id: &str) -> String {
        serde_json::json!({
            "host": "otherbox-99ffee", "seq": seq, "at_ms": at_ms,
            "tbl": "artifact", "op": "delete", "row_id": row_id,
            "actor": "codescout:sess-b", "payload": {"id": row_id}
        })
        .to_string()
    }

    fn foreign_line_for(host: &str, seq: i64, at_ms: i64, row_id: &str) -> String {
        serde_json::json!({
            "host": host, "seq": seq, "at_ms": at_ms,
            "tbl": "artifact", "op": "delete", "row_id": row_id,
            "actor": "codescout:sess-b", "payload": {"id": row_id}
        })
        .to_string()
    }

    #[test]
    fn a_foreign_hosts_rows_are_read_back() {
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line(1, 1_788_220_800_000, "gone-1")],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "gone-1");
        assert_eq!(r.hosts.len(), 1, "coverage names the host");
    }

    #[test]
    fn our_own_hosts_shard_is_not_read_back() {
        // Our rows are already in the local table. Reading our own shard too
        // would double-count them in `filtered_total` — a wrong NUMBER, which
        // nothing downstream can catch.
        let tmp = tempfile::tempdir().unwrap();
        let mut mine: serde_json::Value =
            serde_json::from_str(&foreign_line(1, 1_788_220_800_000, "x")).unwrap();
        mine["host"] = serde_json::json!("me-000000");
        write_shard(tmp.path(), "me-000000-202609.jsonl", &[&mine.to_string()]);
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert!(r.rows.is_empty(), "got {:?}", r.rows);
    }

    #[test]
    fn a_malformed_line_is_counted_not_silently_dropped() {
        // A shard is a git-merged file; a bad line is expected eventually. A
        // silent skip makes a partial answer look complete (IC-13) — the exact
        // class this feature exists to avoid.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(1, 1_788_220_800_000, "ok-1"),
                "{not json",
                "",
                &foreign_line(2, 1_788_220_800_001, "ok-2"),
            ],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 2, "the good lines still arrive");
        assert_eq!(r.malformed, 1, "and the bad one is REPORTED");
        // Pin the sort order too: `at_ms DESC` means the later row (ok-2, at_ms
        // ...001) sorts FIRST. This is the mutation-proof site for an inverted
        // comparator — flipping `b.at_ms.cmp(&a.at_ms)` to `a.at_ms.cmp(&b.at_ms)`
        // swaps rows[0]/rows[1] and fails these two assertions.
        assert_eq!(r.rows[0].row_id, "ok-2", "newer at_ms sorts first");
        assert_eq!(r.rows[1].row_id, "ok-1");
    }

    // Version-escape test (Important, task-6 round-3 review): a line written by
    // an OLDER binary — before some field existed at all — must still parse.
    // Every `ShardLine` field except `host` and `seq` carries
    // `#[serde(default)]` for exactly this reason (task-6 final review: `host`
    // joined `seq` in the exclusion, both being halves of the `(host, seq)`
    // dedup key — see the struct's doc comment); this test pins that a line
    // missing several of them (not just one) still counts as a good row, not
    // `malformed`. Mutation this catches: removing `#[serde(default)]` from
    // any covered field turns this from `rows.len() == 1, malformed == 0`
    // into `rows.len() == 0, malformed == 1` — a real "new field ships, old
    // lines start failing" regression, the exact failure mode the annotation
    // exists to prevent.
    #[test]
    fn a_line_missing_defaulted_fields_still_parses_not_malformed() {
        let tmp = tempfile::tempdir().unwrap();
        // Deliberately omit `actor`, `op`, `tbl`, `at_ms`, `verb`, and `payload`
        // — everything except `host` and `seq`, the two dedup-key fields that
        // are always present on a real line and are never defaulted.
        let line = serde_json::json!({ "host": "otherbox-99ffee", "seq": 1 }).to_string();
        write_shard(tmp.path(), "otherbox-99ffee-202609.jsonl", &[&line]);
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(
            r.malformed, 0,
            "a line missing only #[serde(default)]-covered fields must not be \
             counted malformed: {r:?}"
        );
        assert_eq!(r.rows.len(), 1, "the row must still arrive: {r:?}");
        assert_eq!(r.rows[0].row_id, "", "defaulted to the empty string");
        assert_eq!(r.rows[0].at_ms, 0, "defaulted to 0");
        assert_eq!(r.rows[0].verb, None, "defaulted to None");
    }

    #[test]
    fn duplicate_host_seq_pairs_collapse_to_one_row() {
        // merge=union and crash-re-export both produce duplicates by design.
        // Three copies (not two) so a mutation that dedups on line POSITION
        // rather than (host, seq) — e.g. keeping only the first two lines
        // regardless of content — cannot pass by accident: it must actually
        // collapse on the key to reach the single-row assertion.
        let tmp = tempfile::tempdir().unwrap();
        let l = foreign_line(7, 1_788_220_800_000, "dup");
        write_shard(tmp.path(), "otherbox-99ffee-202609.jsonl", &[&l, &l, &l]);
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1, "deduped on (host, seq)");
        assert_eq!(r.rows[0].row_id, "dup");
    }

    #[test]
    fn two_different_hosts_sharing_the_same_seq_are_both_kept() {
        // Row identity is (host, seq), not seq alone: seq is a per-host
        // autoincrement, so two DIFFERENT hosts legitimately reuse seq=1.
        // A dedup key of `seq` alone would collapse these two distinct rows
        // into one, silently dropping a foreign host's row. This test is
        // the mutation-proof site for that: mutating the dedup key in
        // read_shards from `(line.host.clone(), line.seq)` to `line.seq`
        // makes this fail (2 rows -> 1, hosts.len() 2 -> ... still 2 since
        // `hosts` is updated before the dedup check, so `rows.len()` is the
        // assertion that actually catches it).
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line_for(
                "otherbox-99ffee",
                1,
                1_788_220_800_000,
                "a",
            )],
        );
        write_shard(
            tmp.path(),
            "thirdbox-aa11bb-202609.jsonl",
            &[&foreign_line_for(
                "thirdbox-aa11bb",
                1,
                1_788_220_800_001,
                "b",
            )],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(
            r.rows.len(),
            2,
            "seq=1 from two different hosts are both kept"
        );
        assert_eq!(r.hosts.len(), 2);
    }

    #[test]
    fn filters_apply_to_shard_rows_the_same_way_they_apply_locally() {
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(1, 1_788_220_800_000, "wanted"),
                &foreign_line(2, 1_788_220_800_001, "other"),
            ],
        );
        let f = crate::librarian::catalog::audit::AuditFilter {
            row_id: Some("wanted".into()),
            ..Default::default()
        };
        let r = read_shards(tmp.path(), &f, "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "wanted");
    }

    #[test]
    fn a_since_window_skips_whole_files_by_name() {
        // The filename encodes the month, so an out-of-window file is never
        // opened. This is the property that keeps merge-on-query affordable.
        // Assert `files_read`/`files_skipped_by_window`, not just row count —
        // a reader that opens every file and filters per-line would satisfy a
        // row-count-only assertion while losing the property that makes this
        // affordable.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202507.jsonl",
            &[&foreign_line(1, 1_751_328_000_000, "old")],
        );
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line(2, 1_788_220_800_000, "new")],
        );
        let f = crate::librarian::catalog::audit::AuditFilter {
            since: Some(1_788_220_000_000),
            ..Default::default()
        };
        let r = read_shards(tmp.path(), &f, "me-000000").unwrap();
        assert_eq!(r.files_read, 1, "only the in-window file is opened");
        assert_eq!(r.files_skipped_by_window, 1);
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "new");
        // `hosts` is scoped to files that survived window pruning: the skipped
        // 202507 file's seq=1 line was never parsed, so the coverage window for
        // this host reflects ONLY the seq=2 row that came from the opened file —
        // not (1, 2), which is what it would be if `hosts` covered every
        // committed shard for the host regardless of the query window.
        assert_eq!(r.hosts.get("otherbox-99ffee"), Some(&(2, 2)));
    }

    #[test]
    fn a_straddling_month_is_opened_and_filtered_per_line() {
        // Whole-file pruning is by filename month only, and must be generous
        // at the boundary: a file whose month straddles `since` is opened even
        // though it also carries rows that predate the cutoff, and only the
        // per-line filter (not the file-level window) excludes those rows.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(1, 1_788_220_000_000, "before-cutoff"),
                &foreign_line(2, 1_788_220_900_000, "after-cutoff"),
            ],
        );
        let f = crate::librarian::catalog::audit::AuditFilter {
            since: Some(1_788_220_800_000),
            ..Default::default()
        };
        let r = read_shards(tmp.path(), &f, "me-000000").unwrap();
        assert_eq!(
            r.files_read, 1,
            "the straddling file is opened, not skipped"
        );
        assert_eq!(r.files_skipped_by_window, 0);
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "after-cutoff");
    }

    #[test]
    fn a_missing_audit_directory_is_an_empty_read_not_an_error() {
        // Every clone that has never exported is in this state.
        let tmp = tempfile::tempdir().unwrap();
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert!(r.rows.is_empty());
        assert_eq!(r.files_read, 0);
    }

    #[test]
    fn a_stray_non_shard_file_is_ignored_and_not_counted_malformed() {
        // The audit directory is committed and will accumulate READMEs and
        // stray files. Those must return None from the parser and must NOT
        // inflate `malformed` — reporting them as malformed would train
        // readers to ignore the malformed count that matters.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(tmp.path(), "README.md", &["not a shard at all"]);
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line(1, 1_788_220_800_000, "ok")],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.malformed, 0, "a stray file is not a malformed shard line");
        assert_eq!(r.files_read, 1, "only the real shard was opened");
    }

    #[test]
    fn coverage_reports_the_min_and_max_seq_actually_present() {
        // Coverage must be DERIVED from the rows present, never declared in a
        // header (a header would be duplicated by merge=union). Seed 3
        // out-of-order seqs so a mutation that reports (first, last) by file
        // position rather than true min/max cannot pass by accident.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(5, 1_788_220_800_000, "mid"),
                &foreign_line(1, 1_788_220_800_001, "lo"),
                &foreign_line(9, 1_788_220_800_002, "hi"),
            ],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.hosts.get("otherbox-99ffee"), Some(&(1, 9)));
    }

    #[test]
    fn an_unreadable_shard_file_is_counted_not_silently_skipped() {
        // A name that parses as a shard but whose contents cannot be read
        // (permissions, a race, a dangling symlink) must not vanish with no
        // signal — that is the same silent-partial-answer failure mode as an
        // unreported malformed line, just one level up (whole file vs. one
        // line). A directory sharing the shard's filename triggers the same
        // `read_to_string` failure as a permissions problem, without depending
        // on this test running as non-root.
        let tmp = tempfile::tempdir().unwrap();
        let dir = super::super::host::audit_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir(dir.join("otherbox-99ffee-202609.jsonl")).unwrap();
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.unreadable_files, 1, "the unreadable file is REPORTED");
        assert_eq!(r.files_read, 0, "never counted as successfully read");
        assert_eq!(
            r.malformed, 0,
            "not conflated with a bad LINE inside a file we did read"
        );
        assert_eq!(r.rows.len(), 0);
    }

    #[test]
    fn an_until_bound_before_every_file_skips_them_all() {
        // `month_in_window`'s `since` branch is covered by
        // `a_since_window_skips_whole_files_by_name`; this covers `until`
        // symmetrically. An `until` set to a month before every committed
        // shard must prune all of them — if the `until` comparison were
        // inverted (`>=` instead of `<=`), every file would wrongly survive.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line(1, 1_788_220_800_000, "too-new")],
        );
        let f = crate::librarian::catalog::audit::AuditFilter {
            until: Some(1_751_328_000_000), // 2025-07, well before the 2026-09 shard
            ..Default::default()
        };
        let r = read_shards(tmp.path(), &f, "me-000000").unwrap();
        assert_eq!(r.files_skipped_by_window, 1);
        assert_eq!(r.files_read, 0);
        assert_eq!(r.rows.len(), 0);
    }

    #[test]
    fn matches_filters_by_tbl_actor_op_and_until_not_just_row_id_and_since() {
        // filters_apply_to_shard_rows_the_same_way_they_apply_locally exercises
        // row_id; a_since_window_skips_whole_files_by_name exercises since (at
        // the whole-file level). Neither reaches tbl/actor/op/until inside
        // `matches()` itself — a field dropped from that predicate's `&&` chain
        // would pass both of those tests silently.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(1, 1_788_220_800_000, "keep"),
                &foreign_line(2, 1_788_220_800_001, "wrong-tbl"),
            ],
        );
        let f = crate::librarian::catalog::audit::AuditFilter {
            tbl: Some("artifact".to_string()),
            actor: Some("codescout:sess-b".to_string()),
            op: Some("delete".to_string()),
            until: Some(1_788_220_800_000),
            ..Default::default()
        };
        let r = read_shards(tmp.path(), &f, "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1, "until excludes the later row");
        assert_eq!(r.rows[0].row_id, "keep");

        let f_wrong_tbl = crate::librarian::catalog::audit::AuditFilter {
            tbl: Some("nonexistent-table".to_string()),
            ..Default::default()
        };
        let r2 = read_shards(tmp.path(), &f_wrong_tbl, "me-000000").unwrap();
        assert_eq!(r2.rows.len(), 0, "tbl mismatch excludes everything");

        let f_wrong_actor = crate::librarian::catalog::audit::AuditFilter {
            actor: Some("codescout:nobody".to_string()),
            ..Default::default()
        };
        let r3 = read_shards(tmp.path(), &f_wrong_actor, "me-000000").unwrap();
        assert_eq!(r3.rows.len(), 0, "actor mismatch excludes everything");

        let f_wrong_op = crate::librarian::catalog::audit::AuditFilter {
            op: Some("insert".to_string()),
            ..Default::default()
        };
        let r4 = read_shards(tmp.path(), &f_wrong_op, "me-000000").unwrap();
        assert_eq!(r4.rows.len(), 0, "op mismatch excludes everything");
    }

    #[test]
    fn a_directory_with_both_a_self_and_a_foreign_shard_opens_only_the_foreign_one() {
        // our_own_hosts_shard_is_not_read_back only ever writes ONE file (the
        // self-host one), so it cannot distinguish "correctly skipped" from
        // "there was nothing else to read anyway". Put a foreign shard
        // alongside it and assert `files_read == 1`.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "me-000000-202609.jsonl",
            &[&foreign_line_for("me-000000", 1, 1_788_220_800_000, "mine")],
        );
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line(1, 1_788_220_800_001, "theirs")],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.files_read, 1, "only the foreign shard is opened");
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "theirs");
    }

    #[test]
    fn a_line_claiming_self_host_inside_a_foreign_named_file_is_still_excluded() {
        // Defense in depth: the filename already excludes self_host, but this
        // proves the per-line `line.host == self_host` check also does its
        // job independently — a line's own host field can disagree with the
        // file it lives in (manual edit, a future writer bug), and only the
        // per-line check catches that case.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line_for("me-000000", 1, 1_788_220_800_000, "impersonating-self"),
                &foreign_line(2, 1_788_220_800_001, "genuinely-foreign"),
            ],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1, "the self-claiming line is excluded");
        assert_eq!(r.rows[0].row_id, "genuinely-foreign");
    }
}
