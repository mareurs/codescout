//! Host identity for committed audit shards.
//!
//! The catalog is machine-local and gitignored, so a value persisted in
//! `catalog_meta` IS a per-host identity by construction — no hostname lookup
//! is required for correctness, only for readability. Resolved ONCE and stored:
//! a re-derived id would move when the environment moves, silently forking one
//! machine's shard history across two filenames with no error anywhere.
//!
//! Task 2 (writer, `shard.rs`) now calls into this module, but only from
//! `shard::export`, which is itself only called by `shard.rs`'s own tests —
//! so under `--cfg test` these items are reachable via that chain, and in the
//! non-test build every item here remains genuinely unreached until a real
//! (non-test) caller lands. Task 3 (reader) is still the sole consumer of
//! `parse_shard_file_name`. Each item below carries its own
//! `#[cfg_attr(not(test), expect(dead_code, reason = "..."))]` rather than a
//! file-scoped `#![allow(dead_code)]`: `expect` fires
//! `unfulfilled_lint_expectations` the moment a later task adds the first
//! real (non-test) caller, so a stale suppression cannot ride along silently
//! the way a blanket `allow` would (see `src/server.rs`'s `session_key` field
//! for the same pattern). The `cfg_attr(not(test), ...)` wrapper is needed
//! because this file's own unit tests call every one of these items directly
//! — under `--cfg test` they are genuinely reachable and the plain
//! `#[expect(dead_code)]` form would itself go unfulfilled and fail the
//! `--all-targets` build; gating the expectation to the non-test
//! configuration keeps it honest in both.

use crate::librarian::catalog::gc;
use anyhow::Result;
use rusqlite::Connection;

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via shard::export")
)]
pub(crate) const AUDIT_DIR: &str = ".codescout/audit";
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by Task 2 (writer) via resolve_host_id, the only writer of this key"
    )
)]
pub(crate) const HOST_META_KEY: &str = "audit_host_id";

/// Sources tried in order, first non-empty wins. No `gethostname` crate: the
/// value must be persisted anyway, so a dependency would buy only the readable
/// prefix — and the prefix is a courtesy, not the correctness.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via resolve_host_id")
)]
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
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via mint_host_id")
)]
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

/// Process-local atomic counter mixed into `suffix()`. Two calls in one
/// process share the pid and can share the nanosecond on a coarse clock —
/// that is both a flaky test and a real collision — so a monotonically
/// increasing counter is mixed in as a third, always-distinct source.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via suffix")
)]
static MINT_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Captured once per process, not once per call: `suffix()`'s uniqueness
/// guarantee is that `MINT_COUNTER`'s contribution never collides with
/// itself, and that is only exact if every OTHER term in the mix (nanos, pid)
/// is held constant across calls being compared. Re-reading the clock per
/// call would make it merely probabilistic — a nanosecond term could in
/// principle vary in exactly the bits the counter also touches.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via suffix")
)]
static SUFFIX_NANOS: std::sync::LazyLock<u64> = std::sync::LazyLock::new(|| {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
});

/// 6 hex chars derived from process, time, and call-count entropy. Two
/// machines that both call themselves `arch` must not write the same shard
/// file; the readable prefix cannot guarantee that and the suffix can.
///
/// Deliberately not `RandomState`: its per-call variation is documented as
/// unspecified rather than guaranteed, and an unverified claim is not a
/// foundation for a collision guard.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via mint_host_id")
)]
fn suffix() -> String {
    use std::sync::atomic::Ordering;
    const K1: u64 = 0x9E37_79B9_7F4A_7C15;
    const K2: u64 = 0xC2B2_AE3D_27D4_EB4F;
    let count = MINT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed =
        *SUFFIX_NANOS ^ (std::process::id() as u64).wrapping_mul(K1) ^ count.wrapping_mul(K2);
    format!("{:06x}", mixed & 0xff_ffff)
}

/// Pure: sanitize a candidate name and append a fresh, always-distinct
/// suffix. Split out from `resolve_host_id` so sanitization, path-traversal
/// escape, and the fallback behavior are all testable without touching the
/// environment or a catalog connection.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via resolve_host_id")
)]
pub(crate) fn mint_host_id(candidate: &str) -> String {
    format!("{}-{}", sanitize(candidate), suffix())
}

/// The stable id for this catalog's machine: read from `catalog_meta` if
/// already minted, else minted from `candidate_name()` and persisted. Thin by
/// design — all the logic that needs testing lives in `mint_host_id`.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via shard::export")
)]
pub(crate) fn resolve_host_id(conn: &Connection) -> Result<String> {
    if let Some(existing) = gc::get_meta(conn, HOST_META_KEY)? {
        if !existing.trim().is_empty() {
            return Ok(existing.trim().to_string());
        }
    }
    let id = mint_host_id(&candidate_name());
    gc::set_meta(conn, HOST_META_KEY, &id)?;
    Ok(id)
}

