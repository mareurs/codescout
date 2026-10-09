//! The pool's byte budget: `total_bytes` never passes the limit, whole-file entries are demoted
//! before anything whose text exists nowhere else is evicted, and an entry bigger than the whole
//! limit is never stored as is. Every test runs at the floor limit (`MIN_POOL_BYTES`, 1 MiB)
//! with sizes scaled to it, and checks `assert_accounting` after each step: a counter that
//! drifts from the entries is the defect a budget can hide behind a green ceiling check.

use super::*;
use std::path::Path;
use std::time::{Duration, SystemTime};

// cap-class: NOT_A_CAP — test-only alias of MIN_POOL_BYTES that sizes the pools these tests build; it bounds no runtime path
const LIMIT: usize = MIN_POOL_BYTES;

fn pool() -> OutputBuffer {
    OutputBuffer::with_byte_limit(50, LIMIT)
}

/// Exactly `bytes` of text in numbered lines (`tag-0000001`, …), so a line names its number.
fn numbered(tag: &str, bytes: usize) -> String {
    let mut s = String::with_capacity(bytes + 32);
    let mut i = 1;
    while s.len() < bytes {
        s.push_str(&format!("{tag}-{i:07}\n"));
        i += 1;
    }
    s.truncate(bytes);
    s
}

/// A command entry of exactly `bytes` of stdout.
fn cmd(buf: &OutputBuffer, tag: &str, bytes: usize) -> String {
    buf.store(tag.into(), numbered(tag, bytes), String::new(), 0)
}

/// Write `text` to `dir/name` with its mtime `secs_ago` seconds in the past. Explicit mtimes,
/// so a rewrite inside one clock tick is still a later mtime.
fn write(dir: &Path, name: &str, text: &str, secs_ago: u64) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    let t = SystemTime::now() - Duration::from_secs(secs_ago);
    filetime::set_file_mtime(&p, filetime::FileTime::from_system_time(t)).unwrap();
    p
}

/// Read `path` the way `read_file` does: stat, read, store under the file's one handle.
fn read_into(buf: &OutputBuffer, path: &Path) -> String {
    let mtime = file_mtime_ms(path);
    let text = std::fs::read_to_string(path).unwrap();
    buf.store_file_read(&path.to_string_lossy(), &text, mtime)
}

/// Many mixed stores (commands, tool results, whole files, snapshots) past both the byte limit
/// and the 50-entry cap. The ceiling alone is monotone under removal (an empty pool holds it),
/// so the test also requires the last round to be held whole and the pool to stay in use.
#[test]
fn the_pool_never_holds_more_than_its_byte_limit_across_mixed_stores() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let mut last = Vec::new();
    let mut newest_file = (String::new(), std::path::PathBuf::new());
    for i in 0..40 {
        let c_len = 90_000 + i * 1_000;
        let c = cmd(&buf, &format!("c{i}"), c_len);
        buf.assert_accounting();
        let t = buf.store_tool("t", numbered(&format!("t{i}"), 70_000));
        buf.assert_accounting();
        let p = write(
            dir.path(),
            &format!("f{i}"),
            &numbered(&format!("f{i}"), 120_000),
            100,
        );
        let f = read_into(&buf, &p);
        buf.assert_accounting();
        let s = buf.store_file(
            format!("@memory:m{i}:filtered"),
            numbered(&format!("m{i}"), 30_000),
        );
        buf.assert_accounting();
        last = vec![(c, c_len), (t, 70_000), (s, 30_000)];
        newest_file = (f, p);
    }
    assert!(buf.entry_count() <= 50, "{} entries", buf.entry_count());
    let held: usize = buf.handles().iter().map(|h| buf.bytes_of(h).unwrap()).sum();
    assert_eq!(held, buf.total_bytes(), "the counter is the real sum");
    assert!(buf.total_bytes() <= LIMIT, "{} held", buf.total_bytes());
    for (h, n) in &last {
        assert_eq!(
            buf.bytes_of(h),
            Some(*n),
            "{h}: the newest round is held whole"
        );
    }
    // The newest file may be demoted: a later store demotes files before it evicts any command
    // (ruling 2), however recently the file was read. Its handle still serves it.
    let (f, p) = newest_file;
    assert_eq!(
        buf.get(&f).unwrap().stdout,
        std::fs::read_to_string(&p).unwrap()
    );
    buf.assert_accounting();
    assert!(
        buf.total_bytes() > LIMIT / 2,
        "the pool was drained, not budgeted: {} held",
        buf.total_bytes()
    );
}

