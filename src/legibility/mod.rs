//! Legibility scan engine — ranks refactor candidates from usage.db friction
//! and the AST symbol index. Pure: (db conn, project root) -> ranked candidates.
//! Phase 2a of docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md.

use crate::lsp::symbols::{SymbolInfo, SymbolKind};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// A structural legibility defect kind — the entry gate. A candidate must have one.
/// (`NameCollision` retired 2026-06-13 — only language-agnostic, AST-measurable
/// defects remain; rationale in `docs/adrs/2026-06-13-drop-name-collision-defect.md`.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Defect {
    OverBudgetBody,
    UnMappableFile,
}

/// Tier 1 = biting now (has recorder friction); Tier 2 = latent (structural only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    BitingNow,
    Latent,
}

impl Tier {
    /// 1 for biting-now, 2 for latent — the spec's numeric tier.
    pub fn rank(self) -> u8 {
        match self {
            Tier::BitingNow => 1,
            Tier::Latent => 2,
        }
    }
}

/// Observed cost from the recorder, per target. `retries` is reserved (0 in v1 —
/// same-input-repeat detection is a follow-up).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Friction {
    pub truncations: u32,
    pub retries: u32,
    pub code_class_edit_fails: u32,
    pub other: u32,
    pub sessions: u32,
}

impl Friction {
    pub fn is_empty(&self) -> bool {
        self.truncations == 0
            && self.retries == 0
            && self.code_class_edit_fails == 0
            && self.other == 0
    }
    /// score = 3*truncations + 2*retries + 2*code_class_edit_fails + 1*other.
    /// Infra-class err_family never reaches `code_class_edit_fails` (excluded in the
    /// recorder query), so tool-class noise cannot inflate a code candidate.
    pub fn score(&self) -> u32 {
        3 * self.truncations + 2 * self.retries + 2 * self.code_class_edit_fails + self.other
    }
}

/// A raw structural finding from the index lane, before scoring/tiering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralDefect {
    pub rel_file: String,
    pub name_path: String, // "(file)" for UnMappableFile
    pub defect: Defect,
    pub tokens: usize,
    pub lines: u32,
}

/// A parsed source file: its rel path, its source lines, and its symbol tree.
pub struct FileSymbols {
    pub rel_file: String,
    pub lines: Vec<String>,
    pub symbols: Vec<SymbolInfo>,
}

/// A ranked refactor candidate — the engine's output unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub key: String, // "<rel_file>::<name_path>"
    pub rel_file: String,
    pub name_path: String,
    pub defect: Defect,
    pub tier: Tier,
    pub tokens: usize,
    pub budget: usize,
    pub lines: u32,
    pub friction: Friction,
    pub score: u32,
}

fn is_body_bearing(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor
    )
}

/// Recurse the symbol tree, collecting body-bearing symbols.
fn collect_bodies<'a>(syms: &'a [SymbolInfo], out: &mut Vec<&'a SymbolInfo>) {
    for s in syms {
        if is_body_bearing(&s.kind) {
            out.push(s);
        }
        collect_bodies(&s.children, out);
    }
}

/// The source text of a symbol's body, plus its line count. Empty when the range
/// is degenerate or out of bounds.
fn body_text(lines: &[String], sym: &SymbolInfo) -> (String, u32) {
    if lines.is_empty() {
        return (String::new(), 0);
    }
    let start = sym.range_start_line.unwrap_or(sym.start_line) as usize;
    let end = (sym.end_line as usize).min(lines.len() - 1);
    if start > end {
        return (String::new(), 0);
    }
    (lines[start..=end].join("\n"), (end - start + 1) as u32)
}

/// Index-lane detector: function/method bodies that exceed the inline budget
/// (so `symbols(include_body=true)` truncates them).
pub fn over_budget_bodies(files: &[FileSymbols]) -> Vec<StructuralDefect> {
    let mut out = Vec::new();
    for f in files {
        let mut bodies = Vec::new();
        collect_bodies(&f.symbols, &mut bodies);
        for sym in bodies {
            let (body, lines) = body_text(&f.lines, sym);
            if !body.is_empty() && crate::tools::exceeds_inline_limit(&body) {
                out.push(StructuralDefect {
                    rel_file: f.rel_file.clone(),
                    name_path: sym.name_path.clone(),
                    defect: Defect::OverBudgetBody,
                    tokens: body.len() / 4,
                    lines,
                });
            }
        }
    }
    out
}

