//! Who holds a provenance lock, for the message of a refused acquisition.
//!
//! A refusal used to say only "would block". The lock can outlive the call that took it: a
//! process started inside a fenced shell inherits the descriptor, and a `flock` lasts while any
//! descriptor on its open file description lives. So "a peer is mid-call" and "a daemon started
//! hours ago still holds it" read the same, and only the first clears by itself. Bug
//! `312cde2f4698874b`.
//!
//! Best effort, Linux only (it reads `/proc`). Every failure leaves the message shorter; none
//! turns into an error of its own. It never prints a command line beyond the process name and a
//! plain lowercase subcommand, because a command line can carry a key.

use std::path::Path;

/// How many processes the message lists before it stops looking. Past it the message says
/// "and more": a list that was cut must not read as the whole set.
// cap-class: RESULT_CAP provenance_fence.holders_listed — probed
const MAX_LISTED: usize = 12;

/// One kernel record of an `FLOCK` on the inode asked about.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct LockOwner {
    pub pid: u32,
    pub exclusive: bool,
}

/// The kernel's `FLOCK` records on `inode`, from the text of `/proc/locks`.
///
/// A line reads `286: FLOCK  ADVISORY  READ 3325677 00:1c:404635102 0 EOF`. A blocked waiter has
/// `->` after the index and is not a holder, so it is skipped. The device field is not compared:
/// `stat` and `/proc/locks` disagree about it on some file systems (`0:47` and `00:1c` on the
/// machine this was written on), so the inode alone matches. A collision with another file
/// system's inode can only add a line, never hide one.
pub(super) fn parse_lock_owners(text: &str, inode: u64) -> Vec<LockOwner> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace().skip(1); // the "286:" index
            if fields.next()? != "FLOCK" {
                return None;
            }
            let _advisory = fields.next()?;
            let mode = fields.next()?;
            let pid = fields.next()?.parse::<u32>().ok()?;
            let held_inode = fields.next()?.rsplit(':').next()?.parse::<u64>().ok()?;
            (held_inode == inode).then_some(LockOwner {
                pid,
                exclusive: mode == "WRITE",
            })
        })
        .collect()
}

/// The text appended to a refusal, from what was found. Empty when nothing was found, so the
/// refusal then reads as it always did.
///
/// `open` and `owners` carry a label per pid: `None` means the process is not running. A list
/// that was cut, by its size or by the time limit, ends in "and more": a short list must not read
/// as the whole set.
pub(super) fn render(
    open: &[(u32, Option<String>)],
    open_cut: bool,
    owners: &[(LockOwner, Option<String>)],
    owners_cut: bool,
) -> String {
    let name = |pid: u32, label: &Option<String>| match label {
        Some(label) => format!("pid {pid} ({label})"),
        None => format!("pid {pid} (not running)"),
    };
    let mut parts = Vec::new();
    if !open.is_empty() {
        let list: Vec<String> = open.iter().map(|(pid, label)| name(*pid, label)).collect();
        let more = if open_cut { ", and more" } else { "" };
        parts.push(format!("open in {}{more}", list.join(", ")));
    }
    if !owners.is_empty() {
        let list: Vec<String> = owners
            .iter()
            .map(|(owner, label)| {
                let kind = if owner.exclusive {
                    "exclusive"
                } else {
                    "shared"
                };
                format!("{}, {kind}", name(owner.pid, label))
            })
            .collect();
        let more = if owners_cut { "; and more" } else { "" };
        parts.push(format!(
            "the kernel records the lock as held by {}{more}",
            list.join("; ")
        ));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(
            " ({}; a process started inside a fenced call keeps the lock after the call ends)",
            parts.join("; ")
        )
    }
}