/// Ruling 2, both halves: a store over the limit demotes a whole file even when an OLDER
/// command is the least recently used entry; only once no file is left to demote is the least
/// recently used command evicted.
#[test]
fn a_store_over_the_limit_demotes_a_file_before_it_evicts_an_older_command() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let a = cmd(&buf, "a", 400_000);
    let p = write(dir.path(), "f", &numbered("f", 400_000), 100);
    let f = read_into(&buf, &p);
    let b = cmd(&buf, "b", 400_000);
    buf.assert_accounting();
    assert_eq!(
        buf.bytes_of(&a),
        Some(400_000),
        "the older command must survive: the file is demoted first"
    );
    assert!(buf.is_demoted(&f));
    assert_eq!(buf.bytes_of(&f), Some(0));
    assert_eq!(buf.bytes_of(&b), Some(400_000));

    let c = cmd(&buf, "c", 400_000);
    buf.assert_accounting();
    assert_eq!(
        buf.bytes_of(&a),
        None,
        "no file left to demote: the least recently used command goes"
    );
    assert_eq!(buf.bytes_of(&b), Some(400_000));
    assert_eq!(buf.bytes_of(&c), Some(400_000));
    assert!(buf.is_demoted(&f), "the demoted handle stays live");
}

/// Demotion follows LRU among files: the file read most recently keeps its text.
#[test]
fn demotion_takes_the_least_recently_used_file_first() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let f1 = read_into(
        &buf,
        &write(dir.path(), "f1", &numbered("f1", 300_000), 100),
    );
    let f2 = read_into(
        &buf,
        &write(dir.path(), "f2", &numbered("f2", 300_000), 100),
    );
    buf.get(&f1).unwrap();
    cmd(&buf, "x", 500_000);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f2), "f2 is the least recently used file");
    assert!(!buf.is_demoted(&f1));
    assert_eq!(buf.bytes_of(&f1), Some(300_000));
}

