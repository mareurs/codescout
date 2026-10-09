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
/// to hold text that is already on disk. The file stays demoted and both commands survive. And
/// (review F3) a file that cannot fit even with every other file demoted demotes none of them:
/// the small resident file `g` keeps its text.
#[test]
fn a_path_read_of_a_demoted_file_never_evicts_a_command() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "f", &numbered("f", 400_000), 100);
    let f = read_into(&buf, &p);
    let (c1, c2) = squeeze(&buf, "c");
    assert!(buf.is_demoted(&f));
    let g = read_into(&buf, &write(dir.path(), "g", &numbered("g", 40_000), 100));
    assert!(!buf.is_demoted(&g));

    assert_eq!(read_into(&buf, &p), f);
    buf.assert_accounting();
    assert!(buf.is_demoted(&f), "no room without evicting a command");
    assert_eq!(buf.bytes_of(&c1), Some(500_000), "the command survives");
    assert_eq!(buf.bytes_of(&c2), Some(500_000));
    assert!(!buf.is_demoted(&g), "the other file stays resident");
    assert_eq!(buf.bytes_of(&g), Some(40_000));
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

/// A disk hook that holds the FIRST pass of a read at the disk until released, and lets every
/// later pass through. Returns the hook, the signal that the first pass arrived, and the release.
#[allow(clippy::type_complexity)]
fn hold_first_pass() -> (
    Arc<dyn Fn() + Send + Sync>,
    mpsc::Receiver<()>,
    mpsc::Sender<()>,
) {
    let (entered_tx, entered_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let entered_tx = std::sync::Mutex::new(entered_tx);
    let release_rx = std::sync::Mutex::new(release_rx);
    let first = std::sync::atomic::AtomicBool::new(true);
    let hook = Arc::new(move || {
        if first.swap(false, std::sync::atomic::Ordering::SeqCst) {
            let _ = entered_tx.lock().unwrap().send(());
            let _ = release_rx
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(30));
        }
    });
    (hook, entered_rx, release_tx)
}

/// Review F1: after reading the disk, `resolve` applies what it read only if the entry's WHOLE
/// state (stamp, demoted, hash) is as it saw it, not merely still present. A handle read that saw
/// the entry demoted is held at the disk; a path read makes the entry resident with v1; the file
/// is rewritten to v2 under the SAME mtime; the held read goes on. Applied, it would record v2's
/// hash on an entry that holds v1, so the next path read of v2 would compare equal and store
/// nothing, and the handle would serve v1 for good.
#[test]
fn a_read_held_at_the_disk_applies_nothing_once_a_path_read_changed_the_entry() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let v1 = numbered("v1", 400_000);
    let p = write(dir.path(), "f", &v1, 100);
    let f = read_into(&buf, &p);
    read_into(&buf, &write(dir.path(), "g", &numbered("g", 400_000), 100));
    read_into(&buf, &write(dir.path(), "h", &numbered("h", 400_000), 100));
    assert!(buf.is_demoted(&f), "f was the least recently used file");

    let (hook, entered, release) = hold_first_pass();
    buf.set_disk_hook(Some(hook));
    let reader = {
        let buf = buf.clone();
        let f = f.clone();
        std::thread::spawn(move || buf.get(&f).map(|e| e.stdout))
    };
    entered
        .recv_timeout(Duration::from_secs(30))
        .expect("the read reached the disk");
    assert_eq!(read_into(&buf, &p), f);
    assert!(!buf.is_demoted(&f), "the path read made f resident with v1");
    let mtime = filetime::FileTime::from_last_modification_time(&std::fs::metadata(&p).unwrap());
    let v2 = numbered("v2", 400_000);
    std::fs::write(&p, &v2).unwrap();
    filetime::set_file_mtime(&p, mtime).unwrap();
    release.send(()).unwrap();
    assert!(reader.join().unwrap().is_some(), "the handle stays live");
    buf.set_disk_hook(None);

    assert_eq!(read_into(&buf, &p), f);
    assert_eq!(
        buf.get(&f).unwrap().stdout,
        v2,
        "after a path read of v2 the handle serves v2"
    );
    buf.assert_accounting();
}

