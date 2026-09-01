# Committed Audit Shards (T-7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Export the local catalog audit trail to per-(host, month) JSONL files committed to git, and merge them back on read, so one machine can answer "which session on the other machine deleted these rows".

**Architecture:** A stateless replica. Export appends rows past a watermark to `.codescout/audit/<host>-<YYYYMM>.jsonl` under a file lock; read merges those files with the local table at query time. There is no import step and no second table — the shard files *are* the other hosts' rows, and `(host, seq)` is the global identity that makes both dedup and ordering work regardless of how git merged the lines.

**Tech Stack:** Rust, rusqlite (bundled SQLite), `fs4` file locking, `serde_json`, existing `librarian` tool dispatch.

**Spec:** `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md` — read § *Phase 2*, including its `### Revised 2026-09-01` and `### Settled design` subsections. Phase 1 shipped at `10972335`; the volume prerequisites (T-13) shipped at `40ab56f6`.

## Global Constraints

- **Row identity is `(host, seq)`.** `seq` is per-catalog, monotone, never reused (`AUTOINCREMENT`). Nothing may order or deduplicate by line position — `merge=union` reorders lines freely.
- **Readers dedupe on `(host, seq)`.** This is what makes export crash-safe: export appends *then* advances the watermark, so a crash between the two re-exports rows already on disk, and the duplicate is absorbed on read. Never invert that order.
- **`commits`-table rows are never exported.** Git already records commits; auditing them into git is circular. They stay in the local trail.
- **Reindex-churn updates are never exported:** an `update` row whose changed-key set is a subset of `{file_mtime, file_sha256, updated_at, missing_since}`.
- **Shard files contain data lines only — no header, no stamp line.** A header would be duplicated by `merge=union` on every same-host branch merge. Coverage is *derived* from the rows present (`min`/`max` `seq` per host).
- **A malformed shard line is counted and reported, never silently skipped.** Report it as `malformed_lines` in the response.
- **`filtered_total` and `truncated` must account for shard rows, or state that they do not.** A merged query whose total silently reflects only the local table is an IC-13 committed inside the feature that exists to prevent IC-13.
- **Export never fails a caller.** Folded into `reindex`, it is best-effort: log a warning, return the error in the envelope, never abort the reindex.
- **Host id is resolved once and persisted** in `catalog_meta['audit_host_id']`. Never re-derive it per call.
- **Epoch-ms UTC** for every timestamp; label the unit in every response carrying one.
- Gate before each commit: `cargo fmt`; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`; `cargo test --workspace --no-default-features`; `cargo test --workspace` — in that order.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/librarian/catalog/audit/host.rs` (new) | Resolve + persist the host id; shard filename build/parse |
| `src/librarian/catalog/audit/shard.rs` (new) | Shard line serde, export (append + watermark), shard reading |
| `src/librarian/catalog/audit.rs` (existing → `audit/mod.rs`) | Unchanged capture/query core; re-exports the two new modules |
| `src/librarian/tools/audit_log.rs` | `export` mode; merge-on-query in the read path |
| `src/librarian/tools/doctor.rs` | `audit.unexported_rows` + `audit.host` |
| `src/librarian/tools/reindex.rs` | Best-effort export fold-in |
| `src/librarian/tools/librarian.rs` | Schema + description for `export` |
| `.gitattributes`, `.gitignore` | `merge=union`; a comment recording that `.codescout/audit/` is tracked deliberately |
| `src/prompts/guides/librarian.md` | `audit_log` reference row gains `export` |

**Module move note:** `audit.rs` becomes `audit/mod.rs` verbatim (no content change beyond adding `pub(crate) mod host; pub(crate) mod shard;`). Do the move as its own commit so the real diff in Task 1 is readable.

---

## Task 1: Host identity and shard paths

**Files:**
- Move: `src/librarian/catalog/audit.rs` → `src/librarian/catalog/audit/mod.rs`
- Create: `src/librarian/catalog/audit/host.rs`
- Test: inline `#[cfg(test)] mod tests` in `host.rs`

**Interfaces:**
- Consumes: `crate::librarian::catalog::gc::{get_meta, set_meta}` — the existing `catalog_meta` KV helpers (`src/librarian/catalog/gc.rs:12,24`).
- Produces:
  - `pub(crate) fn resolve_host_id(conn: &Connection) -> anyhow::Result<String>`
  - `pub(crate) fn shard_file_name(host: &str, at_ms: i64) -> String`
  - `pub(crate) fn parse_shard_file_name(name: &str) -> Option<(String, String)>` → `(host, "YYYYMM")`
  - `pub(crate) const AUDIT_DIR: &str = ".codescout/audit";`
  - `pub(crate) const HOST_META_KEY: &str = "audit_host_id";`

- [ ] **Step 1: Move the module**

```bash
git mv src/librarian/catalog/audit.rs src/librarian/catalog/audit/mod.rs
```

Add to the top of `audit/mod.rs`, directly under the existing `use` lines:

```rust
pub(crate) mod host;
pub(crate) mod shard;
```

Create an empty `src/librarian/catalog/audit/shard.rs` containing only `// Task 2.` so the module compiles. Run `cargo build` and commit the move alone.

- [ ] **Step 2: Write the failing tests**