/// A demoted handle still serves the file, line for line, from disk, and the read puts nothing
/// back into the pool. Its refresh notice works as a resident entry's does: a change is reported
/// once, by the reporting read, even when a silent `get` saw it first.
#[test]
fn a_demoted_file_is_read_from_disk_and_the_read_does_not_grow_the_pool() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let text1 = numbered("v1", 300_000);
    let p = write(dir.path(), "f", &text1, 100);
    let f = read_into(&buf, &p);
    cmd(&buf, "x", 800_000);
    assert!(buf.is_demoted(&f));
    let before = buf.total_bytes();

    let e = buf.get(&f).unwrap();
    assert_eq!(e.stdout, text1);
    assert_eq!(e.stdout.lines().nth(999), text1.lines().nth(999));
    assert_eq!(buf.total_bytes(), before, "a read must not grow the pool");
    assert!(buf.is_demoted(&f));
    let (_, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(!refreshed, "an unchanged file reports no refresh");

    let text2 = numbered("v2", 310_000);
    write(dir.path(), "f", &text2, 50);
    let (e, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    assert_eq!(e.stdout, text2);
    assert!(refreshed, "the holder is told the file changed");
    let (_, again) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(!again, "and told once");

    let text3 = numbered("v3", 290_000);
    write(dir.path(), "f", &text3, 40);
    assert_eq!(buf.get(&f).unwrap().stdout, text3);
    let (_, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(
        refreshed,
        "a silent get must not swallow the holder's notice"
    );

    assert_eq!(buf.total_bytes(), before);
    assert!(buf.is_demoted(&f));
    buf.assert_accounting();
}

/// The stale-write rule holds for a demoted entry: a late store of text read under an older
/// mtime is refused and leaves it demoted (taken, it would put the stale text back into the
/// pool). A fresh path read takes the text back, within the limit.
#[test]
fn a_stale_store_leaves_a_demoted_entry_demoted_and_a_fresh_read_takes_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let text1 = numbered("v1", 200_000);
    let p = write(dir.path(), "f", &text1, 100);
    let t1 = file_mtime_ms(&p);
    let f = read_into(&buf, &p);
    let text2 = numbered("v2", 200_000);
    write(dir.path(), "f", &text2, 50);
    assert_eq!(read_into(&buf, &p), f);
    cmd(&buf, "x", 900_000);
    assert!(buf.is_demoted(&f));

    assert_eq!(buf.store_file_read(&p.to_string_lossy(), &text1, t1), f);
    assert!(
        buf.is_demoted(&f),
        "a store of text read under an older mtime is refused"
    );
    buf.assert_accounting();

    assert_eq!(read_into(&buf, &p), f);
    assert!(!buf.is_demoted(&f), "a fresh read takes the text back");
    assert_eq!(buf.bytes_of(&f), Some(text2.len()));
    assert_eq!(buf.get(&f).unwrap().stdout, text2);
    buf.assert_accounting();
}

/// Ruling 3 for a command: output bigger than the whole limit is a marked prefix of WHOLE lines
/// that uses the room, its line N is still line N, and the record, the in-buffer marker and the
/// notice every later read carries state the same numbers.
#[test]
fn an_oversized_command_is_a_marked_prefix_whose_lines_keep_their_numbers() {
    let buf = pool();
    let out: String = (1..=200_000).map(|i| format!("{i}\n")).collect();
    assert!(out.len() > LIMIT);
    let id = buf.store("seq".into(), out, String::new(), 0);
    buf.assert_accounting();

    let e = buf.get(&id).unwrap();
    let t = e
        .truncated
        .expect("an oversized command is recorded as cut");
    assert_eq!(t.total_lines, 200_000);
    assert!(t.kept_lines > 160_000 && t.kept_lines < 200_000, "{t:?}");
    let lines: Vec<&str> = e.stdout.lines().collect();
    assert_eq!(
        lines.len(),
        t.kept_lines + 1,
        "the kept lines and the marker"
    );
    for n in [1, t.kept_lines / 2, t.kept_lines] {
        assert_eq!(lines[n - 1], n.to_string(), "line {n}");
    }
    let counts = format!(
        "{} of 200000 lines kept, {} omitted",
        t.kept_lines,
        200_000 - t.kept_lines
    );
    let marker = lines.last().unwrap();
    assert!(marker.starts_with(TRUNCATION_SENTINEL_PREFIX), "{marker}");
    assert!(marker.contains(&counts), "{marker}");
    let notice = buf.truncation_notice(&id).expect("every read is told");
    assert!(notice.contains(&counts), "{notice}");

    let held = buf.bytes_of(&id).unwrap();
    assert!(held <= LIMIT, "{held}");
    assert!(
        held > LIMIT - 1_000,
        "the prefix uses the room, not a fraction of it: {held}"
    );
}

/// The pool cuts `stderr` too. A cut `stderr` is recorded on its own, and reported by the
/// `.err` handle and by the notices for the bare handle, never by the complete `stdout`. With
/// both streams over, `stderr` keeps at most half the limit.
#[test]
fn an_oversized_stderr_is_cut_and_its_err_handle_says_so() {
    let buf = pool();
    let id = buf.store(
        "x".into(),
        numbered("o", 10_000),
        numbered("e", 1_500_000),
        1,
    );
    buf.assert_accounting();
    let e = buf.get(&id).unwrap();
    assert!(e.truncated.is_none());
    assert_eq!(e.stdout.len(), 10_000);
    assert!(e.stderr_truncated.is_some());
    assert!(e
        .stderr
        .lines()
        .last()
        .unwrap()
        .starts_with(TRUNCATION_SENTINEL_PREFIX));
    assert!(buf.truncation_notice(&id).is_none(), "stdout is complete");
    assert!(buf.truncation_notice(&format!("{id}.err")).is_some());
    let notices = buf.truncation_notices_in(&id);
    assert_eq!(notices.len(), 1, "{notices:?}");
    assert!(
        notices[0].starts_with(&format!("⚠ {id}.err ")),
        "{notices:?}"
    );
    let held = buf.bytes_of(&id).unwrap();
    assert!(held <= LIMIT && held > LIMIT - 1_000, "{held}");

    let both = buf.store(
        "y".into(),
        numbered("o", 1_500_000),
        numbered("e", 1_500_000),
        1,
    );
    buf.assert_accounting();
    let e = buf.get(&both).unwrap();
    assert!(e.truncated.is_some() && e.stderr_truncated.is_some());
    assert!(
        e.stderr.len() <= LIMIT / 2 && e.stderr.len() > LIMIT / 2 - 1_000,
        "stderr keeps half the limit, no more and not much less: {}",
        e.stderr.len()
    );
    let held = buf.bytes_of(&both).unwrap();
    assert!(held <= LIMIT && held > LIMIT - 2_000, "{held}");
    assert_eq!(buf.truncation_notices_in(&both).len(), 2);
}

/// The budget edge is inclusive: two halves fit together, an entry of exactly the limit is
/// stored whole (and, the known limit, leaves nothing else holding text), and one byte more is
/// cut.
#[test]
fn the_budget_edge_admits_an_entry_of_exactly_the_limit() {
    let buf = pool();
    let a = buf.store("a".into(), "a".repeat(LIMIT / 2), String::new(), 0);
    let b = buf.store("b".into(), "b".repeat(LIMIT / 2), String::new(), 0);
    buf.assert_accounting();
    assert_eq!(
        buf.bytes_of(&a),
        Some(LIMIT / 2),
        "a full pool is not over it"
    );
    assert_eq!(buf.total_bytes(), LIMIT);

    let c = buf.store("c".into(), "c".repeat(LIMIT), String::new(), 0);
    buf.assert_accounting();
    let e = buf.get(&c).unwrap();
    assert!(e.truncated.is_none(), "exactly the limit is stored whole");
    assert_eq!(e.stdout.len(), LIMIT);
    assert_eq!(buf.bytes_of(&a), None);
    assert_eq!(buf.bytes_of(&b), None);

    let d = buf.store("d".into(), numbered("d", LIMIT + 1), String::new(), 0);
    buf.assert_accounting();
    assert!(
        buf.get(&d).unwrap().truncated.is_some(),
        "one byte over is cut"
    );
    assert!(buf.bytes_of(&d).unwrap() <= LIMIT);
}

/// The same inclusive edge for every store kind and for an in-place change: a `stderr`, a tool
/// result, a whole file and a snapshot of exactly the limit are held whole, a file that grows by
/// a path read to exactly the limit stays resident, and a refresh that exactly fills the room left
/// stays resident.
#[test]
fn the_budget_edge_holds_for_every_store_kind() {
    let buf = pool();
    let id = buf.store("e".into(), String::new(), "e".repeat(LIMIT), 1);
    let e = buf.get(&id).unwrap();
    assert!(
        e.stderr_truncated.is_none(),
        "a stderr of the limit is whole"
    );
    assert_eq!(e.stderr.len(), LIMIT);

    let buf = pool();
    let id = buf.store_tool("t", "t".repeat(LIMIT));
    let e = buf.get(&id).unwrap();
    assert!(e.truncated.is_none(), "a tool result of the limit is whole");
    assert_eq!(e.stdout.len(), LIMIT);

    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let f = read_into(&buf, &write(dir.path(), "full", &"f".repeat(LIMIT), 100));
    assert!(!buf.is_demoted(&f), "a file of the limit is held");
    assert_eq!(buf.bytes_of(&f), Some(LIMIT));
    let s = buf.store_file("@memory:full:filtered".into(), "s".repeat(LIMIT));
    assert!(
        buf.get(&s).unwrap().truncated.is_none(),
        "a snapshot of the limit is whole"
    );
    buf.assert_accounting();

    let buf = pool();
    let p = write(dir.path(), "grows", "small\n", 100);
    let g = read_into(&buf, &p);
    cmd(&buf, "x", 500_000);
    write(dir.path(), "grows", &"g".repeat(LIMIT), 90);
    assert_eq!(read_into(&buf, &p), g);
    assert!(
        !buf.is_demoted(&g),
        "growth by a path read to the limit stays resident"
    );
    assert_eq!(buf.total_bytes(), LIMIT);
    buf.assert_accounting();

    let buf = pool();
    let p = write(dir.path(), "refresh", &"r".repeat(10_000), 100);
    let r = read_into(&buf, &p);
    cmd(&buf, "y", LIMIT - 300_000);
    write(dir.path(), "refresh", &"R".repeat(300_000), 90);
    assert_eq!(buf.get(&r).unwrap().stdout.len(), 300_000);
    assert!(
        !buf.is_demoted(&r),
        "a refresh that exactly fills the room left stays resident"
    );
    assert_eq!(buf.total_bytes(), LIMIT);
    buf.assert_accounting();
}

/// Ruling 3 for a whole file: bigger than the limit, its handle holds no text from birth and
/// displaces nothing; every read takes the file from disk.
#[test]
fn an_oversized_file_keeps_no_text_and_displaces_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let keep = cmd(&buf, "k", 900_000);
    let text = numbered("big", 1_500_000);
    let p = write(dir.path(), "big", &text, 100);
    let f = read_into(&buf, &p);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f));
    assert_eq!(buf.bytes_of(&f), Some(0));
    assert_eq!(buf.bytes_of(&keep), Some(900_000), "nothing was displaced");

    assert_eq!(buf.get(&f).unwrap().stdout, text);
    assert_eq!(read_into(&buf, &p), f, "one file, one handle");
    assert!(buf.is_demoted(&f));
    assert_eq!(buf.total_bytes(), 900_000);
    buf.assert_accounting();
}