/// Review F2(a): parallel reads of a demoted file whose mtime is in the FUTURE. Each read stamps
/// the entry with the clock, so every read moved the stamp under the others, and each of them
/// started over with nothing changed on disk (probe: 8 reads made 36 disk passes, 1+2+...+8).
/// A read whose text has the hash the entry holds, where only the stamp moved forward, applies.
/// All eight readers see the entry before any applies (a barrier at the disk), so the bound is
/// exact: each read reads the file ONCE, 8 passes in all.
#[test]
fn parallel_reads_of_a_future_mtime_file_read_it_once_each() {
    const READERS: usize = 8;
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let text = numbered("big", 600_000);
    let p = dir.path().join("big");
    std::fs::write(&p, &text).unwrap();
    let future = SystemTime::now() + Duration::from_secs(3600);
    filetime::set_file_mtime(&p, filetime::FileTime::from_system_time(future)).unwrap();
    let f = read_into(&buf, &p);
    assert!(buf.is_demoted(&f), "bigger than one entry's share");
    // The stamp is the clock at the store. Let the clock pass it, so the first read to apply
    // moves the stamp under the others.
    let stamp = buf.inner.lock().unwrap().entries[&f].timestamp;
    while now_ms() <= stamp {
        std::thread::yield_now();
    }

    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(READERS));
    {
        let passes = passes.clone();
        buf.set_disk_hook(Some(Arc::new(move || {
            if passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < READERS {
                barrier.wait();
            }
        })));
    }
    let readers: Vec<_> = (0..READERS)
        .map(|_| {
            let buf = buf.clone();
            let f = f.clone();
            std::thread::spawn(move || buf.get(&f).map(|e| e.stdout))
        })
        .collect();
    for r in readers {
        assert_eq!(r.join().unwrap().as_deref(), Some(text.as_str()));
    }
    buf.set_disk_hook(None);
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        READERS,
        "each read reads the file once"
    );
    buf.assert_accounting();
}

/// Review F2(b): a read that loses the race on every pass (the entry changes while it reads the
/// disk) stops after `RESOLVE_READS` disk reads and serves the text it read last, WITHOUT storing
/// it (probe: a writer every ~9 ms against 4 readers made one call take 100 passes). Here a hook
/// stores v{n} by a path read and rewrites the file to w{n} on each of the first ten passes.
#[test]
fn a_read_that_keeps_losing_the_race_stops_at_the_cap_and_stores_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let p = write(dir.path(), "f", &numbered("v0", 100_000), 1_000);
    let f = read_into(&buf, &p);
    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let v = |n: usize| numbered(&format!("v{n}"), 100_000 + n);
    let w = |n: usize| numbered(&format!("w{n}"), 110_000 + n);
    {
        let passes = passes.clone();
        let buf2 = buf.clone();
        let dir = dir.path().to_path_buf();
        let p = p.clone();
        buf.set_disk_hook(Some(Arc::new(move || {
            let n = passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if n > 10 {
                return;
            }
            let vn = write(&dir, "f", &v(n), 1_000 - 2 * n as u64);
            buf2.store_file_read(&vn.to_string_lossy(), &v(n), file_mtime_ms(&p));
            write(&dir, "f", &w(n), 1_000 - 2 * n as u64 - 1);
        })));
    }
    let served = buf.get(&f).unwrap().stdout;
    buf.set_disk_hook(None);
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        RESOLVE_READS,
        "the read stops at the cap"
    );
    assert_eq!(served, w(RESOLVE_READS), "it serves the text it read last");
    assert_eq!(
        buf.bytes_of(&f),
        Some(v(RESOLVE_READS).len()),
        "and stores none of it: the entry keeps the path read's text"
    );
    assert_eq!(
        buf.get(&f).unwrap().stdout,
        w(RESOLVE_READS),
        "the next read follows the file"
    );
    buf.assert_accounting();
}

/// Review F3: a new file that cannot fit even with every other file demoted demotes nothing. The
/// pool holds two commands and a small resident file; the new file is minted demoted and the
/// small file keeps its text (it was demoted for nothing, and each later read of it cost a full
/// disk read and hash).
#[test]
fn a_file_that_cannot_fit_demotes_no_other_file() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let (c1, c2) = squeeze(&buf, "c");
    let g = read_into(&buf, &write(dir.path(), "g", &numbered("g", 40_000), 100));
    assert!(!buf.is_demoted(&g));
    let f = read_into(&buf, &write(dir.path(), "f", &numbered("f", 300_000), 100));
    buf.assert_accounting();
    assert!(buf.is_demoted(&f), "no room even with g demoted");
    assert!(!buf.is_demoted(&g), "so g keeps its text");
    assert_eq!(buf.bytes_of(&g), Some(40_000));
    assert_eq!(buf.bytes_of(&c1), Some(500_000));
    assert_eq!(buf.bytes_of(&c2), Some(500_000));
}

