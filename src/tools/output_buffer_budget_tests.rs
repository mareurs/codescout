//! The pool's byte budget: `total_bytes` never passes the limit, one entry holds at most half of
//! it, whole-file entries are demoted before anything whose text exists nowhere else is evicted,
//! the text of a whole file never evicts anything, and an entry bigger than its share is never
//! stored as is. Every test runs at the floor limit (`MIN_POOL_BYTES`, 1 MiB, so one entry holds
//! 512 KiB) with sizes scaled to it, and checks `assert_accounting` after each step: a counter
//! that drifts from the entries is the defect a budget can hide behind a green ceiling check.

use super::*;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::time::{Duration, SystemTime};

// cap-class: NOT_A_CAP — test-only alias of MIN_POOL_BYTES that sizes the pools these tests build; it bounds no runtime path
const LIMIT: usize = MIN_POOL_BYTES;
// cap-class: NOT_A_CAP — test-only alias of one entry's share at LIMIT; it bounds no runtime path
const SHARE: usize = LIMIT / 2;

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

/// Two commands of 500 KB: 1,000,000 bytes, which leaves 48,576 of the 1 MiB limit, so any
/// resident file of more than that is demoted to make room for them.
fn squeeze(buf: &OutputBuffer, tag: &str) -> (String, String) {
    (
        cmd(buf, &format!("{tag}1"), 500_000),
        cmd(buf, &format!("{tag}2"), 500_000),
    )
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
    assert!(
        buf.total_bytes() > LIMIT / 2,
        "the pool was drained, not budgeted: {} held",
        buf.total_bytes()
    );
    buf.assert_accounting();
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

/// Fix 1 (review probe RP2): a path read of an unchanged demoted file must not evict a command
/// to hold text that is already on disk. The file stays demoted and both commands survive.
#[test]
fn a_path_read_of_a_demoted_file_never_evicts_a_command() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "f", &numbered("f", 400_000), 100);
    let f = read_into(&buf, &p);
    let (c1, c2) = squeeze(&buf, "c");
    assert!(buf.is_demoted(&f));

    assert_eq!(read_into(&buf, &p), f);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f), "no room without evicting a command");
    assert_eq!(buf.bytes_of(&c1), Some(500_000), "the command survives");
    assert_eq!(buf.bytes_of(&c2), Some(500_000));
}

/// Fix 1 (review probe RP1): a NEW file read into a pool full of commands is minted demoted;
/// its handle still serves the file.
#[test]
fn a_new_file_read_into_a_full_pool_is_minted_demoted() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let (c1, c2) = squeeze(&buf, "c");
    let text = numbered("f", 300_000);
    let f = read_into(&buf, &write(dir.path(), "f", &text, 100));
    buf.assert_accounting();
    assert!(buf.is_demoted(&f));
    assert_eq!(buf.bytes_of(&c1), Some(500_000));
    assert_eq!(buf.bytes_of(&c2), Some(500_000));
    assert_eq!(buf.get(&f).unwrap().stdout, text);
}

/// A path read of a demoted file takes its text back by demoting another FILE when that makes
/// room, never by evicting a command. And the stale-write rule holds for a demoted entry: a late
/// store of text read under an older mtime is refused and leaves it demoted (taken, it would
/// bring the stale text back into the pool).
#[test]
fn a_stale_store_is_refused_and_a_fresh_read_takes_the_text_back_from_another_file() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let text1 = numbered("v1", 100_000);
    let p = write(dir.path(), "f", &text1, 100);
    let t1 = file_mtime_ms(&p);
    let f = read_into(&buf, &p);
    let text2 = numbered("v2", 100_000);
    write(dir.path(), "f", &text2, 50);
    assert_eq!(read_into(&buf, &p), f);
    let g = read_into(&buf, &write(dir.path(), "g", &numbered("g", 300_000), 100));
    let x = cmd(&buf, "x", 500_000);
    let y = cmd(&buf, "y", 200_000);
    assert!(buf.is_demoted(&f), "f was the least recently used file");
    assert!(!buf.is_demoted(&g));

    assert_eq!(buf.store_file_read(&p.to_string_lossy(), &text1, t1), f);
    assert!(
        buf.is_demoted(&f),
        "a store of text read under an older mtime is refused"
    );
    buf.assert_accounting();

    assert_eq!(read_into(&buf, &p), f);
    buf.assert_accounting();
    assert!(!buf.is_demoted(&f), "a fresh read takes the text back");
    assert_eq!(buf.bytes_of(&f), Some(text2.len()));
    assert!(buf.is_demoted(&g), "by demoting the other file");
    assert_eq!(buf.bytes_of(&x), Some(500_000), "no command was evicted");
    assert_eq!(buf.bytes_of(&y), Some(200_000));
    assert_eq!(buf.get(&f).unwrap().stdout, text2);
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
    squeeze(&buf, "x");
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

