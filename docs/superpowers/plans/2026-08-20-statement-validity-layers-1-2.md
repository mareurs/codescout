# Statement Validity — Layers 1–2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every tracker entry a declared decay class and a durable route to its proof, then ship three read-only `doctor` checks that surface the entries where the class is wrong *and* the entry is load-bearing.

**Architecture:** A section-bounded splitter in `link_scan::extract` (one file owns the markdown grammar, so the section rule cannot drift from the definition rule) feeds a pure parser in a new `src/librarian/statements.rs`. The parser never touches git or the catalog — callers supply the fallback date. The entry allocator stamps a default class into the sections it writes. `doctor` gains one exposure helper and three checks modelled on `scan_undefined_entries` / `scan_terminal_status_with_caveat`.

**Tech Stack:** Rust, `rusqlite`, `regex` (already vendored), `serde_json`, `git` CLI for line-grain blame. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md`

## Global Constraints

Copied verbatim from the spec. Every task's requirements implicitly include this section.

- **Exactly three classes**, no fourth without a distinct sweep action: `invariant`, `dated <YYYY-MM-DD>`, `conditional — <event text>`.
- **Detection anchors on line-start structure, never on a keyword.** The parser matches `^\*\*Valid:\*\*\s+` at line start inside the entry's section bounds. A sentence *about* the Valid field must not parse as a declaration.
- **`dated` MUST be followed by an ISO `YYYY-MM-DD`.** A non-date is a `RecoverableError` naming the three valid forms.
- **A bare `conditional` is refused** — the condition text is required.
- **Absence means `dated <the last commit touching the entry's HEADING line range>`** — not the file's mtime and not the file's last commit.
- **An entry's section ends at the next heading of the SAME OR HIGHER level.** Measured 2026-08-20: the naive nearest-preceding-heading rule scores 87.9%, and the 12.1% is the last entry in a file absorbing every citation in the trailing non-entry sections.
- **In-degree must exclude same-file citations.** 28.5% of ledger citations are index-table rows above the first definition; counting them lets an entry's own index row inflate its exposure.
- **Every check reports a worklist, never a verdict**, and every check is read-only — no `fix=`.
- **Layer 2 horizon: 30 days**, one configurable. Deliberately NOT `FRESHNESS_HORIZON_DEFAULT`, which is a commit distance of 50 that every call site passes `None` for.
- **Checks share one exposure gate**: population is `defaulted-or-stale` AND `exposure ≥ threshold`.
- **Pre-commit gate, every task:** `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`.
- **Errors:** `RecoverableError` for anything an agent can fix by re-calling; `anyhow::bail!` for programmer error. See `get_guide("error-handling")`.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/librarian/tools/link_scan/extract.rs` *(modify)* | Owns the markdown grammar. Gains `EntrySection` + `entry_sections()`, sharing `def_re` and the existing fence/frontmatter skipping with `extract()`. |
| `src/librarian/statements.rs` *(create)* | Pure parsing of a Statement's declared fields out of one section's text. No git, no catalog, no I/O. Sibling of `freshness.rs`. |
| `src/librarian/mod.rs` *(modify)* | `pub mod statements;` |
| `src/librarian/catalog/augmentation.rs` *(modify, ~line 1074)* | The allocator stamps a default `**Valid:**` line into the section it writes. |
| `src/librarian/tools/doctor.rs` *(modify)* | One exposure helper + three `scan_*` functions, wired into `call`. |
| `docs/templates/session-log.md` *(modify)* | The two new fields in the entry skeleton. |
| `src/prompts/source.md` *(NOT modified)* | Out of scope — no tool name changes, so no prompt-surface work. Stated so an executor does not go looking. |

---

### Task 1: Section-bounded entry splitting

**Files:**
- Modify: `src/librarian/tools/link_scan/extract.rs`
- Test: `src/librarian/tools/link_scan/extract.rs` (inline `mod tests`, matching the file's existing convention)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub struct EntrySection { pub id: String, pub level: usize, pub heading_line: u32, pub end_line: u32, pub text: String }` and `pub fn entry_sections(text: &str) -> Vec<EntrySection>`.

**Why here and not a new module:** `def_re` is private to this file, and duplicating it elsewhere is how a section rule drifts from the definition rule. `extract()` and `entry_sections()` must agree on what a definition is, what a fence is, and what frontmatter is, forever.

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` in `src/librarian/tools/link_scan/extract.rs`:

```rust
#[test]
fn entry_section_ends_at_next_same_or_higher_heading() {
    let md = "\
## R-1 — first
alpha
### a subheading inside R-1
beta
## R-2 — second
gamma
## Template for new entries
delta
";
    let s = entry_sections(md);
    assert_eq!(s.len(), 2, "two entries defined");
    assert_eq!(s[0].id, "R-1");
    assert_eq!(s[0].heading_line, 1);
    assert_eq!(
        s[0].end_line, 4,
        "the ### subheading is INSIDE R-1; the section ends before ## R-2"
    );
    assert!(s[0].text.contains("a subheading inside R-1"));
    assert_eq!(s[1].id, "R-2");
    assert_eq!(
        s[1].end_line, 6,
        "R-2 ends before `## Template`, a same-level non-entry heading — \
         the last entry must NOT absorb the trailing sections"
    );
    assert!(!s[1].text.contains("delta"));
}