/// Review F4: a path read of a file bigger than one entry's share copies none of its text: it
/// can never be kept, so the copy (made under the pool's lock) was waste. A file that fits, read
/// back into its demoted entry, is copied once, so the counter is live.
#[test]
fn a_path_read_of_an_oversized_file_copies_no_text() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let p = write(dir.path(), "big", &numbered("big", 1_500_000), 100);
    let f = read_into(&buf, &p);
    assert!(buf.is_demoted(&f));
    assert_eq!(read_into(&buf, &p), f, "unchanged");
    write(dir.path(), "big", &numbered("bog", 1_400_000), 90);
    assert_eq!(read_into(&buf, &p), f, "changed");
    assert!(buf.is_demoted(&f));
    assert_eq!(
        buf.retained_copies(),
        0,
        "no copy of text that cannot be kept"
    );
    buf.assert_accounting();

    let q = write(dir.path(), "small", &numbered("s", 100_000), 100);
    let s = read_into(&buf, &q);
    OutputBuffer::demote_locked(&mut buf.inner.lock().unwrap(), &s);
    assert!(buf.is_demoted(&s));
    assert_eq!(read_into(&buf, &q), s);
    assert!(!buf.is_demoted(&s), "the text returned to its entry");
    assert_eq!(buf.retained_copies(), 1, "copied once");
    buf.assert_accounting();
}

/// Review F6(a): an entry that finds no room (only accounting drift or a caller bug can do that)
/// keeps no text, and BOTH streams record the cut, so a `.err` read is never silently empty.
#[test]
fn an_entry_stored_without_its_text_records_both_cuts() {
    let buf = pool();
    let entry = BufferEntry {
        command: "x".into(),
        stdout: "o\n".repeat(LIMIT),
        stderr: "e\n".repeat(3),
        exit_code: 1,
        timestamp: 0,
        source_path: None,
        content_hash: None,
        truncated: None,
        stderr_truncated: None,
        text_hash: None,
        demoted: false,
    };
    let kept = {
        let mut inner = buf.inner.lock().unwrap();
        OutputBuffer::admit_locked(&mut inner, LIMIT, "@cmd_00000001".into(), entry)
    };
    buf.assert_accounting();
    let none = Truncation {
        kept_lines: 0,
        total_lines: LIMIT,
    };
    assert_eq!(kept, Some(none), "admit returns the record it kept");
    assert_eq!(buf.stored_truncation("@cmd_00000001"), Some(none));
    let notice = buf
        .truncation_notice("@cmd_00000001.err")
        .expect("the .err cut is recorded");
    assert!(notice.contains("0 of 3 lines kept"), "{notice}");
}

/// Review F6(b), pinned rather than changed: the `call_content` envelope says `json_path` cannot
/// read a result whose compact JSON is over one entry's share. That is true of both shapes such a
/// result can be stored in. Pretty-printing only adds bytes, so a line form that fits the share
/// must have shrunk by collapsing `\n` escapes into raw newlines, which no JSON string may hold;
/// one with no `\n` escapes is at least as long as the compact form and is cut.
#[test]
fn a_tool_result_over_the_share_is_never_stored_as_json() {
    let buf = pool();
    let cases = [
        serde_json::json!({ "v": "\n".repeat(300_000) }),
        serde_json::json!({ "v": "x".repeat(SHARE) }),
        serde_json::json!("x".repeat(SHARE)),
        serde_json::json!({ "rows": (0..70_000).map(|i| format!("r{i}")).collect::<Vec<_>>() }),
    ];
    for v in cases {
        let json = v.to_string();
        assert!(json.len() > SHARE);
        let id = buf.store_tool("probe", json);
        let held = buf.get(&id).unwrap().stdout;
        assert!(
            serde_json::from_str::<serde_json::Value>(&held).is_err(),
            "{id} holds JSON, so json_path could read it: {:.80}",
            held
        );
    }
}

