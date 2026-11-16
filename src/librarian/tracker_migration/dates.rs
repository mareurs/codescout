//! Derived dates for a migrated record (Plan L-E1 Task 7).
//!
//! Spec: `docs/superpowers/specs/2026-11-15-tracker-migration-design.md`, section Dates.
//! A flat tracker holds no `created`, `status_since` or `closed`. This module takes the dates a
//! record states, the date of its index row, and the `git blame` date of each of its lines, and
//! settles the three dates the lifecycle entry needs.

use chrono::{DateTime, NaiveDate};

use super::index_rows::Facts;
use super::mapping::Override;
use super::records::Lines;
use crate::librarian::lifecycle::write::EntryDates;

/// The date of each line of `rel_path` at `commit`, from `git blame`. Index `i` is line `i + 1`.
///
/// Each date is the local date of the commit that last changed the line, in the offset that the
/// commit recorded. A commit made at 23:30 west of UTC keeps its own date.
/// A shallow clone is refused: every line there would carry the boundary date.
pub fn blame_dates(
    repo: &git2::Repository,
    commit: &git2::Commit,
    rel_path: &str,
) -> anyhow::Result<Vec<NaiveDate>> {
    if repo.is_shallow() {
        anyhow::bail!("this clone is shallow; blame dates would be wrong");
    }
    let mut opts = git2::BlameOptions::new();
    opts.newest_commit(commit.id());
    let blame = repo.blame_file(std::path::Path::new(rel_path), Some(&mut opts))?;
    let mut dates: Vec<NaiveDate> = Vec::new();
    for hunk in blame.iter() {
        let when = hunk.final_signature().when();
        let local = when.seconds() + i64::from(when.offset_minutes()) * 60;
        let date = DateTime::from_timestamp(local, 0)
            .ok_or_else(|| anyhow::anyhow!("commit time {local} is out of range"))?
            .date_naive();
        let start = hunk.final_start_line().saturating_sub(1);
        let end = start + hunk.lines_in_hunk();
        if dates.len() < end {
            // A hunk covers lines in order, so the earlier lines are already filled.
            dates.resize(end, date);
        }
        for slot in &mut dates[start..end] {
            *slot = date;
        }
    }
    Ok(dates)
}

/// Everything `derive_dates` reads for one record.
pub struct DateInputs<'a> {
    pub facts: &'a Facts,
    /// The date the index row states, if any.
    pub row_created: Option<NaiveDate>,
    /// `blame_dates` of the whole file: index `i` is line `i + 1`.
    pub blame: &'a [NaiveDate],
    /// The lines of this record.
    pub lines: &'a Lines,
    /// The target status is terminal, so the record needs `closed`.
    pub terminal: bool,
    /// The target status is blocked or deferred, so the record needs `revisit_by`.
    pub needs_revisit: bool,
    pub ov: Option<&'a Override>,
    /// The record token, used in messages.
    pub token: &'a str,
}

/// The dates settled for one record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derived {
    pub dates: EntryDates,
    pub revisit_by: Option<NaiveDate>,
    /// The fields that came from blame or from a default, not from a stated date or an override.
    pub derived: Vec<&'static str>,
}

/// The blame dates of the record lines. A line outside `blame` has no date and is skipped.
fn line_blame(i: &DateInputs) -> Vec<NaiveDate> {
    i.lines
        .clone()
        .filter_map(|n| (n as usize).checked_sub(1))
        .filter_map(|ix| i.blame.get(ix).copied())
        .collect()
}

