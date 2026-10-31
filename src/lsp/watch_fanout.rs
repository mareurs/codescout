//! Directory symlinks under a workspace root, found on the failure path only.
//!
//! A language server's file watcher can walk a tree of directory symlinks without end. In bug
//! `35bcf086deeb7cbe` 109 of them under the gitignored `.codescout/resident-private` kept
//! rust-analyzer's watcher busy, and `documentSymbol` answered null for 15 minutes. Nothing named
//! the cause. [`find`] looks for that shape, and [`hint_with_fanout`] puts the finding in the hint
//! of the error the user already gets.
//!
//! The walk ignores `.gitignore` on purpose: the directory that hurt was gitignored, and every
//! other walker in this crate honours it. It never follows a link, so a link to an ancestor ends.
//! It is bounded by entries, and a scan that hits the bound says so.

use std::path::{Path, PathBuf};

/// Directory symlinks below this count are normal, so nothing is reported.
pub(crate) const FANOUT_MIN_LINKS: usize = 20;

/// The most entries one scan visits. The 32,192 directories under `.codescout` took 1.9 s to
/// walk, warm, on 2026-10-31, so 50,000 entries costs seconds, not minutes.
pub(crate) const FANOUT_MAX_ENTRIES: usize = 50_000;

/// What [`find`] saw.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Fanout {
    /// Directory symlinks seen. A lower bound when `truncated`.
    pub(crate) links: usize,
    /// The first one found, relative to the root.
    pub(crate) example: PathBuf,
    /// The scan stopped at its entry bound before it finished.
    pub(crate) truncated: bool,
}

/// The directory symlinks under `root`: `Some` when there are at least `min_links`, and the scan
/// visited at most `max_entries` entries. `.git` is not entered.
pub(crate) fn find(root: &Path, min_links: usize, max_entries: usize) -> Option<Fanout> {
    let mut links = 0;
    let mut example: Option<PathBuf> = None;
    let mut visited = 0;
    let mut truncated = false;
    let mut pending = vec![root.to_path_buf()];
    'scan: while let Some(dir) = pending.pop() {
        // An unreadable directory is skipped: the scan is a hint, not an audit.
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            if visited >= max_entries {
                truncated = true;
                break 'scan;
            }
            visited += 1;
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let path = entry.path();
            if kind.is_symlink() {
                // `metadata` follows the link: a dangling link and a link to a file are not
                // directory links. The link itself is never entered, so a cycle ends here.
                if std::fs::metadata(&path).is_ok_and(|m| m.is_dir()) {
                    links += 1;
                    if example.is_none() {
                        example = path.strip_prefix(root).ok().map(Path::to_path_buf);
                    }
                }
            } else if kind.is_dir() && entry.file_name() != ".git" {
                pending.push(path);
            }
        }
    }
    (links >= min_links.max(1)).then(|| Fanout {
        links,
        example: example.unwrap_or_default(),
        truncated,
    })
}

/// [`find`] with this module's limits, on a blocking thread so the walk never stalls the runtime.
/// A walk that panics or is cancelled is `None`: the hint it would improve is still sent.
pub(crate) async fn probe(root: &Path) -> Option<Fanout> {
    let root = root.to_path_buf();
    tokio::task::spawn_blocking(move || find(&root, FANOUT_MIN_LINKS, FANOUT_MAX_ENTRIES))
        .await
        .ok()
        .flatten()
}

