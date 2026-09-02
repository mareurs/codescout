# get_guide Section Grain — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver auto-injected guide text at **section** grain for the `librarian` topic, selected by declarations the guide markdown itself carries.

**Architecture:** A fence-aware parser turns each compiled-in guide into sections carrying `serves:` / `requires:` declarations. A new `Tool::selector_key(&input)` projects a call shape (`"artifact.append_entry"`) *before* `call()` consumes the input. `call_content` matches shape against declarations and emits only matching sections, deduped in `GuideLedger` at `topic#heading` grain. Topics with no declarations keep today's whole-topic path exactly — so this ships touching one topic and one tool.

**Tech Stack:** Rust 2021, `serde_json::Value`, `std::sync::LazyLock`, `tokio::test`. No new dependencies.

**Spec:** [`../specs/2026-08-27-get-guide-section-grain-design.md`](../specs/2026-08-27-get-guide-section-grain-design.md)

## Global Constraints

- **Pre-commit gate, every task:** `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`. A task is not done until all three are green.
- **Branch:** `experiments`. Never commit to `master`.
- **Size cap:** `MAX_DECLARED_SECTION_BYTES = 2500` — bytes, not characters. Guides contain multi-byte em-dashes and arrows; `str::len()` is correct, `chars().count()` is not.
- **Fence rule:** any line inside a ```` ``` ```` or `~~~` fence is never a heading and never a declaration. Guides teach this syntax by example.
- **Fail-safe direction:** every ambiguity resolves toward *re-delivering* a guide, never toward suppressing one. This matches `GuideLedger::load`'s existing contract.
- **Phase 1 scope:** only `librarian` gains declarations; only `LibrarianAdapter` implements `selector_key`. The other nine topics and all other tools must behave byte-identically to today, and Task 10 proves it.
- **Iron Laws apply** to the implementing agent: `symbols` not `read_file` on source, `edit_code` for structural edits, `edit_markdown` for `.md`, no unbounded `run_command` pipes.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/prompts/guide_index.rs` **(new)** | Section splitting, declaration grammar, the built index, shape matching |
| `src/prompts/mod.rs` | Re-export the index; `topic_body` unchanged in behaviour |
| `src/prompts/guides/librarian.md` | Carries the Phase 1 declarations |
| `src/prompts/shape_census.txt` **(new)** | Committed list of observed call shapes — Gate 2's denominator |
| `src/tools/core/types.rs` | `Tool::selector_key`; section emission in `call_content` |
| `src/librarian/adapter.rs` | `selector_key` for `artifact`/`librarian`; generalise the path predicate |
| `src/tools/guide_ledger.rs` | Prefix-aware `re_arm` |
| `src/server.rs` | Gates 2 and 5 (integration-level, need the tool registry) |

---

### Task 1: Fence-aware section splitter

**Files:**
- Create: `src/prompts/guide_index.rs`
- Modify: `src/prompts/mod.rs` (add `pub mod guide_index;`)
- Test: inline `mod tests` in `src/prompts/guide_index.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct RawSection { pub heading: String, pub level: u8, pub body: &'static str }` and `pub fn split_sections(src: &'static str) -> (&'static str, Vec<RawSection>)` returning `(preamble, sections)`. `body` includes the heading line and runs to the **very next heading of any level**, so a parent excludes its `###` children and `preamble + all sections` partitions the file exactly.

- [ ] **Step 1: Write the failing tests**

````rust
#[cfg(test)]
mod tests {
    use super::*;

    const FENCED: &str = "\
intro text
## Real Heading
body one
```markdown
## Not A Heading
```
more body one
## Second Real
body two
";

    #[test]
    fn split_ignores_headings_inside_fences() {
        let (preamble, secs) = split_sections(FENCED);
        assert_eq!(preamble, "intro text\n");
        let names: Vec<&str> = secs.iter().map(|s| s.heading.as_str()).collect();
        assert_eq!(names, vec!["Real Heading", "Second Real"]);
        assert!(secs[0].body.contains("## Not A Heading"));
        assert!(secs[0].body.contains("more body one"));
    }

    #[test]
    fn split_captures_h3_as_its_own_section() {
        let src = "\
## Parent
p body
### Child
c body
## Next
n body
";
        let (_, secs) = split_sections(src);
        let got: Vec<(u8, &str)> =
            secs.iter().map(|s| (s.level, s.heading.as_str())).collect();
        assert_eq!(got, vec![(2, "Parent"), (3, "Child"), (2, "Next")]);
        // A parent's body stops at its child, so bytes are never double-counted.
        assert!(!secs[0].body.contains("c body"));
    }

    #[test]
    fn tilde_fences_toggle_too() {
        let src = "## A\n~~~\n## Fake\n~~~\n";
        let (_, secs) = split_sections(src);
        assert_eq!(secs.len(), 1);
    }
}
````

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout guide_index:: -- --nocapture`
Expected: FAIL — `cannot find function 'split_sections'`.

- [ ] **Step 3: Implement the splitter**

```rust
//! Section-grain index over the compiled-in guide corpus.
//!
//! Delivery used to be all-or-nothing: `relevant_guide_topic` returned a topic
//! NAME and `topic_body` a whole file. Measured 2026-08-27 over 81 injections,
//! 66.7% went unused and 94% of `librarian`'s bytes were never touched. This
//! module makes the section the unit.

/// One `##` or `###` section of a guide, before declarations are parsed.
#[derive(Debug, Clone, PartialEq)]
pub struct RawSection {
    pub heading: String,
    pub level: u8,
    /// Heading line through to the very next heading of ANY level, so a parent
    /// excludes its `###` children and sections partition the file exactly.
    pub body: &'static str,
}

/// Split a guide into `(preamble, sections)`.
///
/// Fence-aware: a `##` line inside a fenced block is content, not a heading.
/// This is not defensive coding — the guides teach this very syntax by example,
/// and three separate measurements taken while designing this feature were wrong
/// because a naive `^## ` split matched a fence line in `tracker-conventions`,
/// inflating its section count and mis-splitting a 17,378 B section.
pub fn split_sections(src: &'static str) -> (&'static str, Vec<RawSection>) {
    let mut fence = false;
    let mut starts: Vec<(usize, u8, String)> = Vec::new();
    let mut offset = 0usize;

    for line in src.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = !fence;
        } else if !fence {
            let level = if line.starts_with("### ") {
                3
            } else if line.starts_with("## ") {
                2
            } else {
                0
            };
            if level > 0 {
                let heading = line[level as usize + 1..].trim().to_string();
                starts.push((offset, level, heading));
            }
        }
        offset += line.len();
    }

    let preamble_end = starts.first().map(|(s, _, _)| *s).unwrap_or(src.len());
    let preamble = &src[..preamble_end];

    let mut sections = Vec::with_capacity(starts.len());
    for (i, (start, level, heading)) in starts.iter().enumerate() {
        // End at the VERY NEXT heading of any level, so a parent EXCLUDES its
        // `###` children and `preamble + every section` partitions the file
        // exactly. "Parent swallows its children" was considered and is wrong:
        // declaring a child would then re-send bytes the parent already carries,
        // and `###` decomposition — the entire remedy for an over-cap section —
        // would buy nothing.
        let end = starts.get(i + 1).map(|(s, _, _)| *s).unwrap_or(src.len());
        sections.push(RawSection {
            heading: heading.clone(),
            level: *level,
            body: &src[*start..end],
        });
    }
    (preamble, sections)
}
```

Add to `src/prompts/mod.rs`, near the other module declarations:

```rust
pub mod guide_index;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout guide_index::`
Expected: 3 passed.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/prompts/guide_index.rs src/prompts/mod.rs
git commit -m "feat(guides): fence-aware section splitter for the guide corpus"
```

---

### Task 2: Declaration grammar