/// A demoted file that is demoted-over again (more stores squeeze the pool) is not taken for
/// changed: its hash is the text it last served, untouched by the squeeze.
#[test]
fn a_demoted_file_survives_a_second_squeeze_without_a_false_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let text = numbered("f", 300_000);
    let f = read_into(&buf, &write(dir.path(), "f", &text, 100));
    squeeze(&buf, "a");
    assert!(buf.is_demoted(&f));
    squeeze(&buf, "b");
    let (e, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    assert_eq!(e.stdout, text);
    assert!(!refreshed, "nothing changed on disk");
    buf.assert_accounting();
}

/// A resident file whose new text does not fit the room left is demoted by the read, and the
/// change is reported once.
#[test]
fn a_refresh_that_does_not_fit_reports_once() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "f", &numbered("v1", 100_000), 100);
    let f = read_into(&buf, &p);
    cmd(&buf, "a", 500_000);
    cmd(&buf, "b", 400_000);
    let v2 = numbered("v2", 300_000);
    write(dir.path(), "f", &v2, 50);
    let (e, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    assert_eq!(e.stdout, v2);
    assert!(refreshed);
    assert!(buf.is_demoted(&f), "the new text did not fit the room left");
    let (_, again) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(!again, "reported once");
    buf.assert_accounting();
}

/// A refresh by a read keeps the new text only in the room left: it demotes no OTHER file to
/// make room (a store may; a read displaces nothing).
#[test]
fn a_refresh_by_a_read_demotes_no_other_file() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let g = read_into(&buf, &write(dir.path(), "g", &numbered("g", 300_000), 100));
    let p = write(dir.path(), "f", &numbered("v1", 100_000), 100);
    let f = read_into(&buf, &p);
    cmd(&buf, "x", 500_000);
    let v2 = numbered("v2", 400_000);
    write(dir.path(), "f", &v2, 50);
    assert_eq!(buf.get(&f).unwrap().stdout, v2);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f), "the new text did not fit the room left");
    assert!(!buf.is_demoted(&g), "and no other file was demoted for it");
}

/// A store records the hash of the text it stored, so the same text stored again is not taken
/// for a change and marks no second refresh.
#[test]
fn a_store_records_the_hash_of_what_it_stored() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "f", &numbered("v1", 100_000), 100);
    let f = read_into(&buf, &p);
    write(dir.path(), "f", &numbered("v2", 100_000), 50);
    assert_eq!(read_into(&buf, &p), f);
    let (_, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(refreshed, "the store changed the text");
    assert_eq!(read_into(&buf, &p), f);
    let (_, again) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(!again, "the same text stored again is no change");
}

/// A path read of an unchanged demoted file marks no refresh: nothing a holder saw changed.
#[test]
fn an_unchanged_path_read_of_a_demoted_file_marks_no_refresh() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "f", &numbered("f", 300_000), 100);
    let f = read_into(&buf, &p);
    squeeze(&buf, "a");
    assert!(buf.is_demoted(&f));
    assert_eq!(read_into(&buf, &p), f);
    let (_, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(!refreshed);
}

