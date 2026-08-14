//! CommonMark-correct fenced-code-block state tracking for line-oriented scanners.
//!
//! Every line-oriented markdown scanner in this tree used to keep a bare
//! `bool in_fence` and flip it on any line starting with three backticks. That
//! is wrong in two ways CommonMark cares about, and the first one bit us in
//! production:
//!
//! - **Run length.** A closing fence must be *at least as long* as the opening
//!   fence, so a three-backtick line nested inside a four-backtick block does
//!   not close it. A boolean toggle closed the outer block early and parsed the
//!   remainder of the block as document body, turning `#`-prefixed code comments
//!   into phantom headings.
//! - **Fence character.** Backtick and tilde fences are distinct; neither closes
//!   the other. A boolean toggle let a backtick run close a tilde block.
//!
//! CommonMark also requires a closing fence to be followed by nothing but
//! whitespace, and forbids backticks in a backtick fence's info string. Both
//! rules are enforced here — they are what distinguish a real delimiter from a
//! line that merely *starts* with backticks.
//!
//! See `docs/issues/2026-08-11-artifact-nested-fence-closes-outer-fence.md`.
//!
//! # Indentation is the caller's business
//!
//! [`FenceState::feed`] reads the fence run from byte 0 of whatever it is given
//! and never trims. Call sites in this tree disagree about leading whitespace —
//! some pass the raw line, some `trim_start()`, some `trim()` — and CommonMark's
//! real rule (up to three spaces, relative to the enclosing container) needs
//! full block parsing to get right. Preserving each site's existing trim keeps
//! this module's blast radius to the fence-length and fence-character rules,
//! which is the defect being fixed.

/// Tracks whether a line-oriented scan is currently inside a fenced code block.
///
/// Feed every line in order; consult [`in_fence`](Self::in_fence) to decide
/// whether the current line is code. The typical replacement for a boolean
/// toggle is:
///
/// ```ignore
/// let mut fence = FenceState::new();
/// for line in body.lines() {
///     if fence.feed(line) {
///         continue; // the delimiter line itself
///     }
///     if fence.in_fence() {
///         continue; // inside a code block
///     }
///     // ... treat `line` as document body
/// }
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FenceState {
    /// The open fence's character and run length, or `None` outside any fence.
    open: Option<(u8, usize)>,
}

/// Measure the leading run of `'`'` or `'~'` at the start of `line`.
///
/// Returns `(fence_char, run_len, rest_after_run)`, or `None` when `line` does
/// not begin with one of the two fence characters.
fn leading_run(line: &str) -> Option<(u8, usize, &str)> {
    let bytes = line.as_bytes();
    let first = *bytes.first()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let run = bytes.iter().take_while(|&&b| b == first).count();
    Some((first, run, &line[run..]))
}

impl FenceState {
    pub fn new() -> Self {
        Self::default()
    }

    /// True when the scan is currently inside a fenced code block.
    pub fn in_fence(&self) -> bool {
        self.open.is_some()
    }

    /// Feed one line and return `true` when that line is itself a fence
    /// delimiter (an opener or a closer) — callers normally skip those.
    ///
    /// A line that merely starts with fence characters but is not a valid
    /// delimiter in the current state returns `false`: a shorter run inside a
    /// longer block, a mismatched fence character, a closer with trailing
    /// content, or an opener whose backtick info string contains a backtick.
    /// Such a line is ordinary content, and while a fence is open
    /// [`in_fence`](Self::in_fence) still reports `true` for it.
    pub fn feed(&mut self, line: &str) -> bool {
        let Some((ch, run, rest)) = leading_run(line) else {
            return false;
        };
        match self.open {
            Some((open_ch, open_run)) => {
                // A closer matches the opening character, is at least as long,
                // and is followed by nothing but whitespace.
                if ch == open_ch && run >= open_run && rest.trim().is_empty() {
                    self.open = None;
                    return true;
                }
                false
            }
            None => {
                if run < 3 {
                    return false;
                }
                // A backtick fence's info string may not contain a backtick,
                // which is what keeps an inline code span such as
                // ```` ```lang ```` from opening a block.
                if ch == b'`' && rest.contains('`') {
                    return false;
                }
                self.open = Some((ch, run));
                true
            }
        }
    }
}

