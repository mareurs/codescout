//! Gate on the `OB-N` ledger's field-label form.
//!
//! `docs/trackers/observer-blindness.md` § *How to mine this* publishes greps as the ledger's
//! read interface, and they are line-anchored on `**Field:**`. A label written inside a single
//! bold span (`**Field: content**`) matches none of them, so the worklist query silently
//! under-reports: it returned 2 where 3 entries declared `none yet`
//! (`docs/issues/archive/2026-09-02-ledger-mining-greps-under-report-on-a-drifted-field-label.md`).
//! Nothing but a template sentence enforced the form, and four of the then fourteen entries had
//! drifted.
//!
//! Two gates, because there are two failures with different remedies:
//!
//! - **Drift is never allowed.** A label present in the single-bold-span form is a mechanical
//!   repair that touches no claim. The corpus has none today, so this gate costs nothing now.
//! - **Absence is a pinned debt that only shrinks.** The template says "Drop no field — write
//!   `unknown` or `none yet`", and 30 (entry, field) pairs are absent today (measured
//!   2026-10-30, 28 entries). Authoring them is each entry's own judgement and is NOT done here.
//!   [`KNOWN_ABSENT`] lists them. A new missing field fails, and so does fixing one without
//!   removing it from the list, so the debt cannot grow and cannot hide a repair.
//!
//! The template's own field list is read from the ledger, so a field added to the template is
//! gated without editing this file.
//!
//! Like `tests/issue_clusters.rs`, this reads the committed (index) copy only: a peer session's
//! unfinished edit to the ledger must not red another session's build.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

const LEDGER: &str = "docs/trackers/observer-blindness.md";

/// `(entry, field)` pairs absent on 2026-10-30. Shrink-only: remove a pair when its field is
/// written, never add one — write the field (`unknown` or `none yet` is allowed) instead.
const KNOWN_ABSENT: &[(&str, &str)] = &[
    ("OB-2", "Instances"),
    ("OB-3", "Instances"),
    ("OB-5", "Instances"),
    ("OB-9", "Instances"),
    ("OB-12", "Instances"),
    ("OB-12", "Status"),
    ("OB-13", "Plausible-answer property"),
    ("OB-13", "Vigilance"),
    ("OB-13", "Instances"),
    ("OB-13", "Status"),
    ("OB-14", "Instances"),
    ("OB-15", "Class"),
    ("OB-15", "Blind party"),
    ("OB-15", "Who can see it"),
    ("OB-15", "Plausible-answer property"),
    ("OB-15", "Vigilance"),
    ("OB-15", "Instances"),
    ("OB-18", "Plausible-answer property"),
    ("OB-18", "Vigilance"),
    ("OB-20", "Instances"),
    ("OB-22", "Plausible-answer property"),
    ("OB-22", "Vigilance"),
    ("OB-22", "Instances"),
    ("OB-22", "Status"),
    ("OB-26", "Rests on"),
    ("OB-26", "Class"),
    ("OB-26", "Plausible-answer property"),
    ("OB-26", "Vigilance"),
    ("OB-26", "Instances"),
    ("OB-26", "Status"),
];

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The ledger as committed (the git index), not the working tree.
fn committed_ledger() -> String {
    let out = Command::new("git")
        .args(["show", &format!(":{LEDGER}")])
        .current_dir(repo_root())
        .output()
        .expect("git must run");
    assert!(
        out.status.success(),
        "{LEDGER} is not in the git index: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("the ledger is UTF-8")
}

/// The field labels of the template block under `## Template for new entries`.
fn template_fields(text: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_template = false;
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            if in_template {
                in_fence = !in_fence;
            }
            continue;
        }
        if !in_fence && line.starts_with("## ") {
            in_template = line.trim() == "## Template for new entries";
            continue;
        }
        if in_template && in_fence {
            if let Some(rest) = line.strip_prefix("**") {
                if let Some((label, _)) = rest.split_once(":**") {
                    fields.push(label.to_string());
                }
            }
        }
    }
    fields
}

/// Each `## OB-N — ` entry with its lines outside code fences. The template section and any other
/// `## ` heading end an entry.
fn entries(text: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    let mut in_fence = false;
    let mut open = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(rest) = line.strip_prefix("## OB-") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if !digits.is_empty() && rest[digits.len()..].starts_with(" — ") {
                out.push((format!("OB-{digits}"), Vec::new()));
                open = true;
                continue;
            }
        }
        if line.starts_with("## ") {
            open = false;
            continue;
        }
        if open {
            if let Some((_, lines)) = out.last_mut() {
                lines.push(line.to_string());
            }
        }
    }
    out
}