**Files:**
- Modify: `src/prompts/guide_index.rs`
- Test: inline `mod tests`

**Interfaces:**
- Consumes: `RawSection` from Task 1.
- Produces:
  - `pub struct Shape { pub tool: String, pub action: Option<String>, pub path_contains: Option<String> }`
  - `pub fn parse_shape(s: &str) -> Result<Shape, String>`
  - `pub fn parse_declarations(body: &str) -> Result<(Vec<Shape>, Vec<String>), String>` returning `(serves, requires)`, reading only HTML comments that appear before the first blank line after the heading, and never inside a fence.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn parse_shape_forms() {
    assert_eq!(
        parse_shape("artifact.append_entry").unwrap(),
        Shape { tool: "artifact".into(), action: Some("append_entry".into()), path_contains: None }
    );
    assert_eq!(
        parse_shape("grep").unwrap(),
        Shape { tool: "grep".into(), action: None, path_contains: None }
    );
    assert_eq!(
        parse_shape("artifact.update(path~docs/issues/)").unwrap(),
        Shape {
            tool: "artifact".into(),
            action: Some("update".into()),
            path_contains: Some("docs/issues/".into()),
        }
    );
}

#[test]
fn malformed_shape_is_an_error_not_a_skip() {
    // Gate 1: a typo must fail loudly. A silently-skipped declaration is
    // indistinguishable from a section nobody declared.
    assert!(parse_shape("artifact.update(path=docs/)").is_err());
    assert!(parse_shape("artifact.update(").is_err());
    assert!(parse_shape("").is_err());
    assert!(parse_shape("artifact.get(mode~x)").is_err());
}

#[test]
fn declarations_are_read_from_the_comment_block_under_the_heading() {
    let body = "## Entry ids\n<!-- serves: artifact.append_entry, artifact.update_entry -->\n<!-- requires: Declaring a ledger -->\n\nprose\n<!-- serves: not.parsed -->\n";
    let (serves, requires) = parse_declarations(body).unwrap();
    assert_eq!(serves.len(), 2);
    assert_eq!(serves[0].action.as_deref(), Some("append_entry"));
    assert_eq!(requires, vec!["Declaring a ledger".to_string()]);
}

#[test]
fn a_fenced_declaration_is_documentation_not_a_declaration() {
    let body = "## Teaching\n```markdown\n<!-- serves: artifact.get -->\n```\n";
    let (serves, requires) = parse_declarations(body).unwrap();
    assert!(serves.is_empty());
    assert!(requires.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout guide_index::`
Expected: FAIL — `cannot find function 'parse_shape'`.

- [ ] **Step 3: Implement the grammar**

```rust
use serde_json::Value;

/// A call shape a section declares itself relevant to.
///
/// Grammar, deliberately minimal — widening it requires amending the spec:
/// ```text
/// shape := tool ["." action] ["(" "path~" substring ")"]
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub tool: String,
    pub action: Option<String>,
    pub path_contains: Option<String>,
}

pub fn parse_shape(s: &str) -> Result<Shape, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty shape".to_string());
    }
    let (head, path_contains) = match s.find('(') {
        Some(open) => {
            if !s.ends_with(')') {
                return Err(format!("unterminated predicate in `{s}`"));
            }
            let inner = &s[open + 1..s.len() - 1];
            let needle = inner
                .strip_prefix("path~")
                .ok_or_else(|| format!("only `path~<substring>` is supported, got `{inner}`"))?;
            if needle.is_empty() {
                return Err(format!("empty path predicate in `{s}`"));
            }
            (&s[..open], Some(needle.to_string()))
        }
        None => (s, None),
    };
    let head = head.trim();
    if head.is_empty() {
        return Err(format!("missing tool name in `{s}`"));
    }
    let (tool, action) = match head.split_once('.') {
        Some((t, a)) if !t.is_empty() && !a.is_empty() => (t.to_string(), Some(a.to_string())),
        Some(_) => return Err(format!("malformed tool.action in `{s}`")),
        None => (head.to_string(), None),
    };
    Ok(Shape { tool, action, path_contains })
}