/// Settle `created`, `status_since`, `closed` and `revisit_by` for one record.
///
/// Every message in the `Err` names the token and the key in `overrides` that settles it.
/// `_now` is part of the call shape; the date-order checks that use it belong to apply and check.
pub fn derive_dates(i: &DateInputs, _now: NaiveDate) -> Result<Derived, Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    let mut derived: Vec<&'static str> = Vec::new();
    let blame = line_blame(i);
    let blame_min = blame.iter().min().copied();
    let blame_max = blame.iter().max().copied();

    // created: the override, else the earliest of every date the record states and blame.
    let stated_min = i
        .facts
        .stated_dates
        .iter()
        .copied()
        .chain(i.row_created)
        .min();
    let created = match i.ov.and_then(|o| o.created) {
        Some(d) => Some(d),
        None => {
            let c = [stated_min, blame_min].into_iter().flatten().min();
            if c.is_some() && c != stated_min {
                derived.push("created");
            }
            c
        }
    };
    if created.is_none() {
        problems.push(format!(
            "{}: no date is known for created. Set overrides.{}.created.",
            i.token, i.token
        ));
    }

    // status_since: a stated date for the current status, else created.
    let status_since = match (i.facts.status_date, created) {
        (Some(d), _) => Some(d),
        (None, Some(c)) => {
            derived.push("status_since");
            Some(c)
        }
        (None, None) => None,
    };

    // closed: terminal records only.
    let mut closed = None;
    if i.terminal {
        closed = match i.ov.and_then(|o| o.closed).or(i.facts.closed_date) {
            Some(d) => Some(d),
            None => {
                if blame_max.is_some() {
                    derived.push("closed");
                } else {
                    problems.push(format!(
                        "{}: no date is known for closed. Set overrides.{}.closed.",
                        i.token, i.token
                    ));
                }
                blame_max
            }
        };
    }

    // revisit_by: carried unchanged, even when it is past.
    let mut revisit_by = None;
    if i.needs_revisit {
        revisit_by = i.ov.and_then(|o| o.revisit_by).or(i.facts.revisit_by);
        if revisit_by.is_none() {
            problems.push(format!(
                "{}: the status needs revisit_by and the record has none. Set overrides.{}.revisit_by.",
                i.token, i.token
            ));
        }
    }

    match (problems.is_empty(), created, status_since) {
        (true, Some(created), Some(status_since)) => Ok(Derived {
            dates: EntryDates {
                created,
                status_since,
                closed,
            },
            revisit_by,
            derived,
        }),
        _ => Err(problems),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn facts() -> Facts {
        Facts {
            raw_status_body: None,
            stated_dates: Vec::new(),
            status_date: None,
            closed_date: None,
            revisit_by: None,
        }
    }

    fn ov() -> Override {
        Override {
            status: None,
            created: None,
            closed: None,
            revisit_by: None,
            slug: None,
        }
    }

    const NOW: fn() -> NaiveDate = || NaiveDate::from_ymd_opt(2026, 11, 15).unwrap();

    /// Inputs for lines 1..=2 of a two-line blame.
    fn inputs<'a>(
        f: &'a Facts,
        blame: &'a [NaiveDate],
        lines: &'a Lines,
        o: Option<&'a Override>,
    ) -> DateInputs<'a> {
        DateInputs {
            facts: f,
            row_created: None,
            blame,
            lines,
            terminal: false,
            needs_revisit: false,
            ov: o,
            token: "T-1",
        }
    }

    // Breaks if `created` takes the maximum, or takes the stated date over blame.
    #[test]
    fn created_is_the_minimum_of_stated_row_and_blame() {
        let mut f = facts();
        f.stated_dates = vec![d(2026, 11, 10)]; // a re-stamped Valid
        let blame = [d(2026, 8, 3), d(2026, 9, 1)];
        let lines = 1..=2;
        let got = derive_dates(&inputs(&f, &blame, &lines, None), NOW()).unwrap();
        assert_eq!(got.dates.created, d(2026, 8, 3));
        assert_eq!(got.derived, vec!["created", "status_since"]);

        // The row date joins the minimum too.
        let mut inp = inputs(&f, &blame, &lines, None);
        inp.row_created = Some(d(2026, 7, 1));
        let got = derive_dates(&inp, NOW()).unwrap();
        assert_eq!(got.dates.created, d(2026, 7, 1));
    }

    // Breaks if blame wins over an earlier stated date, or `created` is marked derived anyway.
    #[test]
    fn a_stated_date_earlier_than_blame_wins_and_is_not_derived() {
        let mut f = facts();
        f.stated_dates = vec![d(2026, 6, 1)];
        let blame = [d(2026, 8, 3)];
        let lines = 1..=1;
        let got = derive_dates(&inputs(&f, &blame, &lines, None), NOW()).unwrap();
        assert_eq!(got.dates.created, d(2026, 6, 1));
        assert!(!got.derived.contains(&"created"));
    }

    // Breaks if blame is ignored when nothing is stated.
    #[test]
    fn blame_decides_when_nothing_is_stated() {
        let f = facts();
        let blame = [d(2026, 9, 9), d(2026, 8, 8), d(2026, 7, 7)];
        let lines = 1..=2; // line 3 is not part of the record
        let got = derive_dates(&inputs(&f, &blame, &lines, None), NOW()).unwrap();
        assert_eq!(got.dates.created, d(2026, 8, 8));
    }

    // Breaks if the record's range is ignored and the whole file is used.
    #[test]
    fn only_the_record_lines_count() {
        let f = facts();
        let blame = [d(2026, 1, 1), d(2026, 9, 9), d(2026, 8, 8)];
        let lines = 2..=3; // 1-indexed: blame[1] and blame[2]
        let got = derive_dates(&inputs(&f, &blame, &lines, None), NOW()).unwrap();
        assert_eq!(got.dates.created, d(2026, 8, 8));
    }

    // Breaks if an empty blame and no stated date produce a default instead of an error.
    #[test]
    fn all_sources_empty_is_an_error_naming_the_token() {
        let f = facts();
        let lines = 1..=2;
        let err = derive_dates(&inputs(&f, &[], &lines, None), NOW()).unwrap_err();
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(err[0].contains("T-1"), "{err:?}");
        assert!(err[0].contains("overrides.T-1.created"), "{err:?}");
    }

    // Breaks if a range past the end of `blame` panics, or counts as a date.
    #[test]
    fn a_range_outside_blame_has_no_date_and_does_not_panic() {
        let f = facts();
        let blame = [d(2026, 8, 8)];
        let lines = 5..=9;
        let err = derive_dates(&inputs(&f, &blame, &lines, None), NOW()).unwrap_err();
        assert!(err[0].contains("T-1"), "{err:?}");
        // A range that straddles the end uses the lines that exist.
        let lines = 1..=9;
        let got = derive_dates(&inputs(&f, &blame, &lines, None), NOW()).unwrap();
        assert_eq!(got.dates.created, d(2026, 8, 8));
    }

    // Breaks if an override loses to any other source.
    #[test]
    fn an_override_beats_everything() {
        let mut f = facts();
        f.stated_dates = vec![d(2026, 1, 1)];
        f.status_date = Some(d(2026, 2, 2));
        f.closed_date = Some(d(2026, 3, 3));
        f.revisit_by = Some(d(2026, 4, 4));
        let mut o = ov();
        o.created = Some(d(2026, 5, 5));
        o.closed = Some(d(2026, 6, 6));
        o.revisit_by = Some(d(2026, 7, 7));
        let blame = [d(2025, 1, 1)];
        let lines = 1..=1;
        let mut inp = inputs(&f, &blame, &lines, Some(&o));
        inp.row_created = Some(d(2025, 2, 2));
        inp.terminal = true;
        inp.needs_revisit = true;
        let got = derive_dates(&inp, NOW()).unwrap();
        assert_eq!(got.dates.created, d(2026, 5, 5));
        assert_eq!(got.dates.status_since, d(2026, 2, 2));
        assert_eq!(got.dates.closed, Some(d(2026, 6, 6)));
        assert_eq!(got.revisit_by, Some(d(2026, 7, 7)));
        assert!(got.derived.is_empty(), "{:?}", got.derived);
    }

    // Breaks if status_since does not fall back to created.
    #[test]
    fn status_since_defaults_to_created() {
        let mut f = facts();
        f.stated_dates = vec![d(2026, 8, 1)];
        let lines = 1..=1;
        let got = derive_dates(&inputs(&f, &[d(2026, 9, 1)], &lines, None), NOW()).unwrap();
        assert_eq!(got.dates.status_since, d(2026, 8, 1));
        assert_eq!(got.derived, vec!["status_since"]);

        f.status_date = Some(d(2026, 8, 20));
        let got = derive_dates(&inputs(&f, &[d(2026, 9, 1)], &lines, None), NOW()).unwrap();
        assert_eq!(got.dates.status_since, d(2026, 8, 20));
        assert!(got.derived.is_empty());
    }

    // Breaks if `closed` uses the earliest blame date or the stated date is skipped.
    #[test]
    fn closed_falls_back_to_the_latest_blame_date() {
        let mut f = facts();
        let blame = [d(2026, 8, 3), d(2026, 10, 2), d(2026, 9, 1)];
        let lines = 1..=3;
        let mut inp = inputs(&f, &blame, &lines, None);
        inp.terminal = true;
        let got = derive_dates(&inp, NOW()).unwrap();
        assert_eq!(got.dates.closed, Some(d(2026, 10, 2)));
        assert!(got.derived.contains(&"closed"));

        f.closed_date = Some(d(2026, 9, 15));
        let mut inp = inputs(&f, &blame, &lines, None);
        inp.terminal = true;
        let got = derive_dates(&inp, NOW()).unwrap();
        assert_eq!(got.dates.closed, Some(d(2026, 9, 15)));
        assert!(!got.derived.contains(&"closed"));
    }

    // Breaks if `closed` is filled for a record whose status is not terminal.
    #[test]
    fn a_non_terminal_record_has_no_closed() {
        let mut f = facts();
        f.closed_date = Some(d(2026, 9, 15));
        let lines = 1..=1;
        let got = derive_dates(&inputs(&f, &[d(2026, 8, 3)], &lines, None), NOW()).unwrap();
        assert_eq!(got.dates.closed, None);
    }

    // Breaks if a past revisit_by is dropped or replaced.
    #[test]
    fn a_past_revisit_by_is_carried() {
        let mut f = facts();
        f.revisit_by = Some(d(2026, 1, 5)); // before NOW
        let lines = 1..=1;
        let blame = [d(2026, 1, 1)];
        let mut inp = inputs(&f, &blame, &lines, None);
        inp.needs_revisit = true;
        let got = derive_dates(&inp, NOW()).unwrap();
        assert_eq!(got.revisit_by, Some(d(2026, 1, 5)));
    }

    // Breaks if a missing revisit_by is defaulted instead of refused.
    #[test]
    fn a_missing_revisit_by_is_an_error() {
        let f = facts();
        let lines = 1..=1;
        let blame = [d(2026, 1, 1)];
        let mut inp = inputs(&f, &blame, &lines, None);
        inp.needs_revisit = true;
        let err = derive_dates(&inp, NOW()).unwrap_err();
        assert_eq!(err.len(), 1, "{err:?}");
        assert!(err[0].contains("T-1"));
        assert!(err[0].contains("overrides.T-1.revisit_by"));
        assert!(!err[0].contains('/'), "no path in the message: {err:?}");
    }

    // Breaks if revisit_by is filled for a status that does not need it.
    #[test]
    fn revisit_by_is_none_when_not_needed() {
        let mut f = facts();
        f.revisit_by = Some(d(2026, 12, 1));
        let lines = 1..=1;
        let got = derive_dates(&inputs(&f, &[d(2026, 1, 1)], &lines, None), NOW()).unwrap();
        assert_eq!(got.revisit_by, None);
    }

    // ---- blame_dates, against real repositories ----

    fn commit_file<'r>(
        repo: &'r git2::Repository,
        text: &str,
        secs: i64,
        offset_minutes: i32,
        parent: Option<&git2::Commit>,
    ) -> git2::Commit<'r> {
        let root = repo.workdir().unwrap();
        std::fs::write(root.join("t.md"), text).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("t.md")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let sig =
            git2::Signature::new("t", "t@e.x", &git2::Time::new(secs, offset_minutes)).unwrap();
        let parents: Vec<&git2::Commit> = parent.into_iter().collect();
        let id = repo
            .commit(Some("HEAD"), &sig, &sig, "m", &tree, &parents)
            .unwrap();
        repo.find_commit(id).unwrap()
    }

    fn utc(date: NaiveDate, h: u32, m: u32) -> i64 {
        date.and_hms_opt(h, m, 0).unwrap().and_utc().timestamp()
    }

    // Breaks if the date is taken in UTC, or the offset is applied with the wrong sign.
    #[test]
    fn blame_dates_use_the_commit_offset() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        // 2026-08-03T23:30:00-0700 is 2026-08-04T06:30:00Z.
        let secs = utc(d(2026, 8, 4), 6, 30);
        let c = commit_file(&repo, "a\nb\n", secs, -7 * 60, None);
        assert_eq!(
            blame_dates(&repo, &c, "t.md").unwrap(),
            vec![d(2026, 8, 3), d(2026, 8, 3)]
        );

        // The mirror case: 2026-08-04T01:00:00+0200 is 2026-08-03T23:00:00Z.
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let c = commit_file(&repo, "a\n", utc(d(2026, 8, 3), 23, 0), 120, None);
        assert_eq!(blame_dates(&repo, &c, "t.md").unwrap(), vec![d(2026, 8, 4)]);
    }

    // Breaks if lines map to the wrong index, or `newest_commit` is ignored.
    #[test]
    fn blame_dates_map_each_line_and_stop_at_the_given_commit() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let c1 = commit_file(&repo, "a\nb\n", utc(d(2026, 8, 1), 12, 0), 0, None);
        let c2 = commit_file(&repo, "a\nB\nc\n", utc(d(2026, 9, 1), 12, 0), 0, Some(&c1));
        assert_eq!(
            blame_dates(&repo, &c2, "t.md").unwrap(),
            vec![d(2026, 8, 1), d(2026, 9, 1), d(2026, 9, 1)]
        );
        // At the first commit the later edit does not exist.
        assert_eq!(
            blame_dates(&repo, &c1, "t.md").unwrap(),
            vec![d(2026, 8, 1), d(2026, 8, 1)]
        );
    }

    // Breaks if the shallow check is removed. No test here spawns `git clone --depth`, so the
    // repository is made shallow the way git does it: a `shallow` file that lists a commit id.
    #[test]
    fn blame_dates_refuses_a_shallow_clone() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let c = commit_file(&repo, "a\n", utc(d(2026, 8, 1), 12, 0), 0, None);
        assert!(blame_dates(&repo, &c, "t.md").is_ok());
        std::fs::write(repo.path().join("shallow"), format!("{}\n", c.id())).unwrap();
        let repo = git2::Repository::open(dir.path()).unwrap();
        assert!(repo.is_shallow());
        let c = repo.find_commit(c.id()).unwrap();
        let err = blame_dates(&repo, &c, "t.md").unwrap_err();
        assert!(
            err.to_string()
                .contains("this clone is shallow; blame dates would be wrong"),
            "{err}"
        );
    }
}