/// `(entry, field)` pairs.
type Pairs = Vec<(String, String)>;

/// Per entry: the template fields written in the drifted single-bold-span form, and the fields
/// not written at all.
fn analyse(text: &str) -> (Pairs, Pairs) {
    let fields = template_fields(text);
    let mut drifted = Vec::new();
    let mut absent = Vec::new();
    for (id, lines) in entries(text) {
        for field in &fields {
            let exact = format!("**{field}:**");
            let single_span = format!("**{field}: ");
            if lines.iter().any(|l| l.starts_with(&exact)) {
                continue;
            }
            if lines.iter().any(|l| l.starts_with(&single_span)) {
                drifted.push((id.clone(), field.clone()));
            } else {
                absent.push((id.clone(), field.clone()));
            }
        }
    }
    (drifted, absent)
}

const SYNTHETIC: &str = "\
# Ledger

## OB-1 — a claim

**Valid:** invariant

**Class:** something

**Mechanism status: none yet.** drifted into one bold span

```markdown
**Class:** inside a fence, so it counts for nothing
```

## OB-2 — another claim

**Valid:** invariant

**Class:** x

**Mechanism status:** shipped

## Template for new entries

```markdown
## OB-N — <the class>

**Valid:** invariant | dated

**Class:** <what>

**Mechanism status:** shipped | none yet
```
";

#[test]
fn the_template_fields_are_read_from_the_ledger_itself() {
    assert_eq!(
        template_fields(SYNTHETIC),
        vec!["Valid", "Class", "Mechanism status"]
    );
}

#[test]
fn a_label_inside_a_single_bold_span_is_drift_and_a_missing_one_is_absence() {
    let (drifted, absent) = analyse(SYNTHETIC);
    assert_eq!(
        drifted,
        vec![("OB-1".to_string(), "Mechanism status".to_string())],
        "the single-bold-span form is the drift the documented greps cannot see"
    );
    assert!(
        absent.is_empty(),
        "OB-1's fenced `**Class:**` must not count as a field, and OB-2 is complete: {absent:?}"
    );
}

#[test]
fn a_field_that_is_not_written_at_all_is_reported_absent() {
    let text = SYNTHETIC.replace("**Class:** x\n\n", "");
    let (_, absent) = analyse(&text);
    assert_eq!(absent, vec![("OB-2".to_string(), "Class".to_string())]);
}

#[test]
fn the_template_section_is_not_an_entry() {
    let ids: Vec<String> = entries(SYNTHETIC).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, vec!["OB-1", "OB-2"]);
}

#[test]
fn no_field_label_in_the_ledger_is_written_inside_a_single_bold_span() {
    let (drifted, _) = analyse(&committed_ledger());
    assert!(
        drifted.is_empty(),
        "these labels are bolded together with their content, so the greps in `How to mine this` \
         cannot see them. Move the closing `**` to just after the colon; no claim changes: {drifted:?}"
    );
}

#[test]
fn the_pinned_absent_fields_are_exactly_the_ones_the_ledger_lacks() {
    let text = committed_ledger();
    let fields = template_fields(&text);
    let entry_count = entries(&text).len();
    assert!(
        fields.len() >= 8 && entry_count >= 20,
        "the gate would pass vacuously: {} template fields, {entry_count} entries",
        fields.len()
    );
    let (_, absent) = analyse(&text);
    let actual: BTreeSet<(String, String)> = absent.into_iter().collect();
    let pinned: BTreeSet<(String, String)> = KNOWN_ABSENT
        .iter()
        .map(|(e, f)| (e.to_string(), f.to_string()))
        .collect();
    let new: Vec<_> = actual.difference(&pinned).collect();
    let repaired: Vec<_> = pinned.difference(&actual).collect();
    assert!(
        new.is_empty(),
        "an entry drops a template field. Write the field (`unknown` or `none yet` is allowed), \
         since an absent label is invisible to a presence sweep: {new:?}"
    );
    assert!(
        repaired.is_empty(),
        "these fields are written now. Remove them from KNOWN_ABSENT so the debt only shrinks: \
         {repaired:?}"
    );
}