/// The counter follows every in-place change of a file's text: growth by a path read (which
/// makes room, here by evicting the command, as no other file is left to demote), shrinking,
/// a refresh by a read that fits, and one that does not fit the room left, which demotes the
/// entry and displaces nothing.
#[test]
fn in_place_changes_of_a_file_keep_the_accounting() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "f", &numbered("v1", 100_000), 100);
    let f = read_into(&buf, &p);
    let c = cmd(&buf, "c", 800_000);
    buf.assert_accounting();

    write(dir.path(), "f", &numbered("v2", 400_000), 90);
    assert_eq!(read_into(&buf, &p), f);
    buf.assert_accounting();
    assert_eq!(buf.bytes_of(&f), Some(400_000));
    assert_eq!(buf.bytes_of(&c), None, "room was made for the growth");
    assert_eq!(buf.total_bytes(), 400_000);

    write(dir.path(), "f", &numbered("v3", 10_000), 80);
    assert_eq!(read_into(&buf, &p), f);
    buf.assert_accounting();
    assert_eq!(buf.total_bytes(), 10_000);

    let v4 = numbered("v4", 50_000);
    write(dir.path(), "f", &v4, 70);
    assert_eq!(buf.get(&f).unwrap().stdout, v4);
    buf.assert_accounting();
    assert!(!buf.is_demoted(&f), "a refresh that fits stays resident");
    assert_eq!(buf.total_bytes(), 50_000);

    let c2 = cmd(&buf, "c2", 900_000);
    let v5 = numbered("v5", 300_000);
    write(dir.path(), "f", &v5, 60);
    assert_eq!(buf.get(&f).unwrap().stdout, v5);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f), "the refresh did not fit the room left");
    assert_eq!(buf.bytes_of(&c2), Some(900_000), "a read displaces nothing");
    assert_eq!(buf.total_bytes(), 900_000);
}

