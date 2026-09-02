# Result-Cap Marker Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a gate that makes every result-shaping cap in codescout either probed for a truncation marker or explicitly classified as not one, so `IC-13` recurrence reds the build instead of arriving as a thirteenth bug file.

**Architecture:** Two source-scanning instruments with deliberately different scopes (constant *declarations*; truncation *call sites*) plus a declarative table of behavioural probe rows that drive real tools past their own caps through `call_content`. All scan logic lives in pure `&str -> Vec<_>` functions so the meta-tests exercise the same code the gate runs, following `tests/issue_clusters.rs`'s `missing_index_rows` precedent.

**Tech Stack:** Rust 2021, `cargo test`, `regex` (already a dependency), `git ls-files` via `std::process::Command`, `serde_json`, `rmcp` Content blocks, `tokio` test runtime.

**Spec:** `docs/superpowers/specs/2026-09-02-result-cap-marker-gate-design.md` (artifact `ed7c767669ca46e3`)

## Global Constraints

- **Scan tracked files only** — `git ls-files src`, never a filesystem walk. An untracked file is a peer's in-flight work and gating on it lets one session red another's build. Precedent and the measured incident: `tests/issue_clusters.rs:1275-1281`; the open bug for getting it wrong is `docs/issues/2026-09-01-cluster-count-gate-lists-the-index-but-reads-the-worktree.md`.
- **The gate is a source-TEXT check, never a compiled-in registry.** `librarian` is a **default** feature (`Cargo.toml`), so caps inside `#[cfg(feature = "librarian")]` exist in source while their probe rows compile out under `--no-default-features`. A gate consulting compiled symbols reds on the lean lane and passes on the default one — a failure reached by *following* `CLAUDE.md`'s gate order.
- **Scan `src/` only, never `tests/`.** `tests/result_caps.rs` contains `cap-class:` example strings as fixtures. Scanning `tests/` would count a teaching example as a real declaration — the documentation-example-as-real-token trap in `CLAUDE.md` § *Parsers Over a Namespace*.
- **Every filter the gate runs is an extracted pure function the meta-tests call directly.** No meta-test may assert about its own re-implementation (`CLAUDE.md` § *Testing Discipline*).
- **Every exemption ships a paired test proving the exemption is narrow**, following `missing_index_rows_exempts_only_unclassified` (`tests/issue_clusters.rs:1412`).
- **Gate command, in order, `;`-chained not `&&`:** `cargo fmt` → `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` → `cargo test --workspace --no-default-features` → `cargo test --workspace`.
- **Annotation grammar, exact:** `cap-class: RESULT_CAP <id>` or `cap-class: NOT_A_CAP — <reason>`. `<id>` is `[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*` (e.g. `grep.lines`). `<reason>` must be non-empty after trimming. Separator accepts `—`, `–`, or `-`.
- **Commit on a shared checkout:** stage, then read `git diff --cached --name-only` in a **separate** tool call, then `git commit -m "..." -- <explicit paths>`. Never `git add -u`, never bare `git commit`.

---

## File Structure

| file | responsibility |
|---|---|
| `tests/result_caps.rs` (create) | Both instruments, the classification gate, the row cross-check, and all meta-tests. Pure parse functions + gate tests in one file because they change together. |
| `src/tools/core/cap_probe.rs` (create) | The declarative probe-row table only. Textually scanned by `tests/result_caps.rs`; deliberately data, not logic. |
| `src/tools/core/cap_probe_tests.rs` (create) | The async probe rows that drive real tools. Needs `CodeScoutServer`, so it lives in `src/`, feature-partitioned. |
| `src/server.rs` (modify) | Lift `call_tool_checked` + `shared_ctx` from the private `guide_hint_tests` module to `pub(crate)` test helpers. |
| `src/tools/core/mod.rs` (modify) | Register the two new modules. |
| `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (modify, via catalog) | Publish the per-site mutation tally in `**Mechanism status:**`. |

Tasks 1–2 deliver a working detector on their own. Task 3 is the data pass it enables. Tasks 4–6 add probing. Task 7 publishes the bound.

---

### Task 1: Instrument A — classify cap constant declarations

**Files:**
- Create: `tests/result_caps.rs`
- Test: same file (integration test; parse fns + gate tests co-located)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `struct CapDecl { pub name: String, pub file: String, pub line: usize, pub annotation: Option<String> }`
  - `fn cap_constants(src: &str, file: &str) -> Vec<CapDecl>`
  - `enum CapClass { ResultCap(String), NotACap(String), Unclassified, MalformedReason }`
  - `fn classify(decl: &CapDecl) -> CapClass`
  - `fn tracked_src_files() -> Vec<String>`
  - `fn repo_root() -> PathBuf`

- [ ] **Step 1: Write the failing tests**

```rust
//! Instruments for `IC-13` (`cluster/capped-result-presented-as-complete`).
//!
//! TWO instruments run here, and their scopes differ ON PURPOSE. `CLAUDE.md`
//! § *Observer Blindness*: two agreeing instruments are evidence only when
//! their scopes differ, because two same-scope instruments agreeing is one
//! blind spot counted twice and is indistinguishable from corroboration at
//! the point of use. Instrument A reads DECLARATIONS, instrument B reads
//! CALL SITES.
//!
//! Instrument A alone would ship `IC-18` (`selector-narrower-than-its-
//! population`) inside the gate for `IC-13`: its name regex cannot see a cap
//! called `PAGE_SIZE`, and cannot see a bare `.next()` at all. The
//! `indexer.rs` first-chunk-only member (fixed at `488192e8`) was exactly
//! that shape — no constant anywhere — which is why B exists.
//!
//! Scans `src/` and never `tests/`: this file contains `cap-class:` strings
//! as FIXTURES, and a scanner that read them would count a teaching example
//! as a declaration.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CapDecl {
    name: String,
    file: String,
    line: usize,
    /// Text after `cap-class:` on the last such line in the contiguous
    /// comment block directly above the declaration, trimmed.
    annotation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CapClass {
    ResultCap(String),
    NotACap(String),
    Unclassified,
    /// `NOT_A_CAP` with an empty reason. Distinguished from `Unclassified`
    /// because "the annotation exists" is not the property we want, and the
    /// two need different failure text.
    MalformedReason,
}

#[test]
fn cap_constants_finds_a_cap_shaped_const_and_its_annotation() {
    let src = "\
// cap-class: RESULT_CAP grep.lines — probed
const GREP_LINE_LIMIT: usize = 50;
";
    let got = cap_constants(src, "src/x.rs");
    assert_eq!(got.len(), 1, "one cap-shaped const");
    assert_eq!(got[0].name, "GREP_LINE_LIMIT");
    assert_eq!(got[0].line, 2);
    assert_eq!(
        got[0].annotation.as_deref(),
        Some("RESULT_CAP grep.lines — probed")
    );
}

#[test]
fn cap_constants_ignores_a_const_whose_name_is_not_cap_shaped() {
    let src = "const EMBED_CONCURRENCY: usize = 8;\n";
    assert!(
        cap_constants(src, "src/x.rs").is_empty(),
        "EMBED_CONCURRENCY is not cap-shaped; instrument B is what covers \
         a bound like this, and that gap is the reason B exists"
    );
}