/// A file born demoted (bigger than one entry's share) reports no refresh while it is unchanged,
/// however often it is read.
#[test]
fn an_oversized_file_reports_no_refresh_while_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "big", &numbered("big", 1_500_000), 100);
    let f = read_into(&buf, &p);
    assert!(buf.is_demoted(&f));
    for _ in 0..2 {
        let (_, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
        assert!(!refreshed);
    }
    assert_eq!(read_into(&buf, &p), f);
    let (_, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(!refreshed);
}

/// Fix 4 (review probe RP8): a read of a demoted file touches the disk with the pool UNLOCKED.
/// The read is held at the disk by a hook while another thread stores; the store must finish
/// before the read is let go. With the disk read under the lock the store would wait, and the
/// timeout below reds.
#[test]
fn a_store_completes_while_a_demoted_file_is_being_read_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let text = numbered("big", 1_500_000);
    let f = read_into(&buf, &write(dir.path(), "big", &text, 100));
    assert!(buf.is_demoted(&f));

    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let entered_tx = std::sync::Mutex::new(entered_tx);
    let release_rx = std::sync::Mutex::new(release_rx);
    buf.set_disk_hook(Some(Arc::new(move || {
        let _ = entered_tx.lock().unwrap().send(());
        let _ = release_rx
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(30));
    })));
    let reader = {
        let buf = buf.clone();
        let f = f.clone();
        std::thread::spawn(move || buf.get(&f).map(|e| e.stdout))
    };
    entered_rx
        .recv_timeout(Duration::from_secs(30))
        .expect("the read reached the disk");
    let (done_tx, done_rx) = mpsc::channel();
    let writer = {
        let buf = buf.clone();
        std::thread::spawn(move || {
            let _ = done_tx.send(buf.store("small".into(), "ok\n".into(), String::new(), 0));
        })
    };
    let stored = done_rx.recv_timeout(Duration::from_secs(5));
    release_tx.send(()).unwrap();
    assert!(
        stored.is_ok(),
        "a store must not wait for another thread's disk read"
    );
    assert_eq!(reader.join().unwrap(), Some(text));
    writer.join().unwrap();
    buf.set_disk_hook(None);
    buf.assert_accounting();
}

/// Ruling 3 for a command: output bigger than one entry's share is a marked prefix of WHOLE
/// lines that uses the share, its line N is still line N, and the record, the in-buffer marker
/// and the notice every later read carries state the same numbers.
#[test]
fn an_oversized_command_is_a_marked_prefix_whose_lines_keep_their_numbers() {
    let buf = pool();
    let out: String = (1..=200_000).map(|i| format!("{i}\n")).collect();
    assert!(out.len() > SHARE);
    let id = buf.store("seq".into(), out, String::new(), 0);
    buf.assert_accounting();

    let e = buf.get(&id).unwrap();
    let t = e
        .truncated
        .expect("an oversized command is recorded as cut");
    assert_eq!(t.total_lines, 200_000);
    assert!(t.kept_lines > 80_000 && t.kept_lines < 200_000, "{t:?}");
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
    assert!(held <= SHARE, "{held}");
    assert!(
        held > SHARE - 1_000,
        "the prefix uses the share, not a fraction of it: {held}"
    );
}

/// The pool cuts `stderr` too. A cut `stderr` is recorded on its own, and reported by the
/// `.err` handle and by the notices for the bare handle, never by the complete `stdout`. With
/// both streams over, `stderr` keeps half the share.
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
    assert!(held <= SHARE && held > SHARE - 1_000, "{held}");

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
        e.stderr.len() <= SHARE / 2 && e.stderr.len() > SHARE / 2 - 1_000,
        "stderr keeps half the share, no more and not much less: {}",
        e.stderr.len()
    );
    let held = buf.bytes_of(&both).unwrap();
    assert!(held <= SHARE && held > SHARE - 2_000, "{held}");
    assert_eq!(buf.truncation_notices_in(&both).len(), 2);
}

