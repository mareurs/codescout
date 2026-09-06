//! Host identity for committed audit shards.
//!
//! The catalog is machine-local and gitignored, so a value persisted in
//! `catalog_meta` IS a per-host identity by construction — no hostname lookup
//! is required for correctness, only for readability. Resolved ONCE and stored:
//! a re-derived id would move when the environment moves, silently forking one
//! machine's shard history across two filenames with no error anywhere.
//!
//! Task 4 wired this module into the live tool surface: `resolve_host_id` is
//! now called from `audit_log::call`'s query and export paths (labelling
//! every merged row with its origin host), and `parse_shard_file_name` /
//! `shard_file_name` are called from `shard::export` and `shard::read_shards`
//! on real (non-test) call paths. Every item in this file is reachable from a
//! non-test caller as of that commit. The `#[cfg_attr(not(test), expect(
//! dead_code, reason = "..."))]` attributes that used to guard each item
//! individually (rather than a file-scoped `#![allow(dead_code)]`, so a
//! stale suppression could not ride along silently — see `src/server.rs`'s
//! `session_key` field for the same pattern) were DELETED, not widened, once
//! `unfulfilled_lint_expectations` confirmed each attribute's item had
//! become live; none remain in this file today.

use crate::librarian::catalog::gc;
use anyhow::Result;
use rusqlite::Connection;

/// The committed audit-shard directory, as SEPARATE path components.
///
/// Two entries and not one string on hygiene grounds: `Path::join` treats its argument as
/// a single opaque component, so `join(".codescout/audit")` produced
/// `C:\…\.tmpXXXX\.codescout/audit` on Windows — backslashes throughout, one forward
/// slash. Building from components avoids the mixed form and lets
/// [`audit_dir_display`] derive the POSIX rendering instead of repeating the literal.
///
/// **This was NOT the cause of the 2026-09-02 Windows failures, and this comment used to
/// say it was.** That diagnosis was drawn from a 2-of-21 sample and the split shipped at
/// `9a156bd4` changed nothing — the next run failed identically. The real cause was
/// `LockFileEx` refusing an append-only handle in `shard.rs`; the mixed path is tolerated
/// by Windows here, and the failure happens after `create_dir_all` and after the file
/// opens. Kept because it is correct, not because it fixed anything.
/// See `docs/issues/archive/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md`.
///
/// **Do not collapse these back into one literal**, and note the Linux-side reason the
/// obvious guard does not work: on Unix `join(".codescout/audit")` DOES split into two
/// components, so asserting the built path's component count passes on the platform
/// everyone develops on and catches nothing. The invariant that survives that is the one
/// asserted by `audit_dir_parts_carry_no_separator` — that no *part* contains a separator
/// — which is checkable everywhere precisely because it is about the input rather than
/// the result.
pub(crate) const AUDIT_DIR_PARTS: [&str; 2] = [".codescout", "audit"];
pub(crate) const HOST_META_KEY: &str = "audit_host_id";

/// Set by [`resolve_host_id`] when it detects that the stored id was minted by a DIFFERENT
/// machine and re-mints. Holds the id being replaced, so the shard that id already wrote
/// stays attributable after this host stops using it.
///
/// Written once per re-mint and never read by the audit path — it exists for the operator
/// and for `doctor`'s `audit_health.host_previous`. A `tracing::warn!` alone would have been
/// cheaper and wrong: the party who needs this is triaging a mixed shard days later, not
/// watching the terminal at the moment of the transport. "Loudness is a property of a PATH,
/// not of a failure" (CLAUDE.md § *Testing Discipline*) — so the finding is persisted where a
/// later query reaches it, and the log line is the courtesy.
pub(crate) const PREV_HOST_META_KEY: &str = "audit_host_id_previous";

/// What [`sanitize`] returns when a name resolves to nothing usable.
///
/// Named rather than inlined because [`resolve_host_id`] must be able to RECOGNISE it: every
/// machine that cannot name itself sanitizes to this same string, so it is exactly the value
/// the transport check cannot discriminate on.
const FALLBACK_NAME: &str = "host";

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