Create `src/librarian/catalog/audit/host.rs` with the tests only (no implementation yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::util::test_env::EnvGuard;

    #[test]
    fn the_host_id_is_resolved_once_and_then_persisted() {
        let cat = Catalog::open_in_memory().unwrap();
        let a = resolve_host_id(&cat.conn).unwrap();
        // Change the environment the first call read. A re-derived id would move;
        // a persisted one cannot. This is the whole point: a host id that drifts
        // silently forks one machine's shard history across two filenames.
        let _g = EnvGuard::set("CODESCOUT_AUDIT_HOST", "something-else");
        let b = resolve_host_id(&cat.conn).unwrap();
        assert_eq!(a, b, "the id must come from catalog_meta after the first call");
    }

    #[test]
    fn an_explicit_host_wins_on_first_resolution() {
        let cat = Catalog::open_in_memory().unwrap();
        let _g = EnvGuard::set("CODESCOUT_AUDIT_HOST", "Laptop.Local");
        let id = resolve_host_id(&cat.conn).unwrap();
        assert!(id.starts_with("laptop-local-"), "sanitized + suffixed, got {id}");
    }

    #[test]
    fn two_catalogs_with_the_same_name_get_different_ids() {
        // Two machines both called `arch` must not write the same shard file.
        // The readable prefix is a courtesy; the suffix is the correctness.
        let _g = EnvGuard::set("CODESCOUT_AUDIT_HOST", "arch");
        let a = resolve_host_id(&Catalog::open_in_memory().unwrap().conn).unwrap();
        let b = resolve_host_id(&Catalog::open_in_memory().unwrap().conn).unwrap();
        assert_ne!(a, b, "same name, different machines: {a} vs {b}");
        assert!(a.starts_with("arch-") && b.starts_with("arch-"));
    }

    #[test]
    fn a_hostile_host_name_cannot_escape_the_audit_directory() {
        // The host id becomes a FILENAME. `../` in it is a path traversal.
        let cat = Catalog::open_in_memory().unwrap();
        let _g = EnvGuard::set("CODESCOUT_AUDIT_HOST", "../../etc/passwd");
        let id = resolve_host_id(&cat.conn).unwrap();
        assert!(!id.contains('/') && !id.contains('.'), "got {id}");
    }

    #[test]
    fn an_empty_or_unresolvable_name_still_yields_a_usable_id() {
        let cat = Catalog::open_in_memory().unwrap();
        let _g = EnvGuard::set("CODESCOUT_AUDIT_HOST", "!!!");
        let id = resolve_host_id(&cat.conn).unwrap();
        assert!(id.starts_with("host-"), "falls back to a literal, got {id}");
        assert!(id.len() > 5);
    }

    #[test]
    fn shard_names_round_trip() {
        // 2026-09-01T00:00:00Z
        let name = shard_file_name("arch-a3f9c2", 1_788_220_800_000);
        assert_eq!(name, "arch-a3f9c2-202609.jsonl");
        let (host, month) = parse_shard_file_name(&name).unwrap();
        assert_eq!((host.as_str(), month.as_str()), ("arch-a3f9c2", "202609"));
    }

    #[test]
    fn a_non_shard_file_name_parses_to_nothing() {
        // The directory is in git; a README or a stray file must not be read as
        // a shard, and must not be reported as a malformed one either.
        assert!(parse_shard_file_name("README.md").is_none());
        assert!(parse_shard_file_name("arch-a3f9c2.jsonl").is_none());
        assert!(parse_shard_file_name("arch-a3f9c2-20260.jsonl").is_none());
    }
}
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test --lib librarian::catalog::audit::host`
Expected: FAIL — `cannot find function resolve_host_id in this scope`.

- [ ] **Step 4: Implement**

Prepend to `host.rs` (above the test module):

```rust
//! Host identity for committed audit shards.
//!
//! The catalog is machine-local and gitignored, so a value persisted in
//! `catalog_meta` IS a per-host identity by construction — no hostname lookup
//! is required for correctness, only for readability. Resolved ONCE and stored:
//! a re-derived id would move when the environment moves, silently forking one
//! machine's shard history across two filenames with no error anywhere.

use crate::librarian::catalog::gc;
use anyhow::Result;
use rusqlite::Connection;

pub(crate) const AUDIT_DIR: &str = ".codescout/audit";
pub(crate) const HOST_META_KEY: &str = "audit_host_id";

/// Sources tried in order, first non-empty wins. No `gethostname` crate: the
/// value must be persisted anyway, so a dependency would buy only the readable
/// prefix — and the prefix is a courtesy, not the correctness.
fn candidate_name() -> String {
    for key in ["CODESCOUT_AUDIT_HOST", "COMPUTERNAME", "HOSTNAME"] {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                return v;
            }
        }
    }
    std::fs::read_to_string("/etc/hostname").unwrap_or_default()
}

/// Lowercase, `[a-z0-9-]` only, collapsed and trimmed, capped at 24 chars.
///
/// This value becomes a FILENAME, so the sanitizer is a security boundary and
/// not cosmetics: an unsanitized `../../etc/passwd` would write outside the
/// audit directory. Allowlist, never a denylist — a denylist over a filename is
/// the addressing-without-an-escape-hatch class (CLAUDE.md § Parsers Over a
/// Namespace).
fn sanitize(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 24 {
            break;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "host".to_string()
    } else {
        trimmed
    }
}