/// A setting can lower the limit and never raise it past 128 MiB; below the floor it is raised
/// to the floor. The lowered limit is the one enforced.
#[test]
fn the_byte_limit_clamps_at_128_mib_and_lowers() {
    assert_eq!(MAX_POOL_BYTES, 128 * 1024 * 1024);
    assert_eq!(OutputBuffer::new(50).max_bytes(), MAX_POOL_BYTES);
    assert_eq!(
        OutputBuffer::with_byte_limit(50, usize::MAX).max_bytes(),
        MAX_POOL_BYTES
    );
    assert_eq!(
        OutputBuffer::with_byte_limit(50, MAX_POOL_BYTES + 1).max_bytes(),
        MAX_POOL_BYTES
    );
    assert_eq!(
        OutputBuffer::with_byte_limit(50, 2 * LIMIT).max_bytes(),
        2 * LIMIT
    );
    assert_eq!(
        OutputBuffer::with_byte_limit(50, 0).max_bytes(),
        MIN_POOL_BYTES
    );

    let out = numbered("o", 1_500_000);
    let two = OutputBuffer::with_byte_limit(50, 2 * LIMIT);
    let id = two.store("o".into(), out.clone(), String::new(), 0);
    assert!(
        two.get(&id).unwrap().truncated.is_none(),
        "whole under 2 MiB"
    );
    let one = pool();
    let id = one.store("o".into(), out, String::new(), 0);
    assert!(one.get(&id).unwrap().truncated.is_some(), "cut under 1 MiB");
}

