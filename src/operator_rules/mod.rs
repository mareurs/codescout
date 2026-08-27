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
pub fn compile(ledger: &str, profiles: &OperatorProfiles) -> Result<Vec<PathBuf>> {
    let block = resident_block(ledger)?;
    let mut written = Vec::new();
    for path in &profiles.paths {
        let doc = std::fs::read_to_string(path)
            .with_context(|| format!("reading profile {}", path.display()))?;
        let next = profiles::splice(&doc, &block)?;
        if next != doc {
            std::fs::write(path, &next)
                .with_context(|| format!("writing profile {}", path.display()))?;
            written.push(path.clone());
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
        let doc = std::fs::read_to_string(path)
            .with_context(|| format!("reading profile {}", path.display()))?;
        let Some(found) = profiles::extract_block(&doc) else {
            drift.push(Drift {
                path: path.clone(),
                reason: format!(
                    "no generated block; expected rules: {}",
                    want_ids.join(", ")
                ),
            });
            continue;
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

    /// Gate 2, the discriminating half. A hand-edit OUTSIDE the markers is the
    /// operator's own prose and must not read as drift — this is exactly the
    /// blank-line difference that made the three real profiles look divergent.
    #[test]
    fn check_ignores_whitespace_outside_the_markers() {
        let dir = tempfile::tempdir().unwrap();
        let profiles = profiles_in(dir.path(), 3);
        compile(LEDGER, &profiles).unwrap();

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