/// 6 hex chars derived from process-and-time entropy. Two machines that both
/// call themselves `arch` must not write the same shard file; the readable
/// prefix cannot guarantee that and the suffix can.
fn suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mixed = nanos ^ (std::process::id() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    format!("{:06x}", mixed & 0xff_ffff)
}

/// The stable id for this catalog's machine, minted on first call.
pub(crate) fn resolve_host_id(conn: &Connection) -> Result<String> {
    if let Some(existing) = gc::get_meta(conn, HOST_META_KEY)? {
        if !existing.trim().is_empty() {
            return Ok(existing);
        }
    }
    let id = format!("{}-{}", sanitize(&candidate_name()), suffix());
    gc::set_meta(conn, HOST_META_KEY, &id)?;
    Ok(id)
}

/// `<host>-<YYYYMM>.jsonl`. One file per host per month: month bounds the file
/// size, and host keeps two machines off each other's lines entirely.
pub(crate) fn shard_file_name(host: &str, at_ms: i64) -> String {
    format!("{host}-{}.jsonl", month_key(at_ms))
}

/// `YYYYMM` for an epoch-ms UTC instant, computed from the SQLite-free civil
/// calendar so it agrees with `at_ms` on every platform.
pub(crate) fn month_key(at_ms: i64) -> String {
    let days = at_ms.div_euclid(86_400_000);
    let (y, m, _d) = civil_from_days(days);
    format!("{y:04}{m:02}")
}

/// Howard Hinnant's days-from-civil, inverted. Public-domain algorithm; keeps
/// this crate free of a chrono dependency for one date field.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Inverse of `shard_file_name`. `None` for anything that is not a shard — a
/// README, a stray file, a partially written temp file. Returning `None` (not
/// an error) is deliberate: the directory is in git and will accumulate
/// non-shard files, and reporting those as malformed would train readers to
/// ignore the malformed count that DOES matter.
pub(crate) fn parse_shard_file_name(name: &str) -> Option<(String, String)> {
    let stem = name.strip_suffix(".jsonl")?;
    let (host, month) = stem.rsplit_once('-')?;
    if month.len() != 6 || !month.chars().all(|c| c.is_ascii_digit()) || host.is_empty() {
        return None;
    }
    Some((host.to_string(), month.to_string()))
}
```

- [ ] **Step 5: Run to verify they pass**

Run: `cargo test --lib librarian::catalog::audit::host`
Expected: 7 passed.

> If `EnvGuard` is not at `crate::util::test_env::EnvGuard`, find it with
> `grep(pattern="struct EnvGuard", glob="*.rs")` and use the real path.
> `docs/conventions/test-env-isolation.md` is the convention it enforces —
> environment mutation in tests must go through the guard or the tests race.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/catalog/audit/host.rs src/librarian/catalog/audit/mod.rs
git commit -m "feat(librarian): persisted host identity for audit shards" -- src/librarian/catalog/audit/host.rs src/librarian/catalog/audit/mod.rs
```

---

## Task 2: Export — serialize, append under lock, advance the watermark

**Files:**
- Modify: `src/librarian/catalog/audit/shard.rs`
- Test: inline `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `host::{resolve_host_id, shard_file_name, month_key, AUDIT_DIR}`; `gc::{get_meta, set_meta}`; `audit::AuditRow`.
- Produces:
  - `pub(crate) const WATERMARK_KEY: &str = "audit_exported_through_seq";`
  - `pub(crate) struct ShardLine { host, seq, at_ms, tbl, op, row_id, actor, verb, payload }` — `serde::{Serialize, Deserialize}`
  - `pub(crate) struct ExportReport { exported: usize, skipped_commits: usize, skipped_churn: usize, files: Vec<String>, through_seq: i64 }`
  - `pub(crate) fn export(conn: &Connection, repo_root: &Path) -> Result<ExportReport>`
  - `pub(crate) fn unexported_count(conn: &Connection) -> Result<i64>`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact, Catalog};

    fn seed(cat: &Catalog, id: &str) {
        let row = artifact::TestArtifactRowBuilder::new(id).with_status("draft").build();
        artifact::upsert(cat, &row).unwrap();
    }

    fn lines(dir: &std::path::Path) -> Vec<ShardLine> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(dir.join(super::super::host::AUDIT_DIR)).unwrap() {
            let p = e.unwrap().path();
            for l in std::fs::read_to_string(&p).unwrap().lines() {
                out.push(serde_json::from_str(l).unwrap());
            }
        }
        out
    }

    #[test]
    fn export_writes_rows_past_the_watermark_and_advances_it() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert!(r.exported >= 1, "{r:?}");
        assert!(r.through_seq > 0);
        let got = lines(tmp.path());
        assert!(got.iter().any(|l| l.row_id == "a1" && l.tbl == "artifact"));
        assert!(got.iter().all(|l| !l.host.is_empty()), "every line names its host");
    }

    #[test]
    fn a_second_export_with_no_new_rows_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        export(&cat.conn, tmp.path()).unwrap();
        let before = lines(tmp.path()).len();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.exported, 0, "the watermark must hold");
        assert_eq!(lines(tmp.path()).len(), before, "and nothing may be appended");
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
    }

    #[test]
    fn reindex_churn_updates_are_never_exported() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
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
    }

    #[test]
    fn a_semantic_update_that_also_touches_mtime_is_still_exported() {
        // Pair of the above: the churn filter is a SUBSET test, not an
        // intersection test. An update carrying `status` alongside the mtime
        // trio is real history, and dropping it would lose exactly the rows the
        // trail exists for.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
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
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor)
                 VALUES(1751328000000,'artifact','delete','old','unknown'),
                       (1788220800000,'artifact','delete','new','unknown')",
                [],
            )
            .unwrap();
        export(&cat.conn, tmp.path()).unwrap();
        let dir = tmp.path().join(super::super::host::AUDIT_DIR);
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
        // first, advance the watermark second: a crash in between re-exports
        // rows already on disk, and readers dedupe on (host, seq). The inverse
        // order would LOSE rows with no signal anywhere.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        export(&cat.conn, tmp.path()).unwrap();
        let n = lines(tmp.path()).len();
        // Simulate the crash: the file is written, the watermark never advanced.
        gc::set_meta(&cat.conn, WATERMARK_KEY, "0").unwrap();
        export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(lines(tmp.path()).len(), n * 2, "re-export duplicates, by design");
        let seqs: std::collections::HashSet<i64> =
            lines(tmp.path()).iter().map(|l| l.seq).collect();
        assert_eq!(seqs.len(), n, "and every duplicate shares its (host, seq)");
    }

    #[test]
    fn unexported_count_matches_what_the_next_export_would_write() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let pending = unexported_count(&cat.conn).unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(
            pending as usize,
            r.exported + r.skipped_commits + r.skipped_churn,
            "doctor's delta must describe the same population export consumes"
        );
        assert_eq!(unexported_count(&cat.conn).unwrap(), 0);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib librarian::catalog::audit::shard`