/// Both edges are inclusive. Two entries of exactly one share each fill the pool and both stay
/// (the limit edge); one byte more evicts the least recently used of them (F1: a `<= max + 1`
/// edge kept both); an entry of exactly one share is stored whole and one byte more is cut.
#[test]
fn the_budget_edges_are_inclusive() {
    let buf = pool();
    let a = buf.store("a".into(), "a".repeat(SHARE), String::new(), 0);
    let b = buf.store("b".into(), "b".repeat(SHARE), String::new(), 0);
    buf.assert_accounting();
    assert_eq!(buf.bytes_of(&a), Some(SHARE), "a full pool is not over it");
    assert!(
        // Read without `get`, which would make `a` the most recently used entry.
        buf.stored_truncation(&a).is_none(),
        "one share is whole"
    );
    assert_eq!(buf.total_bytes(), LIMIT);

    let c = buf.store("c".into(), "c".into(), String::new(), 0);
    buf.assert_accounting();
    assert_eq!(buf.bytes_of(&a), None, "one byte over the limit evicts");
    assert_eq!(buf.bytes_of(&b), Some(SHARE));
    assert_eq!(buf.bytes_of(&c), Some(1));

    let d = buf.store("d".into(), numbered("d", SHARE + 1), String::new(), 0);
    buf.assert_accounting();
    assert!(
        buf.get(&d).unwrap().truncated.is_some(),
        "one byte over the share is cut"
    );
    assert!(buf.bytes_of(&d).unwrap() <= SHARE);
}