/// Review F6(c): a store hands its caller the cut it KEPT, from its own lock hold. Looked up
/// afterwards, the record was gone once another store evicted the handle, and the tee capture
/// then reported no cut for a handle that was cut.
#[test]
fn a_store_returns_the_cut_it_kept_even_once_the_handle_is_gone() {
    let buf = OutputBuffer::with_byte_limit(1, LIMIT);
    let (id, kept) =
        buf.store_truncated_kept("x".into(), numbered("x", LIMIT), String::new(), 0, None);
    let t = kept.expect("the store cut an entry over its share");
    assert_eq!(buf.stored_truncation(&id), Some(t));
    buf.store("y".into(), "y\n".into(), String::new(), 0);
    assert_eq!(buf.bytes_of(&id), None, "the count cap evicted it");
    assert!(t.kept_lines < t.total_lines, "{t:?}");
}

/// Review F2, the other half of the stamp-only rule: a read that finds only the stamp moved
/// applies only when the text it read has the hash the entry holds. Here it does not: a read is
/// held at the disk, another read stamps the entry (the file was rewritten with the same text),
/// then the file changes. The held read must start over, so the text it keeps is stamped with the
/// mtime it saw before reading THAT text, not with the stamp of the other read's text.
#[test]
fn a_read_of_new_text_after_a_stamp_only_change_starts_over() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let v1 = numbered("v1", 100_000);
    let p = write(dir.path(), "f", &v1, 100);
    let f = read_into(&buf, &p);
    assert!(!buf.is_demoted(&f));

    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (hold, entered, release) = hold_first_pass();
    {
        let passes = passes.clone();
        buf.set_disk_hook(Some(Arc::new(move || {
            passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            hold();
        })));
    }
    let reader = {
        let buf = buf.clone();
        let f = f.clone();
        std::thread::spawn(move || buf.get(&f).map(|e| e.stdout))
    };
    entered
        .recv_timeout(Duration::from_secs(30))
        .expect("the read reached the disk");
    write(dir.path(), "f", &v1, 90);
    assert_eq!(buf.get(&f).unwrap().stdout, v1, "the same text, restamped");
    let v2 = numbered("v2", 100_000);
    let m2 = file_mtime_ms(&write(dir.path(), "f", &v2, 80)).unwrap();
    release.send(()).unwrap();
    assert_eq!(reader.join().unwrap().as_deref(), Some(v2.as_str()));
    buf.set_disk_hook(None);

    assert_eq!(
        buf.inner.lock().unwrap().entries[&f].timestamp,
        m2,
        "the text kept is stamped with the mtime seen before reading it"
    );
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        3,
        "the held read started over once"
    );
    buf.assert_accounting();
}

/// Review F2: a read that applies over a stamp-only change keeps the stamp the other read set.
/// The held read stat'ed the file before it was rewritten (same text, later mtime) and another
/// read stamped that later mtime; re-stamped with its own older observation, the entry would
/// look stale again and the next read would read and hash the file for nothing.
#[test]
fn a_stamp_only_apply_never_moves_the_stamp_back() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let v1 = numbered("v1", 100_000);
    let p = write(dir.path(), "f", &v1, 100);
    let f = read_into(&buf, &p);
    write(dir.path(), "f", &v1, 90);

    let (hold, entered, release) = hold_first_pass();
    buf.set_read_done_hook(Some(hold));
    let reader = {
        let buf = buf.clone();
        let f = f.clone();
        std::thread::spawn(move || buf.get(&f).map(|e| e.stdout))
    };
    entered
        .recv_timeout(Duration::from_secs(30))
        .expect("the read read the disk");
    let later = file_mtime_ms(&write(dir.path(), "f", &v1, 80)).unwrap();
    assert_eq!(buf.get(&f).unwrap().stdout, v1);
    assert_eq!(buf.inner.lock().unwrap().entries[&f].timestamp, later);
    release.send(()).unwrap();
    assert_eq!(reader.join().unwrap().as_deref(), Some(v1.as_str()));
    buf.set_read_done_hook(None);
    assert_eq!(
        buf.inner.lock().unwrap().entries[&f].timestamp,
        later,
        "the later stamp is kept"
    );
    buf.assert_accounting();
}