/// The audit-shard directory under `repo_root`, joined one component at a time.
pub(crate) fn audit_dir(repo_root: &std::path::Path) -> std::path::PathBuf {
    AUDIT_DIR_PARTS
        .iter()
        .fold(repo_root.to_path_buf(), |acc, part| acc.join(part))
}

/// The same directory as a display string, always `/`-separated.
///
/// For JSON payloads, log lines and `.gitattributes` patterns — where the POSIX form is
/// the correct rendering on every platform and is what a reader will paste. Derived from
/// [`AUDIT_DIR_PARTS`] rather than written out again, so the two cannot drift: the old
/// code carried the literal twice, and a fix applied to only the joined copy would have
/// left the displayed one correct-looking and unrelated.
pub(crate) fn audit_dir_display() -> String {
    AUDIT_DIR_PARTS.join("/")
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
        FALLBACK_NAME.to_string()
    } else {
        trimmed
    }
}

/// Process-local atomic counter mixed into `suffix()`. Two calls in one
/// process share the pid and can share the nanosecond on a coarse clock —
/// that is both a flaky test and a real collision — so a monotonically
/// increasing counter is mixed in as a third, always-distinct source.
static MINT_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Captured once per process, not once per call: `suffix()`'s uniqueness
/// guarantee is that `MINT_COUNTER`'s contribution never collides with
/// itself, and that is only exact if every OTHER term in the mix (nanos, pid)
/// is held constant across calls being compared. Re-reading the clock per
/// call would make it merely probabilistic — a nanosecond term could in
/// principle vary in exactly the bits the counter also touches.
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
pub(crate) fn mint_host_id(candidate: &str) -> String {
    format!("{}-{}", sanitize(candidate), suffix())
}

/// The name half of an id built by [`mint_host_id`] — everything before the trailing
/// `-<6 lowercase hex>`. `None` when `id` does not have that shape.
///
/// **This is what lets the transport check need no new state and no migration.**
/// `mint_host_id` writes `sanitize(candidate) + "-" + suffix()`, so the sanitized machine
/// name *at mint time* is already carried inside the id. A catalog minted long before this
/// check existed can still be tested against the host now reading it — which is the only
/// case that matters, since the catalogs already in the field are exactly those.
///
/// Returning `None` rather than guessing is load-bearing: [`resolve_host_id`] treats an
/// unparseable id as "cannot discriminate" and leaves it alone. An id in a shape this
/// function does not recognise is a *future or hand-written* format, and re-minting over one
/// would destroy an identity the code does not understand.
///
/// The suffix test is exact — six characters, lowercase hex — because that is precisely what
/// [`suffix`] emits (`{:06x}` over a value masked to 24 bits). Accepting uppercase or a
/// variable length would widen this to match names that merely end in something hex-ish,
/// and `-cafe` is a plausible tail for a real host name.
fn minted_name(id: &str) -> Option<&str> {
    let (name, tail) = id.rsplit_once('-')?;
    let looks_minted = !name.is_empty()
        && tail.len() == 6
        && tail.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'));
    looks_minted.then_some(name)
}

/// Was `stored` minted by a machine other than the one whose sanitized name is `this_name`?
/// `Some(minted_name)` when that can be established positively; `None` when it cannot.
///
/// Pure, and split out from [`resolve_host_id`] for the reason [`mint_host_id`] already is:
/// the whole decision is testable without an environment or a catalog connection, so the
/// tests that matter never race on a process-global env var. `resolve_host_id` keeps only
/// the two impure ends — reading the machine name, and writing the meta rows.
///
/// **`None` is returned for three different reasons and they are not interchangeable:**
/// the host cannot name itself, the stored id is in an unrecognised format, or the id was
/// genuinely minted here. Only the last is "all is well"; the first two are "cannot tell",
/// and both deliberately resolve to *leave it alone*. Adopting silently is the defect this
/// exists to fix — but re-minting on a machine that cannot discriminate would churn a fresh
/// shard on every open, which is the same defect with the sign flipped.
fn foreign_mint<'a>(stored: &'a str, this_name: &str) -> Option<&'a str> {
    if this_name == FALLBACK_NAME {
        return None;
    }
    let minted = minted_name(stored)?;
    (minted != this_name).then_some(minted)
}