Expected: FAIL — `cannot find function export in this scope`.

- [ ] **Step 3: Implement**

```rust
//! Committed audit shards: the export half.
//!
//! The local WAL cannot live in git — its in-transaction guarantee exists only
//! at mutation time on a gitignored database — so what git carries is a
//! REPLICA, and every surface must say so. See the spec's § Phase 2.

use super::host::{self, AUDIT_DIR};
use crate::librarian::catalog::gc;
use anyhow::{Context, Result};
use fs4::fs_std::FileExt;
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

pub(crate) const WATERMARK_KEY: &str = "audit_exported_through_seq";

/// Changed-key sets that are pure reindex bookkeeping. An `update` whose keys
/// are a SUBSET of this is dropped from the export; one that also carries any
/// other key is real history and is kept.
const CHURN_KEYS: &[&str] = &["file_mtime", "file_sha256", "updated_at", "missing_since"];

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ShardLine {
    pub host: String,
    pub seq: i64,
    pub at_ms: i64,
    pub tbl: String,
    pub op: String,
    pub row_id: String,
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
    pub files: Vec<String>,
    pub through_seq: i64,
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

fn watermark(conn: &Connection) -> Result<i64> {
    Ok(gc::get_meta(conn, WATERMARK_KEY)?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0))
}

/// Rows past the watermark that export would consider — the SAME population
/// `export` consumes, including the ones it will drop. Doctor reports this, so
/// it must not describe a different set than the verb does; a delta that
/// counts rows export will never write reads as a permanent backlog.
pub(crate) fn unexported_count(conn: &Connection) -> Result<i64> {
    let w = watermark(conn)?;
    Ok(conn.query_row(
        "SELECT count(*) FROM catalog_audit WHERE seq > ?1",
        [w],
        |r| r.get(0),
    )?)
}

/// Append every eligible row past the watermark to its `(host, month)` shard,
/// then advance the watermark.
///
/// ORDER IS LOAD-BEARING: append, fsync, THEN persist the watermark. A crash in
/// between re-exports rows already on disk and readers absorb the duplicate on
/// `(host, seq)`; the inverse order loses rows silently. Never reorder.
pub(crate) fn export(conn: &Connection, repo_root: &Path) -> Result<ExportReport> {
    let host_id = host::resolve_host_id(conn)?;
    let from = watermark(conn)?;
    let dir = repo_root.join(AUDIT_DIR);

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
        ..Default::default()
    };
    let mut by_month: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for (seq, at_ms, tbl, op, row_id, actor, verb, payload) in rows {
        report.through_seq = report.through_seq.max(seq);
        if tbl == "commits" {
            report.skipped_commits += 1;
            continue;
        }
        if is_pure_churn(&op, payload.as_deref()) {
            report.skipped_churn += 1;
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
            payload: payload
                .as_deref()
                .and_then(|p| serde_json::from_str(p).ok()),
        };
        by_month
            .entry(host::month_key(at_ms))
            .or_default()
            .push(serde_json::to_string(&line)?);
        report.exported += 1;
    }

    if !by_month.is_empty() {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating {}", dir.display()))?;
        for (month, lines) in &by_month {
            let name = format!("{host_id}-{month}.jsonl");
            let path = dir.join(&name);
            // One exclusive lock per file: two sessions reindexing at once must
            // not interleave partial lines into a file that is about to be
            // committed. Same primitive as src/retrieval/index_lock.rs.
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .with_context(|| format!("opening {}", path.display()))?;
            FileExt::lock_exclusive(&f)?;
            let body = lines.join("\n");
            let r = writeln!(f, "{body}").and_then(|_| f.sync_all());
            let _ = FileExt::unlock(&f);
            r.with_context(|| format!("appending to {}", path.display()))?;
            report.files.push(name);
        }
    }

    // Only after every append is durable.
    gc::set_meta(conn, WATERMARK_KEY, &report.through_seq.to_string())?;
    Ok(report)
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --lib librarian::catalog::audit::shard`
Expected: 8 passed.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/catalog/audit/shard.rs
git commit -m "feat(librarian): export audit rows to per-host monthly shards" -- src/librarian/catalog/audit/shard.rs
```

---

## Task 3: Shard reading and merge-on-query

**Files:**
- Modify: `src/librarian/catalog/audit/shard.rs` (reading half)
- Test: same inline module

**Interfaces:**
- Consumes: `host::parse_shard_file_name`, `audit::AuditFilter`.
- Produces:
  - `pub(crate) struct ShardRead { rows: Vec<ShardLine>, matched: usize, malformed: usize, hosts: BTreeMap<String, (i64, i64)>, files_read: usize, files_skipped_by_window: usize }`
  - `pub(crate) fn read_shards(repo_root: &Path, f: &AuditFilter, self_host: &str) -> Result<ShardRead>`

- [ ] **Step 1: Write the failing tests**

```rust
    fn write_shard(root: &std::path::Path, name: &str, lines: &[&str]) {
        let dir = root.join(super::super::host::AUDIT_DIR);
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
    }

    #[test]
    fn duplicate_host_seq_pairs_collapse_to_one_row() {
        // merge=union and crash-re-export both produce duplicates by design.
        let tmp = tempfile::tempdir().unwrap();
        let l = foreign_line(7, 1_788_220_800_000, "dup");
        write_shard(tmp.path(), "otherbox-99ffee-202609.jsonl", &[&l, &l]);
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1, "deduped on (host, seq)");
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
    }

    #[test]
    fn a_missing_audit_directory_is_an_empty_read_not_an_error() {
        // Every clone that has never exported is in this state.
        let tmp = tempfile::tempdir().unwrap();
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert!(r.rows.is_empty());
        assert_eq!(r.files_read, 0);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib librarian::catalog::audit::shard`
Expected: FAIL — `cannot find function read_shards in this scope`.

- [ ] **Step 3: Implement**

Append to `shard.rs`:

```rust
#[derive(Debug, Default)]
pub(crate) struct ShardRead {
    pub rows: Vec<ShardLine>,
    pub malformed: usize,
    /// host → (min seq, max seq) present. This is the coverage window, DERIVED
    /// from the rows rather than declared in a header: a header line would be
    /// duplicated by `merge=union` on every same-host branch merge, and a
    /// declared window that disagrees with the rows is worse than none.
    pub hosts: BTreeMap<String, (i64, i64)>,
    pub files_read: usize,
    pub files_skipped_by_window: usize,
}

fn matches(l: &ShardLine, f: &super::AuditFilter) -> bool {
    f.tbl.as_ref().is_none_or(|v| *v == l.tbl)
        && f.row_id.as_ref().is_none_or(|v| *v == l.row_id)
        && f.actor.as_ref().is_none_or(|v| *v == l.actor)
        && f.op.as_ref().is_none_or(|v| *v == l.op)
        && f.since.is_none_or(|v| l.at_ms >= v)
        && f.until.is_none_or(|v| l.at_ms <= v)
}

/// Read every OTHER host's shards, filtered and deduped.
///
/// `self_host`'s own shard is skipped: those rows are already in the local
/// table, and counting them twice would produce a wrong `filtered_total` —
/// a plausible number rather than an error, which nothing downstream catches.
pub(crate) fn read_shards(
    repo_root: &Path,
    f: &super::AuditFilter,
    self_host: &str,
) -> Result<ShardRead> {
    let dir = repo_root.join(AUDIT_DIR);
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
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --lib librarian::catalog::audit::shard`
Expected: 15 passed (8 from Task 2 + 7 here).

- [ ] **Step 5: Commit**

```bash
git add src/librarian/catalog/audit/shard.rs
git commit -m "feat(librarian): read committed shards, deduped on (host, seq)" -- src/librarian/catalog/audit/shard.rs
```

---

## Task 4: Wire it into `audit_log`, `doctor`, and `reindex`

**Files:**
- Modify: `src/librarian/tools/audit_log.rs`
- Modify: `src/librarian/tools/doctor.rs:494` (the `audit_health` line)
- Modify: `src/librarian/tools/reindex.rs:400` (the result `json!`)
- Modify: `src/librarian/tools/librarian.rs:39-42, 52, 116` (description + schema)
- Create: `.gitattributes` entry; `.gitignore` comment

**Interfaces:**
- Consumes: `shard::{export, read_shards, unexported_count, ExportReport}`, `host::resolve_host_id`.
- Produces: `librarian(action="audit_log", export=true)`; `audit.unexported_rows` and `audit.host` in doctor; `audit_export` in the reindex envelope.

- [ ] **Step 1: Write the failing tests**

Add to `src/librarian/tools/audit_log.rs`'s test module:

```rust
    #[tokio::test]
    async fn export_mode_reports_what_it_wrote() {
        let (ctx, _tmp) = mk_ctx();
        let v = call(&ctx, json!({"export": true})).await.unwrap();
        assert!(v["exported"].is_number(), "{v}");
        assert!(v["through_seq"].is_number());
        assert!(
            v["note"].as_str().unwrap().contains("replica"),
            "the response must SAY it is a replica, not merely be one: {v}"
        );
    }

    #[tokio::test]
    async fn a_merged_query_counts_shard_rows_in_its_totals() {
        // The IC-13 guard named in the spec: a total that silently reflects only
        // the local table is a wrong number inside the feature built to prevent
        // wrong numbers.
        let (ctx, tmp) = mk_ctx();
        write_foreign_shard(tmp.path(), 3);
        let v = call(&ctx, json!({"limit": 500})).await.unwrap();
        let entries = v["entries"].as_array().unwrap();
        assert!(
            entries.iter().any(|e| e["host"] == "otherbox-99ffee"),
            "a foreign row must appear: {v}"
        );
        assert!(
            v["filtered_total"].as_i64().unwrap() >= 3,
            "and be counted: {v}"
        );
        assert_eq!(v["shards"]["files_read"], 1);
        assert_eq!(v["shards"]["malformed_lines"], 0);
        assert!(v["shards"]["coverage"]["otherbox-99ffee"].is_array());
    }

    #[tokio::test]
    async fn a_local_row_is_labelled_with_this_host_not_left_blank() {
        // Every entry names its origin, so "local" is a positive statement
        // rather than the absence of a field.
        let (ctx, _tmp) = mk_ctx();
        let v = call(&ctx, json!({"limit": 5})).await.unwrap();
        for e in v["entries"].as_array().unwrap() {
            assert!(e["host"].is_string(), "{e}");
        }
    }

    #[tokio::test]
    async fn export_refuses_to_combine_with_a_query_filter() {
        // Same shape as the prune guard: a filter silently ignored beside an
        // action that does not read it is IC-15.
        let (ctx, _tmp) = mk_ctx();
        let err = call(&ctx, json!({"export": true, "tbl": "artifact"}))
            .await
            .unwrap_err();
        assert!(format!("{err}").contains("tbl"), "{err}");
    }
```

Add to `doctor.rs`'s test module:

```rust
    #[tokio::test]
    async fn the_audit_block_names_the_host_and_the_unexported_delta() {
        let (ctx, _tmp) = mk_ctx();
        let v = call(&ctx, json!({})).await.unwrap();
        let audit = &v["catalog_health"]["audit"];
        assert!(audit["host"].is_string(), "{audit}");
        assert!(audit["unexported_rows"].is_number(), "{audit}");
        assert!(
            audit["hint"].as_str().unwrap().contains("export"),
            "an unexported delta a reader cannot act on is decoration: {audit}"
        );
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --lib librarian::tools::audit_log librarian::tools::doctor`
Expected: FAIL — missing `export` handling, missing `host`/`shards` fields.

- [ ] **Step 3: Implement — `audit_log.rs`**

At the top of `call()`, **before** the prune branch, add the export branch:

```rust
    // Export mode. Placed before prune so the two mutually exclusive write
    // modes sit together and neither can silently absorb the other's args.
    if args.get("export").and_then(Value::as_bool).unwrap_or(false) {
        let present: Vec<&str> = PRUNE_IGNORED_FILTER_KEYS
            .iter()
            .copied()
            .filter(|k| args.get(*k).is_some_and(|v| !v.is_null()))
            .collect();
        if !present.is_empty() {
            return Err(RecoverableError::new(format!(
                "export does not accept filters; it exports every unexported row — remove: {}",
                present.join(", ")
            )));
        }
        let root = ctx.project_root()?;
        let cat = ctx.catalog.lock();
        let r = shard::export(&cat.conn, &root)?;
        return Ok(json!({
            "exported": r.exported,
            "skipped_commits": r.skipped_commits,
            "skipped_churn": r.skipped_churn,
            "files": r.files,
            "through_seq": r.through_seq,
            "dir": format!("{}/", host::AUDIT_DIR),
            "note": "a committed shard is a REPLICA of the local trail, fresh only as of through_seq — the in-transaction guarantee exists on the local database alone. Commit the files to share them.",
        }));
    }
```

In the read path, after `let filtered_total = audit::count_matching(...)?;`, merge the shards:

```rust
    let self_host = host::resolve_host_id(&cat.conn)?;
    let shards = ctx
        .project_root()
        .ok()
        .map(|root| shard::read_shards(&root, &f, &self_host))
        .transpose()?
        .unwrap_or_default();
```

Build `entries` from both sides, each carrying `host`, newest-first, capped at `limit`; and make the totals honest:

```rust
    let mut merged: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "host": self_host, "seq": r.seq, "at_ms": r.at_ms, "tbl": r.tbl,
                "op": r.op, "row_id": r.row_id, "actor": r.actor, "verb": r.verb,
                "payload": r.payload.as_deref()
                    .and_then(|p| serde_json::from_str::<Value>(p).ok()),
            })
        })
        .chain(shards.rows.iter().map(|l| {
            json!({
                "host": l.host, "seq": l.seq, "at_ms": l.at_ms, "tbl": l.tbl,
                "op": l.op, "row_id": l.row_id, "actor": l.actor, "verb": l.verb,
                "payload": l.payload,
            })
        }))
        .collect();
    merged.sort_by(|a, b| {
        b["at_ms"].as_i64().cmp(&a["at_ms"].as_i64())
            .then_with(|| b["seq"].as_i64().cmp(&a["seq"].as_i64()))
    });
    let entries: Vec<Value> = merged.into_iter().take(limit).collect();
    // `filtered_total` spans BOTH sources, so `truncated` describes the response
    // the caller actually got. A total covering only the local table beside
    // entries drawn from both is a wrong number with nothing to catch it.
    let filtered_total = filtered_total + shards.rows.len() as i64;
    let count = entries.len();
    let truncated = filtered_total > count as i64;