/// `base` with one sentence added when `fanout` is `Some`. With `None` it is `base`, unchanged: a
/// scan that found nothing, or stopped early, proves no absence and says nothing.
pub(crate) fn hint_with_fanout(base: &str, fanout: Option<&Fanout>) -> String {
    let Some(fanout) = fanout else {
        return base.to_owned();
    };
    let count = if fanout.truncated {
        format!("at least {}", fanout.links)
    } else {
        fanout.links.to_string()
    };
    let tail = if fanout.truncated {
        " The scan stopped at its limit."
    } else {
        ""
    };
    format!(
        "{base} The workspace root holds {count} directory symlinks, the first at `{}`. A file \
         watcher can walk such links without end. rust-analyzer has been seen to, and then it \
         answers null for as long as it runs.{tail}",
        fanout.example.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `n` directory symlinks `l0..l{n}` in `dir`, each to a real directory `dir/target`.
    fn links_in(dir: &Path, n: usize) {
        let target = dir.join("target");
        std::fs::create_dir_all(&target).unwrap();
        for i in 0..n {
            std::os::unix::fs::symlink(&target, dir.join(format!("l{i}"))).unwrap();
        }
    }

    #[test]
    fn fewer_links_than_the_minimum_report_nothing() {
        let dir = tempfile::tempdir().unwrap();
        links_in(dir.path(), 19);
        assert_eq!(find(dir.path(), 20, 10_000), None);
    }

    #[test]
    fn the_minimum_of_links_reports_a_count_and_a_relative_example() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        links_in(&nested, 20);
        let found = find(dir.path(), 20, 10_000).expect("20 links meet a minimum of 20");
        assert_eq!(found.links, 20);
        assert!(!found.truncated);
        assert!(
            found.example.starts_with("a/b"),
            "the example is relative to the root and below it: {:?}",
            found.example
        );
    }

    #[test]
    fn links_to_files_and_dangling_links_are_not_counted() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.txt");
        std::fs::write(&file, "x").unwrap();
        for i in 0..30 {
            std::os::unix::fs::symlink(&file, dir.path().join(format!("file{i}"))).unwrap();
            std::os::unix::fs::symlink(
                dir.path().join("nowhere"),
                dir.path().join(format!("dangling{i}")),
            )
            .unwrap();
        }
        assert_eq!(find(dir.path(), 1, 10_000), None);
    }

    #[test]
    fn a_link_to_an_ancestor_is_counted_once_and_never_followed() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        std::os::unix::fs::symlink(dir.path(), inner.join("up")).unwrap();
        let found = find(dir.path(), 1, 10_000).expect("one link meets a minimum of 1");
        assert_eq!(found.links, 1, "following the link would count it again");
    }

    #[test]
    fn the_entry_bound_stops_the_scan_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        // Seven root entries: `target`, five links and `deep`. All are visited inside a bound of
        // 10, so the links are counted. `deep` holds 50 entries, which the bound cuts off.
        links_in(dir.path(), 5);
        for i in 0..50 {
            std::fs::create_dir_all(dir.path().join(format!("deep/d{i}"))).unwrap();
        }
        let found = find(dir.path(), 1, 10).expect("the links are visited before the bound");
        assert_eq!(found.links, 5);
        assert!(
            found.truncated,
            "50 entries cannot be visited in the 3 left"
        );
    }

    #[test]
    fn a_scan_that_finishes_inside_the_bound_is_not_truncated() {
        let dir = tempfile::tempdir().unwrap();
        links_in(dir.path(), 3);
        let found = find(dir.path(), 1, 10_000).unwrap();
        assert!(!found.truncated);
    }

    #[test]
    fn the_git_directory_is_not_entered() {
        let dir = tempfile::tempdir().unwrap();
        let git = dir.path().join(".git");
        std::fs::create_dir_all(&git).unwrap();
        links_in(&git, 30);
        assert_eq!(find(dir.path(), 1, 10_000), None);
    }

    #[test]
    fn a_gitignored_directory_is_still_scanned() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "/hidden/\n").unwrap();
        let hidden = dir.path().join("hidden");
        std::fs::create_dir_all(&hidden).unwrap();
        links_in(&hidden, 25);
        let found = find(dir.path(), 20, 10_000).expect("`.gitignore` must not hide the links");
        assert_eq!(found.links, 25);
    }

    #[test]
    fn the_hint_is_unchanged_when_nothing_was_found() {
        assert_eq!(hint_with_fanout("base hint.", None), "base hint.");
    }

    #[test]
    fn the_hint_names_the_count_and_the_example() {
        let fanout = Fanout {
            links: 109,
            example: PathBuf::from(".codescout/resident-private/run/l0"),
            truncated: false,
        };
        let hint = hint_with_fanout("base hint.", Some(&fanout));
        assert!(hint.starts_with("base hint."), "{hint}");
        assert!(hint.contains("109 directory symlinks"), "{hint}");
        assert!(
            hint.contains(".codescout/resident-private/run/l0"),
            "{hint}"
        );
        assert!(
            !hint.contains("at least"),
            "an exact count is not a bound: {hint}"
        );
    }

    #[test]
    fn a_truncated_scan_gives_its_count_as_a_lower_bound() {
        let fanout = Fanout {
            links: 40,
            example: PathBuf::from("a/l0"),
            truncated: true,
        };
        let hint = hint_with_fanout("base hint.", Some(&fanout));
        assert!(hint.contains("at least 40 directory symlinks"), "{hint}");
    }
}
