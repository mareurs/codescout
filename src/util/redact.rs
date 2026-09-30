//! Credential-shaped values in command output (bug 8df0779550c5b5d8).
//!
//! `run_command` used to return an environment dump verbatim, so a credential in the process
//! environment reached the model's context and, from there, the plaintext session transcript.
//! This module replaces values with a KNOWN SHAPE by a marker before the output is returned or
//! buffered. It is a redaction by value, not a gate on the command: a gate keyed on command text
//! (`env`, `printenv`, ...) is a denylist over a namespace and misses `python -c 'os.environ'`,
//! whereas the value has the same shape however it was printed.
//!
//! **What it does not cover, stated here because the silence would read as safety:** a secret
//! of a shape not listed below (a JWT, a bare 40-hex key, `PASSWORD=hunter2`). Widening the list
//! trades that gap against false positives, and this list is deliberately the shapes the bug
//! named. It also redacts a fixture that merely QUOTES a token-shaped string, which is the price
//! of not deciding what is "real".

use std::borrow::Cow;
use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

/// What replaces one credential-shaped value.
pub const REDACTION_MARKER: &str = "<redacted-credential>";

/// The response key that says output was altered, so an edited response is never
/// indistinguishable from an unedited one.
pub const REDACTED_KEY: &str = "redacted_credentials";

/// Output after redaction, and how many values were replaced.
#[derive(Debug, PartialEq, Eq)]
pub struct Redacted<'a> {
    pub text: Cow<'a, str>,
    pub count: usize,
}

/// Replace every credential-shaped value in `text`. Borrowed and unchanged when there is none.
pub fn redact_credentials(text: &str) -> Redacted<'_> {
    let re = credential_re();
    let count = re.find_iter(text).count();
    if count == 0 {
        return Redacted {
            text: Cow::Borrowed(text),
            count: 0,
        };
    }
    Redacted {
        text: Cow::Owned(
            re.replace_all(text, regex::NoExpand(REDACTION_MARKER))
                .into_owned(),
        ),
        count,
    }
}

/// The shapes redacted. Each is a vendor's published token format, chosen because a value of that
/// shape is a credential wherever it appears; nothing here is keyed on a variable NAME.
///
/// - GitHub tokens: `ghp_`/`gho_`/`ghu_`/`ghs_`/`ghr_` + at least 36 alphanumerics. `{36,}` and
///   not `{36}` so a longer token is consumed whole and no tail is left behind.
/// - GitHub fine-grained PATs: `github_pat_` + at least 50 of `[A-Za-z0-9_]`.
/// - `sk-` keys (OpenAI, Anthropic): at least 20 of `[A-Za-z0-9_-]`, hyphens allowed because
///   `sk-ant-api03-...` has them. The leading `\b` is what keeps `risk-assessment-...` and
///   `task-force-...` out: there the `s` follows a word character.
/// - AWS access key ids: `AKIA` (long-lived) or `ASIA` (temporary) + 16 uppercase/digits.
fn credential_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"gh[pousr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{50,}|\bsk-[A-Za-z0-9_-]{20,}|\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
        )
        .expect("valid regex")
    })
}