/// `comm`, plus the subcommand when the second argument is a plain lowercase word
/// (`codescout mux`, `codescout start`). `None` when the process is gone.
#[cfg(target_os = "linux")]
fn label(pid: u32) -> Option<String> {
    let comm = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let comm = comm.trim().to_string();
    if comm.is_empty() {
        return None;
    }
    let subcommand = std::fs::read(format!("/proc/{pid}/cmdline"))
        .ok()
        .and_then(|raw| {
            let arg = raw.split(|byte| *byte == 0).nth(1)?;
            let arg = std::str::from_utf8(arg).ok()?;
            (!arg.is_empty() && arg.len() <= 16 && arg.bytes().all(|b| b.is_ascii_lowercase()))
                .then(|| arg.to_string())
        });
    Some(match subcommand {
        Some(subcommand) => format!("{comm} {subcommand}"),
        None => comm,
    })
}

/// Pids that have `lock` open, a process that inherited the descriptor included, and whether the
/// search stopped early. Compared by the device and inode each `/proc/<pid>/fd/<n>` resolves to,
/// so a different path to the same file still matches. Bounded in time and in count: stopping at
/// either bound sets the flag, so the caller can say the list is not the whole set.
#[cfg(target_os = "linux")]
fn processes_with_open(lock: &std::fs::Metadata) -> (Vec<u32>, bool) {
    use std::os::unix::fs::MetadataExt;
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(400);
    let mut found = Vec::new();
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return (found, false);
    };
    for entry in procs.flatten() {
        if std::time::Instant::now() >= deadline {
            return (found, true);
        }
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(format!("/proc/{pid}/fd")) else {
            continue;
        };
        let holds = fds.flatten().any(|fd| {
            std::fs::metadata(fd.path())
                .is_ok_and(|m| m.dev() == lock.dev() && m.ino() == lock.ino())
        });
        if holds {
            if found.len() >= MAX_LISTED {
                // One more than the cap exists: say so instead of dropping it silently.
                found.sort_unstable();
                return (found, true);
            }
            found.push(pid);
        }
    }
    found.sort_unstable();
    (found, false)
}