#[test]
fn cap_constants_accepts_pub_and_visibility_qualified_forms() {
    let src = "\
pub const A_MAX: usize = 1;
pub(crate) const B_LIMIT: usize = 2;
    const C_CAP: usize = 3;
";
    let names: Vec<String> = cap_constants(src, "src/x.rs")
        .into_iter()
        .map(|d| d.name)
        .collect();
    assert_eq!(names, vec!["A_MAX", "B_LIMIT", "C_CAP"]);
}

#[test]
fn cap_constants_does_not_read_an_annotation_through_a_blank_line() {
    let src = "\
// cap-class: RESULT_CAP stale.ref — belongs to something else

const OTHER_MAX: usize = 1;
";
    assert_eq!(
        cap_constants(src, "src/x.rs")[0].annotation, None,
        "a blank line ends the comment block; reading through it would let \
         one annotation silently cover an unrelated later constant"
    );
}

#[test]
fn cap_constants_skips_a_cap_class_line_inside_a_doc_fence() {
    let src = "\
/// Example for readers:
/// ```
/// // cap-class: RESULT_CAP example.only
/// ```
const REAL_MAX: usize = 1;
";
    assert_eq!(
        cap_constants(src, "src/x.rs")[0].annotation, None,
        "a fenced example must not classify its neighbour — the \
         documentation-example-as-real-token trap (CLAUDE.md § Parsers Over \
         a Namespace)"
    );
}

#[test]
fn classify_reads_the_three_states_and_rejects_an_empty_reason() {
    let mk = |ann: Option<&str>| CapDecl {
        name: "X_MAX".into(),
        file: "src/x.rs".into(),
        line: 1,
        annotation: ann.map(str::to_owned),
    };
    assert_eq!(
        classify(&mk(Some("RESULT_CAP grep.lines — probed"))),
        CapClass::ResultCap("grep.lines".into())
    );
    assert_eq!(
        classify(&mk(Some("NOT_A_CAP — LSP handshake deadline"))),
        CapClass::NotACap("LSP handshake deadline".into())
    );
    assert_eq!(classify(&mk(None)), CapClass::Unclassified);
    assert_eq!(
        classify(&mk(Some("NOT_A_CAP —   "))),
        CapClass::MalformedReason,
        "a bare NOT_A_CAP token is RED: an annotation that need not say why \
         is satisfied by writing the token, which is not the property wanted"
    );
    assert_eq!(
        classify(&mk(Some("NOT_A_CAP"))),
        CapClass::MalformedReason,
        "no separator, no reason"
    );
}

#[test]
fn classify_accepts_all_three_dash_forms() {
    for sep in ["—", "–", "-"] {
        let decl = CapDecl {
            name: "X_MAX".into(),
            file: "src/x.rs".into(),
            line: 1,
            annotation: Some(format!("NOT_A_CAP {sep} a stated reason")),
        };
        assert_eq!(
            classify(&decl),
            CapClass::NotACap("a stated reason".into()),
            "separator {sep:?} must parse; house style is not uniform and a \
             gate that accepted only one would refuse correct annotations"
        );
    }
}

