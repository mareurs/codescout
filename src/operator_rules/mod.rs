//! Operator rules — engine 5 of the rules programme.
//!
//! Rules that hold across every project, tool and model for one operator. Rules
//! bound `always` compile into a delimited block in each Claude Code profile's
//! `CLAUDE.md`; rules bound `triggered` are routed at runtime in Phase 2.
//!
//! Spec: `docs/superpowers/specs/2026-08-27-operator-rules-engine-design.md`.

pub mod budget;
pub mod profiles;
pub mod render;
pub mod rule;
pub mod validate;

use std::path::PathBuf;

use anyhow::{Context, Result};

pub use profiles::OperatorProfiles;

/// Default ledger path, relative to the repo root.
pub const LEDGER_PATH: &str = "docs/trackers/operator-rules.md";

/// One profile's disagreement with the compiled block.
#[derive(Debug, Clone)]
pub struct Drift {
    pub path: PathBuf,
    pub reason: String,
}

/// The `check` exit-code contract, as a pure function so the CLI arm cannot
/// drift from it: non-zero exactly when there is drift.
pub fn exit_code(drift: &[Drift]) -> i32 {
    if drift.is_empty() {
        0
    } else {
        1
    }
}

/// Parse, validate and budget-check the ledger, then render the resident block.
fn resident_block(ledger: &str) -> Result<String> {
    let rules = rule::parse_ledger(ledger)?;
    validate::validate(&rules)?;
    budget::check_budget(&rules)?;
    Ok(render::render_block(&rules))
}