#[test]
fn entry_sections_skip_fences_and_frontmatter() {
    let md = "\
---
kind: tracker
entry_prefix: R
---
## R-1 — real
body
```
## R-99 — inside a fence, defines nothing
```
tail
";
    let s = entry_sections(md);
    assert_eq!(s.len(), 1, "the fenced heading defines nothing");
    assert_eq!(s[0].id, "R-1");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout entry_section_ends_at_next_same_or_higher_heading`
Expected: FAIL — `cannot find function 'entry_sections' in this scope`

- [ ] **Step 3: Write minimal implementation**

Add near `Definition` in `src/librarian/tools/link_scan/extract.rs`:

```rust
/// One entry's section: its defining heading plus every line up to (not including)
/// the next heading of the SAME OR HIGHER level.
///
/// **Why the level bound and not "the next definition".** Measured 2026-08-20 over 12
/// ledgers: attributing a citation to the nearest preceding definition, without the
/// bound, is wrong on 12.1% of attributed citations — and the error is one mechanism,
/// the LAST entry in a file absorbing every citation in the trailing `## Summary` /
/// `## Template` sections. Four ledgers carried 109 of 123 errors. See
/// `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md`
/// § Layer 3 → Attribution, and `scripts/probe_entry_attribution.py`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySection {
    pub id: String,
    pub level: usize,
    /// 1-indexed line of the defining heading.
    pub heading_line: u32,
    /// 1-indexed last line of the section, inclusive.
    pub end_line: u32,
    /// The section's text, heading line included.
    pub text: String,
}

/// Split a body into entry sections, sharing `def_re` and the fence/frontmatter
/// skipping with [`extract`] so the two can never disagree about what a definition is.
pub fn entry_sections(text: &str) -> Vec<EntrySection> {
    let lines: Vec<&str> = text.split('\n').collect();
    // (level, 1-indexed line) for EVERY heading, entry or not — the bound is about
    // headings in general, which is exactly what the naive rule ignored.
    let mut headings: Vec<(usize, u32)> = Vec::new();
    let mut defs: Vec<(String, usize, u32)> = Vec::new();
    let mut fenced = false;
    let mut in_frontmatter = false;
    for (idx, raw) in lines.iter().enumerate() {
        let lineno = (idx + 1) as u32;
        let s = raw.trim();
        if lineno == 1 && s == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if s == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        if s.starts_with("```") || s.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        if hashes >= 1 && hashes <= 6 && raw.as_bytes().get(hashes) == Some(&b' ') {
            headings.push((hashes, lineno));
        }
        if let Some(c) = def_re().captures(raw) {
            let level = c.get(1).map(|m| m.as_str().len()).unwrap_or(2);
            defs.push((c[2].to_string(), level, lineno));
        }
    }
    let last = lines.len() as u32;
    defs.into_iter()
        .map(|(id, level, heading_line)| {
            let end_line = headings
                .iter()
                .find(|(hl, hln)| *hln > heading_line && *hl <= level)
                .map(|(_, hln)| hln - 1)
                .unwrap_or(last);
            let text = lines[(heading_line as usize - 1)..(end_line as usize)].join("\n");
            EntrySection { id, level, heading_line, end_line, text }
        })
        .collect()
}
```

If `def_re()`'s captures do not already expose the hash run as group 1 and the token as group 2, adjust the capture indices to match the existing pattern rather than changing the pattern.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout entry_section`
Expected: PASS, both tests

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/tools/link_scan/extract.rs
git commit -F- <<'MSG'
feat(link_scan): entry_sections — section-bounded entry splitting

An entry's section ends at the next heading of the SAME OR HIGHER level, not at
the next definition. Measured 2026-08-20 over 12 ledgers: the naive rule is
wrong on 12.1% of attributed citations, and the error is one mechanism — the
last entry in a file absorbing every citation in the trailing non-entry
sections. Lives beside def_re so the section rule cannot drift from the
definition rule.
MSG
```

---

### Task 2: The `**Valid:**` and `**Rests on:**` parser

**Files:**
- Create: `src/librarian/statements.rs`
- Modify: `src/librarian/mod.rs` (add `pub mod statements;`)
- Test: inline `mod tests` in `src/librarian/statements.rs`

**Interfaces:**
- Consumes: `EntrySection` from Task 1 (only its `text` field is used here).
- Produces:
  - `pub enum Validity { Invariant, Dated(String), Conditional { condition: String } }`
  - `pub fn parse_validity(section_text: &str) -> Result<Option<Validity>, RecoverableError>` — `Ok(None)` means no declaration.
  - `pub fn resolve_validity(section_text: &str, fallback_date: &str) -> Result<Validity, RecoverableError>` — applies default-is-decay.
  - `pub fn parse_rests_on(section_text: &str) -> Option<String>`

**Purity constraint:** this module does no I/O. It never reads git, the catalog, or the clock. `resolve_validity` takes the fallback date as a parameter, which is what makes it testable without a fixture repo.

- [ ] **Step 1: Write the failing test**

Create `src/librarian/statements.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_three_classes() {
        assert_eq!(
            parse_validity("## R-1 — x\n\n**Valid:** invariant\n").unwrap(),
            Some(Validity::Invariant)
        );
        assert_eq!(
            parse_validity("**Valid:** dated 2026-08-20\n").unwrap(),
            Some(Validity::Dated("2026-08-20".into()))
        );
        assert_eq!(
            parse_validity("**Valid:** conditional — until the plan edit lands\n").unwrap(),
            Some(Validity::Conditional { condition: "until the plan edit lands".into() })
        );
    }

    /// Structure, not keyword. Prose that merely NAMES the field is not a declaration —
    /// `get_guide("tracker-conventions")` records that `grep -c 'Status:'` counting
    /// sentences about Status is a mistake made twice in one pass by one agent.
    #[test]
    fn prose_mentioning_the_field_is_not_a_declaration() {
        assert_eq!(
            parse_validity("the **Valid:** field is required on every entry\n").unwrap(),
            None,
            "mid-line mention must not parse"
        );
        assert_eq!(parse_validity("Valid: invariant\n").unwrap(), None, "no bold markers");
    }

    #[test]
    fn dated_requires_an_iso_date() {
        let err = parse_validity("**Valid:** dated soon\n").unwrap_err();
        let t = err.to_string();
        assert!(t.contains("YYYY-MM-DD"), "must name the required shape: {t}");
        assert!(t.contains("invariant"), "must name all three forms: {t}");
    }

    #[test]
    fn bare_conditional_is_refused() {
        let err = parse_validity("**Valid:** conditional\n").unwrap_err();
        assert!(
            err.to_string().contains("condition"),
            "a condition nobody named can only produce 'go re-read this'"
        );
    }

    #[test]
    fn unknown_class_is_refused() {
        assert!(parse_validity("**Valid:** eternal\n").is_err());
    }

    /// Absence means decay — the conservative reading, and the one that costs the
    /// common case zero writes.
    #[test]
    fn absent_declaration_defaults_to_dated_fallback() {
        assert_eq!(
            resolve_validity("## R-1 — x\n\nno declaration here\n", "2026-06-14").unwrap(),
            Validity::Dated("2026-06-14".into())
        );
        assert_eq!(
            resolve_validity("**Valid:** invariant\n", "2026-06-14").unwrap(),
            Validity::Invariant,
            "an explicit class always wins over the fallback"
        );
    }

    #[test]
    fn rests_on_is_line_anchored_and_optional() {
        assert_eq!(
            parse_rests_on("**Rests on:** ADR 2026-07-10 — repair-and-continue\n"),
            Some("ADR 2026-07-10 — repair-and-continue".to_string())
        );
        assert_eq!(parse_rests_on("no such field\n"), None);
        assert_eq!(
            parse_rests_on("see the **Rests on:** field\n"),
            None,
            "mid-line mention is not a declaration"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout statements::`
Expected: FAIL — module not declared / `Validity` not found

- [ ] **Step 3: Write minimal implementation**

Prepend to `src/librarian/statements.rs`:

```rust
//! Parsing a Statement's declared fields out of one entry section.
//!
//! A **Statement** is the claim an entry asserts. An entry is the markdown section;
//! not every entry is a Statement — a backlog item asserts nothing. Declaring a
//! validity class is what makes an entry one.
//!
//! This module is pure: no git, no catalog, no clock. `resolve_validity` takes the
//! fallback date as a parameter so the default-is-decay rule is testable without a
//! fixture repository.

use crate::librarian::tools::RecoverableError;
use once_cell::sync::Lazy;
use regex::Regex;

/// The decay class a Statement declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Validity {
    /// A law. No expiry. What gets promoted.
    Invariant,
    /// True of an instant. The ISO `YYYY-MM-DD` it was true on.
    Dated(String),
    /// True until a named event fires.
    Conditional { condition: String },
}

// Line-anchored by construction: prose and field share a vocabulary, so a keyword
// match also counts sentences ABOUT the field.
static VALID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\*\*Valid:\*\*[ \t]+(.+?)[ \t]*$").unwrap());
static RESTS_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\*\*Rests on:\*\*[ \t]+(.+?)[ \t]*$").unwrap());
static ISO_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());