/// Parse the `serves:` / `requires:` comment block directly under a heading.
///
/// Only comments before the first blank line count. Everything after is prose,
/// including worked examples — a guide that teaches this syntax must not
/// declare itself by accident.
pub fn parse_declarations(body: &str) -> Result<(Vec<Shape>, Vec<String>), String> {
    let mut serves = Vec::new();
    let mut requires = Vec::new();
    for line in body.split_inclusive('\n').skip(1) {
        let t = line.trim();
        if t.is_empty() {
            break;
        }
        let Some(inner) = t.strip_prefix("<!--").and_then(|r| r.strip_suffix("-->")) else {
            // A non-comment, non-blank line ends the declaration block.
            break;
        };
        let inner = inner.trim();
        if let Some(rest) = inner.strip_prefix("serves:") {
            for part in rest.split(',') {
                serves.push(parse_shape(part)?);
            }
        } else if let Some(rest) = inner.strip_prefix("requires:") {
            for part in rest.split(',') {
                let h = part.trim();
                if h.is_empty() {
                    return Err("empty heading in `requires:`".to_string());
                }
                requires.push(h.to_string());
            }
        }
    }
    Ok((serves, requires))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout guide_index::`
Expected: 7 passed.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/prompts/guide_index.rs
git commit -m "feat(guides): serves:/requires: declaration grammar"
```

---

### Task 3: The built index

**Files:**
- Modify: `src/prompts/guide_index.rs`
- Test: inline `mod tests`

**Interfaces:**
- Consumes: `split_sections`, `parse_declarations` from Tasks 1-2.
- Produces:
  - `pub struct GuideSection { pub topic: &'static str, pub heading: String, pub level: u8, pub body: &'static str, pub serves: Vec<Shape>, pub requires: Vec<String> }`
  - `pub struct TopicEntry { pub preamble: &'static str, pub sections: Vec<GuideSection> }`
  - `pub struct GuideIndex { topics: std::collections::BTreeMap<&'static str, TopicEntry> }`
  - `impl GuideIndex { pub fn try_build() -> Result<Self, String>; pub fn topic(&self, t: &str) -> Option<&TopicEntry>; pub fn declares(&self, t: &str) -> bool }`
  - `pub static GUIDE_INDEX: std::sync::LazyLock<GuideIndex>`
  - `impl TopicEntry { pub fn declared(&self) -> impl Iterator<Item = &GuideSection> }` — sections with a non-empty `serves`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn index_builds_for_every_registered_topic() {
    // Gate 1 at corpus scale: a malformed declaration anywhere fails the build.
    let idx = GuideIndex::try_build().expect("guide index must build");
    for topic in crate::prompts::GUIDE_TOPICS {
        assert!(idx.topic(topic).is_some(), "topic `{topic}` missing from index");
    }
}

#[test]
fn section_bodies_partition_the_guide_exactly() {
    // No byte is lost between sections and none is counted twice; otherwise a
    // slice can silently omit a rule that lived on a boundary.
    let idx = GuideIndex::try_build().unwrap();
    for topic in crate::prompts::GUIDE_TOPICS {
        let entry = idx.topic(topic).unwrap();
        let body = crate::prompts::topic_body(topic).unwrap();
        let all: usize = entry.sections.iter().map(|s| s.body.len()).sum();
        assert_eq!(
            entry.preamble.len() + all,
            body.len(),
            "sections of `{topic}` do not partition the file"
        );
    }
}

#[test]
fn librarian_has_no_declarations_before_task_6() {
    let idx = GuideIndex::try_build().unwrap();
    assert!(!idx.declares("progressive-disclosure"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout guide_index::`
Expected: FAIL — `cannot find type 'GuideIndex'`.

- [ ] **Step 3: Implement the index**

```rust
use std::collections::BTreeMap;
use std::sync::LazyLock;

/// Maximum size of a section carrying a `serves:` declaration.
///
/// Bytes, not characters — these guides are dense with multi-byte em-dashes and
/// arrows, so `chars().count()` would silently under-report. 2,500 B sits just
/// above `librarian`'s natural p90 section, so most of the corpus already
/// complies: only 6 of 67 sections corpus-wide exceed it.
pub const MAX_DECLARED_SECTION_BYTES: usize = 2500;

/// A guide section with its declarations resolved.
#[derive(Debug, Clone)]
pub struct GuideSection {
    pub topic: &'static str,
    pub heading: String,
    pub level: u8,
    pub body: &'static str,
    pub serves: Vec<Shape>,
    pub requires: Vec<String>,
}

impl GuideSection {
    /// Ledger key. Opaque to `GuideLedger`, which stores `String` keys, so no
    /// on-disk format change is needed: a stale topic-only key simply fails to
    /// match, which re-delivers rather than suppresses.
    pub fn ledger_key(&self) -> String {
        format!("{}#{}", self.topic, self.heading)
    }
}

#[derive(Debug)]
pub struct TopicEntry {
    pub preamble: &'static str,
    pub sections: Vec<GuideSection>,
}

impl TopicEntry {
    pub fn declared(&self) -> impl Iterator<Item = &GuideSection> {
        self.sections.iter().filter(|s| !s.serves.is_empty())
    }
}

#[derive(Debug)]
pub struct GuideIndex {
    topics: BTreeMap<&'static str, TopicEntry>,
}

impl GuideIndex {
    pub fn try_build() -> Result<Self, String> {
        let mut topics = BTreeMap::new();
        for &topic in crate::prompts::GUIDE_TOPICS {
            let src = crate::prompts::topic_body(topic)
                .ok_or_else(|| format!("topic `{topic}` has no body"))?;
            let (preamble, raws) = split_sections(src);
            let mut sections = Vec::with_capacity(raws.len());
            for raw in raws {
                let (serves, requires) = parse_declarations(raw.body)
                    .map_err(|e| format!("{topic} § {}: {e}", raw.heading))?;
                sections.push(GuideSection {
                    topic,
                    heading: raw.heading,
                    level: raw.level,
                    body: raw.body,
                    serves,
                    requires,
                });
            }
            topics.insert(topic, TopicEntry { preamble, sections });
        }
        Ok(Self { topics })
    }

    pub fn topic(&self, t: &str) -> Option<&TopicEntry> {
        self.topics.get(t)
    }

    /// Whether this topic has opted into section-grain delivery. Topics with no
    /// declarations keep the whole-topic path — this is the phase switch.
    pub fn declares(&self, t: &str) -> bool {
        self.topic(t).is_some_and(|e| e.declared().next().is_some())
    }
}

pub static GUIDE_INDEX: LazyLock<GuideIndex> = LazyLock::new(|| {
    GuideIndex::try_build().expect("guide index failed to build; see gate `index_builds_for_every_registered_topic`")
});
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout guide_index::`
Expected: 10 passed.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/prompts/guide_index.rs
git commit -m "feat(guides): build a section index over the compiled-in corpus"
```

---

### Task 4: `selector_key` — let the matcher see the call

**Files:**
- Modify: `src/tools/core/types.rs` (add trait method beside `relevant_guide_topic`, around `:914`)
- Modify: `src/librarian/adapter.rs` (implement it; generalise the path predicate)
- Test: inline tests in `src/librarian/adapter.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `fn selector_key(&self, _input: &Value) -> Option<String>` on `trait Tool`, defaulting to `None`.
  - `pub fn names_path_containing(result: &Value, needle: &str) -> bool` in `src/librarian/adapter.rs`, generalising the existing `names_tracker_path`.

- [ ] **Step 1: Write the failing tests**

```rust
// in src/librarian/adapter.rs tests
#[test]
fn selector_key_projects_tool_and_action() {
    let a = LibrarianAdapter::new_for_test();
    assert_eq!(
        a.selector_key(&serde_json::json!({"action": "append_entry", "id": "x"})),
        Some("artifact.append_entry".to_string())
    );
    // No action ⇒ no key. The matcher then sees `None` and only tool-only
    // shapes can match, which is the correct conservative behaviour.
    assert_eq!(a.selector_key(&serde_json::json!({"id": "x"})), None);
}

#[test]
fn names_path_containing_generalises_and_normalises_separators() {
    let v = serde_json::json!({"abs_path": "docs\\issues\\x.md"});
    assert!(names_path_containing(&v, "docs/issues/"));
    assert!(!names_path_containing(&v, "docs/trackers/"));
    // find-style items and doctor-style violations keep working.
    let items = serde_json::json!({"items": [{"rel_path": "docs/trackers/t.md"}]});
    assert!(names_path_containing(&items, "docs/trackers/"));
    let viol = serde_json::json!({"violations": [{"path": "docs/issues/b.md"}]});
    assert!(names_path_containing(&viol, "docs/issues/"));
}

#[test]
fn names_tracker_path_still_agrees_with_the_generalised_form() {
    // The existing trigger must not change behaviour in Phase 1.
    for p in ["docs/issues/x.md", "docs/trackers/y.md", "src/main.rs"] {
        let v = serde_json::json!({"abs_path": p});
        assert_eq!(
            names_tracker_path(&v),
            names_path_containing(&v, "docs/issues/") || names_path_containing(&v, "docs/trackers/")
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout adapter::`
Expected: FAIL — `no method named 'selector_key'`, `cannot find function 'names_path_containing'`.

- [ ] **Step 3: Implement**

In `src/tools/core/types.rs`, directly above the existing `relevant_guide_topic` default:

```rust
    /// A cheap projection of this call's shape, taken BEFORE `call()` consumes
    /// `input`. Default `None` ⇒ the tool opts out at zero cost.
    ///
    /// Deliberately not a clone of `input`: `create_file` and `edit_file` inputs
    /// carry whole file bodies, and a clone would be paid on 100% of tool calls
    /// to benefit the ~3% that inject a guide.
    fn selector_key(&self, _input: &Value) -> Option<String> {
        None
    }
```

In `src/librarian/adapter.rs`, add to the `impl crate::tools::Tool for LibrarianAdapter` block:

```rust
    fn selector_key(&self, input: &Value) -> Option<String> {
        let action = input.get("action")?.as_str()?;
        Some(format!("{}.{}", self.name(), action))
    }
```

Refactor `names_tracker_path`'s inner helper into a public generalised form, keeping
`names_tracker_path` as a thin wrapper so its callers and doc rationale are untouched:

```rust
/// Whether a librarian response names a path containing `needle`.
///
/// Generalised from `names_tracker_path` so a section declaration can carry an
/// arbitrary `path~<substring>` predicate. The scanned shapes are unchanged and
/// still deliberately shallow: top-level `abs_path`/`rel_path`, one level into a
/// `find`-style `items` array, and `path` inside a `doctor`-style `violations`
/// array. Separators are normalised — a backslash-spelled Windows path matching
/// nothing failed as a *wrong guide*, not an error.
pub fn names_path_containing(result: &Value, needle: &str) -> bool {
    fn hit(v: Option<&Value>, needle: &str) -> bool {
        v.and_then(Value::as_str)
            .is_some_and(|p| p.replace('\\', "/").contains(needle))
    }
    fn any_path_field(obj: &Value, needle: &str) -> bool {
        hit(obj.get("abs_path"), needle) || hit(obj.get("rel_path"), needle)
    }

    if any_path_field(result, needle) {
        return true;
    }
    if result
        .get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|i| any_path_field(i, needle)))
    {
        return true;
    }
    result
        .get("violations")
        .and_then(Value::as_array)
        .is_some_and(|rows| rows.iter().any(|row| hit(row.get("path"), needle)))
}

fn names_tracker_path(result: &Value) -> bool {
    names_path_containing(result, "docs/issues/")
        || names_path_containing(result, "docs/trackers/")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout adapter::`
Expected: all pass, including the pre-existing `names_tracker_path` tests.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/tools/core/types.rs src/librarian/adapter.rs
git commit -m "feat(guides): Tool::selector_key projects a call shape before input is consumed"
```

---

### Task 5: Matching, with transitive `requires:` closure

**Files:**
- Modify: `src/prompts/guide_index.rs`
- Test: inline `mod tests`

**Interfaces:**
- Consumes: `Shape`, `GuideIndex`, `TopicEntry`, `GuideSection`; `names_path_containing` from Task 4.
- Produces:
  - `impl Shape { pub fn matches(&self, sel: Option<&str>, result: &Value) -> bool }`
  - `impl GuideIndex { pub fn match_sections(&self, topic: &str, sel: Option<&str>, result: &Value) -> Vec<&GuideSection> }` — matched sections plus their transitive `requires:` closure, in document order, deduped by heading.

- [ ] **Step 1: Write the failing tests**

```rust
fn fixture() -> GuideIndex {
    // Hand-built so the test does not depend on the live corpus.
    let idx = GuideIndex::try_build().unwrap();
    idx
}

#[test]
fn shape_matching_rules() {
    let empty = serde_json::json!({});
    let s = parse_shape("artifact.get").unwrap();
    assert!(s.matches(Some("artifact.get"), &empty));
    assert!(!s.matches(Some("artifact.find"), &empty));
    assert!(!s.matches(None, &empty));

    // Tool-only shape matches any action of that tool, and a keyless call.
    let t = parse_shape("artifact").unwrap();
    assert!(t.matches(Some("artifact.get"), &empty));
    assert!(t.matches(Some("artifact"), &empty));

    // Path predicate reads the RESULT.
    let p = parse_shape("artifact.get(path~docs/issues/)").unwrap();
    assert!(!p.matches(Some("artifact.get"), &empty));
    assert!(p.matches(
        Some("artifact.get"),
        &serde_json::json!({"abs_path": "docs/issues/x.md"})
    ));
}

#[test]
fn match_sections_closes_requires_transitively_and_orders_by_document() {
    let idx = fixture();
    // Uses the live librarian declarations landed in Task 6; before that this
    // asserts the empty case, which is the correct Phase-0 behaviour.
    let got = idx.match_sections("librarian", Some("artifact.append_entry"), &serde_json::json!({}));
    let headings: Vec<&str> = got.iter().map(|s| s.heading.as_str()).collect();
    // Document order, no duplicates.
    let mut sorted = headings.clone();
    sorted.dedup();
    assert_eq!(headings, sorted);
}

#[test]
fn unknown_topic_matches_nothing() {
    let idx = fixture();
    assert!(idx.match_sections("no-such-topic", Some("artifact.get"), &serde_json::json!({})).is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p codescout guide_index::`
Expected: FAIL — `no method named 'matches'`.

- [ ] **Step 3: Implement matching**

```rust
impl Shape {
    pub fn matches(&self, sel: Option<&str>, result: &Value) -> bool {
        let Some(sel) = sel else { return false };
        let (tool, action) = match sel.split_once('.') {
            Some((t, a)) => (t, Some(a)),
            None => (sel, None),
        };
        if tool != self.tool {
            return false;
        }
        if let Some(want) = &self.action {
            if action != Some(want.as_str()) {
                return false;
            }
        }
        if let Some(needle) = &self.path_contains {
            if !crate::librarian::adapter::names_path_containing(result, needle) {
                return false;
            }
        }
        true
    }
}

impl GuideIndex {
    /// Sections serving this call, plus their transitive `requires:` closure,
    /// in document order with no duplicates.
    ///
    /// The closure matters because sections are not independent: `tracker-conventions`
    /// § *Entry ids* states entry-id law whose precondition lives in § *Declaring a
    /// ledger*. Delivered alone it is true and misleading.
    pub fn match_sections(
        &self,
        topic: &str,
        sel: Option<&str>,
        result: &Value,
    ) -> Vec<&GuideSection> {
        let Some(entry) = self.topic(topic) else {
            return Vec::new();
        };
        let mut wanted: std::collections::BTreeSet<&str> = Default::default();
        for sec in entry.declared() {
            if sec.serves.iter().any(|s| s.matches(sel, result)) {
                wanted.insert(sec.heading.as_str());
            }
        }
        // Transitive closure over `requires:`. Bounded by section count, so a
        // cycle terminates rather than hanging.
        loop {
            let mut added = false;
            let pending: Vec<&str> = entry
                .sections
                .iter()
                .filter(|s| wanted.contains(s.heading.as_str()))
                .flat_map(|s| s.requires.iter().map(|r| r.as_str()))
                .collect();
            for req in pending {
                if entry.sections.iter().any(|s| s.heading == req) && wanted.insert(req) {
                    added = true;
                }
            }
            if !added {
                break;
            }
        }
        entry
            .sections
            .iter()
            .filter(|s| wanted.contains(s.heading.as_str()))
            .collect()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p codescout guide_index::`
Expected: all pass.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/prompts/guide_index.rs
git commit -m "feat(guides): shape matching with transitive requires: closure"
```

---

### Task 6: Declare `librarian`, decomposing its one over-cap section

**Files:**
- Modify: `src/prompts/guides/librarian.md` (use `edit_markdown`, never `edit_file`)
- Test: inline gate in `src/prompts/guide_index.rs`

**Interfaces:**
- Consumes: the grammar from Task 2.
- Produces: `librarian` becomes the first topic where `GUIDE_INDEX.declares("librarian")` is `true`.

`librarian.md` is 20,545 B — a 283 B preamble plus 16 sections (14 `##`, 2 `###`).
Under the Task 1 splitter exactly **one** exceeds the 2,500 B cap and must be decomposed
at `###` before it can be declared: **§ Body Editing Surfaces (4,080 B)**.

§ *Augmentation Lifecycle* does **not** need splitting. It measures 3,085 B only if a
parent is counted together with its `###` child; under true-partition semantics it is
1,387 B, and its child § *Changing ONE entry* is a separate 1,698 B section.

Measured sizes, for reference while declaring:

| section | lvl | bytes |
|---|---|---|
| Artifact Model | 2 | 898 |
| docs/trackers/ — Backing Store, Not a Docs Folder | 2 | 591 |
| Filter Syntax | 2 | 1,983 |
| artifact(action="create") — Required Fields | 2 | 675 |
| Tracker Workflow | 2 | 346 |
|  Reach for augmentation — don't hand-maintain the table | 3 | 1,299 |
| Augmentation Lifecycle | 2 | 1,387 |
|  Changing ONE entry — don't hand-build the array | 3 | 1,698 |
| **Body Editing Surfaces** | 2 | **4,080** |
| librarian(action=...) — Reference | 2 | 1,620 |
| artifact_event — Event Log | 2 | 388 |
| artifact(action="graph") — Relationship Map | 2 | 209 |
| Worktree overlay | 2 | 1,616 |
| Archiving / Moving Trackers | 2 | 1,723 |
| Common Mistakes | 2 | 1,292 |
| Runtime tips | 2 | 457 |

- [ ] **Step 1: Write the failing gate**

```rust
#[test]
fn declared_sections_are_within_the_size_cap() {
    // Gate 3, scoped to topics that have opted in. Widens automatically as
    // Phases 2 and 3 land. `str::len()` is bytes — these guides are full of
    // multi-byte em-dashes, so a char count would silently under-report.
    const MAX: usize = crate::prompts::guide_index::MAX_DECLARED_SECTION_BYTES;
    let idx = GuideIndex::try_build().unwrap();
    for topic in crate::prompts::GUIDE_TOPICS {
        let Some(entry) = idx.topic(topic) else { continue };
        for sec in entry.declared() {
            assert!(
                sec.body.len() <= MAX,
                "{topic} § {} is {} B, over the {MAX} B cap. \
                 Decompose it at `###` and move the declaration onto the child \
                 sections. A slice this large is the failure this feature exists \
                 to fix.",
                sec.heading,
                sec.body.len()
            );
        }
    }
}

#[test]
fn librarian_declares_the_six_highest_volume_artifact_shapes() {
    // These six are 9,695 of 10,920 observed artifact/librarian calls (89%).
    let idx = GuideIndex::try_build().unwrap();
    let entry = idx.topic("librarian").expect("librarian in index");
    for shape in [
        "artifact.update", "artifact.get", "artifact.find",
        "artifact.append_entry", "artifact.create", "artifact.move",
    ] {
        let sel = Some(shape);
        let hits = idx.match_sections("librarian", sel, &serde_json::json!({}));
        assert!(!hits.is_empty(), "no librarian section serves `{shape}`");
    }
    assert!(entry.declared().next().is_some());
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p codescout guide_index::librarian_declares`
Expected: FAIL — "no librarian section serves `artifact.update`".

- [ ] **Step 3: Split the one over-cap section and add declarations**

Split **only** § *Body Editing Surfaces* (L207-273). It already covers three distinct
subjects: choosing between `body`, `body_edits` and frontmatter-only patches; the 50%
shrink guard and its `force` escape; and why a librarian-managed file refuses a direct
`edit_markdown`. Give each its own `###` heading and keep the section's opening paragraph
as the parent body, so the parent remains a valid (undeclared, child-reachable) section.

Use `edit_markdown` with `action="insert_before"` / `action="edit"`. Do **not** rewrite
the file wholesale — the 50% shrink guard will refuse it, correctly.

Then add one declaration block per section. This mapping covers all six high-volume
shapes (89% of observed artifact/librarian traffic) plus every other census shape that
routes to `librarian`:

| section | `serves:` |
|---|---|
| Artifact Model | `artifact.get, artifact.create` |
| Filter Syntax | `artifact.find` |
| artifact(action="create") — Required Fields | `artifact.create` |
|  Reach for augmentation — don't hand-maintain the table | `artifact.append_entry` |
| Augmentation Lifecycle | `artifact_augment, artifact_refresh.gather, artifact_refresh.list_stale` |
|  Changing ONE entry — don't hand-build the array | `artifact.update_entry` |
|  Body Editing child: choosing a mode | `artifact.update` |
|  Body Editing child: the shrink guard | `artifact.update` |
| librarian(action=...) — Reference | `librarian.reindex, librarian.link_scan, librarian.doctor, librarian.audit_doc_refs, librarian.context, librarian.tracker_design, librarian.legibility_scan` |
| artifact_event — Event Log | `artifact_event.create, artifact_event.list` |
| artifact(action="graph") — Relationship Map | `artifact.graph, artifact.link` |
| Worktree overlay | `librarian.merge_worktree` |
| Archiving / Moving Trackers | `artifact.move, artifact.delete` |

Each declaration goes immediately under its heading, before the first blank line:

```markdown
## Artifact Model
<!-- serves: artifact.get, artifact.create -->
```

Three sections stay undeclared on purpose — § *docs/trackers/ — Backing Store*,
§ *Common Mistakes*, § *Runtime tips*. They are orientation prose serving no single call
shape. Task 9's reachability gate will name them; resolve each with either a `requires:`
from a section that genuinely needs it, or a `SECTION_WAIVERS` entry stating why.
**Do not invent a `serves:` to silence the gate** — a too-broad declaration is exactly
what Task 10's byte ceiling exists to catch.

- [ ] **Step 4: Run the gates**

Run: `cargo test -p codescout guide_index::`
Expected: both new tests pass; `section_bodies_partition_the_guide_exactly` still passes.
The expected p50 draw after this task is **~8,600 B** (Filter Syntax 1,983 + Artifact
Model 898 + a Body Editing child ~2,040 + Reach for augmentation 1,299 + Required Fields
675 + Archiving 1,723) against 20,545 B today — a 58% cut, comfortably under Task 10's
ceiling.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/prompts/guides/librarian.md src/prompts/guide_index.rs
git commit -m "docs(guides): declare librarian sections and split the over-cap one"
```

---

### Task 7: Prefix-aware `re_arm`

**Files:**
- Modify: `src/tools/guide_ledger.rs:275-285`
- Test: inline `mod tests` in the same file

**Interfaces:**
- Consumes: the `topic#heading` key convention from Task 3.
- Produces: `re_arm` unchanged in signature, now removing section keys of a named topic.

No on-disk format change and no version field: `emitted` is a
`BTreeMap<String, DateTime<Utc>>` with an opaque key, and `load` already degrades a
malformed file to empty. A stale topic-only key fails to match a section key, which
re-delivers — the safe direction. `re_arm` is the one place that must learn the prefix,
or a project switch silently stops re-arming `librarian`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn re_arm_removes_section_keys_of_the_named_topic() {
    let mut l = GuideLedger::anonymous(None);
    l.insert("librarian#Artifact Model".to_string());
    l.insert("librarian#Filter AST".to_string());
    l.insert("tracker-conventions".to_string());
    l.re_arm(&["librarian"]);
    assert!(!l.contains("librarian#Artifact Model"));
    assert!(!l.contains("librarian#Filter AST"));
    // Unrelated topics survive; and a topic that merely SHARES a prefix must not
    // be swept — `librarian` must not take `librarian-runtime` with it.
    assert!(l.contains("tracker-conventions"));
}

#[test]
fn re_arm_does_not_sweep_a_topic_that_shares_a_name_prefix() {
    let mut l = GuideLedger::anonymous(None);
    l.insert("librarian-runtime".to_string());
    l.insert("librarian-runtime#Trackers".to_string());
    l.re_arm(&["librarian"]);
    assert!(l.contains("librarian-runtime"));
    assert!(l.contains("librarian-runtime#Trackers"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p codescout guide_ledger::`
Expected: FAIL — the section keys survive `re_arm`.

- [ ] **Step 3: Implement**

```rust
    /// Forget the named topics so they fire again — used by the project-switch
    /// scoped re-arm.
    ///
    /// Matches a bare topic key AND its `topic#heading` section keys. The `#`
    /// separator is what keeps `librarian` from sweeping `librarian-runtime`:
    /// a bare `starts_with(topic)` would, and the failure would be silent
    /// starvation of an unrelated guide.
    pub fn re_arm(&mut self, topics: &[&str]) {
        self.emitted.retain(|key, _| {
            !topics.iter().any(|t| {
                key == t || (key.len() > t.len() + 1 && key.starts_with(t) && key.as_bytes()[t.len()] == b'#')
            })
        });
        self.persist();
    }
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p codescout guide_ledger::`
Expected: all pass, including the pre-existing
`re_arm_removes_only_the_named_topics_and_persists`.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/tools/guide_ledger.rs
git commit -m "fix(guides): re_arm must sweep section keys without crossing topic names"
```

---

### Task 8: Wire section delivery into `call_content`

**Files:**
- Modify: `src/tools/core/types.rs:669-820`
- Test: `src/server.rs` `guide_hint_tests` module

**Interfaces:**
- Consumes: `GUIDE_INDEX`, `match_sections`, `GuideSection::ledger_key`, `Tool::selector_key`.
- Produces: no new public API. `call_content` emits N section blocks for a declaring topic, a preamble block on no match, and the unchanged whole-topic block for non-declaring topics.

- [ ] **Step 1: Write the failing tests**

```rust
// in src/server.rs guide_hint_tests
#[tokio::test]
async fn append_entry_receives_only_the_entry_sections_not_the_whole_librarian_guide() {
    let (_dir, server) = make_server().await;
    let out = call_tool(&server, "artifact", json!({"action": "append_entry", "id": "x"})).await;
    let guide = guide_blocks(&out).join("");
    assert!(guide.contains("Entry Collections"), "expected the entry section");
    let whole = crate::prompts::topic_body("librarian").unwrap();
    assert!(
        guide.len() < whole.len() / 2,
        "delivered {} B of a {} B guide — section grain is not engaged",
        guide.len(),
        whole.len()
    );
}

#[tokio::test]
async fn a_second_differently_shaped_call_delivers_a_different_section() {
    let (_dir, server) = make_server().await;
    let first = guide_blocks(&call_tool(&server, "artifact", json!({"action": "append_entry", "id": "x"})).await).join("");
    let second = guide_blocks(&call_tool(&server, "artifact", json!({"action": "find"})).await).join("");
    assert!(!second.is_empty(), "per-section ledger must allow a second slice");
    assert_ne!(first, second);
}

#[tokio::test]
async fn the_same_shape_twice_delivers_nothing_the_second_time() {
    let (_dir, server) = make_server().await;
    let _ = call_tool(&server, "artifact", json!({"action": "find"})).await;
    let again = guide_blocks(&call_tool(&server, "artifact", json!({"action": "find"})).await).join("");
    assert!(again.is_empty());
}

#[tokio::test]
async fn an_unmatched_shape_receives_the_preamble_not_the_whole_topic() {
    let (_dir, server) = make_server().await;
    // `graft` is a real but low-volume action; Task 6 declares nothing for it.
    let out = call_tool(&server, "artifact", json!({"action": "graft"})).await;
    let guide = guide_blocks(&out).join("");
    let entry = crate::prompts::guide_index::GUIDE_INDEX.topic("librarian").unwrap();
    assert!(guide.contains(entry.preamble.trim()));
    assert!(guide.len() < 2000, "preamble fallback must be small, got {} B", guide.len());
    assert!(guide.contains("get_guide"), "fallback must point at the full topic");
}

#[tokio::test]
async fn a_non_declaring_topic_is_byte_identical_to_today() {
    let (_dir, server) = make_server().await;
    // `symbols` routes to `symbol-navigation`, which has no declarations in Phase 1.
    let out = call_tool(&server, "symbols", json!({"path": "src"})).await;
    let guide = guide_blocks(&out).join("");
    assert!(guide.contains(crate::prompts::topic_body("symbol-navigation").unwrap()));
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p codescout guide_hint_tests::`
Expected: FAIL — the whole `librarian` body is delivered.

- [ ] **Step 3: Implement**

In `call_content`, capture the selector before the move:

```rust
    async fn call_content(&self, input: Value, ctx: &ToolContext) -> Result<Vec<Content>> {
        let selector = self.selector_key(&input);
        let mut val = self.call(input, ctx).await?;
```

Replace the single-topic emission. After `hint_topic` is computed, resolve it to blocks:

```rust
        /// Blocks to emit for a resolved topic: section slices when the topic
        /// declares, the whole body when it does not, and the preamble when it
        /// declares but nothing matched.
        ///
        /// Falling back to the preamble rather than the whole topic is safe
        /// because coverage is a finite checklist (`shape_census.txt`), and
        /// starvation degrades to "late", never "never": per-section dedup means
        /// a later call of any matching shape still delivers the section.
        fn guide_blocks_for(
            topic: &str,
            selector: Option<&str>,
            result: &Value,
            emitted: &mut crate::tools::guide_ledger::GuideLedger,
        ) -> Vec<Content> {
            use crate::prompts::guide_index::GUIDE_INDEX;

            if !GUIDE_INDEX.declares(topic) {
                if !emitted.insert(topic.to_string()) {
                    return Vec::new();
                }
                return guide_block(topic).into_iter().collect();
            }

            let matched = GUIDE_INDEX.match_sections(topic, selector, result);
            let mut out = Vec::new();
            for sec in matched {
                let key = sec.ledger_key();
                if emitted.insert(key) {
                    out.push(Content::text(format!(
                        "<!-- auto-injected get_guide('{topic}') § {} — first call this session \
                         that serves this section. Do NOT re-call get_guide for it. -->\n\n{}\n\n\
                         <!-- end auto-injected get_guide('{topic}') § {} -->",
                        sec.heading, sec.body, sec.heading
                    )));
                }
            }
            if out.is_empty() {
                let key = format!("{topic}#<preamble>");
                if emitted.insert(key) {
                    if let Some(entry) = GUIDE_INDEX.topic(topic) {
                        out.push(Content::text(format!(
                            "<!-- auto-injected get_guide('{topic}') preamble — no section \
                             declares this call shape. -->\n\n{}\n\nCall \
                             `get_guide(\"{topic}\")` for the full topic.\n\n\
                             <!-- end auto-injected get_guide('{topic}') preamble -->",
                            entry.preamble.trim()
                        )));
                    }
                }
            }
            out
        }