/// Review F3, the edge: a new file that fits EXACTLY once every other file is demoted is kept,
/// and the other file is demoted for it. Two commands hold 1,000,000 bytes and a resident file
/// 40,000; a new file of the 48,576 bytes left after the commands fills the 1 MiB limit exactly.
#[test]
fn a_file_that_fits_exactly_with_every_other_file_demoted_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let buf = pool();
    let (c1, c2) = squeeze(&buf, "c");
    let g = read_into(&buf, &write(dir.path(), "g", &numbered("g", 40_000), 100));
    let room = LIMIT - 1_000_000;
    let f = read_into(&buf, &write(dir.path(), "f", &numbered("f", room), 100));
    buf.assert_accounting();
    assert!(!buf.is_demoted(&f), "f fits once g is demoted");
    assert!(buf.is_demoted(&g));
    assert_eq!(buf.total_bytes(), LIMIT);
    assert_eq!(buf.bytes_of(&c1), Some(500_000));
    assert_eq!(buf.bytes_of(&c2), Some(500_000));
}

/// Review F2: a change that moved the stamp BACK (a path read of the same text after the file was
/// restored to an older mtime, `cp -p`, `touch -d`) is no change on disk either. A read held at
/// the disk across it applies the text it read without a second disk read, and keeps the stamp
/// that path read set.
#[test]
fn a_stamp_moved_back_by_the_same_text_costs_no_second_read() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let text = numbered("big", 600_000);
    let p = write(dir.path(), "big", &text, 90);
    let f = read_into(&buf, &p);
    assert!(buf.is_demoted(&f), "bigger than one entry's share");

    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (hold, entered, release) = hold_first_pass();
    {
        let passes = passes.clone();
        buf.set_disk_hook(Some(Arc::new(move || {
            passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            hold();
        })));
    }
    let reader = {
        let buf = buf.clone();
        let f = f.clone();
        std::thread::spawn(move || buf.get(&f).map(|e| e.stdout))
    };
    entered
        .recv_timeout(Duration::from_secs(30))
        .expect("the read reached the disk");
    let older = file_mtime_ms(&write(dir.path(), "big", &text, 100)).unwrap();
    assert_eq!(read_into(&buf, &p), f);
    assert_eq!(buf.inner.lock().unwrap().entries[&f].timestamp, older);
    release.send(()).unwrap();
    assert_eq!(reader.join().unwrap().as_deref(), Some(text.as_str()));
    buf.set_disk_hook(None);
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the held read applied without reading the file again"
    );
    assert_eq!(buf.inner.lock().unwrap().entries[&f].timestamp, older);
    buf.assert_accounting();
}

/// Review F2: a read held at the disk while a path read stored the very text it then reads (the
/// entry now describes that text) applies it without a second disk read.
#[test]
fn a_read_of_the_text_a_path_read_just_stored_costs_no_second_read() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let p = write(dir.path(), "f", &numbered("v1", 100_000), 100);
    let f = read_into(&buf, &p);
    write(dir.path(), "f", &numbered("v1", 100_000), 95);

    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (hold, entered, release) = hold_first_pass();
    {
        let passes = passes.clone();
        buf.set_disk_hook(Some(Arc::new(move || {
            passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            hold();
        })));
    }
    let reader = {
        let buf = buf.clone();
        let f = f.clone();
        std::thread::spawn(move || buf.get(&f).map(|e| e.stdout))
    };
    entered
        .recv_timeout(Duration::from_secs(30))
        .expect("the read reached the disk");
    let v2 = numbered("v2", 100_000);
    let m2 = file_mtime_ms(&write(dir.path(), "f", &v2, 90)).unwrap();
    assert_eq!(read_into(&buf, &p), f);
    release.send(()).unwrap();
    assert_eq!(reader.join().unwrap().as_deref(), Some(v2.as_str()));
    buf.set_disk_hook(None);
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the held read applied without reading the file again"
    );
    assert_eq!(buf.inner.lock().unwrap().entries[&f].timestamp, m2);
    assert_eq!(buf.get(&f).unwrap().stdout, v2);
    buf.assert_accounting();
}

