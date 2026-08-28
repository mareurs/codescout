//! The operator-rules ledger, compiled in.

use super::rule::{parse_ledger, Rule};
use std::sync::LazyLock;

/// The shipped ledger source.
///
/// `include_str!` rather than a runtime read: routing must work in every
/// project the server activates against, and this file exists only in the
/// codescout checkout. A disk read would deliver nothing, everywhere else,
/// with no error — the same silent-absence failure Gate 4 exists to rule out.
///
/// `compile`/`check` still read from disk (`super::LEDGER_PATH`), so editing a
/// rule and recompiling profiles needs no rebuild. Only ROUTING is pinned to
/// build time.
pub const LEDGER_SRC: &str = include_str!("../../docs/trackers/operator-rules.md");

/// Every rule in the shipped ledger, parsed once.
///
/// Panics on a malformed ledger. That is correct and deliberate: the test
/// below runs in the same build, so a ledger that would panic here fails
/// `cargo test` before any binary ships.
pub static OPERATOR_RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    parse_ledger(LEDGER_SRC).expect("the compiled-in operator-rules ledger must parse")
});

#[cfg(test)]
mod tests {
    use super::*;

    /// Gates 4 and 6 against the SHIPPED ledger, at build time.
    ///
    /// `compile`/`check` run these against whatever is on disk when a human
    /// invokes them. This runs them against the bytes actually compiled into
    /// the binary, which is what routing reads.
    #[test]
    fn the_shipped_ledger_parses_validates_and_fits_the_budget() {
        let rules = parse_ledger(LEDGER_SRC).expect("shipped ledger must parse");
        super::super::validate::validate(&rules).expect("shipped ledger must validate");
        super::super::budget::check_budget(&rules).expect("shipped ledger must fit the budget");
        assert!(
            rules.iter().any(|r| r.id == "OP-1"),
            "the ledger lost OP-1 — either the file moved or include_str! is pointing \
             at the wrong path; got ids {:?}",
            rules.iter().map(|r| &r.id).collect::<Vec<_>>()
        );
    }

    /// The lazy static must agree with a fresh parse. If `LazyLock` were ever
    /// pointed at a different source this is what catches it.
    #[test]
    fn the_lazy_corpus_matches_a_fresh_parse() {
        let fresh = parse_ledger(LEDGER_SRC).unwrap();
        assert_eq!(OPERATOR_RULES.len(), fresh.len());
    }
}