```

Then, where the single `guide_block(topic)` was pushed onto the response, push the
vector instead, passing `selector.as_deref()` and `&val`. Move the
`emitted.insert(topic)` calls out of the `hint_topic` computation and into
`guide_blocks_for`, so a topic is only marked emitted when something is actually
sent — today's code marks it before knowing that.

**Leave the `SESSION_OPENING_GUIDE` check keyed on the bare topic name.**
`project-activation-bootstrap` has no declarations in Phase 1, so it takes the
whole-topic path and its `!emitted.contains(SESSION_OPENING_GUIDE)` trigger is
unaffected. Mixed keying is correct here and must not be "tidied".

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p codescout guide_hint_tests::`
Expected: all five pass, plus the pre-existing
`first_artifact_call_appends_librarian_guide_body_v2` (update its assertion to expect a
section rather than the whole body, and say so in the commit message).

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/tools/core/types.rs src/server.rs
git commit -m "feat(guides): emit section slices for declaring topics, preamble on no match"
```

---

### Task 9: Gates 2, 4 and 5 — coverage, dangling requires, reachability

**Files:**
- Create: `src/prompts/shape_census.txt`
- Modify: `src/server.rs` (replace `every_guide_topic_is_triggered_or_declared_pull_only`)
- Modify: `src/prompts/guide_index.rs` (Gate 4)

**Interfaces:**
- Consumes: `GUIDE_INDEX`, `Shape::matches`.
- Produces: `pub const SECTION_WAIVERS: &[(&str, &str, &str)]` in `src/prompts/mod.rs` — `(topic, heading, reason)`, mirroring `PULL_ONLY_GUIDE_TOPICS`' convention that a waiver states its rationale.

- [ ] **Step 1: Generate the census**

```bash
python3 - <<'PY' > src/prompts/shape_census.txt
import json, os, collections
ROOTS=[os.path.expanduser('~/.claude%s/projects'%s) for s in ['','-sdd','-kat']]
c=collections.Counter()
for root in ROOTS:
    for dp,_,fs in os.walk(root) if os.path.isdir(root) else []:
        for fn in fs:
            if not fn.endswith('.jsonl'): continue
            for line in open(os.path.join(dp,fn),errors='replace'):
                if 'mcp__codescout__' not in line: continue
                try: ev=json.loads(line)
                except Exception: continue
                if ev.get('type')!='assistant': continue
                for b in (ev.get('message') or {}).get('content') or []:
                    if not isinstance(b,dict) or b.get('type')!='tool_use': continue
                    n=(b.get('name') or '')
                    if not n.startswith('mcp__codescout__'): continue
                    i=b.get('input') or {}
                    a=i.get('action') if isinstance(i,dict) else None
                    n=n.replace('mcp__codescout__','')
                    c[('%s.%s'%(n,a)) if isinstance(a,str) else n]+=1