/// Review F2: a read never puts text back into an entry that was demoted while it read the disk,
/// even when only the stamp and the demotion changed and the text is the same: a read of a
/// demoted handle serves the text without storing it, so a read never grows the pool.
#[test]
fn a_read_never_restores_an_entry_demoted_while_it_read_the_disk() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let v1 = numbered("v1", 40_000);
    let p = write(dir.path(), "f", &v1, 100);
    let f = read_into(&buf, &p);
    write(dir.path(), "f", &v1, 90);

    let (hold, entered, release) = hold_first_pass();
    buf.set_disk_hook(Some(hold));
    let reader = {
        let buf = buf.clone();
        let f = f.clone();
        std::thread::spawn(move || buf.get(&f).map(|e| e.stdout))
    };
    entered
        .recv_timeout(Duration::from_secs(30))
        .expect("the read reached the disk");
    assert_eq!(read_into(&buf, &p), f, "restamped, the same text");
    OutputBuffer::demote_locked(&mut buf.inner.lock().unwrap(), &f);
    let before = buf.total_bytes();
    release.send(()).unwrap();
    assert_eq!(reader.join().unwrap().as_deref(), Some(v1.as_str()));
    buf.set_disk_hook(None);
    assert!(buf.is_demoted(&f), "the read put no text back");
    assert_eq!(buf.total_bytes(), before, "a read never grows the pool");
    buf.assert_accounting();
}

/// Review F2: a read stopped at the cap reports the text it serves as a change when that text is
/// not the one the entry describes, even though nothing recorded a change: here every pass loses
/// to a path read that only restamps the entry (same text), and the file then changes to w{n}.
#[test]
fn a_read_stopped_at_the_cap_reports_new_text_as_a_change() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let v0 = numbered("v0", 100_000);
    let p = write(dir.path(), "f", &v0, 1_000);
    let f = read_into(&buf, &p);
    let w = |n: usize| numbered(&format!("w{n}"), 110_000 + n);
    {
        let buf2 = buf.clone();
        let dir = dir.path().to_path_buf();
        let p = p.clone();
        let v0 = v0.clone();
        let passes = std::sync::atomic::AtomicUsize::new(0);
        buf.set_disk_hook(Some(Arc::new(move || {
            let n = passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u64 + 1;
            if n > 10 {
                return;
            }
            write(&dir, "f", &v0, 1_000 - 2 * n);
            buf2.store_file_read(&p.to_string_lossy(), &v0, file_mtime_ms(&p));
            write(&dir, "f", &w(n as usize), 1_000 - 2 * n - 1);
        })));
    }
    let (e, refreshed) = buf.get_with_refresh_flag(&f).unwrap();
    buf.set_disk_hook(None);
    assert_eq!(e.stdout, w(RESOLVE_READS));
    assert!(
        refreshed,
        "the holder is served text the entry does not describe: a change"
    );
    assert_eq!(buf.bytes_of(&f), Some(v0.len()), "nothing was stored");
    buf.assert_accounting();
}

/// Review F-A (probe rv_p07): a read stopped at the cap serves text the entry does not hold, so
/// it must not consume a refresh notice that belongs to the entry's own, newer text. On each of
/// the first two passes a path read stores x{k} and the file then moves on to y{k}; on the third
/// a path read stores x3 only. The read serves y2 and tells a change; the holder, who last saw y2,
/// must then be told once more when the handle serves x3.
#[test]
fn a_read_stopped_at_the_cap_leaves_the_notice_of_text_it_did_not_serve() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let p = write(dir.path(), "f", &numbered("v0", 40_000), 1_000);
    let f = read_into(&buf, &p);
    write(dir.path(), "f", &numbered("w0", 40_001), 990);
    let x = |n: usize| numbered(&format!("x{n}"), 40_000 + n);
    let y = |n: usize| numbered(&format!("y{n}"), 41_000 + n);
    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    {
        let passes = passes.clone();
        let buf2 = buf.clone();
        let dir = dir.path().to_path_buf();
        let p = p.clone();
        buf.set_read_done_hook(Some(Arc::new(move || {
            let k = passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            if k > 3 {
                return;
            }
            write(&dir, "f", &x(k), 980 - 3 * k as u64);
            read_into(&buf2, &p);
            if k < 3 {
                write(&dir, "f", &y(k), 980 - 3 * k as u64 - 1);
            }
        })));
    }
    let (served, told) = buf.get_with_refresh_flag(&f).unwrap();
    buf.set_read_done_hook(None);
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        RESOLVE_READS
    );
    assert_eq!(
        served.stdout,
        y(2),
        "the read at the cap serves what it read"
    );
    assert!(told, "y2 is not the text the holder saw");
    let (next, next_told) = buf.get_with_refresh_flag(&f).unwrap();
    assert_eq!(next.stdout, x(3));
    assert!(
        next_told,
        "the holder was served y2, then x3: the change to x3 must be told"
    );
    let (_, again) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(!again, "and told once");
    buf.assert_accounting();
}