/// Extract the `<!-- rules: … -->` manifest from a rendered or on-disk block.
fn manifest_of(block: &str) -> Vec<String> {
    block
        .lines()
        .find_map(|l| {
            l.trim()
                .strip_prefix("<!-- rules:")?
                .strip_suffix("-->")
                .map(str::to_string)
        })
        .map(|ids| {
            ids.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Write the resident block into every profile that does not already match.
///
/// Returns the paths actually rewritten — empty on a second run, which is Gate 1's
/// idempotence observed from outside. Nothing is written if the ledger fails
/// validation or the budget: `resident_block` runs to completion first.
///
/// Writes go through [`crate::util::fs::atomic_write`] (write-to-tmp + rename), not
/// `std::fs::write`: these targets are the operator's hand-written, ungit-tracked
/// `CLAUDE.md` profiles, and a crash or disk-full mid-write must not be able to
/// truncate one with no backup and no regeneration path for the prose outside the
/// markers.
pub fn compile(ledger: &str, profiles: &OperatorProfiles) -> Result<Vec<PathBuf>> {
    let block = resident_block(ledger)?;
    let mut written = Vec::new();
    for path in &profiles.paths {
        let step: Result<()> = (|| {
            let doc = std::fs::read_to_string(path)
                .with_context(|| format!("reading profile {}", path.display()))?;
            let next = profiles::splice(&doc, &block)
                .with_context(|| format!("splicing profile {}", path.display()))?;
            if next != doc {
                crate::util::fs::atomic_write(path, &next)
                    .with_context(|| format!("writing profile {}", path.display()))?;
                written.push(path.clone());
            }
            Ok(())
        })();
        // X4 (partial-apply half): compile is idempotent, so a re-run recovers —
        // but the operator sees only the LAST profile's error unless the ones
        // already written before it are named here.
        if let Err(e) = step {
            let already = if written.is_empty() {
                "none".to_string()
            } else {
                written
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            // The partial-write fact is worth keeping — it is what makes a failed
            // compile recoverable — but as anyhow CONTEXT it printed outermost,
            // so every compile failure opened with "profiles already written
            // before this error: none" and buried the only line naming what to
            // repair two levels down. Rendered inline instead, so the diagnosis
            // leads and the partial-apply note trails.
            // docs/issues/2026-08-27-operator-rules-check-discards-healthy-profiles-on-one-unreadable-file.md
            return Err(anyhow::anyhow!(
                "{e:#}\n\nprofiles already written before this error: {already}"
            ));
        }
    }
    Ok(written)
}

/// Report every profile whose generated block disagrees with the ledger.
///
/// Comparison is over rule ids first, then rendered bytes — so drift reads as
/// "missing OP-2" rather than as an opaque byte difference. Content OUTSIDE the
/// markers is never compared, which is what lets an operator keep hand-written
/// prose in one profile without it reading as drift.
pub fn check(ledger: &str, profiles: &OperatorProfiles) -> Result<Vec<Drift>> {
    let expected = resident_block(ledger)?;
    let want_ids = manifest_of(&expected);
    let mut drift = Vec::new();

    for path in &profiles.paths {
        // A profile that cannot be READ is a state of that profile, not a failure
        // of the run. Propagating it with `?` discarded every Drift already
        // collected for the healthy profiles, so deleting one profile directory
        // made the two real, actionable drifts in the other two invisible and
        // reported only the one the operator may not care about.
        //
        // Same collect-and-continue shape the `BlockScan::Absent` arm below
        // already uses, and it lands in the same `Drift` list, so the CLI prints
        // it as one more DRIFT line and `exit_code` still returns 1 — no gate
        // semantics change.
        // docs/issues/2026-08-27-operator-rules-check-discards-healthy-profiles-on-one-unreadable-file.md
        let doc = match std::fs::read_to_string(path) {
            Ok(doc) => doc,
            Err(e) => {
                drift.push(Drift {
                    path: path.clone(),
                    reason: format!("could not be read: {e}"),
                });
                continue;
            }
        };
        let found = match profiles::extract_block(&doc) {
            profiles::BlockScan::Absent => {
                drift.push(Drift {
                    path: path.clone(),
                    reason: format!(
                        "no generated block; expected rules: {}",
                        want_ids.join(", ")
                    ),
                });
                continue;
            }
            profiles::BlockScan::Malformed => {
                // X4: a distinct diagnosis from "no generated block" — `compile` will
                // refuse this exact profile (`splice`'s unterminated-BEGIN error), so
                // reporting it as ordinary "missing" drift would send the operator
                // toward a remedy that cannot succeed.
                drift.push(Drift {
                    path: path.clone(),
                    reason: "a BEGIN operator-rules marker has no matching END marker; \
                             compile will refuse to write this profile until it is \
                             repaired by hand"
                        .to_string(),
                });
                continue;
            }
            profiles::BlockScan::Duplicate(_) => {
                // X2: surfaced as drift rather than silently comparing against only
                // the first block found.
                drift.push(Drift {
                    path: path.clone(),
                    reason: "a second BEGIN operator-rules marker was found after the \
                             first block ends; compile will refuse to write this \
                             profile until the duplicate is removed by hand"
                        .to_string(),
                });
                continue;
            }
            profiles::BlockScan::Present(found) => found,
        };
        let got_ids = manifest_of(found);
        if got_ids != want_ids {
            let missing: Vec<&String> = want_ids.iter().filter(|i| !got_ids.contains(i)).collect();
            let extra: Vec<&String> = got_ids.iter().filter(|i| !want_ids.contains(i)).collect();
            drift.push(Drift {
                path: path.clone(),
                reason: format!("rule set differs — missing: {missing:?}, unexpected: {extra:?}"),
            });
            continue;
        }
        if found.trim_end() != expected.trim_end() {
            drift.push(Drift {
                path: path.clone(),
                reason: "same rules, different rendered text — recompile".to_string(),
            });
        }
    }
    Ok(drift)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = r#"---
kind: tracker
entry_prefix: OP
---

## OP-1 — Always verify

**Imperative:** Do not hypothesise — ALWAYS VERIFY.
**Binding:** always
**Shape:** imperative
**Covers:** unverified-assertion
**Evidence:** unmeasured
**Status:** active
"#;

    fn profiles_in(dir: &std::path::Path, n: usize) -> OperatorProfiles {
        let paths: Vec<std::path::PathBuf> = (0..n)
            .map(|i| {
                let p = dir.join(format!("profile{i}"));
                std::fs::create_dir_all(&p).unwrap();
                p.join("CLAUDE.md")
            })
            .collect();
        for p in &paths {
            std::fs::write(p, "# Operator notes\n\nHand-written prose.\n").unwrap();
        }
        OperatorProfiles { paths }
    }

    #[test]
    fn check_reports_drift_before_the_first_compile_and_none_after() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);

        let drift = check(LEDGER, &profiles).unwrap();
        assert_eq!(drift.len(), 3, "no profile has the block yet: {drift:#?}");

        compile(LEDGER, &profiles).unwrap();
        assert!(
            check(LEDGER, &profiles).unwrap().is_empty(),
            "clean after compile"
        );
    }

    /// One unreadable profile must not erase the findings for the healthy ones.
    ///
    /// The suite could not express this before: every other test here drives
    /// `check` over in-memory documents or freshly-written temp files, where a
    /// file that cannot be read does not occur. So the abort path had no
    /// coverage at all, and the defect was found by an end-to-end CLI probe
    /// rather than by any test.
    ///
    /// Shaped like the real incident: two profiles carry genuine, actionable
    /// drift, and it was exactly those two that vanished from the report when
    /// the third was deleted.
    #[test]
    fn check_reports_every_profile_even_when_one_cannot_be_read() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        std::fs::remove_file(&profiles.paths[2]).unwrap();

        let drift = check(LEDGER, &profiles).expect("one missing file must not abort the run");

        assert_eq!(drift.len(), 3, "every profile gets a line: {drift:#?}");
        for p in &profiles.paths[..2] {
            assert!(
                drift
                    .iter()
                    .any(|d| &d.path == p && d.reason.contains("no generated block")),
                "the readable profiles must keep their OWN reason, not be replaced by \
                 the neighbour's failure: {drift:#?}"
            );
        }
        assert!(
            drift
                .iter()
                .any(|d| d.path == profiles.paths[2] && d.reason.contains("could not be read")),
            "and the missing one is reported as a state of that profile: {drift:#?}"
        );
        assert_eq!(
            exit_code(&drift),
            1,
            "gate semantics unchanged — this still fails"
        );
    }

    /// The rendered error must LEAD with the cause.
    ///
    /// Asserts on the formatted string, deliberately. Every other test here
    /// asserts on `Drift` values and error *types*, so anyhow's context ORDER
    /// had no assertion surface anywhere — which is why a message whose first
    /// line was `profiles already written before this error: none` survived
    /// review. Ordering is the whole defect, so the test pins positions rather
    /// than mere presence: both facts were present before, in the wrong order.
    #[test]
    fn a_compile_failure_leads_with_the_cause_not_the_partial_write_note() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 1);
        let dup = format!(
            "prose\n{b}\nstale\n{e}\nmore\n{b}\nalso stale\n{e}\n",
            b = render::BEGIN,
            e = render::END
        );
        std::fs::write(&profiles.paths[0], dup).unwrap();

        let err = compile(LEDGER, &profiles).expect_err("a duplicate block must refuse to write");
        let text = format!("{err:#}");

        let cause = text
            .find("second BEGIN")
            .unwrap_or_else(|| panic!("the diagnosis must be present at all: {text}"));
        let note = text
            .find("profiles already written")
            .unwrap_or_else(|| panic!("the partial-write fact must be RETAINED: {text}"));
        assert!(
            cause < note,
            "the line naming what to repair must come before the partial-write note: {text}"
        );
        assert!(
            text.starts_with("splicing profile"),
            "and the first line must be the actionable one: {text}"
        );
    }

    /// Gate 2, the discriminating half. A hand-edit OUTSIDE the markers is the
    /// operator's own prose and must not read as drift — this is exactly the
    /// blank-line difference that made the three real profiles look divergent.
    #[test]
    fn check_ignores_whitespace_outside_the_markers() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        compile(LEDGER, &profiles).unwrap();

        // Gate 1 observed through the `compile` write path, not just through
        // `splice` directly: replacing `splice(&doc, &block)` with `block` alone
        // (obliterating the operator's hand-written file) would still pass every
        // other assertion in this test unless this line is here.
        let untouched = std::fs::read_to_string(&profiles.paths[0]).unwrap();
        assert!(
            untouched.contains("Hand-written prose."),
            "compile must preserve prose outside the markers: {untouched}"
        );

        let p = &profiles.paths[1];
        let doc = std::fs::read_to_string(p).unwrap();
        std::fs::write(
            p,
            format!("{doc}\n\n\nA trailing note from the operator.\n"),
        )
        .unwrap();

        assert!(
            check(LEDGER, &profiles).unwrap().is_empty(),
            "prose outside the block is not drift"
        );
    }

    /// Gate 2, the firing half — and it must name the RULE, not just the bytes.
    #[test]
    fn check_names_the_missing_rule_when_a_profile_falls_behind() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        compile(LEDGER, &profiles).unwrap();

        let extended = format!(
            "{LEDGER}\n## OP-2 — Prefer the tracker\n\n\
             **Imperative:** Record durable facts in a tracker.\n\
             **Binding:** always\n**Shape:** imperative\n\
             **Covers:** durable-fact-placement\n**Evidence:** unmeasured\n\
             **Status:** active\n"
        );
        let drift = check(&extended, &profiles).unwrap();
        assert_eq!(drift.len(), 3, "every profile is now behind");
        assert!(
            drift[0].reason.contains("OP-2"),
            "names the rule: {}",
            drift[0].reason
        );
    }
    /// Gate 2, the third case — same rule set, different rendered bytes. Neither of
    /// the two tests above ever reaches this branch: one exits at "no block", the
    /// other at "id sets differ". Mutating the byte-comparison at the end of `check`
    /// to `if false` (or deleting it) leaves every other test in this module green;
    /// this is the one that would catch it.
    #[test]
    fn check_fires_when_rendered_text_differs_but_the_rule_set_matches() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        compile(LEDGER, &profiles).unwrap();

        // Hand-edit the rendered imperative INSIDE the markers on one profile,
        // leaving the `<!-- rules: … -->` manifest line untouched — so the id
        // sets still match and only the trailing byte comparison can catch it.
        let p = &profiles.paths[2];
        let doc = std::fs::read_to_string(p).unwrap();
        let mutated = doc.replace(
            "Do not hypothesise — ALWAYS VERIFY.",
            "Do not hypothesise — ALWAYS VERIFY, twice.",
        );
        assert_ne!(
            mutated, doc,
            "the imperative text must actually be present to mutate"
        );
        std::fs::write(p, &mutated).unwrap();

        let drift = check(LEDGER, &profiles).unwrap();
        assert_eq!(
            drift.len(),
            1,
            "only the hand-edited profile is drifted: {drift:#?}"
        );
        assert_eq!(drift[0].path, *p);
        assert!(
            drift[0].reason.contains("different rendered text"),
            "names it as a same-ids byte mismatch, not a missing rule: {}",
            drift[0].reason
        );
    }

    #[test]
    fn compile_rewrites_only_profiles_that_changed() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        assert_eq!(
            compile(LEDGER, &profiles).unwrap().len(),
            3,
            "all three on first run"
        );
        assert!(
            compile(LEDGER, &profiles).unwrap().is_empty(),
            "second run is a no-op"
        );
    }

    #[test]
    fn compile_refuses_a_ledger_that_fails_the_budget_and_writes_nothing() {
        let two_same_mode = format!(
            "{LEDGER}\n## OP-2 — Duplicate mode\n\n\
             **Imperative:** Also verify.\n**Binding:** always\n**Shape:** imperative\n\
             **Covers:** unverified-assertion\n**Evidence:** unmeasured\n**Status:** active\n"
        );
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        let err = compile(&two_same_mode, &profiles).unwrap_err().to_string();
        assert!(err.contains("unverified-assertion"), "{err}");
        let doc = std::fs::read_to_string(&profiles.paths[0]).unwrap();
        assert!(
            !doc.contains("operator-rules"),
            "nothing written on a refused compile"
        );
    }

    #[test]
    fn exit_code_is_one_on_drift_and_zero_when_clean() {
        assert_eq!(exit_code(&[]), 0);
        assert_eq!(
            exit_code(&[Drift {
                path: "x".into(),
                reason: "r".into()
            }]),
            1
        );
    }
}