/// The stable id for the machine reading this catalog — read from `catalog_meta` if it was
/// minted **here**, otherwise minted fresh and persisted.
///
/// **The old contract said "this catalog's machine", and that phrase was the bug.** It
/// assumed a catalog belongs to one machine for life. It does not: this repo ships a
/// cross-machine catalog-integration design, and on 2026-09-04 a catalog was transported
/// wholesale onto a second host, which then silently adopted the sender's id and wrote its
/// own rows into the sender's monthly shard — a file tracked in git and declared
/// `merge=union`, so the mixing was published and a union merge folds both streams together
/// with nothing marking the seam. `shard_file_name`'s doc comment states the invariant that
/// was lost: *"host keeps two machines off each other's lines entirely."*
/// `docs/issues/2026-09-04-a-transported-catalog-carries-its-host-identity.md`.
///
/// **The check costs no new state.** [`minted_name`] recovers the sanitized machine name
/// from the stored id itself, so a catalog minted before this code existed is still
/// testable — which is the only population that matters, those being the catalogs already in
/// the field.
///
/// **It re-mints rather than refusing, and that is a deliberate rejection of the louder
/// option.** The bug file offered "refuse at open, naming both ids". A hard error is the
/// better report and the worse behaviour: the catalogs this fires on are *already* in the
/// mismatched state, so refusing bricks every tool call on that host until a human
/// intervenes. Re-minting costs at worst one spare shard file; refusing costs the running
/// system. The previous id is preserved in [`PREV_HOST_META_KEY`] so the report survives.
///
/// **What this does NOT do, stated because the bug file's Workarounds section reads
/// otherwise:** it does not recover rows already exported under the foreign id. The export
/// watermark is per-REPO (`shard::watermark`, keyed on repo path) and entirely independent of
/// the host id, so re-minting does not roll it back — those rows stay counted as exported and
/// never re-emit under the new id. Re-attributing them needs a deliberate watermark rollback,
/// which is an operator decision and not something this function should do unasked.
///
/// A renamed-but-not-transported host also trips this, and that is correct rather than
/// tolerated: for audit purposes a machine answering to a new name is a new identity, and a
/// fresh shard is the honest record of the change.
pub(crate) fn resolve_host_id(conn: &Connection) -> Result<String> {
    resolve_host_id_for(conn, &candidate_name())
}

/// [`resolve_host_id`] with the machine name injected.
///
/// The split exists so every branch above is reachable from a test **without touching a
/// process-global environment variable**. `candidate_name()` reads `CODESCOUT_AUDIT_HOST` and
/// friends; a test that set one would be mutating shared state under a parallel test runner,
/// and this repo already carries bug files about load-sensitive flakes. With the name as a
/// parameter the interesting cases — foreign, native, unparseable, unnameable — are ordinary
/// deterministic calls.
fn resolve_host_id_for(conn: &Connection, candidate: &str) -> Result<String> {
    let this_name = sanitize(candidate);

    if let Some(stored) = gc::get_meta(conn, HOST_META_KEY)? {
        let stored = stored.trim().to_string();
        if !stored.is_empty() {
            let Some(minted) = foreign_mint(&stored, &this_name) else {
                return Ok(stored);
            };
            let id = mint_host_id(candidate);
            tracing::warn!(
                stored_id = %stored,
                minted_by = %minted,
                this_host = %this_name,
                new_id = %id,
                "audit host id was minted by a different machine — re-minting. Rows this \
                 host already exported under the stored id remain in that shard; the export \
                 watermark is per-repo and does not roll back."
            );
            gc::set_meta(conn, PREV_HOST_META_KEY, &stored)?;
            gc::set_meta(conn, HOST_META_KEY, &id)?;
            return Ok(id);
        }
    }

    let id = mint_host_id(candidate);
    gc::set_meta(conn, HOST_META_KEY, &id)?;
    Ok(id)
}