/// The F-D scenario: a read whose every pass loses only because the entry's residency flips
/// (a path read makes it resident, then it is demoted, then a path read makes it resident again),
/// so it stops at the cap having read, on its last pass, exactly the text the entry then holds.
/// `change` rewrites the file once before the read, so the first path read changes the entry.
/// Returns what the read served and told, and what the next reporting read told.
fn read_at_the_cap_of_the_entrys_own_text(change: bool) -> (String, bool, bool) {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let v0 = numbered("v0", 40_000);
    let p = write(dir.path(), "f", &v0, 1_000);
    let f = read_into(&buf, &p);
    OutputBuffer::demote_locked(&mut buf.inner.lock().unwrap(), &f);
    let text = if change {
        numbered("x", 40_100)
    } else {
        v0.clone()
    };
    write(dir.path(), "f", &text, 990);
    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    {
        let passes = passes.clone();
        let buf2 = buf.clone();
        let dir = dir.path().to_path_buf();
        let p = p.clone();
        let f = f.clone();
        let text = text.clone();
        buf.set_read_done_hook(Some(Arc::new(move || {
            let k = passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u64 + 1;
            match k {
                1 | 3 => {
                    write(&dir, "f", &text, 980 - 3 * k);
                    read_into(&buf2, &p);
                    // Newer than the stamp, so the next pass reads the disk.
                    write(&dir, "f", &text, 980 - 3 * k - 1);
                }
                2 => OutputBuffer::demote_locked(&mut buf2.inner.lock().unwrap(), &f),
                _ => {}
            }
        })));
    }
    let (served, told) = buf.get_with_refresh_flag(&f).unwrap();
    buf.set_read_done_hook(None);
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        RESOLVE_READS,
        "every pass lost, and the read stopped at the cap"
    );
    let (_, next_told) = buf.get_with_refresh_flag(&f).unwrap();
    buf.assert_accounting();
    (served.stdout, told, next_told)
}

/// Review F-D (mutant M09, `differs = true`): a read stopped at the cap that serves the entry's
/// OWN text, unchanged since the holder last saw it, tells no change.
#[test]
fn a_read_stopped_at_the_cap_serving_unchanged_text_tells_no_change() {
    let (served, told, next_told) = read_at_the_cap_of_the_entrys_own_text(false);
    assert_eq!(served, numbered("v0", 40_000));
    assert!(!told, "nothing the holder saw changed");
    assert!(!next_told);
}

/// Review F-A (mutant M23, the cap's serve never consuming the notice): a read stopped at the cap
/// that serves the entry's own text, which changed since the holder last saw it, tells the change
/// and consumes the notice, so the next read does not tell it again.
#[test]
fn a_read_stopped_at_the_cap_serving_the_entrys_new_text_tells_it_once() {
    let (served, told, next_told) = read_at_the_cap_of_the_entrys_own_text(true);
    assert_eq!(served, numbered("x", 40_100));
    assert!(told, "the entry's text changed since the holder saw it");
    assert!(!next_told, "told once");
}

/// Review F-D (mutant M05): only passes that READ the file count toward the cap. Four passes that
/// only stat the file lose (a path read stores new text each time, stamped at the file's mtime,
/// so the next pass finds it fresh), then three passes read and lose: the read stops after those
/// three, on its seventh pass, and serves what the last of them read.
#[test]
fn passes_that_only_stat_the_file_do_not_count_toward_the_cap() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let p = write(dir.path(), "f", &numbered("v0", 40_000), 1_000);
    let f = read_into(&buf, &p);
    let v = |n: u64| numbered(&format!("v{n}"), 40_000 + n as usize);
    let w = |n: u64| numbered(&format!("w{n}"), 41_000 + n as usize);
    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    {
        let passes = passes.clone();
        let buf2 = buf.clone();
        let dir = dir.path().to_path_buf();
        let p = p.clone();
        buf.set_read_done_hook(Some(Arc::new(move || {
            let k = passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u64 + 1;
            if k > 7 {
                return;
            }
            write(&dir, "f", &v(k), 980 - 3 * k);
            read_into(&buf2, &p);
            if k >= 4 {
                write(&dir, "f", &w(k), 980 - 3 * k - 1);
            }
        })));
    }
    let served = buf.get(&f).unwrap().stdout;
    buf.set_read_done_hook(None);
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        4 + RESOLVE_READS,
        "four stat-only passes, then the cap's three reads"
    );
    assert_eq!(served, w(6), "the third read read w6");
    buf.assert_accounting();
}