/// The same inclusive share edge for every store kind and for an in-place change: a `stderr`, a
/// tool result, a whole file and a snapshot of exactly one share are held whole, a file that
/// grows by a path read to exactly one share stays resident, and a refresh that exactly fills
/// the room left stays resident.
#[test]
fn the_budget_edge_holds_for_every_store_kind() {
    let buf = pool();
    let id = buf.store("e".into(), String::new(), "e".repeat(SHARE), 1);
    let e = buf.get(&id).unwrap();
    assert!(
        e.stderr_truncated.is_none(),
        "a stderr of one share is whole"
    );
    assert_eq!(e.stderr.len(), SHARE);

    let buf = pool();
    // JSON of exactly one share: held as given, compact. One byte of slack in the gate and its
    // line form (pretty-printed, larger) would be cut instead.
    let json = format!(r#"{{"v":"{}"}}"#, "t".repeat(SHARE - r#"{"v":""}"#.len()));
    assert_eq!(json.len(), SHARE);
    let id = buf.store_tool("t", json.clone());
    let e = buf.get(&id).unwrap();
    assert!(e.truncated.is_none(), "a tool result of one share is whole");
    assert_eq!(e.stdout, json, "and kept as JSON");

    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let f = read_into(&buf, &write(dir.path(), "full", &"f".repeat(SHARE), 100));
    assert!(!buf.is_demoted(&f), "a file of one share is held");
    assert_eq!(buf.bytes_of(&f), Some(SHARE));
    let over = read_into(
        &buf,
        &write(dir.path(), "over", &"o".repeat(SHARE + 1), 100),
    );
    assert!(
        buf.is_demoted(&over),
        "a file one byte over is demoted at birth"
    );
    let s = buf.store_file("@memory:full:filtered".into(), "s".repeat(SHARE));
    assert!(
        buf.get(&s).unwrap().truncated.is_none(),
        "a snapshot of one share is whole"
    );
    buf.assert_accounting();

    let buf = pool();
    let p = write(dir.path(), "grows", "small\n", 100);
    let g = read_into(&buf, &p);
    cmd(&buf, "x", 500_000);
    write(dir.path(), "grows", &"g".repeat(SHARE), 90);
    assert_eq!(read_into(&buf, &p), g);
    assert!(
        !buf.is_demoted(&g),
        "growth by a path read to one share stays resident"
    );
    assert_eq!(buf.total_bytes(), 500_000 + SHARE);
    buf.assert_accounting();

    let buf = pool();
    let p = write(dir.path(), "refresh", &"r".repeat(10_000), 100);
    let r = read_into(&buf, &p);
    cmd(&buf, "y", 500_000);
    cmd(&buf, "z", LIMIT - 800_000);
    write(dir.path(), "refresh", &"R".repeat(300_000), 90);
    assert_eq!(buf.get(&r).unwrap().stdout.len(), 300_000);
    assert!(
        !buf.is_demoted(&r),
        "a refresh that exactly fills the room left stays resident"
    );
    assert_eq!(buf.total_bytes(), LIMIT);
    buf.assert_accounting();
}

/// F6: the entry cap is applied before the byte budget. At two entries, a third store evicts the
/// oldest entry (a command) and then fits, so the file is not demoted.
#[test]
fn the_entry_cap_evicts_before_any_file_is_demoted() {
    let dir = tempfile::tempdir().unwrap();
    let buf = OutputBuffer::with_byte_limit(2, LIMIT);
    let a = cmd(&buf, "a", 500_000);
    let f = read_into(&buf, &write(dir.path(), "f", &numbered("f", 500_000), 100));
    cmd(&buf, "b", 100_000);
    buf.assert_accounting();
    assert_eq!(
        buf.bytes_of(&a),
        None,
        "the entry cap took the oldest entry"
    );
    assert!(!buf.is_demoted(&f), "after which the store fit");
}

/// F7: an entry holding no bytes is not evicted to make room: it frees nothing.
#[test]
fn an_empty_entry_is_not_evicted_to_make_room() {
    let buf = pool();
    let empty = buf.store("e".into(), String::new(), String::new(), 0);
    let a = buf.store("a".into(), "a".repeat(SHARE), String::new(), 0);
    buf.store("b".into(), "b".repeat(SHARE), String::new(), 0);
    buf.store("c".into(), "c".into(), String::new(), 0);
    buf.assert_accounting();
    assert_eq!(buf.bytes_of(&a), None);
    assert_eq!(buf.bytes_of(&empty), Some(0), "the empty entry stays");
}

/// Ruling 3 for a whole file: bigger than one entry's share, its handle holds no text from birth
/// and displaces nothing; every read takes the file from disk.
#[test]
fn an_oversized_file_keeps_no_text_and_displaces_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let keep = cmd(&buf, "k", 500_000);
    let text = numbered("big", 1_500_000);
    let p = write(dir.path(), "big", &text, 100);
    let f = read_into(&buf, &p);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f));
    assert_eq!(buf.bytes_of(&f), Some(0));
    assert_eq!(buf.bytes_of(&keep), Some(500_000), "nothing was displaced");

    assert_eq!(buf.get(&f).unwrap().stdout, text);
    assert_eq!(read_into(&buf, &p), f, "one file, one handle");
    assert!(buf.is_demoted(&f));
    assert_eq!(buf.total_bytes(), 500_000);
    buf.assert_accounting();
}

/// The counter follows every in-place change of a file's text: growth by a path read that fits,
/// growth that does not fit (demoted, no command evicted), shrinking back in, a refresh by a
/// read that fits, and one that does not fit the room left, which demotes the entry and
/// displaces nothing.
#[test]
fn in_place_changes_of_a_file_keep_the_accounting() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "f", &numbered("v1", 100_000), 100);
    let f = read_into(&buf, &p);
    let c = cmd(&buf, "c", 500_000);
    buf.assert_accounting();

    write(dir.path(), "f", &numbered("v2", 400_000), 90);
    assert_eq!(read_into(&buf, &p), f);
    buf.assert_accounting();
    assert_eq!(buf.bytes_of(&f), Some(400_000));
    assert_eq!(buf.total_bytes(), 900_000);

    let c2 = cmd(&buf, "c2", 100_000);
    write(dir.path(), "f", &numbered("v3", 500_000), 80);
    assert_eq!(read_into(&buf, &p), f);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f), "growth that does not fit is demoted");
    assert_eq!(
        buf.bytes_of(&c),
        Some(500_000),
        "no command was evicted for it"
    );
    assert_eq!(buf.bytes_of(&c2), Some(100_000));
    assert_eq!(buf.total_bytes(), 600_000);

    write(dir.path(), "f", &numbered("v4", 10_000), 70);
    assert_eq!(read_into(&buf, &p), f);
    buf.assert_accounting();
    assert!(!buf.is_demoted(&f));
    assert_eq!(buf.total_bytes(), 610_000);

    let v5 = numbered("v5", 50_000);
    write(dir.path(), "f", &v5, 60);
    assert_eq!(buf.get(&f).unwrap().stdout, v5);
    buf.assert_accounting();
    assert!(!buf.is_demoted(&f), "a refresh that fits stays resident");
    assert_eq!(buf.total_bytes(), 650_000);

    let c3 = cmd(&buf, "c3", 390_000);
    let v6 = numbered("v6", 300_000);
    write(dir.path(), "f", &v6, 50);
    assert_eq!(buf.get(&f).unwrap().stdout, v6);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f), "the refresh did not fit the room left");
    assert_eq!(buf.bytes_of(&c3), Some(390_000), "a read displaces nothing");
    assert_eq!(buf.total_bytes(), 990_000);
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

    let out = numbered("o", 700_000);
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