/// The id [`resolve_host_id`] replaced, if it has ever re-minted on this catalog.
///
/// `None` is the ordinary state and means only that no transport was ever *detected* — not
/// that none happened. A catalog transported before this check shipped, whose stored id
/// happens to parse and happens to match the receiving host's name, is indistinguishable
/// from a native one and always will be.
///
/// Exists so the re-mint has a reader. `doctor` surfaces it as
/// `audit_health.host_previous`, which is the query a person triaging a mixed shard actually
/// runs — days after the `tracing::warn!` scrolled past in someone else's terminal.
pub(crate) fn previous_host_id(conn: &Connection) -> Result<Option<String>> {
    Ok(gc::get_meta(conn, PREV_HOST_META_KEY)?
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty()))
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
///
/// The host segment is allowlisted with the same `[a-z0-9-]` charset as
/// `sanitize()` — a parsed host becomes half of the `(host, seq)` row
/// identity and appears verbatim in a reader's `coverage` map, so an
/// unvalidated `..` or `/` here is not merely cosmetic. A name that fails
/// the allowlist is `None` (not-a-shard), never an error: it may simply be
/// a stray committed file, and this parser owes that escape the same way
/// it owes one for READMEs.
pub(crate) fn parse_shard_file_name(name: &str) -> Option<(String, String)> {
    let stem = name.strip_suffix(".jsonl")?;
    let (host, month) = stem.rsplit_once('-')?;
    if month.len() != 6 || !month.chars().all(|c| c.is_ascii_digit()) || host.is_empty() {
        return None;
    }
    if !host
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
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

    /// The suffix in these fixtures is exactly six lowercase hex characters because that is
    /// what `suffix()` emits. Shortening it, or using an uppercase digit, makes
    /// `minted_name` return `None` and turns every re-mint assertion below into a silent
    /// no-op that still passes — the id would read as "unrecognised format", which is a
    /// leave-it-alone case.
    const FIXTURE_SUFFIX: &str = "65e654";

    #[test]
    fn minted_name_accepts_only_the_shape_mint_host_id_emits() {
        assert_eq!(minted_name("ripper-65e654"), Some("ripper"));
        // A sanitized name may itself contain `-`; the split takes the LAST one, so a
        // hyphenated host name survives intact.
        assert_eq!(minted_name("build-box-2-abc123"), Some("build-box-2"));

        // Everything below is a leave-it-alone case, and each fails for its own reason.
        assert_eq!(minted_name("ripper"), None, "no suffix at all");
        assert_eq!(
            minted_name("ripper-65E654"),
            None,
            "uppercase is not our format"
        );
        assert_eq!(minted_name("ripper-65e65"), None, "five chars, not six");
        assert_eq!(minted_name("ripper-65e6544"), None, "seven chars, not six");
        assert_eq!(minted_name("ripper-65g654"), None, "`g` is not hex");
        assert_eq!(minted_name("-65e654"), None, "empty name half");
        // The reason the length test is exact rather than "ends in something hex-ish":
        // `-cafe` is a plausible tail for a real host name and must not be read as a mint.
        assert_eq!(
            minted_name("coffee-cafe"),
            None,
            "a hex-looking 4-char tail"
        );
    }

    /// The three `None` reasons are not interchangeable, and only one of them means
    /// "all is well". Pinning them together because a future simplification that collapsed
    /// them would still pass any test that only checked the foreign case.
    #[test]
    fn foreign_mint_distinguishes_cannot_tell_from_all_is_well() {
        let foreign = format!("ripper-{FIXTURE_SUFFIX}");

        assert_eq!(
            foreign_mint(&foreign, "archlinux"),
            Some("ripper"),
            "different machine, both names known — the only case that re-mints"
        );
        assert_eq!(
            foreign_mint(&foreign, "ripper"),
            None,
            "minted here: all is well"
        );
        assert_eq!(
            foreign_mint("hand-written-id", "archlinux"),
            None,
            "unrecognised format: cannot tell, so leave an identity we do not understand"
        );
        assert_eq!(
            foreign_mint(&foreign, FALLBACK_NAME),
            None,
            "this host cannot name itself, so EVERY machine looks like `host` and a \
             mismatch says nothing — re-minting here would churn a new shard on every open, \
             which is the defect with its sign flipped"
        );
    }

    /// The motivating case, end to end: a catalog carrying another machine's id is
    /// re-minted, and the id it replaced is preserved rather than discarded.
    ///
    /// Deterministic without touching the environment — `resolve_host_id_for` takes the
    /// machine name, so nothing here races a parallel test on a process-global env var.
    #[test]
    fn a_transported_catalog_is_re_minted_and_names_what_it_replaced() {
        let cat = Catalog::open_in_memory().unwrap();
        let foreign = format!("ripper-{FIXTURE_SUFFIX}");
        gc::set_meta(&cat.conn, HOST_META_KEY, &foreign).unwrap();

        let got = resolve_host_id_for(&cat.conn, "archlinux").unwrap();

        assert_ne!(got, foreign, "the sender's id must not be adopted");
        assert!(
            got.starts_with("archlinux-"),
            "the new id must name THIS host: {got}"
        );
        assert_eq!(
            previous_host_id(&cat.conn).unwrap().as_deref(),
            Some(foreign.as_str()),
            "the replaced id is what makes the already-written shard attributable \
             afterwards; dropping it would close the record while the rows stay stranded"
        );
        // Persisted, not merely returned: the next call must agree with this one, or the
        // host forks its own shard history across two filenames.
        assert_eq!(resolve_host_id_for(&cat.conn, "archlinux").unwrap(), got);
    }

    /// The control, and without it every assertion above is satisfied by a function that
    /// re-mints unconditionally. A native id must survive untouched AND leave no
    /// `host_previous` behind — a spurious one would make `doctor` report stranded rows on a
    /// healthy catalog.
    #[test]
    fn an_id_minted_here_is_returned_unchanged_and_records_no_previous() {
        let cat = Catalog::open_in_memory().unwrap();
        let native = format!("archlinux-{FIXTURE_SUFFIX}");
        gc::set_meta(&cat.conn, HOST_META_KEY, &native).unwrap();

        assert_eq!(
            resolve_host_id_for(&cat.conn, "archlinux").unwrap(),
            native,
            "same machine: nothing to do"
        );
        assert_eq!(
            previous_host_id(&cat.conn).unwrap(),
            None,
            "no re-mint happened, so nothing may claim one did"
        );
    }

    /// Both leave-it-alone cases, end to end. They are separate assertions rather than one
    /// parameterised loop because they are separate GUARDS: a change that dropped the
    /// unparseable check would leave the unnameable one passing, and vice versa.
    #[test]
    fn a_catalog_that_cannot_be_judged_is_left_exactly_as_found() {
        // Unrecognised id format.
        let cat = Catalog::open_in_memory().unwrap();
        gc::set_meta(&cat.conn, HOST_META_KEY, "legacy-hand-written").unwrap();
        assert_eq!(
            resolve_host_id_for(&cat.conn, "archlinux").unwrap(),
            "legacy-hand-written"
        );
        assert_eq!(previous_host_id(&cat.conn).unwrap(), None);

        // Host cannot name itself: `sanitize("")` is the shared fallback, so the comparison
        // has nothing to discriminate on.
        let cat = Catalog::open_in_memory().unwrap();
        let foreign = format!("ripper-{FIXTURE_SUFFIX}");
        gc::set_meta(&cat.conn, HOST_META_KEY, &foreign).unwrap();
        assert_eq!(resolve_host_id_for(&cat.conn, "").unwrap(), foreign);
        assert_eq!(previous_host_id(&cat.conn).unwrap(), None);
    }

    /// An empty or whitespace-only stored value is not an identity, and must mint rather
    /// than being treated as a foreign id. Pinned because the emptiness check sits BEFORE
    /// the foreign-mint branch and a reordering would send `""` down the re-mint path,
    /// writing a `host_previous` of `""` that `doctor` would then report as a real
    /// predecessor.
    #[test]
    fn a_blank_stored_id_mints_without_claiming_a_predecessor() {
        let cat = Catalog::open_in_memory().unwrap();
        gc::set_meta(&cat.conn, HOST_META_KEY, "   ").unwrap();

        let got = resolve_host_id_for(&cat.conn, "archlinux").unwrap();

        assert!(got.starts_with("archlinux-"), "minted fresh: {got}");
        assert_eq!(
            previous_host_id(&cat.conn).unwrap(),
            None,
            "a blank is an absence, not a machine that once owned this catalog"
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

    #[test]
    fn a_host_segment_outside_the_allowlist_is_not_a_shard() {
        // RULING 8: the host segment had no allowlist, so a stray committed
        // file like `..-202609.jsonl` parsed to host "..". That parsed value
        // becomes half of the (host, seq) row identity in the reader and
        // appears verbatim in its `coverage` map, so this is not cosmetic.
        // `None` (not-a-shard), never an error — it may simply be a stray file.
        assert!(parse_shard_file_name("..-202609.jsonl").is_none());
        assert!(parse_shard_file_name("../etc/passwd-202609.jsonl").is_none());
        assert!(parse_shard_file_name("ARCH-202609.jsonl").is_none());
        assert!(parse_shard_file_name("arch_a3f9c2-202609.jsonl").is_none());
    }

    /// No part of the audit directory may contain a path separator.
    ///
    /// **A hygiene guard, not a regression test — and the distinction is the point.** It
    /// was written believing the joined literal caused the 2026-09-02 Windows failures. It
    /// did not: the split shipped at `9a156bd4` and the next run failed identically, 21
    /// tests, same names, same lines. The real cause was `LockFileEx` refusing an
    /// append-only handle in `shard.rs`. So this test guards a property worth having and
    /// **guards no known defect**; do not credit it with coverage of the Windows lanes.
    /// See `docs/issues/archive/2026-09-02-lockfileex-refuses-an-append-only-handle-on-windows.md`.
    ///
    /// **Why it asserts on the PARTS and not on the built path.** The obvious test —
    /// "`audit_dir(root)` has two components below `root`" — passes on Unix whether the
    /// parts are split or not, because `/` IS the Unix separator and `join` normalises it
    /// away. Asserting on the input is what makes the property checkable at all on the
    /// platform everyone develops on. That reasoning survives the retraction above; only
    /// the claim about what it prevents does not.
    ///
    /// Mutation caught: collapsing `AUDIT_DIR_PARTS` back to a single
    /// `[".codescout/audit"]`, on any platform — verified 2026-09-02, this test reds and
    /// [`the_display_form_and_the_joined_path_agree`] stays green.
    #[test]
    fn audit_dir_parts_carry_no_separator() {
        for part in AUDIT_DIR_PARTS {
            assert!(
                !part.contains('/') && !part.contains('\\'),
                "AUDIT_DIR_PARTS entry {part:?} contains a path separator. Path::join takes \
                 it as ONE component, so create_dir_all never creates the parent and every \
                 Windows lane reds with ERROR_PATH_NOT_FOUND while Linux stays green. Split \
                 it into separate entries instead."
            );
            assert!(
                !part.is_empty(),
                "an empty part would silently vanish from the joined path"
            );
        }
    }

    /// The joined path and the displayed string must describe the same directory.
    ///
    /// They were two independent literals before 2026-09-02, which is how a fix could have
    /// landed on one and left the other reading correctly about a different place. Now the
    /// display form is derived, and this pins that it stays derived rather than re-forked.
    #[test]
    fn the_display_form_and_the_joined_path_agree() {
        let root = std::path::Path::new("/tmp/x");
        let joined = audit_dir(root);
        let tail: Vec<String> = joined
            .strip_prefix(root)
            .expect("audit_dir must build under the root it was given")
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();

        assert_eq!(
            tail,
            vec![".codescout".to_string(), "audit".to_string()],
            "the joined path must descend exactly two real components below the root"
        );
        assert_eq!(
            audit_dir_display(),
            tail.join("/"),
            "the display string must be the same components, POSIX-rendered"
        );
    }
}