/// Recurse the symbol tree, collecting ALL symbols (any kind).
fn collect_all<'a>(syms: &'a [SymbolInfo], out: &mut Vec<&'a SymbolInfo>) {
    for s in syms {
        out.push(s);
        collect_all(&s.children, out);
    }
}


/// Estimated byte size of a `symbols(path)` overview: ~one line per symbol,
/// dominated by the name_path + the optional detail (signature), plus a fixed
/// per-line overhead (kind label, line range, indentation).
fn overview_bytes(files_syms: &[&SymbolInfo]) -> usize {
    const PER_SYMBOL_OVERHEAD: usize = 24;
    files_syms
        .iter()
        .map(|s| PER_SYMBOL_OVERHEAD + s.name_path.len() + s.detail.as_deref().map_or(0, str::len))
        .sum()
}

/// Index-lane detector: a file whose `symbols(path)` overview would exceed the
/// inline budget (can't be mapped in one call). Driven by symbol count/size, NOT
/// line count — a cleanly-mapped long file is left alone (verified: longer files
/// comprehend better; the hazard is total context, not within-file length).
pub fn un_mappable_files(files: &[FileSymbols]) -> Vec<StructuralDefect> {
    let mut out = Vec::new();
    for f in files {
        let mut all = Vec::new();
        collect_all(&f.symbols, &mut all);
        let bytes = overview_bytes(&all);
        if bytes > crate::tools::MAX_INLINE_TOKENS * 4 {
            out.push(StructuralDefect {
                rel_file: f.rel_file.clone(),
                name_path: "(file)".to_string(),
                defect: Defect::UnMappableFile,
                tokens: bytes / 4,
                lines: f.lines.len() as u32,
            });
        }
    }
    out
}

/// Walk the project (gitignore-aware), parse every recognized source file's symbols.
pub fn parse_project(root: &Path) -> Vec<FileSymbols> {
    let mut out = Vec::new();
    for entry in ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .build()
        .flatten()
    {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let Some(lang) = crate::ast::detect_language(path) else {
            continue;
        };
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(symbols) =
            crate::ast::parser::extract_symbols_from_source(&source, Some(lang), path)
        else {
            continue; // unparseable / unsupported → skip, never fail the whole scan
        };
        let rel_file = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        let lines = source.lines().map(str::to_string).collect();
        out.push(FileSymbols {
            rel_file,
            lines,
            symbols,
        });
    }
    out
}

/// Run the structural detectors over the parsed project. (NameCollision retired
/// 2026-06-13 — see the `Defect` docs + ADR; only AST-measurable defects remain.)
pub fn index_lane(root: &Path) -> Vec<StructuralDefect> {
    let files = parse_project(root);
    let mut defects = over_budget_bodies(&files);
    defects.extend(un_mappable_files(&files));
    defects
}

/// `err_family` values that indicate a code/extractor-shape problem (count toward
/// the score). Infra families (lsp_disconnect, …) are tool-class and excluded.
const CODE_FAMILIES: &str = "'ast_extent_fail','ambiguous_name_path','replace_dropped_sibling'";
const INFRA_FAMILIES: &str =
    "'lsp_disconnect','lsp_index_locked','mux_startup_fail','lsp_not_running'";