#[test]
fn tracked_src_files_returns_rust_files_under_src_and_excludes_tests() {
    let files = tracked_src_files();
    assert!(
        files.iter().any(|f| f == "src/tools/grep.rs"),
        "a known src file must be present; got {} files",
        files.len()
    );
    assert!(
        files.iter().all(|f| f.starts_with("src/") && f.ends_with(".rs")),
        "only tracked .rs under src/"
    );
    assert!(
        !files.iter().any(|f| f.starts_with("tests/")),
        "tests/ carries cap-class fixtures and must never be scanned"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test result_caps`
Expected: FAIL to compile — `cannot find function 'cap_constants' in this scope`, and the same for `classify` / `tracked_src_files`.

- [ ] **Step 3: Write the implementation**

Append to `tests/result_caps.rs`:

```rust
/// Tracked `.rs` files under `src/`.
///
/// `git ls-files`, not a walk: an untracked file is a peer's in-flight work
/// and gating on it lets one session red another's build. Same reasoning and
/// the same measured incident as `tracked_all_bug_files` in
/// `tests/issue_clusters.rs`.
fn tracked_src_files() -> Vec<String> {
    let out = Command::new("git")
        .args(["ls-files", "src"])
        .current_dir(repo_root())
        .output()
        .expect("git ls-files failed to run — this gate needs a git checkout");
    assert!(
        out.status.success(),
        "git ls-files exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|p| p.ends_with(".rs"))
        .map(str::to_owned)
        .collect()
}

/// True when a constant's name is cap-shaped.
///
/// A FLOOR, never a census. `PAGE_SIZE` and `DEFAULT_DEPTH` are caps this
/// predicate cannot see; instrument B is what covers them, and the two
/// instruments' disagreement is the signal.
fn is_cap_shaped(name: &str) -> bool {
    ["CAP", "LIMIT", "MAX", "BUDGET", "THRESHOLD"]
        .iter()
        .any(|t| name.contains(t))
}

/// Cap-shaped `const` declarations in `src`, each with the `cap-class:`
/// annotation from the contiguous comment block directly above it.
///
/// Takes `&str` rather than reading the file so the meta-tests above drive
/// THIS function on fixtures — not a second copy that could drift from it
/// (`missing_index_rows` precedent, `tests/issue_clusters.rs:461-471`).
fn cap_constants(src: &str, file: &str) -> Vec<CapDecl> {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = vec![];

    for (idx, raw) in lines.iter().enumerate() {
        let t = raw.trim_start();
        let after_vis = t
            .strip_prefix("pub(crate) ")
            .or_else(|| t.strip_prefix("pub(super) "))
            .or_else(|| t.strip_prefix("pub "))
            .unwrap_or(t);
        let Some(rest) = after_vis.strip_prefix("const ") else {
            continue;
        };
        let Some((name, _)) = rest.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            || !is_cap_shaped(name)
        {
            continue;
        }

        out.push(CapDecl {
            name: name.to_string(),
            file: file.to_string(),
            line: idx + 1,
            annotation: annotation_above(&lines, idx),
        });
    }
    out
}

/// The last `cap-class:` payload in the contiguous comment block immediately
/// above `decl_idx`, skipping lines inside a fenced block within that
/// comment.
///
/// A blank line ends the block: reading through one would let a stray
/// annotation silently classify an unrelated later constant, which is a
/// wrong classification rather than a missing one.
fn annotation_above(lines: &[&str], decl_idx: usize) -> Option<String> {
    let mut block: Vec<&str> = vec![];
    for i in (0..decl_idx).rev() {
        let t = lines[i].trim_start();
        if t.starts_with("#[") {
            continue; // attributes sit between the doc block and the item
        }
        if t.starts_with("//") {
            block.push(t);
            continue;
        }
        break;
    }
    block.reverse();

    let mut in_fence = false;
    let mut found = None;
    for t in block {
        let body = t
            .trim_start_matches('/')
            .trim_start_matches('!')
            .trim_start();
        if body.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some((_, payload)) = body.split_once("cap-class:") {
            found = Some(payload.trim().to_string());
        }
    }
    found
}

/// Split an annotation payload into its class and the text after the dash.
fn split_reason(payload: &str) -> Option<&str> {
    for sep in ["—", "–", "-"] {
        if let Some((_, rest)) = payload.split_once(sep) {
            return Some(rest.trim());
        }
    }
    None
}

fn classify(decl: &CapDecl) -> CapClass {
    let Some(payload) = decl.annotation.as_deref() else {
        return CapClass::Unclassified;
    };
    let payload = payload.trim();

    if let Some(rest) = payload.strip_prefix("RESULT_CAP") {
        let id = rest
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '_');
        if id.is_empty() {
            return CapClass::MalformedReason;
        }
        return CapClass::ResultCap(id.to_string());
    }

    if payload.starts_with("NOT_A_CAP") {
        return match split_reason(payload) {
            Some(r) if !r.is_empty() => CapClass::NotACap(r.to_string()),
            _ => CapClass::MalformedReason,
        };
    }

    CapClass::Unclassified
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test result_caps`
Expected: PASS, 7 tests.

- [ ] **Step 5: Commit**

```bash
git add tests/result_caps.rs
```

Then, in a **separate** call: `git diff --cached --name-only`

Then:

```bash
git commit -m "test(caps): instrument A parses cap declarations and their class

Pure &str functions so the meta-tests drive the same code the gate runs,
following missing_index_rows. Three parser obligations pinned by test: a
blank line ends the comment block (so a stray annotation cannot classify an
unrelated later const), a fenced example never classifies its neighbour, and
a bare NOT_A_CAP with no reason is a distinct RED from an absent annotation.

is_cap_shaped is a FLOOR and says so — PAGE_SIZE is invisible to it. That
gap is instrument B's job, which is Task 2." -- tests/result_caps.rs
```

---

### Task 2: Instrument B — find truncation call sites, and the gate that joins both

**Files:**
- Modify: `tests/result_caps.rs`

**Interfaces:**
- Consumes: `CapDecl`, `CapClass`, `cap_constants`, `classify`, `tracked_src_files` (Task 1).
- Produces:
  - `struct TruncSite { pub op: String, pub file: String, pub line: usize, pub annotation: Option<String> }`
  - `fn truncation_sites(src: &str, file: &str) -> Vec<TruncSite>`
  - `fn unclassified_decls(decls: &[CapDecl]) -> Vec<String>`

- [ ] **Step 1: Write the failing tests**

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
struct TruncSite {
    op: String,
    file: String,
    line: usize,
    annotation: Option<String>,
}

#[test]
fn truncation_sites_finds_the_operations_instrument_a_cannot_see() {
    let src = "\
let first = chunk_markdown(body).next();
let head = items.take(10);
s.truncate(80);
";
    let ops: Vec<String> = truncation_sites(src, "src/x.rs")
        .into_iter()
        .map(|s| s.op)
        .collect();
    assert!(ops.contains(&".next()".to_string()), "got {ops:?}");
    assert!(ops.contains(&".take(".to_string()), "got {ops:?}");
    assert!(ops.contains(&".truncate(".to_string()), "got {ops:?}");
}

#[test]
fn truncation_sites_ignores_stream_next_which_is_iteration_not_capping() {
    let src = "while let Some(res) = stream.next().await {\n";
    assert!(
        truncation_sites(src, "src/x.rs").is_empty(),
        "`stream.next().await` drains an async stream — it caps nothing. \
         src/librarian/indexer.rs:918,1051 are exactly this and must stay \
         silent, or the gate cries wolf where the real member (a bare \
         `.next()` on a chunk iterator) was one line away"
    );
}

#[test]
fn truncation_sites_reads_an_annotation_the_same_way_declarations_do() {
    let src = "\
// cap-class: NOT_A_CAP — bounded by the caller's explicit line range
let head = items.take(n);
";
    assert_eq!(
        truncation_sites(src, "src/x.rs")[0].annotation.as_deref(),
        Some("NOT_A_CAP — bounded by the caller's explicit line range"),
        "one annotation grammar for both instruments — a second grammar is \
         a second thing to get wrong"
    );
}

#[test]
fn unclassified_decls_names_every_offender_and_is_not_a_bare_count() {
    let decls = vec![
        CapDecl { name: "A_MAX".into(), file: "src/a.rs".into(), line: 3, annotation: None },
        CapDecl {
            name: "B_LIMIT".into(),
            file: "src/b.rs".into(),
            line: 9,
            annotation: Some("RESULT_CAP b.rows — probed".into()),
        },
        CapDecl {
            name: "C_CAP".into(),
            file: "src/c.rs".into(),
            line: 4,
            annotation: Some("NOT_A_CAP".into()),
        },
    ];
    let got = unclassified_decls(&decls);
    assert_eq!(
        got,
        vec![
            "src/a.rs:3 A_MAX — no cap-class annotation".to_string(),
            "src/c.rs:4 C_CAP — NOT_A_CAP with no reason".to_string(),
        ],
        "the classified one must not appear, and each offender must arrive \
         with its file:line — a count tells nobody which constant to go fix"
    );
}

/// THE GATE. Every cap-shaped constant in tracked `src/` is classified.
#[test]
fn every_cap_constant_is_classified() {
    let mut offenders = vec![];
    for file in tracked_src_files() {
        let path = repo_root().join(&file);
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        offenders.extend(unclassified_decls(&cap_constants(&src, &file)));
    }
    assert!(
        offenders.is_empty(),
        "{} cap constant(s) carry no usable `cap-class:` annotation.\n\n{}\n\n\
         Add ONE of these on the line above each, in its doc comment:\n  \
         // cap-class: RESULT_CAP <surface>.<what> — probed\n  \
         // cap-class: NOT_A_CAP — <why this never shapes a result>\n\n\
         RESULT_CAP means a caller can receive a partial result because of \
         this bound; it then needs a probe row in \
         src/tools/core/cap_probe.rs. NOT_A_CAP needs a REASON, not just \
         the token — a timeout, a batch size, a retry ceiling. Why this \
         gate exists: docs/trackers/issue-clusters/\
         IC-13-capped-result-presented-as-complete.md",
        offenders.len(),
        offenders.join("\n")
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test result_caps`
Expected: FAIL — `cannot find function 'truncation_sites'`; and once it compiles, `every_cap_constant_is_classified` FAILS listing ~105 offenders. **That red is the point of Task 3** — record the number it prints.

- [ ] **Step 3: Write the implementation**

```rust
/// Truncation OPERATIONS, the scope instrument A cannot reach.
///
/// `.next()` is included because the `indexer.rs` first-chunk-only member
/// (fixed at `488192e8`) was a bare `.next()` on a chunk iterator with no
/// constant anywhere — invisible to a declaration scan by construction.
///
/// `stream.next()` / `.next().await` are EXCLUDED: draining an async stream
/// caps nothing. That exclusion is narrow and pinned by
/// `truncation_sites_ignores_stream_next_which_is_iteration_not_capping`,
/// because an over-broad instrument that fires on every iterator teaches
/// readers to annotate noise, and an annotation written to silence a gate
/// classifies nothing.
fn truncation_sites(src: &str, file: &str) -> Vec<TruncSite> {
    const OPS: [&str; 4] = [".take(", ".truncate(", "truncate_compact(", ".next()"];
    let lines: Vec<&str> = src.lines().collect();
    let mut out = vec![];

    for (idx, raw) in lines.iter().enumerate() {
        let code = raw.trim_start();
        if code.starts_with("//") {
            continue;
        }
        for op in OPS {
            if !code.contains(op) {
                continue;
            }
            if op == ".next()"
                && (code.contains("stream.next()")
                    || code.contains(".next().await")
                    || code.contains("s.next()"))
            {
                continue;
            }
            out.push(TruncSite {
                op: op.to_string(),
                file: file.to_string(),
                line: idx + 1,
                annotation: annotation_above(&lines, idx),
            });
        }
    }
    out
}

/// Declarations the gate refuses, each named with its location.
///
/// Extracted so [`every_cap_constant_is_classified`] and
/// [`unclassified_decls_names_every_offender_and_is_not_a_bare_count`] run
/// the SAME filter rather than two copies that could drift.
fn unclassified_decls(decls: &[CapDecl]) -> Vec<String> {
    let mut out: Vec<String> = decls
        .iter()
        .filter_map(|d| match classify(d) {
            CapClass::Unclassified => {
                Some(format!("{}:{} {} — no cap-class annotation", d.file, d.line, d.name))
            }
            CapClass::MalformedReason => {
                Some(format!("{}:{} {} — NOT_A_CAP with no reason", d.file, d.line, d.name))
            }
            CapClass::ResultCap(_) | CapClass::NotACap(_) => None,
        })
        .collect();
    out.sort();
    out
}
```

- [ ] **Step 4: Run and confirm the shape of the failure**

Run: `cargo test --test result_caps`
Expected: the four unit tests PASS; `every_cap_constant_is_classified` FAILS naming every unclassified constant. Verify the failure text prints `file:line NAME` lines, not a bare count.

- [ ] **Step 5: Mark the gate `#[ignore]` with a reason, so the tree is green between tasks**

```rust
#[test]
#[ignore = "un-ignored by Task 3, which classifies the backlog this names. \
            Kept as a test rather than deleted so `cargo test -- --ignored` \
            prints the live worklist."]
fn every_cap_constant_is_classified() {
```

- [ ] **Step 6: Run the full gate**

Run, `;`-chained not `&&`:
```
cargo fmt ; cargo clippy --workspace --all-targets --features local-embed -- -D warnings ; cargo test --workspace --no-default-features ; cargo test --workspace
```
Expected: all four green. Read each exit code; do not rely on the chain stopping.

- [ ] **Step 7: Commit**

```bash
git add tests/result_caps.rs
```

Separate call: `git diff --cached --name-only`

```bash
git commit -m "test(caps): instrument B reads truncation sites; the gate lands ignored

B scans OPERATIONS, not declarations, because the scopes must differ: two
same-scope instruments agreeing is one blind spot counted twice. B is the
only one that reaches a bare .next() on a chunk iterator — the shape of the
indexer member fixed at 488192e8, which had no constant anywhere.

The stream.next() exclusion is narrow and pinned. An instrument that fired
on every iterator would teach readers to annotate noise, and an annotation
written to silence a gate classifies nothing.

The gate lands #[ignore]d with its reason, naming Task 3 as what un-ignores
it: it currently reds on the whole pre-existing backlog, and a red tree
between tasks is a red tree for every concurrent session." -- tests/result_caps.rs
```

---

### Task 3: Classify the backlog, and un-ignore the gate

**Files:**
- Modify: every tracked `src/**/*.rs` the gate names (~52 files by the 2026-09-02 measurement)
- Modify: `tests/result_caps.rs` (remove the `#[ignore]`)

**Interfaces:**
- Consumes: `every_cap_constant_is_classified` (Task 2) as the worklist.
- Produces: every cap-shaped constant carrying `RESULT_CAP <id>` or `NOT_A_CAP — <reason>`. The set of `RESULT_CAP` ids is Task 5's input.

- [ ] **Step 1: Get the worklist from the gate itself**

Run: `cargo test --test result_caps -- --ignored every_cap_constant_is_classified`

The failure lists every offender as `file:line NAME`. **Do not** re-derive this list with a shell grep: the gate's own parser is the definition of the question, and a second selector would answer a slightly different one — the `IC-18` mistake this gate exists to catch.

- [ ] **Step 2: Classify each, applying one decision rule**

For each named constant, read its declaration and its use, then annotate:

- **`RESULT_CAP <surface>.<what>`** if a caller can receive a **partial result** because of this bound — a page size, a display limit, a byte budget on returned content, a heading count.
- **`NOT_A_CAP — <reason>`** otherwise: timeouts, retry ceilings, batch sizes, buffer capacities, concurrency limits, log rotation sizes. The reason must say *why it never shapes a result*.

Worked examples, from constants confirmed present on 2026-09-02:

```rust
// In src/tools/grep.rs — a caller CAN get fewer matches than exist.
// cap-class: RESULT_CAP grep.match_bytes — probed
const MAX_MATCH_BYTES: usize = 2_000;

// cap-class: RESULT_CAP grep.total_bytes — probed
const MAX_TOTAL_MATCH_BYTES: usize = 60_000;

// In src/lsp/client.rs — shapes no result; a breach is an error, not a short list.
// cap-class: NOT_A_CAP — LSP request deadline; a breach surfaces as an
// error, never as a shortened result
const REQUEST_TIMEOUT_MS: u64 = 30_000;
```

**Two rules for this pass.** Annotate the constant you are reading — never
classify from its name, which is this ledger's own *never classify on
description* rule, and it has already failed on `IC-13` itself (a member was
filed under `doctor` because the word appeared in its prose). And when
unsure, choose `RESULT_CAP`: a needless probe row costs one test, while a
wrong `NOT_A_CAP` is a silent exemption that reads as coverage.

- [ ] **Step 3: Re-run until the gate is empty**

Run: `cargo test --test result_caps -- --ignored every_cap_constant_is_classified`
Repeat Step 2 until it PASSES. Expected end state: zero offenders.

- [ ] **Step 4: Remove the `#[ignore]`**

Delete the `#[ignore = "..."]` attribute added in Task 2 Step 5.

- [ ] **Step 5: Record the census in the module header, with its unit**

Add to `tests/result_caps.rs`'s `//!` block, substituting the real figures:

```rust
//! ## Census — 2026-09-02
//!
//! **N cap-shaped constants** in tracked `src/` (unit: `const` declarations
//! matching [`is_cap_shaped`], one count per declaration, not per use site):
//! **R** `RESULT_CAP`, **K** `NOT_A_CAP`. Derived by
//! [`every_cap_constant_is_classified`] over `git ls-files src`, not by a
//! shell grep — the earlier `grep -c` figure of 105 counted a different
//! population and is not this number.
```

- [ ] **Step 6: Run the full gate**

Run, `;`-chained: `cargo fmt ; cargo clippy --workspace --all-targets --features local-embed -- -D warnings ; cargo test --workspace --no-default-features ; cargo test --workspace`
Expected: four green. The lean lane matters here — annotations are comments, but Task 3 touches feature-gated files.

- [ ] **Step 7: Commit**

```bash
git add tests/result_caps.rs src
```

Separate call: `git diff --cached --name-only` — confirm every path is one you classified and none is a peer's in-flight file.

```bash
git commit -m "refactor(caps): classify every cap constant, and arm the gate

Each cap-shaped const in tracked src/ now says whether a caller can receive
a partial result because of it. RESULT_CAP ids are the input to the probe
rows; NOT_A_CAP carries a reason, because an exemption that need not say why
is satisfied by writing the token.

Classified by reading each declaration and its use, never from the name —
IC-13's own ledger recorded a member mis-filed under 'doctor' because the
word appeared in its prose, so this is the ledger's rule applied to itself.

The worklist came from the gate's own parser rather than a shell grep: a
second selector answers a slightly different question, which is the IC-18
mistake this gate exists to catch.

Census with its unit is in the module header." -- tests/result_caps.rs src
```

---

### Task 4: Lift the tool driver into a shared test helper

**Files:**
- Modify: `src/server.rs` — `call_tool_checked` (`:6430`), `shared_ctx` (`:6319`), `guide_blocks` (`:6464`)
- Create: `src/tools/core/cap_probe.rs` (empty table, populated in Task 5)
- Modify: `src/tools/core/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub(crate) async fn call_tool_checked(server: &CodeScoutServer, name: &str, input: serde_json::Value, label: &str) -> Vec<rmcp::model::Content>`
  - `pub(crate) fn shared_ctx(server: &CodeScoutServer) -> crate::tools::ToolContext`
  - both `#[cfg(test)]`, reachable from `src/tools/core/cap_probe_tests.rs`

- [ ] **Step 1: Write the failing test**

Create `src/tools/core/cap_probe_tests.rs`:

```rust
//! Behavioural probe rows for `IC-13`.
//!
//! Each row drives a real tool PAST ITS OWN CAP through the same
//! `call_content` path an agent uses, and asserts a truncation marker
//! ARRIVES. Not that it is correct — `IC-13`'s clause deliberately excludes
//! a visible-but-wrong marker, whose true total is sometimes unknowable
//! (`grep`'s old `Showing N of N`). Arrival is the property.
#![cfg(test)]

use super::super::super::server::test_support::{call_tool_checked, shared_ctx};
use serde_json::Value;

/// Proves the lifted driver is reachable from here at all.
///
/// Task 4's whole deliverable is that reachability: a second copy of
/// `call_tool_checked` would be a second place to get the
/// `RecoverableError`-routes-to-success subtlety wrong, and that mistake
/// makes a BAD ROW look like a finding.
#[tokio::test]
async fn the_lifted_driver_reaches_a_real_tool_from_this_module() {
    let server = crate::server::CodeScoutServer::new_for_test();
    let out = call_tool_checked(&server, "grep", serde_json::json!({"pattern": "fn "}), "smoke").await;
    let primary = out[0].as_text().expect("primary block is text");
    let v: Value = serde_json::from_str(&primary.text).expect("primary block is JSON");
    assert!(
        v.is_object(),
        "a real tool response arrived through the lifted driver"
    );
    let _ = shared_ctx(&server);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib cap_probe`
Expected: FAIL to compile — `module 'test_support' is private` / `function 'call_tool_checked' is private`.

- [ ] **Step 3: Move the three helpers into a shared module**

In `src/server.rs`, move `shared_ctx`, `call_tool_checked` and `guide_blocks` out of `mod guide_hint_tests` into a sibling module, preserving their doc comments **verbatim** — `call_tool_checked`'s records the `RecoverableError` trap and is load-bearing:

```rust
#[cfg(test)]
pub(crate) mod test_support {
    //! Test-only helpers shared by more than one test module.
    //!
    //! `call_tool_checked` lives here rather than in each consumer because
    //! codescout routes `RecoverableError` to a SUCCESS result carrying
    //! `{"ok": false}` — so `is_error` alone silently passes a failed call
    //! (pinned by `recoverable_error_routes_to_success_not_is_error`). A
    //! second copy is a second place to get that wrong, and a probe that
    //! scored a rejected call as "capped, no marker" would report a
    //! plausible finding instead of an error.
    use super::*;

    // ... the three functions, moved unchanged except `pub(crate)`
}
```

In `mod guide_hint_tests`, replace the definitions with:

```rust
use super::test_support::{call_tool_checked, guide_blocks, shared_ctx};
```

Add `CodeScoutServer::new_for_test()` if no equivalent constructor exists, matching however `guide_hint_tests` builds its server today; reuse that code rather than writing a second constructor.

In `src/tools/core/mod.rs`:

```rust
// cap-class: NOT_A_CAP — module registration, declares no bound
#[cfg(test)]
mod cap_probe_tests;
pub(crate) mod cap_probe;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib cap_probe` → PASS.
Run: `cargo test --lib guide_hint_tests` → PASS, unchanged. **This second run is the point of the task**: the p50 emission-ceiling test lives in that module, so a botched move would move a load-bearing gate.

Note: `a_p50_session_stays_under_the_committed_emission_byte_ceiling` was **already red** before this work began, at 12,330 B against a 12,244 B ceiling, filed as `docs/issues/2026-09-02-a-corrected-ceiling-reds-within-minutes-on-a-shared-checkout.md` and owned by another session. Confirm it fails with **that same byte figure** and no other test in the module changes state — a different figure means this move perturbed the emission.

- [ ] **Step 5: Commit**

```bash
git add src/server.rs src/tools/core/mod.rs src/tools/core/cap_probe.rs src/tools/core/cap_probe_tests.rs
```

Separate call: `git diff --cached --name-only`

```bash
git commit -m "refactor(test): lift call_tool_checked into a shared test_support module

Probe rows need the same driver guide_hint_tests uses, and a second copy is
a second place to get the RecoverableError-routes-to-success subtlety wrong
— that mistake makes a bad row look like a finding, so the doc comment moves
verbatim with the function.

guide_hint_tests re-imports rather than keeping its own. Verified the p50
emission-ceiling test still reports its pre-existing 12,330 B failure and no
other test in that module changed state: a different figure would mean the
move perturbed the emission it measures." -- src/server.rs src/tools/core/mod.rs src/tools/core/cap_probe.rs src/tools/core/cap_probe_tests.rs
```

---

### Task 5: The probe table, three positive controls, and the row cross-check

**Files:**
- Modify: `src/tools/core/cap_probe.rs`
- Modify: `src/tools/core/cap_probe_tests.rs`
- Modify: `tests/result_caps.rs`

**Interfaces:**
- Consumes: `call_tool_checked`, `shared_ctx` (Task 4); the `RESULT_CAP` ids (Task 3).
- Produces:
  - `pub(crate) struct ProbeRow { pub id: &'static str, pub marker_path: &'static str, pub mutation: Mutation }`
  - `pub(crate) enum Mutation { Killed, NotYet(&'static str) }`
  - `pub(crate) const PROBE_ROWS: &[ProbeRow]`
  - `fn probe_row_ids(src: &str) -> BTreeSet<String>` in `tests/result_caps.rs`

- [ ] **Step 1: Write the table**

`src/tools/core/cap_probe.rs`:

```rust
//! The declarative probe-row table.
//!
//! DATA, not logic, and in its own file on purpose: `tests/result_caps.rs`
//! scans this file as TEXT to cross-check ids against `RESULT_CAP`
//! annotations. A text scan is not fastidiousness — `librarian` is a default
//! feature, so rows behind `#[cfg(feature = "librarian")]` compile out under
//! `--no-default-features`, and a gate reading compiled symbols would red on
//! the lean lane while passing on the default one. That is a failure reached
//! by FOLLOWING `CLAUDE.md`'s gate order.

/// Whether a row's ability to fail has been demonstrated by breaking the
/// production path it guards.
///
/// `CLAUDE.md` § *Testing Discipline*: demand an observed RED, and mutate
/// once per guarded SITE — one kill says nothing about the other N−1. A
/// `NotYet` row is an assertion whose ability to fail is UNPROVEN, and
/// naming it is what stops it being credited as coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mutation {
    /// The production marker emission was deleted, this row was observed
    /// red, and the deletion was reverted.
    Killed,
    /// Not yet mutated. The string says why, and is not optional.
    NotYet(&'static str),
}

pub(crate) struct ProbeRow {
    /// Matches a `cap-class: RESULT_CAP <id>` annotation in `src/`.
    pub id: &'static str,
    /// Where the marker must be reachable in the shape the caller reads.
    pub marker_path: &'static str,
    pub mutation: Mutation,
}

/// Phase 1 rows.
///
/// The first three are POSITIVE CONTROLS: their markers already work. They
/// are not redundant — without a row that should PASS, a probe reporting
/// "marker missing" everywhere is indistinguishable from a probe that cannot
/// read a marker at all. They are the denominator.
pub(crate) const PROBE_ROWS: &[ProbeRow] = &[
    ProbeRow {
        id: "run_command.inline_bytes",
        marker_path: "$.unfiltered_truncated",
        mutation: Mutation::NotYet("positive control; mutated in Task 6"),
    },
    ProbeRow {
        id: "grep.total_bytes",
        marker_path: "$.overflow.total_is_lower_bound",
        mutation: Mutation::NotYet("positive control; mutated in Task 6"),
    },
    ProbeRow {
        id: "link_scan.findings",
        marker_path: "$.counts.truncated",
        mutation: Mutation::NotYet("positive control; mutated in Task 6"),
    },
    ProbeRow {
        id: "artifact.find_limit",
        marker_path: "$.hints.more_in_scope",
        mutation: Mutation::NotYet("added Task 5; mutation pending"),
    },
];
```

- [ ] **Step 2: Write the failing probe test**

Append to `src/tools/core/cap_probe_tests.rs`:

```rust
use super::cap_probe::PROBE_ROWS;

/// Read a `$.a.b` path out of a JSON value.
fn at_path<'v>(v: &'v Value, path: &str) -> Option<&'v Value> {
    let mut cur = v;
    for seg in path.trim_start_matches("$.").split('.') {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

#[tokio::test]
async fn grep_past_its_byte_budget_marks_the_result_as_a_lower_bound() {
    let server = crate::server::CodeScoutServer::new_for_test();
    // `fn ` over the whole tree exceeds MAX_TOTAL_MATCH_BYTES by a wide
    // margin, so collection stops early.
    let out = call_tool_checked(
        &server,
        "grep",
        serde_json::json!({"pattern": "fn ", "limit": 500}),
        "grep.total_bytes",
    )
    .await;
    let v: Value = serde_json::from_str(&out[0].as_text().unwrap().text).unwrap();

    // (1) Establish capped-ness FROM THE RESPONSE. Asserting it from the
    //     input would make this row vacuous-but-passing if the cap moved —
    //     the monotone-assertion failure in CLAUDE.md § Testing Discipline.
    let overflow = v.get("overflow").expect(
        "no `overflow` key: this call was NOT capped, so the row proves \
         nothing. Widen the pattern rather than relaxing the assertion",
    );

    // (2) The marker must ARRIVE.
    assert_eq!(
        overflow.get("total_is_lower_bound"),
        Some(&Value::Bool(true)),
        "capped, but `total` is presented without saying it is a floor — \
         IC-13. Response: {v}"
    );
}

/// Every row's id is unique, and every `NotYet` states a reason.
#[test]
fn probe_rows_are_well_formed() {
    use std::collections::BTreeSet;
    let ids: BTreeSet<&str> = PROBE_ROWS.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), PROBE_ROWS.len(), "duplicate probe-row id");
    for r in PROBE_ROWS {
        assert!(r.marker_path.starts_with("$."), "{}: bad path", r.id);
        if let super::cap_probe::Mutation::NotYet(why) = r.mutation {
            assert!(!why.trim().is_empty(), "{}: NotYet needs a reason", r.id);
        }
    }
}

/// Prints the per-site mutation tally on every run.
///
/// Asserts nothing about it — a test cannot verify that a human mutated a
/// production line. It PUBLISHES, so a green run still reports how much of
/// its own coverage is unproven.
#[test]
fn print_mutation_tally() {
    let killed = PROBE_ROWS
        .iter()
        .filter(|r| r.mutation == super::cap_probe::Mutation::Killed)
        .count();
    println!(
        "IC-13 probe rows: {} total, {} mutation-verified, {} not-yet",
        PROBE_ROWS.len(),
        killed,
        PROBE_ROWS.len() - killed
    );
}
```

- [ ] **Step 3: Run to verify the grep row fails or passes for the right reason**

Run: `cargo test --lib cap_probe -- --nocapture`
Expected: `probe_rows_are_well_formed` PASS; `print_mutation_tally` PASS and prints `4 total, 0 mutation-verified, 4 not-yet`; the grep row PASSES (its marker already works — it is a positive control). If it fails on the `overflow` key, the call was not capped: widen the pattern, do **not** relax the assertion.

- [ ] **Step 4: Add the row cross-check to the gate**

Append to `tests/result_caps.rs`:

```rust
const PROBE_TABLE: &str = "src/tools/core/cap_probe.rs";

/// Row ids declared in the probe table, read as TEXT.
///
/// Text, not a compiled import: rows behind `#[cfg(feature = "librarian")]`
/// vanish under `--no-default-features`, and a gate that could not see them
/// would demand their removal on the lean lane and their presence on the
/// default one.
fn probe_row_ids(src: &str) -> BTreeSet<String> {
    src.lines()
        .filter_map(|l| l.trim().strip_prefix("id: \""))
        .filter_map(|r| r.split('"').next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn probe_row_ids_parses_the_table_form() {
    let src = "        id: \"grep.total_bytes\",\n        marker_path: \"$.x\",\n";
    let got = probe_row_ids(src);
    assert_eq!(got.len(), 1);
    assert!(got.contains("grep.total_bytes"));
}

/// Both directions: no `RESULT_CAP` without a row, no row without a
/// `RESULT_CAP`.
///
/// The second direction is not symmetry for its own sake — it is what makes
/// a DELETED constant loud. Without it, removing a cap leaves an orphan row
/// asserting about a bound that no longer exists, which passes and reads as
/// coverage.
#[test]
fn result_caps_and_probe_rows_correspond_in_both_directions() {
    let mut declared = BTreeSet::new();
    for file in tracked_src_files() {
        let Ok(src) = std::fs::read_to_string(repo_root().join(&file)) else {
            continue;
        };
        for d in cap_constants(&src, &file) {
            if let CapClass::ResultCap(id) = classify(&d) {
                declared.insert(id);
            }
        }
    }
    let rows = probe_row_ids(
        &std::fs::read_to_string(repo_root().join(PROBE_TABLE))
            .expect("probe table must exist"),
    );

    let unprobed: Vec<_> = declared.difference(&rows).cloned().collect();
    let orphaned: Vec<_> = rows.difference(&declared).cloned().collect();

    assert!(
        unprobed.is_empty() && orphaned.is_empty(),
        "RESULT_CAP ids and probe rows disagree.\n\n\
         RESULT_CAP with no row ({}): {:?}\n  \
         → add a row in {PROBE_TABLE}, or re-classify the constant \
         NOT_A_CAP with a reason.\n\n\
         Row with no live RESULT_CAP ({}): {:?}\n  \
         → the constant was renamed or deleted; the row now asserts about a \
         bound that does not exist and would pass while proving nothing.",
        unprobed.len(),
        unprobed,
        orphaned.len(),
        orphaned
    );
}
```

- [ ] **Step 5: Reconcile until both directions are empty**

Run: `cargo test --test result_caps`
Every `RESULT_CAP` from Task 3 either gets a row or is re-classified with a reason. Rows added now, unmutated, carry `Mutation::NotYet("added Task 5; mutation pending")`.

- [ ] **Step 6: Run the full gate**

Run, `;`-chained: `cargo fmt ; cargo clippy --workspace --all-targets --features local-embed -- -D warnings ; cargo test --workspace --no-default-features ; cargo test --workspace`
Expected: four green. **Check the lean lane specifically** — it is the one that would catch a compiled-in row list.

- [ ] **Step 7: Commit**

```bash
git add src/tools/core/cap_probe.rs src/tools/core/cap_probe_tests.rs tests/result_caps.rs
```

Separate call: `git diff --cached --name-only`

```bash
git commit -m "test(caps): probe rows, three positive controls, both-directions check

Each row asserts a conjunction: the call was capped ESTABLISHED FROM THE
RESPONSE, and a marker arrived. Capped-ness from the response rather than
from the input is what stops a row going vacuous-but-passing when a cap
moves.

Three rows are positive controls whose markers already work. Without a row
that should pass, a probe reporting 'marker missing' everywhere is
indistinguishable from one that cannot read a marker at all.

The correspondence check runs BOTH directions. The second is what makes a
deleted constant loud: an orphan row asserts about a bound that no longer
exists, passes, and reads as coverage.

Ids cross-check by TEXT because librarian is a default feature — a compiled
row list would red on the lean lane and pass on the default one." -- src/tools/core/cap_probe.rs src/tools/core/cap_probe_tests.rs tests/result_caps.rs
```

---

### Task 6: Mutate the three positive controls, and publish the tally

**Files:**
- Modify: `src/tools/grep.rs`, `src/tools/run_command/output.rs`, `src/librarian/tools/link_scan/mod.rs` (each edit is **reverted** within the task)
- Modify: `src/tools/core/cap_probe.rs`
- Modify (via catalog): `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (`8a9dd5a27cd03480`)

**Interfaces:**
- Consumes: `PROBE_ROWS`, `Mutation` (Task 5).
- Produces: three rows at `Mutation::Killed`; the tally published at IC-13's read surface.

- [ ] **Step 1: Mutate `grep`'s marker and observe the red**

In `src/tools/grep.rs`, inside the `if hit_cap` block that sets it (near `:472`), comment out one line:

```rust
                if hit_cap {
                    // overflow["total_is_lower_bound"] = json!(true);
                }
```

Run: `cargo test --lib grep_past_its_byte_budget_marks_the_result_as_a_lower_bound`
Expected: **FAIL** with `capped, but 'total' is presented without saying it is a floor — IC-13`.

**If it passes, stop.** The row does not guard what it claims and the marker path is wrong. Fix the row before continuing; a green result here is the failure this step exists to find.

- [ ] **Step 2: Revert, and confirm green**

```bash
git checkout -- src/tools/grep.rs
```

Run the same test: PASS.

- [ ] **Step 3: Repeat for the other two positive controls**

Same three-step shape, one site at a time — `CLAUDE.md`: *mutate once per guarded SITE, not once per feature; one kill says nothing about the other N−1.*

- `run_command.inline_bytes` — suppress the `unfiltered_truncated` field where the response is built in `src/tools/run_command/output.rs`; expect the row red; `git checkout --` it.
- `link_scan.findings` — suppress `counts.truncated` in `src/librarian/tools/link_scan/mod.rs`; expect red; revert.

Confirm `git status --short` shows **none** of the three files modified before continuing.

- [ ] **Step 4: Promote the three rows to `Killed`**

```rust
    ProbeRow {
        id: "run_command.inline_bytes",
        marker_path: "$.unfiltered_truncated",
        mutation: Mutation::Killed,
    },
    ProbeRow {
        id: "grep.total_bytes",
        marker_path: "$.overflow.total_is_lower_bound",
        mutation: Mutation::Killed,
    },
    ProbeRow {
        id: "link_scan.findings",
        marker_path: "$.counts.truncated",
        mutation: Mutation::Killed,
    },
```

- [ ] **Step 5: Verify the tally moved**

Run: `cargo test --lib print_mutation_tally -- --nocapture`
Expected: `IC-13 probe rows: N total, 3 mutation-verified, N-3 not-yet`.

- [ ] **Step 6: Publish the tally at IC-13's read surface**

Through the catalog — `IC-13` declares `entry_prefix`, so a direct edit is refused, and a frontmatter edit would not reach the catalog (BL-48):

```
artifact(action="update", id="8a9dd5a27cd03480", patch={body_edits: [{
  heading: "IC-13 — a capped result is presented as complete, so a partial answer reads as the whole one",
  action: "edit",
  old_string: "**Mechanism status:** none yet, and the shape is now specific rather than gestural.",
  new_string: "**Mechanism status:** `shipped (detector)` — `tests/result_caps.rs`, two instruments with different scopes plus <N> probe rows, of which **3 are mutation-verified**. The tally is the honest part: a row that has not been mutated is an assertion whose ability to fail is unproven, and `cargo test --lib print_mutation_tally -- --nocapture` prints the live figure. Superseded: `none yet, and the shape is now specific rather than gestural.`"
}]})
```

Publishing here, not only in the test header: a bound living in the enforcement layer is published to an audience that never reads it (`OB-1`, `reconnaissance-patterns:R-170`).

- [ ] **Step 7: Run the full gate**

Run, `;`-chained: `cargo fmt ; cargo clippy --workspace --all-targets --features local-embed -- -D warnings ; cargo test --workspace --no-default-features ; cargo test --workspace`
Expected: four green.

- [ ] **Step 8: Commit**

```bash
git add src/tools/core/cap_probe.rs docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md
```

Separate call: `git diff --cached --name-only` — confirm **none** of the three mutated production files appear.

```bash
git commit -m "test(caps): three positive controls observed red, tally published

Each marker was deleted in production, its row watched fail, and the
deletion reverted — one site at a time, because one kill says nothing about
the other N-1. A row that had passed under its own mutation would have been
guarding nothing while reading as coverage.

The tally goes into IC-13's Mechanism status, not just the test header: a
bound published in the enforcement layer reaches an audience that never
reads it. A not-yet row is unproven, and the printed figure is what stops it
being credited." -- src/tools/core/cap_probe.rs docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md
```

---

### Task 7: Prove each gate red is reachable

**Files:**
- Modify: `tests/result_caps.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: five meta-tests, each proving one gate failure fires on a fixture.

- [ ] **Step 1: Write the exemption-narrowness tests**

```rust
/// The five reds this gate can produce, each on a fixture.
///
/// Following `missing_index_rows_exempts_only_unclassified`
/// (`tests/issue_clusters.rs:1412`): every exemption ships a test proving
/// it is NARROW. An unproven red is decoration — `CLAUDE.md`: loudness is a
/// property of a PATH, and an alarm nothing reaches is exactly as
/// informative as no alarm.
#[test]
fn an_unclassified_constant_is_reported_and_a_classified_one_is_not() {
    let src = "\
const BARE_MAX: usize = 1;
// cap-class: NOT_A_CAP — a stated reason
const FINE_LIMIT: usize = 2;
";
    let got = unclassified_decls(&cap_constants(src, "src/f.rs"));
    assert_eq!(got.len(), 1, "exactly one offender: {got:?}");
    assert!(got[0].contains("BARE_MAX"));
    assert!(
        !got[0].contains("FINE_LIMIT"),
        "the exemption must be narrow — a classified constant must not be \
         swept in, or the gate stops discriminating"
    );
}

#[test]
fn a_not_a_cap_without_a_reason_is_reported_distinctly_from_an_absent_one() {
    let src = "\
// cap-class: NOT_A_CAP
const TOKEN_ONLY_MAX: usize = 1;
";
    let got = unclassified_decls(&cap_constants(src, "src/f.rs"));
    assert_eq!(got.len(), 1);
    assert!(
        got[0].contains("NOT_A_CAP with no reason"),
        "distinct text from the absent case: the two need different fixes, \
         and one message for both sends readers to the wrong one. Got: {}",
        got[0]
    );
}

#[test]
fn a_truncation_site_is_found_where_no_constant_exists_at_all() {
    let src = "let first = chunk_markdown(body).next();\n";
    assert!(
        cap_constants(src, "src/f.rs").is_empty(),
        "instrument A sees nothing here — that is the point"
    );
    assert_eq!(
        truncation_sites(src, "src/f.rs").len(),
        1,
        "instrument B must see it. This is the indexer member's exact shape \
         (fixed at 488192e8) and the reason two instruments exist: A would \
         have reported full coverage of a population it could not see"
    );
}

#[test]
fn a_result_cap_with_no_row_and_a_row_with_no_result_cap_both_report() {
    let declared: BTreeSet<String> = ["a.one", "b.two"].iter().map(|s| (*s).into()).collect();
    let rows: BTreeSet<String> = ["b.two", "c.gone"].iter().map(|s| (*s).into()).collect();

    let unprobed: Vec<_> = declared.difference(&rows).cloned().collect();
    let orphaned: Vec<_> = rows.difference(&declared).cloned().collect();

    assert_eq!(unprobed, vec!["a.one".to_string()], "RESULT_CAP with no row");
    assert_eq!(orphaned, vec!["c.gone".to_string()], "row with no RESULT_CAP");
}

#[test]
fn the_gate_scans_src_and_would_miss_a_cap_class_written_in_tests() {
    // This very file contains `cap-class:` fixture strings above. If the
    // scan reached tests/, they would be counted as declarations.
    assert!(
        tracked_src_files().iter().all(|f| f.starts_with("src/")),
        "scanning tests/ would read this file's own teaching examples as \
         real classifications — the documentation-example-as-real-token \
         trap (CLAUDE.md § Parsers Over a Namespace)"
    );
}
```

- [ ] **Step 2: Run to verify they pass**

Run: `cargo test --test result_caps`
Expected: all PASS.

- [ ] **Step 3: Verify each real gate red is observable, not just fixture-proven**

`CLAUDE.md`: *mutate the PRODUCTION path, not the test's inputs — a second level asserting about its own re-implementation is indistinguishable from coverage until you break the thing that ships.* So drive each red through the **live** gate, one at a time, reverting between:

1. Delete one `cap-class:` line from a real `src/` file → `every_cap_constant_is_classified` reds naming that constant → `git checkout --` it.
2. Change one real `NOT_A_CAP — <reason>` to bare `NOT_A_CAP` → reds with the *no reason* text → revert.
3. Delete one row from `PROBE_ROWS` → `result_caps_and_probe_rows_correspond_in_both_directions` reds as *RESULT_CAP with no row* → revert.
4. Add `ProbeRow { id: "nope.nope", marker_path: "$.x", mutation: Mutation::NotYet("probe") }` → reds as *row with no live RESULT_CAP* → revert.

Confirm `git status --short` is clean of all four before continuing.

- [ ] **Step 4: Record the four observed reds in the module header**

```rust
//! ## Observed reds — 2026-09-02
//!
//! Each driven through the LIVE gate by mutating real source, not a
//! fixture, then reverted:
//!
//! 1. `cap-class:` line deleted from `<file:line>` → named that constant.
//! 2. `NOT_A_CAP — <reason>` reduced to the bare token → *no reason* text.
//! 3. Row deleted from `PROBE_ROWS` → *RESULT_CAP with no row*.
//! 4. Row added for a nonexistent id → *row with no live RESULT_CAP*.
//!
//! The fixture tests above prove the same four on synthetic input. Both
//! levels are kept deliberately: a fixture test asserting about its own
//! re-implementation is indistinguishable from coverage until the shipping
//! path is broken.
```

- [ ] **Step 5: Run the full gate**

Run, `;`-chained: `cargo fmt ; cargo clippy --workspace --all-targets --features local-embed -- -D warnings ; cargo test --workspace --no-default-features ; cargo test --workspace`
Expected: four green.

- [ ] **Step 6: Commit**

```bash
git add tests/result_caps.rs
```

Separate call: `git diff --cached --name-only`

```bash
git commit -m "test(caps): prove all five gate reds are reachable

Four driven through the LIVE gate by mutating real source and reverting, one
at a time; the fifth (scan excludes tests/) is structural. Both the fixture
level and the production level are kept: a fixture test asserting about its
own re-implementation is indistinguishable from coverage until the shipping
path is broken.

The NOT_A_CAP-without-a-reason red is deliberately distinct text from the
absent-annotation red — the two need different fixes, and one message for
both sends readers to the wrong one." -- tests/result_caps.rs
```

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: the invariant's conjunction → Task 5 Step 2; three-outcome distinction → Task 4 (driver) + Task 5; out-of-scope marker *accuracy* → stated in `cap_probe_tests.rs`'s header, asserted nowhere; inline classification → Tasks 1+3; tracked-files-only → Global Constraints + Task 1 Step 3; two instruments → Tasks 1+2, narrowness proven in Task 7 Step 1; placement table → Task 4; source-text-not-compiled → Task 5 Step 4; positive controls → Tasks 5+6; deferred `symbols` row → **not scheduled**, correctly: the spec defers it behind `e2e-rust` because a row without a warm rust-analyzer passes vacuously; deferred `indexer` → **not scheduled**, fixed at `488192e8`, and its shape is instead pinned as a test in Task 7 Step 1; mutation tally → Task 6; success criteria 1–2 → Task 7; 3–4 → Tasks 1,2,7; 5 → Task 3 Step 5; 6 → Task 6; 7 → every task's gate step.

**Placeholder scan.** No TBD/TODO. Every code step carries real code. Task 3 is data entry whose worklist the gate emits, with the decision rule and three worked examples given rather than "classify appropriately". `<N>`/`R`/`K` in Task 3 Step 5 and Task 6 Step 6 are measurements the executor substitutes — flagged as such, not hidden.

**Type consistency.** `CapDecl`, `CapClass`, `TruncSite`, `ProbeRow`, `Mutation` are defined once and used with the same field names throughout. `annotation_above` is shared by both instruments (one grammar). `cap_constants`/`truncation_sites` both take `(src, file)` and return `line` 1-indexed. `probe_row_ids` parses the exact `id: "..."` form the Task 5 table emits — pinned by `probe_row_ids_parses_the_table_form`, so a table reformat that breaks the parser reds rather than silently returning an empty set.

**One risk stated rather than designed away.** `probe_row_ids` is a text parser over Rust source, which makes it an `IC-6` candidate: a row id written across a line break, or via a macro, would be invisible and read as "no rows declared" — a silent zero in the *permissive* direction for `unprobed`. Its unit test pins the supported form only. The mitigation is the both-directions check: an invisible row still shows up as its `RESULT_CAP` being unprobed. Worth a follow-up bug file if the table ever grows a macro.