/// True when every fenced block across `lines` is closed — i.e. a full scan
/// ends outside any fence.
///
/// Callers use this as a pre-scan to decide whether fence tracking is
/// trustworthy at all. An in-flight batch edit can leave a half-fence behind,
/// and CommonMark would extend it to EOF, hiding every heading after it; an
/// editor would rather treat unbalanced fences as plain text and keep those
/// headings addressable. See
/// `docs/issues/2026-05-21-edit-markdown-last-heading-unaddressable.md`.
///
/// Pass the lines exactly as the main scan will see them (same trimming), or
/// the pre-scan and the scan can disagree.
pub fn fences_balanced<'a>(lines: impl Iterator<Item = &'a str>) -> bool {
    let mut state = FenceState::new();
    for line in lines {
        state.feed(line);
    }
    !state.in_fence()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect the lines a scanner would treat as document body.
    fn body_lines(text: &str) -> Vec<&str> {
        let mut fence = FenceState::new();
        let mut out = Vec::new();
        for line in text.lines() {
            if fence.feed(line) {
                continue;
            }
            if fence.in_fence() {
                continue;
            }
            out.push(line);
        }
        out
    }

    #[test]
    fn a_shorter_run_does_not_close_a_longer_fence() {
        // The regression under test: the inner three-backtick fence must not
        // close the outer four-backtick block, so the TOML comment stays code.
        let text = "\
before
````markdown
# Page Title
```toml
# .codescout/project.toml
```
````
after";
        assert_eq!(body_lines(text), vec!["before", "after"]);
    }

    #[test]
    fn a_backtick_run_does_not_close_a_tilde_fence() {
        let text = "\
before
~~~
```
# Not a heading
~~~
after";
        assert_eq!(body_lines(text), vec!["before", "after"]);
    }

    #[test]
    fn a_longer_run_closes_a_shorter_fence() {
        // CommonMark: the closer must be *at least* as long, not exactly.
        let text = "\
```
# code
````
after";
        assert_eq!(body_lines(text), vec!["after"]);
    }

    #[test]
    fn a_closer_with_trailing_content_is_not_a_closer() {
        let text = "\
````
# code
```` still code
# also code
````
after";
        assert_eq!(body_lines(text), vec!["after"]);
    }

    #[test]
    fn an_inline_code_span_of_a_fence_does_not_open_a_block() {
        // `feed` must reject this as an opener: a backtick fence's info string
        // may not contain backticks. Without that rule the rest of the document
        // is swallowed as code.
        let text = "\
```` ```markdown ````
# Real heading";
        assert_eq!(
            body_lines(text),
            vec!["```` ```markdown ````", "# Real heading"]
        );
    }

    #[test]
    fn a_run_shorter_than_three_never_opens_a_fence() {
        let text = "``\n# Real heading";
        assert_eq!(body_lines(text), vec!["``", "# Real heading"]);
    }

    #[test]
    fn a_tilde_fence_info_string_may_contain_backticks() {
        // The info-string restriction is backtick-only in CommonMark.
        let text = "\
~~~ `lang`
# code
~~~
after";
        assert_eq!(body_lines(text), vec!["after"]);
    }

    #[test]
    fn plain_nested_fences_still_round_trip() {
        let text = "\
# Title
```
## Not a heading
```
## Real heading";
        assert_eq!(body_lines(text), vec!["# Title", "## Real heading"]);
    }

    #[test]
    fn balanced_reports_an_unclosed_fence() {
        assert!(fences_balanced("```\ncode\n```".lines()));
        assert!(!fences_balanced("```\ncode".lines()));
    }

    #[test]
    fn balanced_is_not_a_parity_count() {
        // Three fence-ish lines, yet every block is closed: the middle line is
        // content inside the four-backtick block, not a delimiter. A parity
        // count calls this unbalanced and disables fence tracking entirely.
        let text = "````\n```\n````";
        assert_eq!(text.lines().filter(|l| l.starts_with("```")).count() % 2, 1);
        assert!(fences_balanced(text.lines()));
    }

    #[test]
    fn feed_reports_only_real_delimiters() {
        let mut fence = FenceState::new();
        assert!(fence.feed("````"), "opener");
        assert!(fence.in_fence());
        assert!(
            !fence.feed("```"),
            "shorter run is content, not a delimiter"
        );
        assert!(fence.in_fence(), "still inside the outer block");
        assert!(fence.feed("````"), "closer");
        assert!(!fence.in_fence());
    }
}