print("# Observed codescout call shapes. Gate 2's denominator.")
print("# Regenerate with the script in docs/superpowers/plans/2026-08-27-get-guide-section-grain.md Task 9.")
for k,v in sorted(c.items(), key=lambda x:(-x[1],x[0])):
    print('%s %d'%(k,v))
PY
```

Expected: ~88 rows, `artifact.update 3820` first.

- [ ] **Step 2: Write the failing gates**

```rust
// src/prompts/guide_index.rs
#[test]
fn no_dangling_requires() {
    // Gate 4.
    let idx = GuideIndex::try_build().unwrap();
    for topic in crate::prompts::GUIDE_TOPICS {
        let Some(entry) = idx.topic(topic) else { continue };
        for sec in &entry.sections {
            for req in &sec.requires {
                assert!(
                    entry.sections.iter().any(|s| &s.heading == req),
                    "{topic} § {} requires `{req}`, which no heading in that topic defines",
                    sec.heading
                );
            }
        }
    }
}

#[test]
fn every_section_of_a_declaring_topic_is_reachable() {
    // Gate 5, scoped to opted-in topics. A section that is neither declared,
    // transitively required, nor waived is unreachable — the section-grain form
    // of "authoring a guide nothing fires is the same as deleting it".
    let idx = GuideIndex::try_build().unwrap();
    for topic in crate::prompts::GUIDE_TOPICS {
        if !idx.declares(topic) { continue; }
        let entry = idx.topic(topic).unwrap();
        let required: std::collections::BTreeSet<&str> = entry
            .sections.iter().flat_map(|s| s.requires.iter().map(|r| r.as_str())).collect();
        for sec in &entry.sections {
            // A parent whose children carry the declarations is reachable through them.
            let child_declares = entry.sections.iter().any(|c| {
                c.level > sec.level && !c.serves.is_empty()
            });
            let waived = crate::prompts::SECTION_WAIVERS
                .iter()
                .any(|(t, h, _)| *t == topic && *h == sec.heading);
            assert!(
                !sec.serves.is_empty()
                    || required.contains(sec.heading.as_str())
                    || child_declares
                    || waived,
                "{topic} § {} is unreachable: declare a `serves:`, have a section \
                 `requires:` it, or add it to prompts::SECTION_WAIVERS with a reason",
                sec.heading
            );
        }
    }
}
```

```rust
// src/server.rs — replaces every_guide_topic_is_triggered_or_declared_pull_only
#[tokio::test]
async fn every_observed_shape_of_a_declaring_topic_has_a_section() {
    // Gate 2. Finite because call shapes are: 88 distinct across 170,465 observed
    // calls. Scoped to topics that have opted into section grain, so it is
    // meaningful in Phase 1 and widens automatically as Phases 2-3 land.
    use crate::prompts::guide_index::GUIDE_INDEX;
    let census = include_str!("prompts/shape_census.txt");
    let (_dir, server) = make_server().await;

    let probes = [
        serde_json::json!({}),
        serde_json::json!({"abs_path": "docs/issues/x.md"}),
        serde_json::json!({"abs_path": "docs/trackers/x.md"}),
    ];

    for line in census.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let shape = line.split_whitespace().next().unwrap();
        let tool_name = shape.split('.').next().unwrap();
        let Some(tool) = server.tools.iter().find(|t| t.name() == tool_name) else { continue };
        let Some(topic) = probes.iter().find_map(|p| tool.relevant_guide_topic(p)) else { continue };
        if !GUIDE_INDEX.declares(topic) { continue; }
        let covered = probes.iter().any(|p| {
            !GUIDE_INDEX.match_sections(topic, Some(shape), p).is_empty()
        });
        let waived = crate::prompts::SECTION_WAIVERS.iter().any(|(t, _, r)| {
            *t == topic && r.contains(shape)
        });
        assert!(
            covered || waived,
            "call shape `{shape}` routes to declaring topic `{topic}` but no section \
             serves it. Add a `serves:` declaration, or a SECTION_WAIVERS entry naming \
             the shape and saying why. An undeclared shape gets only the preamble."
        );
    }
}
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p codescout every_observed_shape no_dangling_requires every_section_of_a_declaring`
Expected: FAIL — `SECTION_WAIVERS` not found.

- [ ] **Step 4: Add `SECTION_WAIVERS` and satisfy the gates**

In `src/prompts/mod.rs`, beside `PULL_ONLY_GUIDE_TOPICS`:

```rust
/// Sections and shapes deliberately left unserved, each with its reason.
///
/// `(topic, heading, reason)`. The reason must name the shape when it waives a
/// shape, and must exceed 40 characters — a placeholder turns this gate back
/// into the silent default it replaced, which is exactly how 7 of 10 topics came
/// to fire for nothing before 2026-08-16.
pub const SECTION_WAIVERS: &[(&str, &str, &str)] = &[];
```

Then run the gates and add declarations (preferred) or waivers (where honest) until
green. Delete the old `every_guide_topic_is_triggered_or_declared_pull_only` **only
after** the new gate passes, and say in the commit message that Gate 2 replaces it.

- [ ] **Step 5: Gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/prompts/shape_census.txt src/prompts/mod.rs src/prompts/guide_index.rs src/server.rs
git commit -m "test(guides): shape-coverage, dangling-requires and reachability gates"
```