/// An oversized tool result is cut in the line space its readers number
/// (`line_addressable_text`), so line N of the prefix is line N of the whole result, and the
/// same result stored again is the same handle.
#[test]
fn an_oversized_tool_result_keeps_the_line_numbers_its_readers_use() {
    let buf = pool();
    let rows: Vec<String> = (0..60_000).map(|i| format!("row {i:06}\nsecond")).collect();
    let json = serde_json::json!({ "rows": rows }).to_string();
    assert!(json.len() > LIMIT);
    let id = buf.store_tool("probe", json.clone());
    buf.assert_accounting();
    let e = buf.get(&id).unwrap();
    let t = e.truncated.expect("cut");
    let whole = line_addressable_text(&id, json.clone());
    assert_eq!(t.total_lines, whole.lines().count());
    let kept: Vec<&str> = e.stdout.lines().collect();
    for n in [1, 2, 3, t.kept_lines / 2, t.kept_lines] {
        assert_eq!(Some(kept[n - 1]), whole.lines().nth(n - 1), "line {n}");
    }
    assert_eq!(
        line_addressable_text(&id, e.stdout.clone()),
        e.stdout,
        "readers number the stored prefix as it is"
    );
    assert!(buf.bytes_of(&id).unwrap() <= LIMIT);
    assert_eq!(
        buf.store_tool("probe", json),
        id,
        "dedup by the whole result"
    );
}

/// A line longer than the room is dropped whole, never split; and re-cutting a capture that was
/// already cut keeps counting the whole capture and leaves exactly one marker.
#[test]
fn a_cut_keeps_whole_lines_and_counts_the_whole_capture() {
    let (stored, t) = marked_prefix(&"x".repeat(5_000), 4_000, 1);
    assert_eq!(t.kept_lines, 0);
    assert_eq!(stored, format!("{}\n", truncation_marker(t)));

    let buf = pool();
    let precut = Truncation {
        kept_lines: 150_000,
        total_lines: 1_000_000,
    };
    let text = format!(
        "{}{}\n",
        numbered("p", 1_500_000),
        truncation_marker(precut)
    );
    let id = buf.store_truncated("p".into(), text, String::new(), 0, Some(precut));
    buf.assert_accounting();
    let e = buf.get(&id).unwrap();
    assert_eq!(e.truncated.unwrap().total_lines, 1_000_000);
    let markers = e
        .stdout
        .lines()
        .filter(|l| l.starts_with(TRUNCATION_SENTINEL_PREFIX))
        .count();
    assert_eq!(markers, 1);
}