/// `<host>-<YYYYMM>.jsonl`. One file per host per month: month bounds the file
/// size, and host keeps two machines off each other's lines entirely.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via shard::export")
)]
pub(crate) fn shard_file_name(host: &str, at_ms: i64) -> String {
    format!("{host}-{}.jsonl", month_key(at_ms))
}

/// `YYYYMM` for an epoch-ms UTC instant, computed from the SQLite-free civil
/// calendar so it agrees with `at_ms` on every platform.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via shard_file_name")
)]
pub(crate) fn month_key(at_ms: i64) -> String {
    let days = at_ms.div_euclid(86_400_000);
    let (y, m, _d) = civil_from_days(days);
    format!("{y:04}{m:02}")
}

/// Howard Hinnant's days-from-civil, inverted. Public-domain algorithm; keeps
/// this crate free of a chrono dependency for one date field.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by Task 2 (writer) via month_key")
)]
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
#[cfg_attr(not(test), expect(dead_code, reason = "consumed by Task 3 (reader)"))]
pub(crate) fn parse_shard_file_name(name: &str) -> Option<(String, String)> {
    let stem = name.strip_suffix(".jsonl")?;
    let (host, month) = stem.rsplit_once('-')?;
    if month.len() != 6 || !month.chars().all(|c| c.is_ascii_digit()) || host.is_empty() {
        return None;
    }
    Some((host.to_string(), month.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;

    #[test]
    fn the_host_id_is_resolved_once_and_then_persisted() {
        let cat = Catalog::open_in_memory().unwrap();
        let a = resolve_host_id(&cat.conn).unwrap();
        // Calling twice on the same connection must return the persisted value,
        // not a fresh mint. This is the whole point: a host id that drifts
        // silently forks one machine's shard history across two filenames.
        let b = resolve_host_id(&cat.conn).unwrap();
        assert_eq!(
            a, b,
            "the id must come from catalog_meta after the first call"
        );
    }

    #[test]
    fn mint_host_id_sanitizes_and_suffixes() {
        let id = mint_host_id("Laptop.Local");
        assert!(
            id.starts_with("laptop-local-"),
            "sanitized + suffixed, got {id}"
        );
        assert_eq!(id.len(), "laptop-local".len() + 1 + 6);
    }

    #[test]
    fn two_mints_of_the_same_name_get_different_ids() {
        // Two machines both called `arch` must not write the same shard file.
        // The readable prefix is a courtesy; the suffix is the correctness.
        let a = mint_host_id("arch");
        let b = mint_host_id("arch");
        assert_ne!(a, b, "same name, different mints: {a} vs {b}");
        assert!(a.starts_with("arch-") && b.starts_with("arch-"));
    }

    #[test]
    fn a_hostile_host_name_cannot_escape_the_audit_directory() {
        // The host id becomes a FILENAME. `../` in it is a path traversal. The
        // 'ä' is deliberate, not decorative: it is Unicode-alphanumeric but not
        // ASCII-alphanumeric, so it is what actually distinguishes the two
        // sanitize() implementations below — a plain ASCII-only traversal
        // string is indistinguishable to both, and would make this assertion
        // (and the mutation it targets) untestable.
        //
        // Allowlist assertion, not a denylist: checking for the presence of
        // '/' and '.' survives a sanitize() mutated from is_ascii_alphanumeric
        // to is_alphanumeric — 'ä' would then pass through unsanitized, and
        // neither '/' nor '.' would notice. Asserting the exact allowed
        // charset does notice, and confirmed empirically to fail under that
        // mutation (see the fix report).
        let id = mint_host_id("../../etc/pässwd");
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "got {id}"
        );
    }

    #[test]
    fn an_empty_or_unresolvable_name_still_yields_a_usable_id() {
        let id = mint_host_id("!!!");
        assert!(id.starts_with("host-"), "falls back to a literal, got {id}");
        assert!(id.len() > 5);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "got {id}"
        );
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
    fn month_key_handles_the_year_boundary() {
        // 2027-01-01T00:00:00Z — executes the `m <= 2 => y + 1` branch in
        // civil_from_days; a September-only vector never reaches it.
        assert_eq!(month_key(1_798_761_600_000), "202701");
        // 1ms earlier: 2026-12-31T23:59:59.999Z — pins Dec/Jan on both sides
        // of the boundary rather than just the Jan side.
        assert_eq!(month_key(1_798_761_599_000), "202612");
        // 2028-02-29T00:00:00Z — leap day.
        assert_eq!(month_key(1_835_438_400_000), "202802");
        // 2026-09-01T00:00:00Z — the original vector, kept for continuity.
        assert_eq!(month_key(1_788_220_800_000), "202609");
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