---

### Task 10: Golden byte budget and live verification

**Files:**
- Modify: `src/server.rs` (`guide_hint_tests`)
- Modify: `docs/PROBES.md`

**Interfaces:**
- Consumes: everything above.
- Produces: a committed byte ceiling that fails the build when a corpus edit erodes the win.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn a_p50_session_stays_under_the_committed_guide_byte_ceiling() {
    // The p50 session issues 6 distinct artifact/librarian shapes (measured over
    // 105 main sessions). Today that draws the whole 20,545 B librarian guide on
    // the first call and nothing after. Section grain must land well under it.
    //
    // This ceiling is the mechanism that keeps the win from eroding: guides grow.
    // `tracker-conventions` gained bytes mid-study, and `iron-laws-detail` gained
    // 769 B (5d3f8ebe) during the half hour the spec was being written.
    const CEILING: usize = 12_000;

    let (_dir, server) = make_server().await;
    let shapes = ["find", "get", "update", "append_entry", "create", "move"];
    let mut total = 0usize;
    for action in shapes {
        let out = call_tool(&server, "artifact", json!({"action": action, "id": "x"})).await;
        total += guide_blocks(&out).join("").len();
    }
    let whole = crate::prompts::topic_body("librarian").unwrap().len();
    assert!(
        total <= CEILING,
        "p50 session drew {total} B of guide (whole topic is {whole} B, ceiling {CEILING} B). \
         Either a section grew past the cap or a declaration is too broad."
    );
    assert!(total > 0, "the session must still receive guidance");
}
```

- [ ] **Step 2: Run to verify it fails or passes**

Run: `cargo test -p codescout a_p50_session_stays_under`
Expected: PASS if Task 6's declarations are tight; FAIL with the measured byte count if
any declaration is too broad. If it fails, narrow the declarations — do not raise the
ceiling.

- [ ] **Step 3: Add the PROBES.md row**

Use `edit_markdown` to add one row to the instrument table:

```markdown
| `cargo test a_p50_session_stays_under_the_committed_guide_byte_ceiling` | Guide bytes a median session receives | Simulates 6 shapes against a live server; does NOT model multi-topic sessions or the session opener, so it under-reports total session guide load |
```

- [ ] **Step 4: Full gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

- [ ] **Step 5: Commit and record the live prediction**

```bash
git add src/server.rs docs/PROBES.md
git commit -m "test(guides): golden byte ceiling for a p50 session"
```

Then rebuild the live server and record the prediction for later falsification:

```bash
cargo rb    # then /mcp to reconnect
```

The spec's falsifiable claim, to be checked with
`scripts/probe_guide_injection.py` once post-change sessions accumulate:

> **`librarian`'s contribution falls from ~20,545 B to ~10,000 B at p50**, with
> injection **count up** and total bytes **down**.

If bytes do not fall, declarations are too broad. If sessions start missing guidance
that used to arrive, the census was under-covered. Both are diagnosable from the same
probe output.

---

## Divergences from the spec, and why

Four places where this plan deliberately departs from
`2026-08-27-get-guide-section-grain-design.md`. Each is a simplification found by reading
the code the spec describes; none changes what Phase 1 delivers.

1. **No ledger format-version field.** The spec asks for one so a half-migrated session
   cannot silently suppress delivery. Reading `GuideLedger`, it is unnecessary: `emitted`
   is a `BTreeMap<String, DateTime<Utc>>` with an opaque key, and `load` already degrades
   a malformed file to empty — *"re-sending a guide, never suppressing one."* A stale
   `"librarian"` key simply fails to match `"librarian#Artifact Model"`, which re-delivers.
   What **does** need changing is `re_arm`, which would otherwise stop re-arming section
   keys on a project switch — that is Task 7.

2. **`topic_body()`, `GUIDE_TOPICS` and `GetGuide`'s `summaries` map are NOT yet derived
   from the index.** The spec has all three become projections of the parsed corpus. Doing
   that in Phase 1 would enlarge the diff across the `get_guide` tool and its four pinning
   tests for no Phase-1 benefit — the index is *built from* them instead. Deferred to
   Phase 3, when every topic declares and the derivation is total rather than partial.

3. **The spec's "stale ledger reads as empty" runtime test is replaced** by Task 7's two
   `re_arm` tests, which cover the only behaviour that actually changes. The stale-read
   path is already covered by `GuideLedger`'s existing tests and is untouched here.

4. **The spec's `requires-decl` grammar was never implemented as comma-separated.**
   § 1 originally wrote `requires-decl := "requires:" heading ("," heading)*`, mirroring
   `serves-decl`. `parse_declarations` (`src/prompts/guide_index.rs`) deliberately does
   NOT comma-split `requires:` — a heading is prose and commonly contains its own commas
   (e.g. "docs/trackers/ — Backing Store, Not a Docs Folder"), so one `requires:` line
   names exactly one heading, and multiple requirements are multiple
   `<!-- requires: ... -->` lines. The spec's grammar line has been corrected to match;
   this entry records that the divergence was a spec bug, not a Phase-1 simplification
   like 1-3 above.

---

## Out of scope for Phase 1

- **`tracker-conventions`** (Phase 2) — blocked on decomposing § *Entry-level standard*
  (17,378 B) and § *Bug files* (10,170 B) at `###`.