/// Record that `count` values were redacted, adding to a count already on the response. Writes
/// nothing for zero, so an unedited response carries no key.
pub fn note_in(result: &mut Value, count: usize) {
    if count == 0 {
        return;
    }
    let Some(obj) = result.as_object_mut() else {
        return;
    };
    let previous = obj.get(REDACTED_KEY).and_then(Value::as_u64).unwrap_or(0);
    obj.insert(
        REDACTED_KEY.to_string(),
        Value::from(previous + count as u64),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    // Every secret below is BUILT, never written out: a token-shaped literal in a committed file
    // is what this module exists to keep out of transcripts, and it would trip secret scanners.
    fn gh(n: usize) -> String {
        format!("ghp_{}", "A1".repeat(n).chars().take(n).collect::<String>())
    }
    fn pat(n: usize) -> String {
        format!(
            "github_pat_{}",
            "b2".repeat(n).chars().take(n).collect::<String>()
        )
    }
    fn sk(n: usize) -> String {
        format!("sk-{}", "c3".repeat(n).chars().take(n).collect::<String>())
    }
    fn aws(n: usize) -> String {
        format!("AKIA{}", "D4".repeat(n).chars().take(n).collect::<String>())
    }

    fn redacted(text: &str) -> (String, usize) {
        let r = redact_credentials(text);
        (r.text.into_owned(), r.count)
    }

    #[test]
    fn a_github_token_is_replaced_and_one_character_short_is_not() {
        assert_eq!(redacted(&gh(36)), (REDACTION_MARKER.to_string(), 1));
        let short = gh(35);
        assert_eq!(
            redacted(&short),
            (short.clone(), 0),
            "35 characters is not a token"
        );
    }

    #[test]
    fn every_github_token_prefix_is_covered() {
        for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"] {
            let tok = format!("{prefix}{}", "Z9".repeat(18));
            assert_eq!(
                redacted(&tok),
                (REDACTION_MARKER.to_string(), 1),
                "{prefix}"
            );
        }
        let other = format!("ghx_{}", "Z9".repeat(18));
        assert_eq!(redacted(&other).1, 0, "ghx_ is not a GitHub token prefix");
    }

    #[test]
    fn a_token_longer_than_the_minimum_is_consumed_whole_so_no_tail_leaks() {
        let (out, n) = redacted(&format!("t={}\n", gh(60)));
        assert_eq!(
            (out.as_str(), n),
            (format!("t={REDACTION_MARKER}\n").as_str(), 1)
        );
        assert!(
            !out.contains("A1"),
            "no character of the token may remain: {out}"
        );
    }

    #[test]
    fn a_fine_grained_pat_needs_fifty_characters() {
        assert_eq!(redacted(&pat(50)), (REDACTION_MARKER.to_string(), 1));
        let short = pat(49);
        assert_eq!(redacted(&short), (short.clone(), 0));
    }

    #[test]
    fn an_sk_key_needs_twenty_characters_and_may_contain_hyphens() {
        assert_eq!(redacted(&sk(20)), (REDACTION_MARKER.to_string(), 1));
        let short = sk(19);
        assert_eq!(redacted(&short), (short.clone(), 0));
        let anthropic = format!("sk-ant-api03-{}", "e5".repeat(15));
        assert_eq!(redacted(&anthropic), (REDACTION_MARKER.to_string(), 1));
    }

    #[test]
    fn sk_inside_an_ordinary_word_is_left_alone() {
        // the `\b` before `sk-` is what keeps these out: the s follows a word character
        for word in [
            "risk-assessment-and-mitigation-plan",
            "task-force-with-a-long-suffix-abcd",
            "desk-lamp-with-a-very-long-hyphenated-name",
        ] {
            assert_eq!(redacted(word), (word.to_string(), 0), "{word}");
        }
    }

    #[test]
    fn an_aws_access_key_id_needs_sixteen_uppercase_or_digits_after_akia() {
        assert_eq!(redacted(&aws(16)), (REDACTION_MARKER.to_string(), 1));
        let short = format!("AKIA{}", "D4".repeat(7));
        assert_eq!(
            redacted(&short),
            (short.clone(), 0),
            "15 characters is not a key id"
        );
        let lower = format!("AKIA{}", "d4".repeat(8));
        assert_eq!(
            redacted(&lower),
            (lower.clone(), 0),
            "the id is uppercase and digits only"
        );
        let asia = format!("ASIA{}", "D4".repeat(8));
        assert_eq!(
            redacted(&asia),
            (REDACTION_MARKER.to_string(), 1),
            "a temporary key id"
        );
    }
    #[test]
    fn a_longer_uppercase_run_after_akia_is_not_a_key_id() {
        // the trailing `\b` keeps a 18-character run from being partly redacted as a 16-character id
        let run = format!("AKIA{}", "D4".repeat(9));
        assert_eq!(redacted(&run), (run.clone(), 0));
    }

    #[test]
    fn an_environment_dump_keeps_its_shape_and_loses_only_the_value() {
        let dump = format!("HOME=/home/x\nGITHUB_TOKEN={}\nPATH=/usr/bin\n", gh(36));
        let (out, n) = redacted(&dump);
        assert_eq!(n, 1);
        assert_eq!(
            out,
            format!("HOME=/home/x\nGITHUB_TOKEN={REDACTION_MARKER}\nPATH=/usr/bin\n")
        );
    }

    #[test]
    fn several_values_of_several_kinds_are_all_replaced_and_counted() {
        let text = format!("a {} b {} c {} d {} e", gh(36), pat(50), sk(20), aws(16));
        let (out, n) = redacted(&text);
        assert_eq!(n, 4);
        assert_eq!(
            out,
            format!("a {m} b {m} c {m} d {m} e", m = REDACTION_MARKER)
        );
    }

    #[test]
    fn redaction_is_idempotent() {
        let once = redacted(&format!("x {} y", gh(36))).0;
        assert_eq!(redacted(&once), (once.clone(), 0));
    }

    #[test]
    fn text_without_a_credential_is_returned_borrowed_and_unchanged() {
        let text = "cargo test\nrunning 3 tests\ntest result: ok. 3 passed\n";
        let r = redact_credentials(text);
        assert_eq!(r.count, 0);
        assert!(
            matches!(r.text, Cow::Borrowed(s) if s == text),
            "no allocation when nothing matched"
        );
    }

    #[test]
    fn a_quoted_token_keeps_its_quotes() {
        let (out, n) = redacted(&format!("\"{}\",\n", gh(36)));
        assert_eq!(
            (out.as_str(), n),
            (format!("\"{REDACTION_MARKER}\",\n").as_str(), 1)
        );
    }

    #[test]
    fn note_in_sums_into_an_existing_count_and_writes_nothing_for_zero() {
        let mut r = serde_json::json!({"stdout": "x"});
        note_in(&mut r, 0);
        assert!(
            r.get(REDACTED_KEY).is_none(),
            "an unedited response carries no key"
        );
        note_in(&mut r, 2);
        assert_eq!(r[REDACTED_KEY], 2);
        note_in(&mut r, 3);
        assert_eq!(r[REDACTED_KEY], 5, "counts from different sites add");
    }
}