/// Review F-C (probe rv_p12): a pass that only stat'ed the file, while another caller only
/// restamped the entry (same text, same residency), finishes with the entry as it is. Every
/// such pass used to start over, and a stream of restamps kept one read going for as long as it
/// lasted (probe: 501 passes, 587 ms).
#[test]
fn a_stat_only_pass_over_a_restamped_entry_finishes() {
    const RESTAMPS: u64 = 50;
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let v1 = numbered("v1", 40_000);
    let p = write(dir.path(), "f", &v1, 10_000);
    let f = read_into(&buf, &p);
    let passes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    {
        let passes = passes.clone();
        let buf2 = buf.clone();
        let p = p.clone();
        buf.set_read_done_hook(Some(Arc::new(move || {
            let k = passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u64 + 1;
            if k > RESTAMPS {
                return;
            }
            let t = SystemTime::now() - Duration::from_secs(10_000 - k);
            filetime::set_file_mtime(&p, filetime::FileTime::from_system_time(t)).unwrap();
            read_into(&buf2, &p);
        })));
    }
    let (e, told) = buf.get_with_refresh_flag(&f).unwrap();
    buf.set_read_done_hook(None);
    assert_eq!(
        passes.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the restamp changed nothing the pass saw"
    );
    assert_eq!(e.stdout, v1);
    assert!(!told);
    buf.assert_accounting();
}

/// Review F-E (probe rv_p17): the byte limit evicts a memory view's `@file_*` snapshot, which the
/// refusal names, with the route back to it.
#[test]
fn the_byte_limit_drops_a_memory_view_and_the_refusal_names_it() {
    let buf = pool();
    let s = buf.store_file("@memory:t:filtered".into(), numbered("m", 400_000));
    cmd(&buf, "c1", 500_000);
    cmd(&buf, "c2", 500_000);
    assert!(!buf.handles().contains(&s), "the view was evicted");
    let hint = ref_gone_hint(&s);
    assert!(hint.contains("memory-view handles"), "{hint}");
    assert!(hint.contains("read the memory topic again"), "{hint}");
}

/// Review F-B, a KNOWN LIMIT pinned so its fix shows up here: after a read stopped at the cap
/// served text the entry does not hold (and told it), the next reporting read serves the SAME
/// text and tells it again, because nothing records what the read at the cap served. A
/// `run_command` of the handle then prints one spurious `↻ … refreshed from disk`. When this
/// reds, the limit is fixed: drop it from the comment at the cap in `resolve`.
#[test]
fn after_a_read_stopped_at_the_cap_the_next_read_tells_the_same_text_again_known_limitation() {
    let dir = tempfile::tempdir().unwrap();
    let buf = Arc::new(pool());
    let v0 = numbered("v0", 100_000);
    let p = write(dir.path(), "f", &v0, 1_000);
    let f = read_into(&buf, &p);
    let w = |n: usize| numbered(&format!("w{n}"), 110_000 + n);
    {
        let buf2 = buf.clone();
        let dir = dir.path().to_path_buf();
        let p = p.clone();
        let v0 = v0.clone();
        let passes = std::sync::atomic::AtomicUsize::new(0);
        buf.set_disk_hook(Some(Arc::new(move || {
            let n = passes.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as u64 + 1;
            if n > 10 {
                return;
            }
            write(&dir, "f", &v0, 1_000 - 2 * n);
            buf2.store_file_read(&p.to_string_lossy(), &v0, file_mtime_ms(&p));
            write(&dir, "f", &w(n as usize), 1_000 - 2 * n - 1);
        })));
    }
    let (first, told) = buf.get_with_refresh_flag(&f).unwrap();
    buf.set_disk_hook(None);
    assert_eq!(first.stdout, w(RESOLVE_READS));
    assert!(told);
    let (second, told_again) = buf.get_with_refresh_flag(&f).unwrap();
    assert_eq!(second.stdout, first.stdout, "the same text");
    assert!(told_again, "told a second time: the known limit");
    let (_, third) = buf.get_with_refresh_flag(&f).unwrap();
    assert!(!third, "but only twice");
    buf.assert_accounting();
}