- **The remaining eight topics** (Phase 3), including the four that have never
  auto-injected: `iron-laws-detail`, `librarian-runtime`, `untrusted-content`,
  `error-handling` — 28,955 B, 27% of the corpus.
- **The 44.4% `contradicted` rate.** Enforcement, not grain; its own design.
- **Decomposing `librarian` § *Body Editing Surfaces* further.** Task 10's p50 ceiling
  test measures the current draw at 11,946 B against a 12,000 B ceiling — a 54 B
  (0.5%) margin. `artifact.update` alone is 3,265 B, the largest single shape,
  because both `###` children of *Body Editing Surfaces* (`Choosing a mode —
  anti-patterns` and `The shrink guard, force, and event forensics`) declare
  `serves: artifact.update` and are delivered as two separate wrapped blocks.
  Merging them was attempted and reverted: the merged section measured 2,696 B,
  over `MAX_DECLARED_SECTION_BYTES = 2500` — the cap's own error message
  recommends decomposing further at `###`, not merging, so the remedy here is
  splitting each child into smaller declaring sections, not consolidating them.

  > **DONE for one of the two, 2026-09-02.** `The shrink guard, force, and event
  > forensics` carried three paragraphs, one of which — the `field_patch` payload and
  > `artifact_event(action="list")` — is addressed to `artifact_event`, not to
  > `artifact.update`, and duplicated the one fact an update caller needs
  > (`replaced_subsections` reveals a destroyed child), which the sibling
  > anti-patterns section already states with *"read it."* Moved into
  > § *artifact_event — Event Log*, which already lists `field_patch` among its kinds;
  > the heading is now *The shrink guard, `force`, and `patch`'s accepted keys*.
  >
  > **The measurement is the interesting part: `librarian.md` shrank 3 B and the served
  > draw fell 445 B** — 12,330 → 11,885 against a 12,244 ceiling, margin 359 B.
  > Decomposition is not byte-trimming; it is serving bytes to the action that needs
  > them. The figures above (11,946 / 12,000 / 3,265 B) are the pre-decomposition state
  > and are kept for their derivation.
  >
  > *Choosing a mode — anti-patterns* is untouched and remains the open half.