const FORMS: &str = "**Valid:** invariant | dated YYYY-MM-DD | conditional — <event>";

/// Parse a declared class. `Ok(None)` means the section declares nothing.
pub fn parse_validity(section_text: &str) -> Result<Option<Validity>, RecoverableError> {
    let Some(c) = VALID_RE.captures(section_text) else {
        return Ok(None);
    };
    let rest = c[1].trim();
    if rest == "invariant" {
        return Ok(Some(Validity::Invariant));
    }
    if let Some(d) = rest.strip_prefix("dated ") {
        let d = d.trim();
        if !ISO_RE.is_match(d) {
            return Err(RecoverableError::with_hint(
                format!("`**Valid:** dated {d}` is not an ISO date"),
                format!("Use `dated YYYY-MM-DD`. The three forms are: {FORMS}"),
            ));
        }
        return Ok(Some(Validity::Dated(d.to_string())));
    }
    if rest == "conditional" || rest.starts_with("conditional") {
        let cond = rest
            .trim_start_matches("conditional")
            .trim_start_matches(['—', '–', '-'])
            .trim();
        if cond.is_empty() {
            return Err(RecoverableError::with_hint(
                "`**Valid:** conditional` names no condition".to_string(),
                format!(
                    "Name the event that ends validity: `conditional — <event>`. A \
                     condition nobody named can only produce \"go re-read this\". {FORMS}"
                ),
            ));
        }
        return Ok(Some(Validity::Conditional { condition: cond.to_string() }));
    }
    Err(RecoverableError::with_hint(
        format!("`**Valid:** {rest}` is not a known class"),
        format!("The three forms are: {FORMS}"),
    ))
}

/// Apply default-is-decay: an undeclared Statement is `dated <fallback_date>`.
pub fn resolve_validity(
    section_text: &str,
    fallback_date: &str,
) -> Result<Validity, RecoverableError> {
    Ok(parse_validity(section_text)?
        .unwrap_or_else(|| Validity::Dated(fallback_date.to_string())))
}

/// The durable route to this Statement's proof, if it declares one.
pub fn parse_rests_on(section_text: &str) -> Option<String> {
    RESTS_RE
        .captures(section_text)
        .map(|c| c[1].trim().to_string())
}
```

Add to `src/librarian/mod.rs`, alphabetically among the existing `pub mod` lines:

```rust
pub mod statements;
```

If `once_cell` is not already a dependency, use the same lazy-static idiom `extract.rs` uses (`OnceLock<Regex>` + an accessor fn) rather than adding a crate.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout statements::`
Expected: PASS, 6 tests

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/statements.rs src/librarian/mod.rs
git commit -F- <<'MSG'
feat(statements): parse **Valid:** and **Rests on:** from an entry section

Three classes, line-anchored so prose naming the field does not parse as a
declaration. `dated` requires an ISO date and a bare `conditional` is refused —
a condition nobody named can only produce "go re-read this", which is the nudge
that left 34 of 61 entries status-less for three months.