```

Add to the response object:

```rust
        "shards": {
            "files_read": shards.files_read,
            "files_skipped_by_window": shards.files_skipped_by_window,
            "malformed_lines": shards.malformed,
            "coverage": shards.hosts.iter()
                .map(|(h, (lo, hi))| (h.clone(), json!([lo, hi])))
                .collect::<serde_json::Map<_, _>>(),
            "self_host": self_host,
        },
```

and, when `shards.malformed > 0`, a `shards_warning` naming the count and that the rows are otherwise intact.

- [ ] **Step 4: Implement — `doctor.rs`**

At line 494, after `let audit_health = ...`, extend the object:

```rust
    let mut audit_health = crate::librarian::catalog::audit::health(&cat.conn)?;
    let pending = crate::librarian::catalog::audit::shard::unexported_count(&cat.conn)?;
    audit_health["host"] =
        json!(crate::librarian::catalog::audit::host::resolve_host_id(&cat.conn)?);
    audit_health["unexported_rows"] = json!(pending);
    if pending > 0 {
        audit_health["hint"] = json!(format!(
            "{pending} audit rows are not in a committed shard — run librarian(action=\"audit_log\", export=true) and commit .codescout/audit/. A shard is a replica: it is only as fresh as its last export."
        ));
    }
```

- [ ] **Step 5: Implement — `reindex.rs`**

Immediately before the result `json!` at line 400:

```rust
    // Fold-in, best effort: an export failure must never fail a reindex. The
    // envelope reports it so a silently-never-exporting machine is visible
    // (a committed replica that quietly stops updating is the IC-13 this
    // whole phase exists to avoid).
    let audit_export = match crate::librarian::catalog::audit::shard::export(
        &ctx.catalog.lock().conn,
        &root,
    ) {
        Ok(r) => json!({"exported": r.exported, "through_seq": r.through_seq}),
        Err(e) => {
            tracing::warn!("audit shard export failed: {e}");
            json!({"error": e.to_string()})
        }
    };