/// Recorder lane: aggregate per-`friction_target` cost from usage.db, scoped to
/// this repo by `project_root` (the F-1 cross-project contamination fix).
pub fn recorder_lane(
    conn: &rusqlite::Connection,
    project_root: &str,
) -> rusqlite::Result<HashMap<String, Friction>> {
    let sql = format!(
        "SELECT friction_target,
                SUM(CASE WHEN overflowed = 1 THEN 1 ELSE 0 END),
                SUM(CASE WHEN err_family IN ({code}) THEN 1 ELSE 0 END),
                SUM(CASE WHEN outcome != 'success'
                          AND (err_family IS NULL OR err_family NOT IN ({code}, {infra}))
                         THEN 1 ELSE 0 END),
                COUNT(DISTINCT cc_session_id)
         FROM tool_calls
         WHERE project_root = ?1 AND friction_target IS NOT NULL AND friction_target != ''
         GROUP BY friction_target",
        code = CODE_FAMILIES,
        infra = INFRA_FAMILIES,
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([project_root], |r| {
        Ok((
            r.get::<_, String>(0)?,
            Friction {
                truncations: r.get::<_, i64>(1)? as u32,
                retries: 0,
                code_class_edit_fails: r.get::<_, i64>(2)? as u32,
                other: r.get::<_, i64>(3)? as u32,
                sessions: r.get::<_, i64>(4)? as u32,
            },
        ))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (k, fr) = row?;
        map.insert(k, fr);
    }
    Ok(map)
}

/// Combine structural defects with recorder friction → tiered, scored, ranked
/// candidates. A candidate's friction is matched by its name_path, falling back to
/// its rel_file (for un-mappable files, whose `friction_target` is the path).
pub fn score_and_rank(
    structural: Vec<StructuralDefect>,
    friction: &HashMap<String, Friction>,
) -> Vec<Candidate> {
    let mut cands: Vec<Candidate> = structural
        .into_iter()
        .map(|d| {
            let fr = friction
                .get(&d.name_path)
                .or_else(|| friction.get(&d.rel_file))
                .cloned()
                .unwrap_or_default();
            let tier = if fr.is_empty() {
                Tier::Latent
            } else {
                Tier::BitingNow
            };
            let score = fr.score();
            Candidate {
                key: format!("{}::{}", d.rel_file, d.name_path),
                rel_file: d.rel_file,
                name_path: d.name_path,
                defect: d.defect,
                tier,
                tokens: d.tokens,
                budget: crate::tools::MAX_INLINE_TOKENS,
                lines: d.lines,
                friction: fr,
                score,
            }
        })
        .collect();

    // Tier 1 before Tier 2; within tier 1 by score desc; ties and tier 2 by tokens
    // over budget (proxy: tokens) desc; final tie-break by key for determinism.
    cands.sort_by(|a, b| {
        a.tier
            .rank()
            .cmp(&b.tier.rank())
            .then(b.score.cmp(&a.score))
            .then(b.tokens.cmp(&a.tokens))
            .then(a.key.cmp(&b.key))
    });
    cands
}

/// The engine entry point: index lane + recorder lane → ranked candidates.
pub fn scan(
    conn: &rusqlite::Connection,
    root: &Path,
    project_root: &str,
) -> anyhow::Result<Vec<Candidate>> {
    let structural = index_lane(root);
    let friction = recorder_lane(conn, project_root)?;
    Ok(score_and_rank(structural, &friction))
}

/// Re-measure a single target's current cost (tokens, lines), independent of whether
/// it is still a defect. Used by Phase 2b to fill the `after` delta when a candidate
/// auto-closes (its defect is gone). For a symbol key, measures the body; for an
/// un-mappable file (`name_path == "(file)"`), measures the overview size.
pub fn measure_target(
    files: &[FileSymbols],
    rel_file: &str,
    name_path: &str,
) -> Option<(usize, u32)> {
    let f = files.iter().find(|f| f.rel_file == rel_file)?;
    if name_path == "(file)" {
        let mut all = Vec::new();
        collect_all(&f.symbols, &mut all);
        return Some((overview_bytes(&all) / 4, f.lines.len() as u32));
    }
    let mut all = Vec::new();
    collect_all(&f.symbols, &mut all);
    let sym = all.iter().find(|s| s.name_path == name_path)?;
    let (body, lines) = body_text(&f.lines, sym);
    Some((body.len() / 4, lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friction_score_and_emptiness() {
        let empty = Friction::default();
        assert!(empty.is_empty());
        assert_eq!(empty.score(), 0);

        let f = Friction {
            truncations: 14,
            retries: 0,
            code_class_edit_fails: 1,
            other: 2,
            sessions: 2,
        };
        assert!(!f.is_empty());
        // 3*14 + 2*0 + 2*1 + 1*2 = 46
        assert_eq!(f.score(), 46);
    }

    fn sym(name_path: &str, kind: SymbolKind, start: u32, end: u32) -> SymbolInfo {
        SymbolInfo {
            name: name_path
                .rsplit('/')
                .next()
                .unwrap_or(name_path)
                .to_string(),
            name_path: name_path.to_string(),
            kind,
            file: std::path::PathBuf::from("x.rs"),
            start_line: start,
            end_line: end,
            range_start_line: None,
            start_col: 0,
            children: vec![],
            detail: None,
        }
    }

    fn file_with(rel: &str, body_lines: usize, syms: Vec<SymbolInfo>) -> FileSymbols {
        // each line is 200 bytes so `body_lines` lines ≈ body_lines*200 bytes
        FileSymbols {
            rel_file: rel.to_string(),
            lines: (0..body_lines).map(|_| "x".repeat(200)).collect(),
            symbols: syms,
        }
    }

    #[test]
    fn over_budget_bodies_flags_only_over_budget_functions() {
        // big fn spans lines 0..=70 → ~70*201 ≈ 14k bytes > 10k budget
        let big = sym("Foo/big", SymbolKind::Method, 0, 70);
        // small fn spans lines 0..=5 → ~6*201 ≈ 1.2k bytes < budget
        let small = sym("Foo/small", SymbolKind::Method, 0, 5);
        let files = vec![file_with("src/foo.rs", 71, vec![big, small])];
        let defects = over_budget_bodies(&files);
        assert_eq!(defects.len(), 1, "only the big body");
        assert_eq!(defects[0].name_path, "Foo/big");
        assert_eq!(defects[0].defect, Defect::OverBudgetBody);
        assert!(defects[0].tokens > crate::tools::MAX_INLINE_TOKENS);
    }

    #[test]
    fn over_budget_ignores_non_body_kinds() {
        // a Struct spanning many lines is NOT a body-bearing symbol
        let s = sym("BigStruct", SymbolKind::Struct, 0, 70);
        let files = vec![file_with("src/foo.rs", 71, vec![s])];
        assert!(over_budget_bodies(&files).is_empty());
    }


    #[test]
    fn un_mappable_files_flags_overview_over_budget_not_line_count() {
        // Many symbols → estimated overview exceeds the budget.
        let many: Vec<SymbolInfo> = (0..400)
            .map(|i| {
                sym(
                    &format!("Mod/sym_{i:04}_with_a_longish_name"),
                    SymbolKind::Function,
                    i,
                    i,
                )
            })
            .collect();
        let big_map = file_with("src/huge.rs", 400, many);

        // A long file (1500 lines) with FEW symbols maps cleanly → NOT flagged.
        // (Encodes the verified longer-files-better finding: line count is not a trigger.)
        let long_clean = file_with(
            "src/long_clean.rs",
            1500,
            vec![
                sym("A/f", SymbolKind::Function, 0, 700),
                sym("A/g", SymbolKind::Function, 701, 1499),
            ],
        );

        let defects = un_mappable_files(&[big_map, long_clean]);
        assert_eq!(defects.len(), 1, "only the many-symbol file");
        assert_eq!(defects[0].rel_file, "src/huge.rs");
        assert_eq!(defects[0].name_path, "(file)");
        assert_eq!(defects[0].defect, Defect::UnMappableFile);
    }

    #[test]
    fn index_lane_finds_over_budget_body_in_real_file() {
        let dir = tempfile::tempdir().unwrap();
        // a Rust file with one huge function (each line padded so the body exceeds budget)
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(dir.path().join("huge.rs"), src).unwrap();

        let defects = index_lane(dir.path());
        assert!(
            defects
                .iter()
                .any(|d| d.defect == Defect::OverBudgetBody && d.name_path.contains("huge")),
            "expected an over-budget-body defect for `huge`, got: {defects:?}"
        );
    }


    #[test]
    fn index_lane_does_not_flag_name_collisions() {
        // Guard for docs/adrs/2026-06-13-drop-name-collision-defect.md: a file with two
        // same-name methods (the classic inherent + trait `fmt` collision) must produce
        // ZERO defects. name_collision was retired — its disambiguator is per-language
        // and the qualified symbol form already resolves the ambiguity, so flagging it
        // is per-language-incorrect (TypeScript declaration merging is benign, not a bug).
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("s.rs"),
            "struct S;\n\
             impl std::fmt::Debug for S {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        Ok(())\n    }\n}\n\
             impl std::fmt::Display for S {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        Ok(())\n    }\n}\n",
        )
        .unwrap();
        let defects = index_lane(tmp.path());
        assert!(
            defects.is_empty(),
            "a name collision must not be flagged after NameCollision was retired: {defects:?}"
        );
    }

    #[test]
    fn recorder_lane_aggregates_friction_and_filters_by_project_root() {
        use crate::usage::db::{open_db, write_record};
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let conn = open_db(dir.path()).unwrap();

        // 2 truncations on the same target, this repo
        for _ in 0..2 {
            write_record(
                &conn,
                "symbols",
                1,
                "success",
                true,
                None,
                "cs",
                None,
                "s1",
                None,
                None,
                Some("ccs1"),
                Some("Foo/bar"),
                Some(1000),
                None,
                Some("/repo"),
            )
            .unwrap();
        }
        // 1 code-class edit fail on the same target, this repo
        write_record(
            &conn,
            "edit_code",
            1,
            "error",
            false,
            Some("ambiguous name_path \"Foo/bar\" matches 2 symbols"),
            "cs",
            None,
            "s1",
            None,
            None,
            Some("ccs1"),
            Some("Foo/bar"),
            None,
            Some("ambiguous_name_path"),
            Some("/repo"),
        )
        .unwrap();
        // a FOREIGN-project row for the same target — must be excluded (F-1)
        write_record(
            &conn,
            "symbols",
            1,
            "success",
            true,
            None,
            "cs",
            None,
            "s9",
            None,
            None,
            Some("ccs9"),
            Some("Foo/bar"),
            Some(9999),
            None,
            Some("/other-repo"),
        )
        .unwrap();

        let map = recorder_lane(&conn, "/repo").unwrap();
        let fr = map.get("Foo/bar").expect("Foo/bar present");
        assert_eq!(
            fr.truncations, 2,
            "foreign-repo truncation must be excluded"
        );
        assert_eq!(fr.code_class_edit_fails, 1);
        assert_eq!(fr.sessions, 1);
        assert_eq!(fr.score(), 3 * 2 + 2 * 1); // 8
        assert!(!map.contains_key(""), "empty friction_target excluded");
    }

    #[test]
    fn score_and_rank_tiers_and_orders() {
        let structural = vec![
            // biting-now: has friction
            StructuralDefect {
                rel_file: "src/a.rs".into(),
                name_path: "A/hot".into(),
                defect: Defect::OverBudgetBody,
                tokens: 4000,
                lines: 242,
            },
            // latent: no friction, bigger body
            StructuralDefect {
                rel_file: "src/b.rs".into(),
                name_path: "B/cold".into(),
                defect: Defect::OverBudgetBody,
                tokens: 6000,
                lines: 331,
            },
        ];
        let mut friction = HashMap::new();
        friction.insert(
            "A/hot".to_string(),
            Friction {
                truncations: 5,
                ..Default::default()
            },
        );

        let ranked = score_and_rank(structural, &friction);
        assert_eq!(ranked.len(), 2);
        // biting-now (A/hot) ranks above latent (B/cold) despite smaller body
        assert_eq!(ranked[0].name_path, "A/hot");
        assert_eq!(ranked[0].tier, Tier::BitingNow);
        assert_eq!(ranked[0].key, "src/a.rs::A/hot");
        assert_eq!(ranked[0].score, 15); // 3*5
        assert_eq!(ranked[1].name_path, "B/cold");
        assert_eq!(ranked[1].tier, Tier::Latent);
    }

    #[test]
    fn scan_end_to_end_ranks_a_real_over_budget_body() {
        use crate::usage::db::{open_db, write_record};
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        // a real over-budget function in a source file
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(dir.path().join("huge.rs"), src).unwrap();

        let conn = open_db(dir.path()).unwrap();
        // friction targeting the function's name_path (whatever the extractor calls it: "huge")
        write_record(
            &conn,
            "symbols",
            1,
            "success",
            true,
            None,
            "cs",
            None,
            "s1",
            None,
            None,
            Some("ccs1"),
            Some("huge"),
            Some(3500),
            None,
            Some(&dir.path().to_string_lossy()),
        )
        .unwrap();

        let cands = scan(&conn, dir.path(), &dir.path().to_string_lossy()).unwrap();
        assert!(
            cands
                .iter()
                .any(|c| c.defect == Defect::OverBudgetBody && c.name_path.contains("huge")),
            "expected ranked over-budget candidate for huge: {cands:?}"
        );
    }

    #[test]
    fn measure_target_returns_body_size_for_a_symbol() {
        let big = sym("Foo/big", SymbolKind::Method, 0, 70);
        let files = vec![file_with("src/foo.rs", 71, vec![big])];
        let (tokens, lines) = measure_target(&files, "src/foo.rs", "Foo/big").unwrap();
        assert!(tokens > crate::tools::MAX_INLINE_TOKENS);
        assert_eq!(lines, 71);
        assert!(measure_target(&files, "src/foo.rs", "Foo/missing").is_none());
    }
}