Pure by construction: resolve_validity takes the fallback date as a parameter,
so default-is-decay is testable without a fixture repository.
MSG
```

---

### Task 3: The allocator stamps a default class

**Files:**
- Modify: `src/librarian/catalog/augmentation.rs` (the `section_text` construction, ~line 1074)
- Test: inline `mod tests` in the same file, beside the existing `PendingSection` tests

**Interfaces:**
- Consumes: nothing (the stamp is a literal string; it does not call Task 2).
- Produces: sections written by `allocate_entry_id` carry `**Valid:** dated <today>` unless `PendingSection.body` already declares a class.

**Why the allocator and not the caller:** `PendingSection`'s own doc comment states it — a caller that wrote the section afterwards would do a second read-modify-write outside the `IMMEDIATE` transaction, and a peer session allocating in between gets its committed mark clobbered, walking the counter backwards. One file write, one transaction. Callers never format the heading; they now never format this line either, for the same reason (CAP-5 defect class 2).

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn allocator_stamps_a_default_validity_into_the_section_it_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("ledger.md").to_string_lossy().to_string();
    std::fs::write(
        &md,
        "---\nkind: tracker\nentry_prefix: U\n---\n\n## U-1 — first\n\nx\n\n## Template for new entries\n",
    )
    .unwrap();
    let mut cat = Catalog::open_in_memory().unwrap();
    let mut art = sample_art("art1");
    art.abs_path = md.clone();
    art_upsert(&cat, &art).unwrap();

    let section = PendingSection {
        title: "server wrote this".to_string(),
        body: "the prose".to_string(),
        anchor_heading: "## Template for new entries".to_string(),
    };
    allocate_entry_id(&mut cat, "art1", "U", Some(&section)).unwrap();

    let written = std::fs::read_to_string(&md).unwrap();
    assert!(
        written.contains("**Valid:** dated "),
        "a section the server writes must be born with a declared class:\n{written}"
    );
    assert!(
        written.contains("the prose"),
        "the caller's prose survives the stamp"
    );
}

#[test]
fn allocator_does_not_double_stamp_a_caller_declared_class() {
    let tmp = tempfile::tempdir().unwrap();
    let md = tmp.path().join("ledger.md").to_string_lossy().to_string();
    std::fs::write(
        &md,
        "---\nkind: tracker\nentry_prefix: U\n---\n\n## U-1 — first\n\nx\n\n## Template for new entries\n",
    )
    .unwrap();
    let mut cat = Catalog::open_in_memory().unwrap();
    let mut art = sample_art("art1");
    art.abs_path = md.clone();
    art_upsert(&cat, &art).unwrap();

    let section = PendingSection {
        title: "a law".to_string(),
        body: "**Valid:** invariant\n\nthe prose".to_string(),
        anchor_heading: "## Template for new entries".to_string(),
    };
    allocate_entry_id(&mut cat, "art1", "U", Some(&section)).unwrap();

    let written = std::fs::read_to_string(&md).unwrap();
    assert_eq!(
        written.matches("**Valid:**").count(),
        1,
        "an explicit class must not be joined by a stamped default:\n{written}"
    );
    assert!(written.contains("**Valid:** invariant"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout allocator_stamps_a_default_validity`
Expected: FAIL — assertion `written.contains("**Valid:** dated ")` fails; the section carries no class

- [ ] **Step 3: Write minimal implementation**

Replace the `section_text` construction in `allocate_entry_id`:

```rust
            let heading = "#".repeat(level);
            // Every section the server writes is born with a declared decay class, the
            // same way it is born with a def_re-conformant heading: by construction,
            // not by convention. A caller that already declared one is left alone —
            // double-stamping would make the parser's first match arbitrary.
            let prose = s.body.trim_end();
            let stamped = if crate::librarian::statements::parse_validity(prose)
                .ok()
                .flatten()
                .is_some()
            {
                prose.to_string()
            } else {
                format!("**Valid:** dated {}\n\n{prose}", today_iso())
            };
            // Trailing blank line so the anchor heading that follows is not glued to
            // this section's last prose line. Caught by reading a mutation test's
            // failure output, which printed `the prose\n## Template for new entries`.
            let section_text = format!("{heading} {id} — {}\n\n{stamped}\n\n", s.title);
```

And add, near the other private helpers in the same file:

```rust
/// Today as `YYYY-MM-DD`, UTC. Split out so the stamp has one source of truth.
fn today_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    // days since epoch -> civil date (Howard Hinnant's algorithm)
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}
```

If the crate already exposes a date helper (check `src/util/`), call that instead of adding this one — a second date formatter is exactly the kind of duplication that drifts.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout allocator`
Expected: PASS, including the pre-existing `PendingSection` tests (they assert on the heading and the prose, both preserved)

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/catalog/augmentation.rs
git commit -F- <<'MSG'
feat(augmentation): allocator stamps a default **Valid:** into the section it writes

New entries are born with a declared decay class the same way they are born with
a def_re-conformant heading — by construction. A caller that declared one
explicitly is left alone; double-stamping would make the parser's first match
arbitrary.

The stamp lives in allocate_entry_id rather than in a caller for the reason
PendingSection's own doc comment gives: a second read-modify-write outside the
IMMEDIATE transaction lets a peer session clobber the committed high-water mark.
MSG
```

---

### Task 4: Per-entry exposure — cross-file citation in-degree

**Files:**
- Modify: `src/librarian/tools/doctor.rs`
- Test: inline `mod tests` in the same file

**Interfaces:**
- Consumes: `extract` and `CitationKind` (already imported by `corpus_cited_tokens`); `entry_sections` from Task 1.
- Produces: `fn entry_indegree(conn: &rusqlite::Connection) -> Result<std::collections::BTreeMap<String, usize>>` — token → number of citations from files that do NOT define it.

**Why this needs no `entry_cite` and no slugs:** exposure is a *target-side* count. Which entry a citation came FROM requires attribution; which entry it points AT does not. That is what lets Layer 2 ship before Layer 3.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn indegree_counts_cross_file_citations_and_excludes_the_definer() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.md");
    let b = tmp.path().join("b.md");
    // a.md DEFINES R-1 and cites it twice in its own index table and prose.
    std::fs::write(
        &a,
        "## Index\n| R-1 | x |\n\n## R-1 — the law\n\nas R-1 says, ...\n",
    )
    .unwrap();
    // b.md cites R-1 twice from outside.
    std::fs::write(&b, "see R-1\n\nand again R-1\n").unwrap();

    let cat = Catalog::open_in_memory().unwrap();
    for (id, p) in [("a", &a), ("b", &b)] {
        let mut art = sample_art(id);
        art.abs_path = p.to_string_lossy().to_string();
        art_upsert(&cat, &art).unwrap();
    }

    let deg = entry_indegree(&cat.conn).unwrap();
    assert_eq!(
        deg.get("R-1").copied(),
        Some(2),
        "only b.md's two citations count — a.md defines R-1, so its own index row \
         and prose must not inflate its exposure"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout indegree_counts_cross_file`