```

and add `"audit_export": audit_export,` to the returned object.

- [ ] **Step 6: Implement — git config**

Append to `.gitattributes`:

```gitattributes
# Audit shards are append-only line logs. Different hosts write different
# files; the same host on two branches unions cleanly, because global order is
# re-derived from (host, seq, at_ms) and never from line position.
.codescout/audit/*.jsonl merge=union
```

Append to `.gitignore`, in the `.codescout` block:

```gitignore
# NOT ignored, deliberately: /.codescout/audit/*.jsonl are the committed audit
# shards (T-7). Adding a blanket `.codescout/` rule would silently stop sharing
# them, with no error anywhere — the same shape as the projects/ note above.
```

- [ ] **Step 7: Implement — `librarian.rs` schema**

Extend the `audit_log` sentence in the tool description with: `export=true appends unexported rows to .codescout/audit/<host>-<YYYYMM>.jsonl (a committed replica, fresh only as of its watermark); queries merge any other host's shards found there.` Add the property:

```json
"export": { "type": "boolean", "description": "audit_log: append every unexported row to this host's committed shard under .codescout/audit/ and advance the watermark. Not combinable with filters. The shard is a REPLICA — only as fresh as its last export." }
```

- [ ] **Step 8: Run the full gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features
cargo test --workspace
git add src/librarian/tools/ .gitattributes .gitignore
git commit -m "feat(librarian): merge committed shards on read; export folds into reindex" -- src/librarian/tools/ .gitattributes .gitignore
```

---

## Task 5: End-to-end cross-machine test and documentation

**Files:**
- Create: `tests/audit_shards_cross_machine.rs`
- Modify: `src/prompts/guides/librarian.md` (`## librarian(action=...) — Reference`)
- Modify: `docs/conventions/cross-machine-catalog-resume.md`
- Modify: `docs/PROBES.md`

**Interfaces:** consumes the shipped `librarian(action="audit_log", export=true)` surface only — this task must not reach into internals.

- [ ] **Step 1: Write the failing end-to-end test**

```rust
//! The acceptance criterion from the tracker, end to end: a row mutated on
//! host A is queryable on host B after a pull.
//!
//! "Host B" is a second catalog with its own `audit_host_id`, sharing one repo
//! directory — which is exactly what a pull produces. The two catalogs never
//! see each other's tables; the only channel is the committed file.

#[tokio::test]
async fn a_row_deleted_on_host_a_is_answerable_on_host_b() {
    let repo = tempfile::tempdir().unwrap();

    // --- Host A: create, then delete, then export ---
    let a = ctx_for(repo.path(), "hosta-aaa111");
    seed_artifact(&a, "vanished-1").await;
    delete_artifact(&a, "vanished-1").await;
    let exported = audit_log(&a, json!({"export": true})).await;
    assert!(exported["exported"].as_i64().unwrap() >= 2, "{exported}");

    // --- Host B: a different catalog, same repo directory ---
    let b = ctx_for(repo.path(), "hostb-bbb222");
    let v = audit_log(&b, json!({"row_id": "vanished-1", "limit": 50})).await;

    let entries = v["entries"].as_array().unwrap();
    let del = entries
        .iter()
        .find(|e| e["op"] == "delete")
        .expect("host B must see host A's delete");
    assert_eq!(del["host"], "hosta-aaa111", "and know which machine did it");
    assert_eq!(del["payload"]["id"], "vanished-1", "with the OLD row image");
    assert!(
        del["actor"].as_str().unwrap().starts_with("codescout:"),
        "and which session: {del}"
    );
    assert_eq!(v["shards"]["self_host"], "hostb-bbb222");
}

#[tokio::test]
async fn host_b_does_not_re_export_host_as_rows_as_its_own() {
    // The failure this guards: B reads A's shard, treats those rows as local,
    // and writes them into B's own shard — inflating the record with rows B
    // never saw, attributed to B. Shards are read-only to every host but their
    // author, and nothing but this test says so.
    let repo = tempfile::tempdir().unwrap();
    let a = ctx_for(repo.path(), "hosta-aaa111");
    seed_artifact(&a, "x1").await;
    audit_log(&a, json!({"export": true})).await;

    let b = ctx_for(repo.path(), "hostb-bbb222");
    let r = audit_log(&b, json!({"export": true})).await;
    assert_eq!(r["exported"], 0, "B has no rows of its own to export: {r}");

    let dir = repo.path().join(".codescout/audit");
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names.len(), 1, "B must not have written a shard: {names:?}");
}
```

Write `ctx_for`, `seed_artifact`, `delete_artifact`, `audit_log` as local helpers in the same file, following the harness in `tests/cli_artifact.rs`. `ctx_for` sets `CODESCOUT_AUDIT_HOST` before the catalog's first `resolve_host_id`, then opens a catalog at its own temp path with the shared repo root.

- [ ] **Step 2: Run to verify it fails, then passes**

Run: `cargo test --test audit_shards_cross_machine`
Expected first: FAIL. After Tasks 1-4 are in place it should pass; if it does not, the defect is real integration drift — report it rather than adjusting the assertions.

- [ ] **Step 3: Documentation**

In `src/prompts/guides/librarian.md`, update the `audit_log` row of `## librarian(action=...) — Reference` to mention `export=true` and that queries merge other hosts' shards. **Check the 1900-character slice cap** (`src/prompts/README.md`) — if the addition would exceed it, shorten elsewhere in the same row rather than dropping the mention.

In `docs/conventions/cross-machine-catalog-resume.md`, add the audit trail as a layer that a fresh clone arrives holding *partially*: the shards are in git, the local WAL is not, so `audit_log` on a fresh clone answers about other hosts and knows nothing of its own past. Name it explicitly — that asymmetry is exactly the kind the page exists to make legible.

In `docs/PROBES.md`, add a row for `librarian(action="audit_log")` as the cross-machine mutation-forensics instrument, with its blind spot: **it is only as fresh as each host's last export and commit**, so a quiet host looks like an idle one.

- [ ] **Step 4: Full gate and commit**

```bash
cargo fmt
cargo clippy --workspace --all-targets --features local-embed -- -D warnings
cargo test --workspace --no-default-features
cargo test --workspace
git add tests/audit_shards_cross_machine.rs src/prompts/guides/librarian.md docs/
git commit -m "test(librarian): cross-machine shard read, end to end" -- tests/audit_shards_cross_machine.rs src/prompts/guides/librarian.md docs/
```

---

## Self-review notes

**Spec coverage.** Every bullet of the spec's `### Settled design` maps to a task: shard path + host identity → Task 1; export, watermark, churn/commits filtering, `fs4` lock → Task 2; merge-on-query, dedup, coverage → Task 3; honesty markers, doctor delta, reindex fold-in, `.gitattributes`/`.gitignore` → Task 4; the acceptance criteria ("a row mutated on host A is queryable on host B", "doctor names the unexported delta", "a merged read labels each host's coverage window") → Task 5 plus Task 4's doctor test.

**Deliberately deferred.** `merge_worktree` fold-in — the spec names it beside `reindex`, but a worktree session already refuses ledger-id allocation on the same grounds, and folding an export into it needs a decision about which catalog's watermark advances. Out of scope; note it in the tracker when Task 5 lands rather than guessing here.

**The riskiest task is 3, not 2.** Export is a straight-line append; the read path has the dedup, the window pruning, and the totals arithmetic that the spec itself flags as this feature's own IC-13 trap. Review it with that lens.

**Known monotonicity traps in these tests.** `a_foreign_hosts_rows_are_read_back` is satisfied by a reader that returns *too much*, so it is paired with `our_own_hosts_shard_is_not_read_back` and `duplicate_host_seq_pairs_collapse_to_one_row`. `a_since_window_skips_whole_files_by_name` asserts `files_read == 1` rather than only checking the row count, because a reader that opens every file and filters per-line would satisfy a row-count assertion while losing the property that makes this affordable.