/// F8: the server builds its pool at the full limit. Through the function `from_parts` calls,
/// so a server pool built at another limit reds here.
#[test]
fn the_servers_pool_has_the_full_limit() {
    assert_eq!(
        crate::server::server_output_buffer().max_bytes(),
        MAX_POOL_BYTES
    );
}

/// An oversized tool result is cut in the line space its readers number
/// (`line_addressable_text`), so line N of the prefix is line N of the whole result, and the
/// same result stored again is the same handle.
#[test]
fn an_oversized_tool_result_keeps_the_line_numbers_its_readers_use() {
    let buf = pool();
    let rows: Vec<String> = (0..60_000).map(|i| format!("row {i:06}\nsecond")).collect();
    let json = serde_json::json!({ "rows": rows }).to_string();
    assert!(json.len() > SHARE);
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
    assert!(buf.bytes_of(&id).unwrap() <= SHARE);
    assert_eq!(
        buf.store_tool("probe", json),
        id,
        "dedup by the whole result"
    );
}

/// Fix 6: the decision to cut is made on the LINE form, the one that is stored. A result whose
/// compact JSON is over the share but whose line form fits is stored whole, final `}` line
/// included (it was cut, losing that line: 600002 of 600003 kept).
#[test]
fn a_tool_result_whose_line_form_fits_is_stored_whole() {
    let buf = pool();
    let json = serde_json::json!({ "v": "\n".repeat(300_000) }).to_string();
    assert!(json.len() > SHARE);
    let id = buf.store_tool("probe", json.clone());
    buf.assert_accounting();
    let e = buf.get(&id).unwrap();
    assert!(e.truncated.is_none(), "the line form fits: nothing is cut");
    assert_eq!(e.stdout, line_addressable_text(&id, json));
    assert_eq!(e.stdout.lines().last(), Some("}"));
}

/// Fix 5 (review probe RP3): an oversized snapshot is a marked prefix, and the same view stored
/// twice is one handle: it dedups by the hash of the whole view, not by the stored prefix.
#[test]
fn an_oversized_snapshot_is_cut_and_dedups() {
    let buf = pool();
    let view = numbered("m", 1_500_000);
    let first = buf.store_file("@memory:t:filtered".into(), view.clone());
    let second = buf.store_file("@memory:t:filtered".into(), view.clone());
    buf.assert_accounting();
    assert_eq!(first, second, "one view, one handle");
    assert_eq!(buf.entry_count(), 1);
    let e = buf.get(&first).unwrap();
    let t = e
        .truncated
        .expect("an oversized snapshot is recorded as cut");
    assert_eq!(t.total_lines, crate::util::text::count_lines(&view));
    assert!(
        !buf.is_demoted(&first),
        "a snapshot has no disk copy to fall back on"
    );
    assert!(buf.bytes_of(&first).unwrap() > SHARE - 1_000);
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