Expected: FAIL — `cannot find function 'entry_indegree'`

- [ ] **Step 3: Write minimal implementation**

Add beside `corpus_cited_tokens` in `src/librarian/tools/doctor.rs`:

```rust
/// Token → how many citations reach it from files that do NOT define it.
///
/// **Exposure is a target-side count**, which is why Layer 2 does not need the entry
/// graph: knowing which entry a citation came FROM requires attribution and slugs;
/// knowing which entry it points AT does not.
///
/// **Same-file citations are excluded, and that is load-bearing.** Measured 2026-08-20:
/// 407 of 1427 ledger citations (28.5%) sit above the first definition — they are
/// hand-maintained `## Index` rows. Counting them would let an entry's own index row
/// inflate its own exposure, which is a self-reference wearing exposure's clothes.
/// `link_scan` already reports these separately as `SelfCite`.
///
/// Computed fresh from the files for the same reason `corpus_cited_tokens` is: a check
/// that reads a materialized table reports on whatever the last scan left behind.
fn entry_indegree(
    conn: &rusqlite::Connection,
) -> Result<std::collections::BTreeMap<String, usize>> {
    use crate::librarian::tools::link_scan::extract::{entry_sections, extract, CitationKind};

    let mut stmt = conn.prepare("SELECT abs_path FROM artifact ORDER BY abs_path")?;
    let paths: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;

    let mut bodies: Vec<(String, String)> = Vec::new();
    let mut definer: std::collections::BTreeMap<String, std::collections::BTreeSet<String>> =
        Default::default();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for s in entry_sections(&text) {
            definer.entry(s.id).or_default().insert(path.clone());
        }
        bodies.push((path, text));
    }

    let mut deg: std::collections::BTreeMap<String, usize> = Default::default();
    for (path, text) in &bodies {
        for c in extract(text).citations {
            let token = match c.kind {
                CitationKind::EntryToken => c.raw,
                // A file-stem or repo qualifier is one syntactic shape extraction cannot
                // tell apart; taking the token half is what stops a qualified citation
                // reading as no citation at all. Same choice `corpus_cited_tokens` makes.
                CitationKind::CrossRepoToken => match c.raw.rsplit_once(':') {
                    Some((_, t)) => t.to_string(),
                    None => continue,
                },
                _ => continue,
            };
            if definer.get(&token).is_some_and(|d| d.contains(path)) {
                continue; // same-file: SelfCite, never exposure
            }
            *deg.entry(token).or_insert(0) += 1;
        }
    }
    Ok(deg)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p codescout indegree_counts_cross_file`
Expected: PASS

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/tools/doctor.rs
git commit -F- <<'MSG'
feat(doctor): entry_indegree — per-entry cross-file citation exposure

Exposure is a target-side count, so it needs neither the entry graph nor slugs:
which entry a citation points AT requires no attribution. That is what lets the
Layer 2 checks ship before Layer 3.

Same-file citations are excluded. Measured 2026-08-20: 407 of 1427 ledger
citations sit above the first definition and are hand-maintained index rows;
counting them would let an entry's own index row inflate its own exposure.
MSG
```

---

### Task 5: `entry_conditional_past_due`

**Files:**
- Modify: `src/librarian/tools/doctor.rs`
- Test: inline `mod tests`

**Interfaces:**
- Consumes: `entry_sections` (Task 1), `parse_validity` (Task 2), `entry_indegree` (Task 4).
- Produces: `fn scan_conditional_past_due(conn: &rusqlite::Connection, indegree: &BTreeMap<String, usize>) -> Result<Vec<Violation>>`, and the constants `const VALIDITY_HORIZON_DAYS: i64 = 30;` / `const EXPOSURE_THRESHOLD: usize = 5;` shared by Tasks 5–7.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn conditional_past_due_fires_only_above_the_exposure_gate() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("led.md");
    std::fs::write(
        &p,
        "## R-1 — exposed\n\n**Valid:** conditional — until the plan edit lands\n\n\
         ## R-2 — ignored\n\n**Valid:** conditional — until something else\n",
    )
    .unwrap();
    let cat = Catalog::open_in_memory().unwrap();
    let mut art = sample_art("led");
    art.abs_path = p.to_string_lossy().to_string();
    art_upsert(&cat, &art).unwrap();

    let mut deg = std::collections::BTreeMap::new();
    deg.insert("R-1".to_string(), 9usize);
    deg.insert("R-2".to_string(), 1usize);

    let v = scan_conditional_past_due(&cat.conn, &deg).unwrap();
    let ids: Vec<&str> = v.iter().map(|x| x.detail.as_str()).collect();
    assert_eq!(v.len(), 1, "only the exposed conditional is worth anyone's time: {ids:?}");
    assert!(v[0].detail.contains("R-1"));
    assert!(
        v[0].detail.contains("until the plan edit lands"),
        "the worklist must carry the condition to adjudicate, not just an id"
    );
    assert_eq!(v[0].check, "entry_conditional_past_due");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout conditional_past_due_fires_only`
Expected: FAIL — `cannot find function 'scan_conditional_past_due'`

- [ ] **Step 3: Write minimal implementation**

```rust
/// Days after which a declared `conditional` is worth re-reading.
///
/// Deliberately NOT `FRESHNESS_HORIZON_DEFAULT`: that is a commit distance of 50 whose
/// own doc comment says every call site passes `topo_distance_from_head: None`, so it
/// has never been exercised. 30 days is a guess — the verify-open cadence in CLAUDE.md
/// uses 14 for `Status: open`, and a decay horizon should be looser than a triage one.
/// Re-tune from the first month's output.
const VALIDITY_HORIZON_DAYS: i64 = 30;