/// What to append to a refusal for the lock at `lock`.
pub(super) fn describe(lock: &Path) -> String {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let Ok(meta) = std::fs::metadata(lock) else {
            return String::new();
        };
        let (pids, open_cut) = processes_with_open(&meta);
        let open: Vec<(u32, Option<String>)> =
            pids.into_iter().map(|pid| (pid, label(pid))).collect();
        let mut all_owners = std::fs::read_to_string("/proc/locks")
            .map(|text| parse_lock_owners(&text, meta.ino()))
            .unwrap_or_default();
        let owners_cut = all_owners.len() > MAX_LISTED;
        all_owners.truncate(MAX_LISTED);
        let owners: Vec<(LockOwner, Option<String>)> = all_owners
            .into_iter()
            .map(|owner| {
                let label = label(owner.pid);
                (owner, label)
            })
            .collect();
        render(&open, open_cut, &owners, owners_cut)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = lock;
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lines measured on 2026-11-09 from `/proc/locks`, with a POSIX lock and a blocked waiter
    /// added: neither is a holder of an `FLOCK`.
    const LOCKS: &str = "\
79: FLOCK  ADVISORY  READ 662110 00:1c:404635102 0 EOF
286: FLOCK  ADVISORY  READ 3325677 00:1c:404635102 0 EOF
291: POSIX  ADVISORY  WRITE 4242 00:1c:404635102 0 EOF
292: -> FLOCK  ADVISORY  WRITE 5151 00:1c:404635102 0 EOF
310: FLOCK  ADVISORY  WRITE 777 00:1c:999 0 EOF
393: FLOCK  ADVISORY  WRITE 1731442 00:1c:404635102 0 EOF
not a lock line at all
";

    #[test]
    fn parse_keeps_the_flock_holders_of_that_inode_and_nothing_else() {
        let owners = parse_lock_owners(LOCKS, 404_635_102);
        assert_eq!(
            owners,
            vec![
                LockOwner {
                    pid: 662_110,
                    exclusive: false
                },
                LockOwner {
                    pid: 3_325_677,
                    exclusive: false
                },
                LockOwner {
                    pid: 1_731_442,
                    exclusive: true
                },
            ],
            "a POSIX lock, a blocked waiter, another inode and a junk line must all be skipped"
        );
    }

    #[test]
    fn parse_of_an_inode_nobody_holds_is_empty() {
        assert!(parse_lock_owners(LOCKS, 1).is_empty());
        assert!(parse_lock_owners("", 404_635_102).is_empty());
    }

    #[test]
    fn render_names_a_dead_owner_and_a_live_holder() {
        let text = render(
            &[(3_690_455, Some("codescout mux".to_string()))],
            false,
            &[(
                LockOwner {
                    pid: 3_325_677,
                    exclusive: false,
                },
                None,
            )],
            false,
        );
        assert!(
            text.contains("pid 3690455 (codescout mux)"),
            "the live holder must be named: {text}"
        );
        assert!(
            text.contains("pid 3325677 (not running), shared"),
            "a dead owner must say so: {text}"
        );
        assert!(
            !text.contains("and more"),
            "a complete list must not claim to be cut: {text}"
        );
    }

    #[test]
    fn render_is_empty_when_nothing_was_found_so_the_refusal_reads_as_before() {
        assert_eq!(render(&[], false, &[], false), "");
    }

    /// A list cut by its size or by the time limit must say so. A short list that reads as the
    /// whole set would send the reader after the wrong holder.
    #[test]
    fn render_says_and_more_when_a_list_was_cut() {
        let open = [(10, Some("a".to_string())), (11, Some("b".to_string()))];
        let owners = [(
            LockOwner {
                pid: 12,
                exclusive: false,
            },
            Some("c".to_string()),
        )];
        let both = render(&open, true, &owners, true);
        assert!(both.contains("pid 11 (b), and more"), "{both}");
        assert!(both.contains("pid 12 (c), shared; and more"), "{both}");
        let only_open = render(&open, true, &owners, false);
        assert!(only_open.contains(", and more"), "{only_open}");
        assert!(
            !only_open.contains("shared; and more"),
            "the owners list was not cut and must not say so: {only_open}"
        );
    }
    /// More holders than the cap: the search must say it stopped early. Fourteen real processes
    /// hold the file open (as their stdin), so this exercises the scan of `/proc` and not only
    /// `render`. The flag is also set when the time limit cuts the scan, so on a very busy machine
    /// this can pass through that bound; it cannot pass with the flag missing.
    #[test]
    fn a_search_that_stops_early_reports_that_it_was_cut() {
        use std::process::{Command, Stdio};
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("lock");
        std::fs::write(&lock, b"").unwrap();
        let mut children: Vec<std::process::Child> = (0..MAX_LISTED + 2)
            .map(|_| {
                Command::new("sleep")
                    .arg("20")
                    .stdin(std::fs::File::open(&lock).unwrap())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect();
        let meta = std::fs::metadata(&lock).unwrap();
        let (found, cut) = processes_with_open(&meta);
        for child in &mut children {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(
            found.len() <= MAX_LISTED,
            "the list must not exceed its cap: {found:?}"
        );
        assert!(
            cut,
            "{} processes held the file open and the search must say it stopped early: {found:?}",
            MAX_LISTED + 2
        );
    }
    /// The cited test of the `provenance_fence.holders_listed` probe row. More processes hold the
    /// file open than the message lists, so the text `describe` returns must say the list was cut:
    /// a short list that reads as the whole set would send the reader after the wrong holder.
    #[test]
    fn describe_says_and_more_when_more_processes_hold_the_file_than_it_lists() {
        use std::process::{Command, Stdio};
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join("lock");
        std::fs::write(&lock, b"").unwrap();
        let mut children: Vec<std::process::Child> = (0..MAX_LISTED + 2)
            .map(|_| {
                Command::new("sleep")
                    .arg("20")
                    .stdin(std::fs::File::open(&lock).unwrap())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .unwrap()
            })
            .collect();
        let text = describe(&lock);
        for child in &mut children {
            let _ = child.kill();
            let _ = child.wait();
        }
        assert!(
            text.contains("and more"),
            "the list was cut and the text must say so: {text}"
        );
    }
}
