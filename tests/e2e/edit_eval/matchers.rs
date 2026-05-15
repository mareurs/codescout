use crate::e2e::edit_eval::types::{CompilerExpected, ContentInvariant, EditCase, ReturnExpected};
use crate::e2e::eval_common::Verdict;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub verdict: Verdict,
    pub evidence: String,
}

pub struct Observation<'a> {
    pub return_: ReturnObservation<'a>,
    pub disk: Option<&'a str>,
    pub compiler_ok: bool,
}

// Payloads + TransientLsp variant consumed by the runner in T8
// (LSP retry policy / verbose evidence dumps).
#[allow(dead_code)]
pub enum ReturnObservation<'a> {
    Ok(&'a serde_json::Value),
    Recoverable(&'a str),
    Fatal(&'a str),
    TransientLsp(&'a str),
}

pub fn grade(case: &EditCase, obs: Observation<'_>) -> MatchResult {
    // Fatal / transient — short-circuit before considering disk.
    match &obs.return_ {
        ReturnObservation::Fatal(msg) => {
            return MatchResult {
                verdict: Verdict::Panic,
                evidence: format!("fatal: {msg}"),
            }
        }
        ReturnObservation::TransientLsp(msg) => {
            return MatchResult {
                verdict: Verdict::SilentWrong,
                evidence: format!("transient LSP (retryable): {msg}"),
            }
        }
        _ => {}
    }

    let return_ok = matches!(
        (&case.expected.return_, &obs.return_),
        (ReturnExpected::Ok, ReturnObservation::Ok(_))
            | (
                ReturnExpected::CleanError,
                ReturnObservation::Recoverable(_)
            )
    );

    if !return_ok {
        let got = match &obs.return_ {
            ReturnObservation::Ok(_) => "Ok",
            ReturnObservation::Recoverable(_) => "RecoverableError",
            _ => "?",
        };
        let want = match case.expected.return_ {
            ReturnExpected::Ok => "Ok",
            ReturnExpected::CleanError => "RecoverableError",
        };
        return MatchResult {
            verdict: match (&case.expected.return_, &obs.return_) {
                (ReturnExpected::CleanError, ReturnObservation::Ok(_)) => Verdict::SilentWrong,
                (ReturnExpected::Ok, ReturnObservation::Recoverable(_)) => Verdict::CleanError,
                _ => Verdict::SilentWrong,
            },
            evidence: format!("return: want {want}, got {got}"),
        };
    }

    // Disk invariants — only checked when return matched.
    let disk_violation = match obs.disk {
        Some(content) => first_disk_violation(content, &case.expected.disk, case.target_file),
        None if case.expected.disk.is_empty() => None,
        None => Some(String::from(
            "disk content unreadable; case expected invariants",
        )),
    };

    if let Some(v) = disk_violation {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("disk: {v}"),
        };
    }

    // Compiler oracle — graded last.
    let compiler_match = match case.expected.compiler {
        CompilerExpected::Builds => obs.compiler_ok,
        CompilerExpected::Breaks => !obs.compiler_ok,
        CompilerExpected::DontCare => true,
    };

    if !compiler_match {
        let got = if obs.compiler_ok { "builds" } else { "breaks" };
        let want = match case.expected.compiler {
            CompilerExpected::Builds => "builds",
            CompilerExpected::Breaks => "breaks",
            CompilerExpected::DontCare => "n/a",
        };
        return MatchResult {
            verdict: Verdict::Corrupt,
            evidence: format!("compiler: want {want}, got {got}"),
        };
    }

    MatchResult {
        verdict: Verdict::Correct,
        evidence: String::from("triplet matched"),
    }
}

fn first_disk_violation(
    content: &str,
    invs: &[ContentInvariant],
    default_file: &str,
) -> Option<String> {
    for inv in invs {
        match inv {
            ContentInvariant::Contains {
                file,
                needle,
                count,
            } => {
                let _ = file; // single-file fixtures common; multi-file rename overrides target_file
                let _ = default_file;
                let actual = content.matches(needle).count();
                if actual != *count {
                    return Some(format!(
                        "needle {needle:?} appears {actual}× (want {count}×)"
                    ));
                }
            }
            ContentInvariant::NotContains { needle, .. } => {
                if content.contains(needle) {
                    return Some(format!("forbidden needle {needle:?} present"));
                }
            }
            ContentInvariant::LineEquals { line, text, .. } => {
                let got = content.lines().nth((*line as usize).saturating_sub(1));
                if got.map(|l| l.trim_end()) != Some(text) {
                    return Some(format!(
                        "line {line}: want {text:?}, got {:?}",
                        got.unwrap_or("<missing>")
                    ));
                }
            }
        }
    }
    None
}

pub fn h1_exempt_for(case: &EditCase, v: &Verdict) -> bool {
    matches!((&case.h1_exempt, v), (Some(_), Verdict::SilentWrong))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::edit_eval::types::{EditAction, Expected};

    fn case_ok_disk(invs: Vec<ContentInvariant>) -> EditCase {
        EditCase {
            id: "T",
            action: EditAction::Replace,
            input: serde_json::json!({}),
            target_file: "x.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: invs,
                compiler: CompilerExpected::Builds,
            },
            rationale: "test",
            h1_exempt: None,
        }
    }

    #[test]
    fn correct_when_all_three_match() {
        let c = case_ok_disk(vec![ContentInvariant::Contains {
            file: "x.rs",
            needle: "fn foo",
            count: 1,
        }]);
        let v = serde_json::json!({"ok": true});
        let obs = Observation {
            return_: ReturnObservation::Ok(&v),
            disk: Some("fn foo() {}"),
            compiler_ok: true,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::Correct);
    }

    #[test]
    fn silent_wrong_when_expected_cleanerror_got_ok() {
        let mut c = case_ok_disk(vec![]);
        c.expected.return_ = ReturnExpected::CleanError;
        let v = serde_json::json!({});
        let obs = Observation {
            return_: ReturnObservation::Ok(&v),
            disk: None,
            compiler_ok: true,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::SilentWrong);
    }

    #[test]
    fn cleanerror_when_expected_ok_got_recoverable() {
        let c = case_ok_disk(vec![]);
        let obs = Observation {
            return_: ReturnObservation::Recoverable("dropped definition"),
            disk: None,
            compiler_ok: true,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::CleanError);
    }

    #[test]
    fn corrupt_when_return_ok_but_compiler_breaks_unexpectedly() {
        let c = case_ok_disk(vec![]);
        let v = serde_json::json!({});
        let obs = Observation {
            return_: ReturnObservation::Ok(&v),
            disk: None,
            compiler_ok: false,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::Corrupt);
    }

    #[test]
    fn breaks_expected_grades_correct_when_compiler_breaks() {
        let mut c = case_ok_disk(vec![]);
        c.expected.compiler = CompilerExpected::Breaks;
        let v = serde_json::json!({});
        let obs = Observation {
            return_: ReturnObservation::Ok(&v),
            disk: None,
            compiler_ok: false,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::Correct);
    }

    #[test]
    fn panic_when_fatal_error_short_circuits() {
        let c = case_ok_disk(vec![]);
        let obs = Observation {
            return_: ReturnObservation::Fatal("boom"),
            disk: None,
            compiler_ok: true,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::Panic);
    }

    #[test]
    fn h1_exempt_fires_only_for_silent_wrong() {
        let mut c = case_ok_disk(vec![]);
        c.h1_exempt = Some("BUG-054");
        assert!(h1_exempt_for(&c, &Verdict::SilentWrong));
        assert!(!h1_exempt_for(&c, &Verdict::Correct));
        c.h1_exempt = None;
        assert!(!h1_exempt_for(&c, &Verdict::SilentWrong));
    }
}