/// Cross-file citations below which a Statement is not worth anyone's attention.
///
/// The gate is shared by every check in this family, on purpose. Two checks producing
/// work independently is how a backlog becomes the steady state: as of June 2025 more
/// than 604,000 English Wikipedia pages carried at least one `{{citation needed}}`.
const EXPOSURE_THRESHOLD: usize = 5;

/// A declared `conditional` whose named event may already have fired.
///
/// **Reports a worklist, never a verdict.** Selection is syntactic and cheap; whether
/// the condition actually fired is the reader's judgement, and always will be. The
/// detail carries the condition text so the reader can adjudicate without reopening
/// the file.
fn scan_conditional_past_due(
    conn: &rusqlite::Connection,
    indegree: &std::collections::BTreeMap<String, usize>,
) -> Result<Vec<Violation>> {
    use crate::librarian::statements::{parse_validity, Validity};
    use crate::librarian::tools::link_scan::extract::entry_sections;

    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for (aid, path) in rows {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for s in entry_sections(&text) {
            let exposure = indegree.get(&s.id).copied().unwrap_or(0);
            if exposure < EXPOSURE_THRESHOLD {
                continue;
            }
            // A malformed declaration is `validity_unparseable`'s business, not this
            // check's — swallowing it here would hide it, reporting it here would
            // duplicate it.
            let Ok(Some(Validity::Conditional { condition })) = parse_validity(&s.text) else {
                continue;
            };
            out.push(Violation::new(
                "entry_conditional_past_due",
                Some(aid.clone()),
                path.clone(),
                format!(
                    "{} (exposure {exposure}) is conditional on: {condition} — \
                     check whether that has happened; this is a worklist, not a verdict",
                    s.id
                ),
            ));
        }
    }
    Ok(out)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p codescout conditional_past_due`
Expected: PASS

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/tools/doctor.rs
git commit -F- <<'MSG'
feat(doctor): entry_conditional_past_due, gated on shared exposure

A declared conditional whose named event may already have fired. Selection is
syntactic; adjudication is the reader's, and the detail carries the condition
text so it can be judged without reopening the file.

The exposure gate is shared with the checks that follow, deliberately. Two
checks producing work independently is how a backlog becomes the steady state.
MSG
```

---

### Task 6: `entry_dated_stale`, ranked by exposure

**Files:**
- Modify: `src/librarian/tools/doctor.rs`
- Test: inline `mod tests`

**Interfaces:**
- Consumes: `entry_sections`, `resolve_validity`, `entry_indegree`, `VALIDITY_HORIZON_DAYS`, `EXPOSURE_THRESHOLD`.
- Produces: `fn scan_dated_stale(conn, indegree, today_epoch_days: i64) -> Result<Vec<Violation>>`, ordered by exposure descending.

**On the fallback date:** the spec's default-is-decay uses the entry heading's last commit. That needs `git blame --line-porcelain`, which is one subprocess per file. This task takes `today_epoch_days` as a parameter and uses the *declared* date only; entries with no declaration are handled by Task 7, which reports them as undeclared rather than guessing their age. Wiring blame in is a follow-on with its own cost measurement — see the spec's Layer 1 § Default, which names three options and defers the choice to measurement.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn dated_stale_ranks_by_exposure_descending() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("led.md");
    std::fs::write(
        &p,
        "## R-1 — low\n\n**Valid:** dated 2020-01-01\n\n\
         ## R-2 — high\n\n**Valid:** dated 2020-01-01\n\n\
         ## R-3 — fresh\n\n**Valid:** dated 2999-01-01\n",
    )
    .unwrap();
    let cat = Catalog::open_in_memory().unwrap();
    let mut art = sample_art("led");
    art.abs_path = p.to_string_lossy().to_string();
    art_upsert(&cat, &art).unwrap();

    let mut deg = std::collections::BTreeMap::new();
    deg.insert("R-1".to_string(), 6usize);
    deg.insert("R-2".to_string(), 40usize);
    deg.insert("R-3".to_string(), 99usize);

    // 2026-08-20 as days since epoch.
    let v = scan_dated_stale(&cat.conn, &deg, 20_685).unwrap();
    let ids: Vec<String> = v
        .iter()
        .map(|x| x.detail.split_whitespace().next().unwrap().to_string())
        .collect();
    assert_eq!(
        ids,
        vec!["R-2", "R-1"],
        "R-3 is inside the horizon; the rest are ordered by exposure, \
         because an unranked list of every dated entry will be ignored"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout dated_stale_ranks_by_exposure`
Expected: FAIL — `cannot find function 'scan_dated_stale'`

- [ ] **Step 3: Write minimal implementation**

```rust
/// Parse `YYYY-MM-DD` to days since the Unix epoch. `None` if not a valid date.
fn iso_to_epoch_days(iso: &str) -> Option<i64> {
    let mut it = iso.split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: i64 = it.next()?.parse().ok()?;
    let d: i64 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Declared `dated` Statements past the horizon, **ranked by exposure descending**.
///
/// The ranking is load-bearing, not a nicety. A decayed fact nothing cites costs
/// nothing; one cited from a promoted skill costs a lot. An unranked list of every
/// dated entry past a horizon is thousands of rows and will be ignored — which is the
/// same outcome as not shipping the check, at higher cost. False-positive rates of
/// 18–86% are enough to make developers discard a static-analysis tool's entire output.
fn scan_dated_stale(
    conn: &rusqlite::Connection,
    indegree: &std::collections::BTreeMap<String, usize>,
    today_epoch_days: i64,
) -> Result<Vec<Violation>> {
    use crate::librarian::statements::{parse_validity, Validity};
    use crate::librarian::tools::link_scan::extract::entry_sections;

    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut scored: Vec<(usize, Violation)> = Vec::new();
    for (aid, path) in rows {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for s in entry_sections(&text) {
            let exposure = indegree.get(&s.id).copied().unwrap_or(0);
            if exposure < EXPOSURE_THRESHOLD {
                continue;
            }
            let Ok(Some(Validity::Dated(iso))) = parse_validity(&s.text) else {
                continue;
            };
            let Some(days) = iso_to_epoch_days(&iso) else {
                continue;
            };
            let age = today_epoch_days - days;
            if age < VALIDITY_HORIZON_DAYS {
                continue;
            }
            scored.push((
                exposure,
                Violation::new(
                    "entry_dated_stale",
                    Some(aid.clone()),
                    path.clone(),
                    format!(
                        "{} dated {iso} ({age}d old, exposure {exposure}) — re-run the \
                         measurement and record the new figure; this is a worklist, \
                         not a verdict",
                        s.id
                    ),
                ),
            ));
        }
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(scored.into_iter().map(|(_, v)| v).collect())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p codescout dated_stale`
Expected: PASS

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/tools/doctor.rs
git commit -F- <<'MSG'
feat(doctor): entry_dated_stale, ranked by exposure descending

The ranking is load-bearing. A decayed fact nothing cites costs nothing; one
cited from a promoted skill costs a lot. An unranked list of every dated entry
past a horizon is thousands of rows and will be ignored — the same outcome as
not shipping the check, at higher cost.
MSG
```

---

### Task 7: `entry_cited_from_outside_but_undeclared`, and wire all three in

**Files:**
- Modify: `src/librarian/tools/doctor.rs` (new scan + three `all_violations.extend(...)` lines in `call`)
- Test: inline `mod tests`

**Interfaces:**
- Consumes: everything from Tasks 1, 2, 4–6.
- Produces: `fn scan_cited_but_undeclared(conn, indegree) -> Result<Vec<Violation>>`; all three checks appear in `doctor`'s report.

**What this check deliberately does NOT claim.** It reports *"this is load-bearing and undeclared"*, never *"this is promoted"*. Measured 2026-08-20: a promotion, an eval-fixture list, and a kin reference are syntactically identical — `grep -c '<id>'` counts any mention, and using it as a promotion predicate mislabelled three of five entries in commit `9a982ed5`; a narrowed regex was also wrong. That direction stays human.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn cited_but_undeclared_reports_load_bearing_entries_with_no_class() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("led.md");
    std::fs::write(
        &p,
        "## R-1 — declared\n\n**Valid:** invariant\n\n\
         ## R-2 — undeclared but load-bearing\n\nprose with no class\n\n\
         ## R-3 — undeclared and unread\n\nalso nothing\n",
    )
    .unwrap();
    let cat = Catalog::open_in_memory().unwrap();
    let mut art = sample_art("led");
    art.abs_path = p.to_string_lossy().to_string();
    art_upsert(&cat, &art).unwrap();

    let mut deg = std::collections::BTreeMap::new();
    deg.insert("R-1".to_string(), 20usize);
    deg.insert("R-2".to_string(), 20usize);
    deg.insert("R-3".to_string(), 1usize);

    let v = scan_cited_but_undeclared(&cat.conn, &deg).unwrap();
    assert_eq!(v.len(), 1, "R-1 declares a class; R-3 is below the gate");
    assert!(v[0].detail.contains("R-2"));
    assert_eq!(v[0].check, "entry_cited_from_outside_but_undeclared");
    assert!(
        !v[0].detail.contains("promoted"),
        "this check must never claim to know WHY an entry is cited — a promotion, \
         an eval-fixture list and a kin reference are syntactically identical"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p codescout cited_but_undeclared_reports`
Expected: FAIL — `cannot find function 'scan_cited_but_undeclared'`

- [ ] **Step 3: Write minimal implementation**

```rust
/// A Statement other files depend on that declares no decay class at all.
///
/// The inverse of the checks above: they read a declaration, this one reports its
/// absence where absence costs something. These are the de-facto promotions — `R-41`
/// and `R-42` were genuinely promoted and declared nowhere.
///
/// **It reports "load-bearing and undeclared", never "promoted".** Measured 2026-08-20:
/// a promotion, an eval-fixture list and a kin reference are syntactically identical.
fn scan_cited_but_undeclared(
    conn: &rusqlite::Connection,
    indegree: &std::collections::BTreeMap<String, usize>,
) -> Result<Vec<Violation>> {
    use crate::librarian::statements::parse_validity;
    use crate::librarian::tools::link_scan::extract::entry_sections;

    let mut stmt = conn.prepare("SELECT id, abs_path FROM artifact ORDER BY abs_path")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for (aid, path) in rows {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for s in entry_sections(&text) {
            let exposure = indegree.get(&s.id).copied().unwrap_or(0);
            if exposure < EXPOSURE_THRESHOLD {
                continue;
            }
            if !matches!(parse_validity(&s.text), Ok(None)) {
                continue; // declares one, or is malformed — either way not this check
            }
            out.push(Violation::new(
                "entry_cited_from_outside_but_undeclared",
                Some(aid.clone()),
                path.clone(),
                format!(
                    "{} is cited {exposure}× from other files and declares no \
                     **Valid:** class — add one; this is a worklist, not a verdict",
                    s.id
                ),
            ));
        }
    }
    Ok(out)
}
```

Then wire all three into `call`, immediately after the `scan_terminal_status_with_caveat` line (~213):

```rust
    // One exposure computation, three consumers. Sharing it is not an optimisation:
    // checks that gate independently sum their backlogs, and a worklist nobody reads
    // is the same outcome as no check at all.
    let indegree = entry_indegree(&cat.conn)?;
    all_violations.extend(scan_conditional_past_due(&cat.conn, &indegree)?);
    all_violations.extend(scan_dated_stale(&cat.conn, &indegree, today_epoch_days())?);
    all_violations.extend(scan_cited_but_undeclared(&cat.conn, &indegree)?);
```

Add the small helper beside `iso_to_epoch_days`:

```rust
fn today_epoch_days() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}
```

- [ ] **Step 4: Run tests and verify the checks appear in a live report**

Run: `cargo test -p codescout` (whole suite — `call`'s report shape has existing assertions)
Expected: PASS

Then, after `cargo rb` and `/mcp`:

```
librarian(action="doctor")
```

Expected: the report contains `entry_conditional_past_due`, `entry_dated_stale`, and `entry_cited_from_outside_but_undeclared`. Every count may legitimately be 0 on a corpus that has no `**Valid:**` lines yet except those the allocator has stamped — **a zero here is evidence about the corpus, not about the check.** Confirm the checks are present in the report keys, not that they fired.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/librarian/tools/doctor.rs
git commit -F- <<'MSG'
feat(doctor): entry_cited_from_outside_but_undeclared, and wire the three checks

Reports a Statement other files depend on that declares no decay class — the
de-facto promotions. It says "load-bearing and undeclared", never "promoted":
measured 2026-08-20, a promotion, an eval-fixture list and a kin reference are
syntactically identical, and using a mention count as a promotion predicate
mislabelled three of five entries in 9a982ed5.

One exposure computation, three consumers. Checks that gate independently sum
their backlogs.
MSG
```

---

### Task 8: Document the two fields where authors will meet them

**Files:**
- Modify: `docs/templates/session-log.md`
- Modify: `src/librarian/tools/get_guide/` — the `tracker-conventions` topic source (locate with `grep(pattern="Required fields", glob="src/**")`; it is an `include_str!`'d markdown file)
- Test: `cargo test -p codescout` — the guide is `include_str!`'d, and `*_invariants` tests assert on its rendered content

**Interfaces:**
- Consumes: the grammar fixed in Task 2.
- Produces: nothing code-facing.

**Before editing the guide source, enumerate its tests.** `get_guide` topics are `include_str!`'d constants, and size-cap / required-mention invariants fire downstream of "it's just a doc change":

```
grep(pattern="tracker-conventions|TRACKER_CONVENTIONS", glob="src/**/*.rs", mode="content")
```

- [ ] **Step 1: Add the fields to the session-log template**

In `docs/templates/session-log.md`, in the entry skeleton beneath the `**Status:**` line:

```markdown
**Valid:** invariant | dated YYYY-MM-DD | conditional — <the event that ends it>

**Rests on:** <one durable sentence — an ADR, a decision, or the principle this
instantiates. Names something the resolver can reach and it becomes an edge; names
nothing and it is still the route back to the proof after every path:line has rotted.>
```

- [ ] **Step 2: Add a subsection to the `tracker-conventions` guide**

Under its `### Required fields` section:

```markdown
- **`**Valid:**` declares the decay class**, and absence means `dated`. Three forms,
  no fourth: `invariant` (a law), `dated YYYY-MM-DD` (true of an instant), and
  `conditional — <event>` (true until that fires). An entry that declares one is a
  **Statement**: a claim that can be true or false, and that owes a proof. An entry
  that declares none — a backlog item, a proposal — is not, and owes nothing.
- **`**Rests on:**` is the durable route back to the proof.** Code rots and
  `path:line` rots with it; an ADR, a decision, or a principle does not. This is the
  shape three of codescout's seven ADRs already converged on by practice: `Decision`
  is the claim, `Confidence` is the evidence, `Revisit-when` is the condition, and
  `Sites (initial)` labels the rotting pointers as rotting in its own heading.
- Both are detected **line-anchored**, so a sentence naming the field is not a
  declaration. `librarian(action="doctor")` reports
  `entry_conditional_past_due`, `entry_dated_stale` and
  `entry_cited_from_outside_but_undeclared`, all gated on cross-file citation
  exposure so an entry nothing depends on never generates work.
```

- [ ] **Step 3: Run the invariant tests**

Run: `cargo test -p codescout guide`
Expected: PASS. If a size-cap invariant fails, shorten the guide text rather than raising the cap — the cap exists because these surfaces are always loaded.

- [ ] **Step 4: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add docs/templates/session-log.md src/
git commit -F- <<'MSG'
docs(trackers): document **Valid:** and **Rests on:** where authors meet them

The session-log template and the tracker-conventions guide. An entry that
declares a class is a Statement and owes a proof; one that declares none — a
backlog item, a proposal — is not, and owes nothing.
MSG
```

---

## Self-Review

**1. Spec coverage.** Layer 1: grammar (T2), detection/line-anchoring (T2), validation (T2), default-is-decay (T2 `resolve_validity`), allocator stamping (T3), `**Rests on:**` parsing (T2). Layer 2: all three checks (T5–T7), shared exposure gate (T4, used by all three), horizon constant (T5). **Two gaps, both deliberate and named in-plan:** (a) the git-blame fallback date is not wired — `resolve_validity` accepts it and no caller computes it yet; Task 6's Interfaces block states why (one subprocess per file, cost unmeasured, three options named in the spec) and Task 7's third check covers undeclared entries without guessing their age. (b) `**Rests on:**` is parsed but nothing consumes it — its consumer is Layer 3's `rel='rests-on'` materializer, which is out of scope here. Both belong in the spec's Layer 3/5a slots, not smuggled into this plan.

**2. Placeholder scan.** No TBD/TODO. Every code step carries real code. Two steps say "if X already exists, use it instead" (`once_cell` in T2, a date helper in T3) — those are instructions to check a fact, not deferred work, and each names the exact thing to look for.

**3. Type consistency.** `EntrySection { id, level, heading_line, end_line, text }` is produced in T1 and consumed by `.id` / `.text` in T4–T7. `Validity` is produced in T2 and matched in T5 (`Conditional { condition }`), T6 (`Dated(iso)`), T7 (`Ok(None)`). `entry_indegree` returns `BTreeMap<String, usize>` in T4 and is taken by that type in T5–T7. `VALIDITY_HORIZON_DAYS` and `EXPOSURE_THRESHOLD` are defined once in T5 and used in T6–T7. `Violation::new(check, artifact_id, path, detail)` matches the existing signature at `doctor.rs:142`.