### Carried deferred minors (from the SDD review loop, 2026-08-27)

Fifteen findings were consciously deferred across the ten task reviews and the final
whole-branch review. Each was judged real but non-blocking. Recorded here because the SDD
workspace that held them is deleted at finish — the same failure mode the final review
caught in the `edit_code` bug file.

**Structural / testability**

- `guide_blocks_for`, `inject_hint`, `GuideDeliveryShape` and `guide_block` are ~190 lines
  nested inside a ~408-line trait method, touching neither `self` nor `ctx`. Extracting them
  to a module-private `guide_emit` would turn three end-to-end tests into unit tests. The
  final reviewer promoted this but called it *follow-up, not a merge blocker*: the code is
  correct, only its testability is the problem. **Highest-value item on this list.**
- `guide_index.rs` is now ~860 lines across three concerns (split → parse → index), of which
  ~435 is the test module. Re-assess at Phase 2 when `tracker-conventions` decomposes.

**Latent correctness (none live against today's corpus)**

- Duplicate heading text under different parents yields one `ledger_key`; `guide_blocks_for`
  drops the second **permanently** for the session. Now gated by
  `no_topic_has_duplicate_section_headings`, so it fails the build rather than shipping.
- `call_tool_checked` keys only on `body.get("ok")`. `route_tool_error`'s LSP-transient
  branch returns `is_error: false` with `{"error":…, "hint":…}` and no `ok` key — defeating
  the predicate the same way. Not live: no p50 shape touches an LSP path.
- `re_arm`'s `key.len() > t.len() + 1` leaves a degenerate `topic#` empty-heading key
  unswept — the one place "prefer the duplicate" is not honoured. Such a key would indicate
  a key-construction bug upstream.
- `parse_declarations` comma-splits `serves:`, so a `path~` substring containing a literal
  comma mis-splits. Fails loudly (unterminated predicate → `Err`), never mis-parses.
- `fence_run` does not enforce CommonMark's rule that a closing fence contain only the
  delimiter run; a ` ```rust ` line would be accepted as a closer.
- `#` and `####` are not section boundaries. Worth knowing because Gate 3's failure message
  prescribes decomposition at `###`, so an over-cap `###` has no legal decomposition.
- Indented ATX headings are unrecognised (fence detection trims, heading detection does not).

**Cosmetic**

- Doc rationale is duplicated between `names_path_containing` and `names_tracker_path`.
- `adapter_for_test()` depends on `lib_all_tools()` registration order.
- `heading.clone()` re-allocates ~15 short strings per guide.
- `guide_index.rs`'s module doc never mentions the `GuideIndex` build/lookup surface.
- The `progressive-disclosure` comment now reads as documenting a whole `match` rather than
  the one arm whose condition it explains.
- The `whole` baseline in the ceiling's panic message uses the raw compiled-in body length,
  not the on-the-wire cost.
