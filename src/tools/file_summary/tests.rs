use super::*;

#[test]
fn detect_rust_as_source() {
    assert!(matches!(
        detect_file_type("src/main.rs"),
        FileSummaryType::Source
    ));
    assert!(matches!(
        detect_file_type("lib.py"),
        FileSummaryType::Source
    ));
}

#[test]
fn detect_md_as_markdown() {
    assert!(matches!(
        detect_file_type("README.md"),
        FileSummaryType::Markdown
    ));
    assert!(matches!(
        detect_file_type("docs/guide.mdx"),
        FileSummaryType::Markdown
    ));
}

#[test]
fn detect_json_as_json() {
    assert!(matches!(
        detect_file_type("data.json"),
        FileSummaryType::Json
    ));
    assert!(matches!(
        detect_file_type("package.json"),
        FileSummaryType::Json
    ));
}

#[test]
fn detect_yaml_as_yaml() {
    assert!(matches!(
        detect_file_type("config.yaml"),
        FileSummaryType::Yaml
    ));
    assert!(matches!(
        detect_file_type("docker-compose.yml"),
        FileSummaryType::Yaml
    ));
}

#[test]
fn detect_toml_as_toml() {
    assert!(matches!(
        detect_file_type("Cargo.toml"),
        FileSummaryType::Toml
    ));
    assert!(matches!(
        detect_file_type("pyproject.toml"),
        FileSummaryType::Toml
    ));
}

#[test]
fn detect_other_config_still_works() {
    // .xml, .ini, .env, .lock, .cfg stay as Config
    assert!(matches!(
        detect_file_type("web.xml"),
        FileSummaryType::Config
    ));
    assert!(matches!(detect_file_type(".env"), FileSummaryType::Config));
    assert!(matches!(
        detect_file_type("Cargo.lock"),
        FileSummaryType::Config
    ));
}

#[test]
fn detect_unknown_as_generic() {
    assert!(matches!(
        detect_file_type("data.csv"),
        FileSummaryType::Generic
    ));
    assert!(matches!(
        detect_file_type("Makefile"),
        FileSummaryType::Generic
    ));
}

#[test]
fn markdown_summary_basic_structure() {
    let content = "# Title\nsome text\n## Section\nmore text\n### Sub\nnope";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    // Now includes H3
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Title");
    assert_eq!(headings[0]["level"].as_u64().unwrap(), 1);
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Section");
    assert_eq!(headings[2]["heading"].as_str().unwrap(), "### Sub");
    assert_eq!(s["line_count"].as_u64().unwrap(), 6);
}

#[test]
fn markdown_summary_includes_line_ranges() {
    let content = "# Title\ntext\n## Section A\nmore text\nstill more\n## Section B\nfinal text";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Title");
    assert_eq!(headings[0]["level"].as_u64().unwrap(), 1);
    assert_eq!(headings[0]["line"].as_u64().unwrap(), 1);
    assert_eq!(headings[0]["end_line"].as_u64().unwrap(), 7); // H1 covers everything
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Section A");
    assert_eq!(headings[1]["line"].as_u64().unwrap(), 3);
    assert_eq!(headings[1]["end_line"].as_u64().unwrap(), 5);
    assert_eq!(headings[2]["line"].as_u64().unwrap(), 6);
    assert_eq!(headings[2]["end_line"].as_u64().unwrap(), 7);
}

#[test]
fn markdown_summary_includes_h3_headings() {
    let content = "# Top\n## Mid\n### Deep\ntext\n## Other";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 4);
    assert_eq!(headings[2]["heading"].as_str().unwrap(), "### Deep");
    assert_eq!(headings[2]["level"].as_u64().unwrap(), 3);
}

#[test]
fn markdown_summary_ignores_headings_in_code_blocks() {
    let content = "# Real\n```\n# Not a heading\n## Also not\n```\n## Real Too";
    let s = summarize_markdown(content);
    let headings = s["headings"].as_array().unwrap();
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0]["heading"].as_str().unwrap(), "# Real");
    assert_eq!(headings[1]["heading"].as_str().unwrap(), "## Real Too");
}

#[test]
fn config_summary_returns_first_30_lines() {
    let content: String = (1..=50).map(|i| format!("key_{} = {}\n", i, i)).collect();
    let s = summarize_config(&content);
    let preview = s["preview"].as_str().unwrap();
    assert!(preview.contains("key_1"));
    assert!(!preview.contains("key_31"));
    assert!(
        preview.contains("key_30"),
        "preview should include up to line 30"
    );
    assert_eq!(s["line_count"].as_u64().unwrap(), 50);
}

#[test]
fn generic_summary_includes_head_and_tail() {
    let content: String = (1..=100).map(|i| format!("line {}\n", i)).collect();
    let s = summarize_generic_file(&content);
    assert!(s["head"].as_str().unwrap().contains("line 1"));
    assert!(!s["head"].as_str().unwrap().contains("line 21"));
    assert!(
        s["head"].as_str().unwrap().contains("line 20"),
        "head should include line 20"
    );
    assert!(s["tail"].as_str().unwrap().contains("line 100"));
    assert!(
        !s["tail"].as_str().unwrap().contains("line 90"),
        "tail should not include line 90"
    );
    assert!(
        s["tail"].as_str().unwrap().contains("line 91"),
        "tail should start at line 91"
    );
    assert_eq!(s["line_count"].as_u64().unwrap(), 100);
}

#[test]
fn json_summary_shows_top_level_keys() {
    let content = r#"{
  "name": "my-project",
  "version": "1.0.0",
  "dependencies": {
"serde": "1.0",
"tokio": "1.0"
  },
  "scripts": {
"build": "cargo build"
  }
}"#;
    let s = summarize_json(content);
    assert_eq!(s["type"].as_str().unwrap(), "json");
    let schema = &s["schema"];
    assert_eq!(schema["root_type"].as_str().unwrap(), "object");
    let keys = schema["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 4);
    assert_eq!(keys[0]["path"].as_str().unwrap(), "$.name");
    assert_eq!(keys[0]["type"].as_str().unwrap(), "string");
    assert_eq!(keys[2]["path"].as_str().unwrap(), "$.dependencies");
    assert_eq!(keys[2]["type"].as_str().unwrap(), "object");
    assert_eq!(keys[2]["count"].as_u64().unwrap(), 2);
}

#[test]
fn json_summary_handles_root_array() {
    let content = r#"[{"id": 1}, {"id": 2}, {"id": 3}]"#;
    let s = summarize_json(content);
    let schema = &s["schema"];
    assert_eq!(schema["root_type"].as_str().unwrap(), "array");
    assert_eq!(schema["count"].as_u64().unwrap(), 3);
    assert_eq!(schema["element_type"].as_str().unwrap(), "object");
}

#[test]
fn json_summary_handles_malformed_json() {
    let content = "{ not valid json !!";
    let s = summarize_json(content);
    assert_eq!(s["type"].as_str().unwrap(), "json");
    assert!(s["head"].is_string()); // generic fallback shape
}

#[test]
fn toml_summary_shows_tables() {
    let content = "[package]\nname = \"foo\"\nversion = \"1.0\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = \"1.0\"\n\n[dev-dependencies]\ntempfile = \"3\"";
    let s = summarize_toml(content);
    assert_eq!(s["type"].as_str().unwrap(), "toml");
    assert_eq!(s["format"].as_str().unwrap(), "toml");
    let sections = s["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0]["key"].as_str().unwrap(), "[package]");
    assert!(sections[0]["line"].as_u64().unwrap() >= 1);
    assert!(sections[0]["end_line"].as_u64().is_some());
}

#[test]
fn toml_summary_handles_nested_tables() {
    let content = "[package]\nname = \"foo\"\n\n[profile.release]\nopt-level = 3\nlto = true";
    let s = summarize_toml(content);
    let sections = s["sections"].as_array().unwrap();
    assert!(sections
        .iter()
        .any(|s| s["key"].as_str().unwrap() == "[profile.release]"));
}

#[test]
fn markdown_summary_signals_truncated_headings() {
    // Silent-cap regression: >30 headings are capped, so the summary must
    // report the true total. docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md
    let content: String = (0..40)
        .map(|i| format!("## H{i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let s = summarize_markdown(&content);
    assert_eq!(s["headings"].as_array().unwrap().len(), 30, "capped at 30");
    assert_eq!(s["total_headings"].as_u64().unwrap(), 40);
    assert_eq!(s["headings_truncated"], serde_json::json!(true));
}

#[test]
fn markdown_summary_no_truncation_flag_when_under_cap() {
    let content = "# A\n## B\n### C";
    let s = summarize_markdown(content);
    assert!(s.get("headings_truncated").is_none());
    assert!(s.get("total_headings").is_none());
}
// ---- bound_summary: the byte bound on a whole-file summary ----
//
// Every guarded site of `bound_summary` / `cut_array_middle` has a case here that ONLY it can
// refuse (the other bounds admit the input), so a mutation of one site cannot hide behind
// another.

/// The budget the unit tests below exercise. `bound_summary` takes it as a parameter; these
/// tests pin the allocation arithmetic at one fixed value.
// cap-class: NOT_A_CAP — a test fixture value passed to bound_summary; it shapes no result
const SUMMARY_BYTE_BUDGET: usize = 6_000;

/// `bound_summary` at the test budget, so each test reads as "this input, this outcome".
fn bound_summary(s: serde_json::Value, file_id: &str) -> (serde_json::Value, Vec<String>) {
    super::bound_summary(s, file_id, SUMMARY_BYTE_BUDGET)
}

fn ser_len(v: &serde_json::Value) -> usize {
    v.to_string().len()
}

#[test]
fn bound_summary_leaves_a_summary_at_the_budget_untouched() {
    // Exactly at the budget is returned as built; one byte over is cut. Pins `<=` against `<`.
    let overhead = ser_len(&serde_json::json!({"type": "generic", "head": ""}));
    let at = serde_json::json!({
        "type": "generic",
        "head": "x".repeat(SUMMARY_BYTE_BUDGET - overhead),
    });
    assert_eq!(
        ser_len(&at),
        SUMMARY_BYTE_BUDGET,
        "fixture must sit on the budget"
    );
    let (kept, notes) = bound_summary(at.clone(), "@file_t");
    assert_eq!(kept, at);
    assert!(notes.is_empty());

    let over = serde_json::json!({
        "type": "generic",
        "head": "x".repeat(SUMMARY_BYTE_BUDGET - overhead + 1),
    });
    let (cut, _) = bound_summary(over, "@file_t");
    assert!(cut["head"].as_str().unwrap().contains("bytes shown"));
}

#[test]
fn bound_summary_cuts_a_string_to_the_room_the_fixed_part_leaves() {
    // The nested part (untouched: it is neither a top-level string nor an array) costs ~5,000 B,
    // so only ~1,000 B of the 6,000 B budget is left for `head`. The string takes exactly what
    // is left, keeping its start, and the whole still fits.
    let s = serde_json::json!({
        "type": "generic",
        "meta": {"pad": "p".repeat(5_000)},
        "head": "h".repeat(10_000),
    });
    let (cut, notes) = bound_summary(s, "@file_t");
    let head = cut["head"].as_str().unwrap();
    assert!(
        head.starts_with(&"h".repeat(100)),
        "the head of the field was lost: {head:.120}"
    );
    assert!(head.contains("bytes shown"));
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
    assert!(notes.is_empty(), "no array was cut: {notes:?}");
    assert!(cut.get("summary_omitted").is_none());
}
#[test]
fn bound_summary_fits_the_budget_whatever_json_escaping_does_to_the_text() {
    // A raw byte serializes to 2 bytes for `"`, `\` and tab and to 6 for `\x01` and the ESC of
    // ANSI colour codes. A cut sized in RAW bytes left each of these over the budget (measured:
    // 11.6 KB for the 2-byte kinds, 33.8 KB for `\x01`) and the response was buffered a second
    // time. The input is `head`/`tail` of 6,000 raw bytes each: far over the budget.
    for ch in ['"', '\\', '\t', '\n', '\u{1}', '\u{1b}'] {
        let s = serde_json::json!({
            "type": "generic",
            "line_count": 12,
            "head": ch.to_string().repeat(6_000),
            "tail": ch.to_string().repeat(6_000),
        });
        let (cut, _) = bound_summary(s, "@file_t");
        assert!(
            ser_len(&cut) <= SUMMARY_BYTE_BUDGET,
            "{ch:?}: serialized {} B is over the {SUMMARY_BYTE_BUDGET} B budget",
            ser_len(&cut)
        );
        assert!(
            cut["head"].as_str().unwrap().contains("bytes shown"),
            "{ch:?}: head was not cut"
        );
        assert!(
            cut.get("summary_omitted").is_none(),
            "{ch:?}: dropped whole"
        );
    }
}

#[test]
fn bound_summary_gives_a_heavier_escaping_string_less_raw_room_than_a_light_one() {
    // Same budget, same raw length: the `\x01` string costs 6x per byte, so it must keep fewer
    // raw bytes than the plain one beside it, or the sum cannot fit. One common raw share for
    // both would not do that; the common share is the largest that fits, so the heavy string
    // is cut to it and the plain one, below it, may stay whole.
    let s = serde_json::json!({
        "type": "generic",
        "head": "x".repeat(600),
        "tail": "\u{1}".repeat(8_000),
    });
    let (cut, _) = bound_summary(s, "@file_t");
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
    assert_eq!(
        cut["head"].as_str().unwrap(),
        "x".repeat(600),
        "the plain string fit and must be left whole"
    );
    assert!(cut["tail"].as_str().unwrap().contains("bytes shown"));
}

#[test]
fn bound_summary_last_resort_drops_what_it_cannot_cut_and_says_so() {
    // A wide string NESTED two levels down is neither a top-level string nor an array, so
    // neither pass can reach it. The result must still fit, and say it was dropped.
    let s = serde_json::json!({
        "type": "json",
        "format": "json",
        "line_count": 7,
        "schema": {"deep": {"blob": "z".repeat(20_000)}},
    });
    let (cut, notes) = bound_summary(s, "@file_t");
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
    assert_eq!(cut["summary_omitted"], true);
    assert_eq!(cut["type"], "json", "what the file is must survive");
    assert_eq!(cut["format"], "json");
    assert_eq!(cut["line_count"], 7);
    assert!(cut.get("schema").is_none());
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].starts_with("summary: ") && notes[0].contains("omitted entirely"),
        "{}",
        notes[0]
    );
    assert!(
        notes[0].contains("read_file(path=\"@file_t\""),
        "{}",
        notes[0]
    );
}

#[test]
fn bound_summary_last_resort_also_catches_a_budget_the_fixed_part_cannot_meet() {
    // The summary has no string and no array to cut and is over budget by itself.
    let s = serde_json::json!({"type": "x", "meta": {"pad": "p".repeat(7_000)}});
    let (cut, _) = bound_summary(s, "@file_t");
    assert_eq!(cut["summary_omitted"], true);
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET);
}
// ---- fit_envelope: the budget comes from the MEASURED envelope ----

/// A caller's `finish`, as `read_full_file` has one: keys added around the summary, and a hint
/// that carries the cut notes. `pad` stands for whatever else the envelope holds.
fn envelope_with(pad: usize) -> impl Fn(serde_json::Value, &[String]) -> serde_json::Value {
    move |summary, notes| {
        serde_json::json!({
            "s": summary,
            "hint": notes.join(" "),
            "pad": "p".repeat(pad),
        })
    }
}

#[test]
fn fit_envelope_returns_an_envelope_that_fits_exactly_as_the_caller_built_it() {
    // ~9 KB of summary: over the OLD fixed 6,000 B budget, under the limit once wrapped.
    let summary = serde_json::json!({"type": "source", "symbols": numbered_entries(200)});
    let finish = envelope_with(100);
    let whole = finish(summary.clone(), &[]);
    assert!(ser_len(&summary) > 6_000, "{} B", ser_len(&summary));
    assert!(!crate::tools::exceeds_inline_limit(&whole.to_string()));

    let got = fit_envelope(summary, "@file_t", &finish);

    assert_eq!(got, whole, "an envelope that fits must not change at all");
}

#[test]
fn fit_envelope_cuts_only_the_excess_not_down_to_a_fixed_budget() {
    // The envelope is over the limit by a few hundred bytes. Cutting down to the 9,000 B target
    // costs about 1.3 KB, ~30 entries: the old fixed 6,000 B budget cost about 3x that.
    let summary = serde_json::json!({"type": "source", "symbols": numbered_entries(200)});
    let s = ser_len(&summary);
    let pad = 10_100usize.saturating_sub(s + 60);
    let finish = envelope_with(pad);
    let whole = finish(summary.clone(), &[]);
    assert!(
        crate::tools::exceeds_inline_limit(&whole.to_string()),
        "precondition: the envelope must be over the limit, got {} B",
        ser_len(&whole)
    );

    let got = fit_envelope(summary, "@file_t", &finish);

    assert!(
        ser_len(&got) <= SUMMARY_ENVELOPE_BUDGET,
        "{} B over the {SUMMARY_ENVELOPE_BUDGET} B target",
        ser_len(&got)
    );
    let kept = got["s"]["symbols"].as_array().unwrap().len();
    assert!(kept < 200, "nothing was cut");
    assert!(
        kept >= 200 - 45,
        "cut far more than the excess: {kept} of 200 kept"
    );
}

#[test]
fn fit_envelope_prices_the_cut_notes_that_land_in_the_hint() {
    // The notes a cut produces are put in the hint by `finish`, which lengthens the envelope
    // AFTER the cut was sized. The first pass overshoots by the notes' length; the loop must
    // shrink the budget by exactly that and cut again from the ORIGINAL.
    let summary = serde_json::json!({"type": "source", "symbols": numbered_entries(400)});
    let got = fit_envelope(summary, "@file_t", envelope_with(400));

    assert!(
        ser_len(&got) <= SUMMARY_ENVELOPE_BUDGET,
        "{} B",
        ser_len(&got)
    );
    assert!(
        got["hint"].as_str().unwrap().contains("entries omitted"),
        "{}",
        got["hint"]
    );
    assert!(got["s"]["symbols_truncated"] == true);
}

#[test]
fn fit_envelope_falls_back_to_the_minimal_summary_when_cutting_cannot_converge() {
    // A `finish` whose cost does not shrink with the summary (a 20 KB key present whenever a
    // note is): no budget makes this fit. After the retries the result is the MINIMAL summary
    // and says so, not the last half-cut attempt.
    let summary = serde_json::json!({
        "type": "source", "line_count": 400, "symbols": numbered_entries(400),
    });
    let finish = |s: serde_json::Value, notes: &[String]| serde_json::json!({"s": s, "pad": "p".repeat(if notes.is_empty() { 0 } else { 20_000 })});
    let got = fit_envelope(summary, "@file_t", finish);
    assert_eq!(got["s"]["summary_omitted"], true, "{:.300}", got["s"]);
    assert_eq!(got["s"]["line_count"], 400);
    assert!(got["s"].get("symbols").is_none());
}
// ---- the allocation ARITHMETIC, pinned by exact kept counts ----
//
// The earlier tests assert "fits" and "starts with / ends with", which a one-entry rounding
// error satisfies. Each test below uses entries that serialize to ONE byte (`7`, cost 2 with the
// comma) so the count is a closed-form function of the allowance, and asserts that count exactly.

fn sevens(n: usize) -> Vec<serde_json::Value> {
    vec![serde_json::json!(7); n]
}

/// Entries kept (head + tail) and the entries omitted, from `cut_array_middle` on `n` sevens
/// with `allowance` bytes; `None` when nothing was cut.
fn kept_and_omitted(n: usize, allowance: usize) -> Option<(usize, usize)> {
    let mut summary = serde_json::json!({"type": "x", "a": []});
    cut_array_middle(&mut summary, "", "a", sevens(n), allowance, "@file_t")?;
    let kept = summary["a"].as_array().unwrap().len();
    Some((
        kept,
        summary["a_omitted"]["count"].as_u64().unwrap() as usize,
    ))
}

#[test]
fn cut_array_middle_rounds_each_half_down_not_up() {
    // Allowance 39 (odd): half = 19 floor, so each side takes 9 entries (18 B); a half rounded
    // UP is 20, which takes 10 per side. Odd and even array lengths give the same count.
    for n in [50, 51] {
        assert_eq!(kept_and_omitted(n, 39), Some((18, n - 18)), "n={n}");
    }
    // The even allowance next to it, as the anchor: 40 -> half 20 -> 10 per side.
    for n in [50, 51] {
        assert_eq!(kept_and_omitted(n, 40), Some((20, n - 20)), "n={n}");
    }
}

#[test]
fn cut_array_middle_prices_the_comma_between_entries() {
    // Each `7` costs 2 B: itself and the comma that joins it. Unpriced it costs 1 and twice as
    // many fit. Allowance 40 -> half 20 -> 10 per side priced, 20 per side unpriced.
    for n in [100, 101] {
        assert_eq!(kept_and_omitted(n, 40), Some((20, n - 20)), "n={n}");
    }
}

#[test]
fn cut_array_middle_cuts_at_exactly_one_more_entry_than_fits_and_not_before() {
    // Allowance 36 -> half 18 -> 9 per side. 18 entries are ALL kept: nothing to omit, so
    // nothing is cut and nothing is marked. A 19th makes one entry omitted. This pins the
    // `head + tail >= total` boundary (`>` would mark a cut with `count: 0`).
    let mut whole = serde_json::json!({"type": "x", "a": []});
    assert!(cut_array_middle(&mut whole, "", "a", sevens(18), 36, "@file_t").is_none());
    assert_eq!(
        whole["a"].as_array().unwrap().len(),
        18,
        "an array that fit was cut"
    );
    assert!(whole.get("a_truncated").is_none() && whole.get("a_omitted").is_none());

    assert_eq!(kept_and_omitted(19, 36), Some((18, 1)));
    assert_eq!(kept_and_omitted(20, 36), Some((18, 2)));
}
// ---- a line route is offered only when the entries are in line order ----
//
// `summarize_toml` lists flat keys through `toml::Table`, which is ALPHABETICAL, not by line,
// while `cut_array_middle` read the gap as "the line after the last kept head entry up to the line
// before the first kept tail entry", which holds only when lines ascend. With keys written
// z..a the gap came out `from_line: 15, to_line: 4` and the hint's ready-made
// `read_file(start_line=15, end_line=4)` failed with "invalid line range".

fn keyed_lines(lines: &[u64]) -> Vec<serde_json::Value> {
    lines
        .iter()
        .map(|l| serde_json::json!({"key": format!("{}{l:03}", "k".repeat(30)), "line": l}))
        .collect()
}

fn cut_keys(lines: &[u64], allowance: usize) -> (serde_json::Value, String) {
    let mut summary = serde_json::json!({"type": "toml", "keys": []});
    let note = cut_array_middle(
        &mut summary,
        "",
        "keys",
        keyed_lines(lines),
        allowance,
        "@file_t",
    )
    .expect("the array must be cut");
    (summary, note)
}

#[test]
fn cut_array_middle_gives_no_line_route_when_the_lines_run_backwards() {
    let lines: Vec<u64> = (1..=60).rev().collect();
    let (cut, note) = cut_keys(&lines, 600);
    assert!(
        cut["keys_omitted"]["from_line"].is_null(),
        "{}",
        cut["keys_omitted"]
    );
    assert!(
        cut["keys_omitted"]["to_line"].is_null(),
        "{}",
        cut["keys_omitted"]
    );
    assert!(note.contains("read the file in ranges with"), "{note}");
    assert!(
        note.contains("start_line=N, end_line=M"),
        "no ready-made call may carry made-up numbers: {note}"
    );
    assert!(cut["keys_omitted"]["count"].as_u64().unwrap() > 0);
}

#[test]
fn cut_array_middle_gives_no_line_route_when_one_omitted_entry_is_out_of_order() {
    // Ascending everywhere the kept entries are, but one line in the OMITTED middle breaks the
    // order: the gap's end points would be wrong for the omitted region, so no route is offered.
    let mut lines: Vec<u64> = (1..=60).collect();
    lines.swap(28, 31);
    let (cut, note) = cut_keys(&lines, 600);
    let after = cut["keys_omitted"]["after"].as_u64().unwrap() as usize;
    assert!(
        after < 28,
        "the swapped entries must be in the omitted region: after={after}"
    );
    assert!(
        cut["keys_omitted"]["from_line"].is_null(),
        "{}",
        cut["keys_omitted"]
    );
    assert!(note.contains("start_line=N"), "{note}");
}

#[test]
fn cut_array_middle_gives_no_line_route_when_an_entry_has_no_line() {
    let mut entries = keyed_lines(&(1..=60).collect::<Vec<_>>());
    entries[10] = serde_json::json!({"key": "k".repeat(33)});
    let mut summary = serde_json::json!({"type": "toml", "keys": []});
    let note = cut_array_middle(&mut summary, "", "keys", entries, 600, "@file_t").unwrap();
    assert!(summary["keys_omitted"]["from_line"].is_null());
    assert!(note.contains("start_line=N"), "{note}");
}

#[test]
fn cut_array_middle_still_gives_the_exact_route_when_the_lines_ascend() {
    // The control: lines ascend, so the route is the exact gap, and a line of 0 (a key the
    // summarizer could not find) or a repeat would not be "ascending".
    let lines: Vec<u64> = (1..=60).collect();
    let (cut, note) = cut_keys(&lines, 600);
    let after = cut["keys_omitted"]["after"].as_u64().unwrap();
    let count = cut["keys_omitted"]["count"].as_u64().unwrap();
    assert_eq!(cut["keys_omitted"]["from_line"], after + 1);
    assert_eq!(cut["keys_omitted"]["to_line"], after + count);
    assert!(
        note.contains(&format!(
            "start_line={}, end_line={}",
            after + 1,
            after + count
        )),
        "{note}"
    );
    // Equal neighbours are not strictly ascending: the gap would be ambiguous.
    let (dup, _) = cut_keys(
        &[1, 2, 3, 3]
            .repeat(15)
            .iter()
            .copied()
            .enumerate()
            .map(|(i, l)| l + (i as u64 / 4) * 3)
            .collect::<Vec<_>>(),
        600,
    );
    assert!(
        dup["keys_omitted"]["from_line"].is_null(),
        "{}",
        dup["keys_omitted"]
    );
}

#[test]
fn the_other_key_sources_list_their_entries_in_line_order() {
    // The assumption `cut_array_middle` makes, checked at every source of `line`d entries rather
    // than only the one that broke: YAML top-level keys and TOML table headers are found by
    // scanning the file top to bottom, whatever order the names sort in.
    let yaml: String = (0..40).rev().map(|i| format!("key{i:02}: 1\n")).collect();
    let sections = summarize_yaml(&yaml);
    let lines: Vec<u64> = sections["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["line"].as_u64().unwrap())
        .collect();
    assert!(
        lines.windows(2).all(|w| w[0] < w[1]),
        "yaml sections: {lines:?}"
    );

    let toml: String = (0..40)
        .rev()
        .map(|i| format!("[t{i:02}]\nv = 1\n"))
        .collect();
    let sections = summarize_toml(&toml);
    let lines: Vec<u64> = sections["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["line"].as_u64().unwrap())
        .collect();
    assert!(
        lines.windows(2).all(|w| w[0] < w[1]),
        "toml tables: {lines:?}"
    );

    // The one that is NOT: flat TOML keys come from `toml::Table`, alphabetical. Pinned here so
    // the guard in `cut_array_middle` is known to be load-bearing, not hypothetical.
    let flat: String = (0..30).rev().map(|i| format!("key{i:02} = 1\n")).collect();
    let keys = summarize_toml(&flat);
    let lines: Vec<u64> = keys["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["line"].as_u64().unwrap())
        .collect();
    assert!(
        lines.windows(2).all(|w| w[0] > w[1]),
        "flat toml keys: {lines:?}"
    );
}

#[test]
fn bound_summary_gives_the_first_of_two_arrays_the_rounded_down_share() {
    // Two equal arrays, smallest-first so `a` is allocated with `count - i == 2`. The remaining
    // room R is odd, so the share is floor(R/2) or, with a rounded-up division, one more. R =
    // 8k - 1 (here k = 8, R = 63) puts that one byte across a boundary: share 31 keeps 7+7,
    // share 32 keeps 8+8. `b` then takes the remainder (32) whichever way `a` was rounded, so
    // only `a` tells the two apart.
    let base = ser_len(&serde_json::json!({"type": "x", "a": [], "b": []}));
    let budget = base + 600 + 63; // 300 B reserved per array for its markers
    let s = serde_json::json!({"type": "x", "a": sevens(400), "b": sevens(400)});
    let (cut, _) = super::bound_summary(s, "@file_t", budget);

    assert!(cut.get("summary_omitted").is_none(), "{cut:.200}");
    assert_eq!(
        cut["a"].as_array().unwrap().len(),
        14,
        "a: floor(63/2) = 31 -> 7 per side"
    );
    assert_eq!(
        cut["b"].as_array().unwrap().len(),
        16,
        "b: the remaining 32 -> 8 per side"
    );
}

#[test]
fn bound_summary_string_threshold_is_exactly_500() {
    // A 500-byte string is part of the fixed cost, never cut; the next one is wide. Tight budget
    // so a string counted as wide WOULD be cut: with `>= 500` the 500-byte `a` joins `b` and the
    // common share (about 370) cuts it too.
    let s = serde_json::json!({"type": "x", "a": "a".repeat(500), "b": "b".repeat(20_000)});
    let (cut, _) = super::bound_summary(s, "@file_t", 1_000);
    assert_eq!(
        cut["a"].as_str().unwrap(),
        "a".repeat(500),
        "a 500 B string was cut"
    );
    assert!(cut["b"].as_str().unwrap().contains("bytes shown"));
    assert!(ser_len(&cut) <= 1_000, "{} B", ser_len(&cut));

    // 501 bytes IS wide: with `> 501` it would be treated as fixed, nothing could be cut, and
    // the last resort would drop the whole summary.
    let s = serde_json::json!({"type": "x", "a": "a".repeat(501)});
    let (cut, _) = super::bound_summary(s, "@file_t", 400);
    assert!(
        cut["a"].as_str().unwrap().contains("bytes shown"),
        "a 501 B string was not cut: {cut:.200}"
    );
    assert!(cut.get("summary_omitted").is_none());
    assert!(ser_len(&cut) <= 400, "{} B", ser_len(&cut));
}
#[test]
fn bound_summary_finds_a_share_over_half_the_string_when_the_budget_allows_it() {
    // M1 (`hi = longest / 2`). One 1,000 B string and a 900 B budget: the largest share that
    // fits is ~770 B, over HALF the string (500). A search capped at half stops at ~499 and
    // leaves ~280 B of the budget unused. The result must sit within 4 B of the budget: a
    // serialized size moves in steps of 2 (head and tail each take share/2), plus the digits of
    // the marker's count.
    let s = serde_json::json!({"type": "x", "head": "h".repeat(1_000)});
    let (cut, _) = super::bound_summary(s, "@file_t", 900);
    let size = ser_len(&cut);
    assert!(size <= 900, "{size} B over the budget");
    assert!(
        size >= 896,
        "{size} B: the search stopped short of the best share"
    );
    assert!(cut["head"].as_str().unwrap().contains("bytes shown"));
}

// ---- the six survivors of the final mutation run: tests where ONLY the arithmetic decides ----

#[test]
fn bound_summary_takes_the_smallest_cut_when_no_share_fits_and_lets_the_arrays_finish() {
    // B4. The fixed part leaves no room for `head` at ANY share: even with `head` cut to its
    // marker alone, the whole array still does not fit. The smallest cut (share 0, marker only)
    // is then taken and the ARRAY pass finishes the job. Taking no cut instead leaves 5,000 B of
    // `head` that nothing can reduce, and the last resort drops the whole summary.
    let budget = 900;
    let marker = "\n--- head: 0 of 5000 bytes shown; the rest: read_file(path=\"@file_t\", \
                  start_line=N, end_line=M) ---\n";
    let s = serde_json::json!({"type": "x", "head": "h".repeat(5_000), "symbols": sevens(400)});
    // Premise: with head at its smallest the summary is STILL over budget.
    let floor = serde_json::json!({"type": "x", "head": marker, "symbols": sevens(400)});
    assert!(ser_len(&floor) > budget, "{} B", ser_len(&floor));
    // The array's allowance: what is left after the fixed part and 300 B of markers; cost 2 each.
    let base = ser_len(&serde_json::json!({"type": "x", "head": marker, "symbols": []}));
    let remaining = budget - base - 300;
    let expected_kept = 2 * ((remaining / 2) / 2);

    let (cut, notes) = super::bound_summary(s, "@file_t", budget);

    assert!(
        cut.get("summary_omitted").is_none(),
        "dropped whole: {cut:.200}"
    );
    assert_eq!(cut["head"], marker, "head must be exactly its marker");
    assert_eq!(cut["symbols"].as_array().unwrap().len(), expected_kept);
    assert_eq!(
        expected_kept, 230,
        "fixture drifted: {remaining} B left for the array"
    );
    assert!(ser_len(&cut) <= budget, "{} B", ser_len(&cut));
    assert_eq!(notes.len(), 1, "{notes:?}");
}

#[test]
fn fit_envelope_accepts_an_envelope_that_lands_exactly_on_the_target_in_one_pass() {
    // E5 and E3. The hint carries the cut note plus `hp` bytes of padding that exist ONLY when a
    // note does, so the overhead probe (no note) and therefore the cut are the same for every
    // `hp`: the first pass's envelope is a fixed ~8.8 KB plus `hp`, rising one byte per step.
    // Sweeping `hp` from 0 therefore passes through EXACTLY the 9,000 B target, and that
    // envelope must be accepted as it is: three `finish` calls (the whole, the overhead probe,
    // ONE pass). `size < target` would reject it and cut again; an overhead not subtracted would
    // overshoot on pass 1 and need a second pass. Neither can produce an envelope of exactly
    // the target, so the search below fails for both.
    let summary = serde_json::json!({"type": "source", "symbols": numbered_entries(400)});
    for hp in 0..400 {
        let calls = std::cell::Cell::new(0);
        let finish = |s: serde_json::Value, notes: &[String]| {
            calls.set(calls.get() + 1);
            let hint = if notes.is_empty() {
                String::new()
            } else {
                format!("{} {}", notes.join(" "), "x".repeat(hp))
            };
            serde_json::json!({"s": s, "hint": hint})
        };
        let got = fit_envelope(summary.clone(), "@file_t", finish);
        if ser_len(&got) == SUMMARY_ENVELOPE_BUDGET {
            assert_eq!(
                calls.get(),
                3,
                "hp {hp}: an envelope on the target must be accepted"
            );
            return;
        }
    }
    panic!("no hint padding of 0..400 lands the envelope exactly on {SUMMARY_ENVELOPE_BUDGET} B");
}
#[test]
fn fit_envelope_subtracts_what_the_envelope_costs_around_the_summary() {
    // E3. 2,000 B of the envelope are NOT summary. Cutting the summary to the whole 9,000 B
    // target (the overhead not subtracted) leaves the envelope ~2,000 B over, so a second pass
    // is forced; cutting to 9,000 minus the measured overhead fits on the FIRST: three finish
    // calls (whole, overhead probe, one pass), the envelope within the target.
    let summary = serde_json::json!({"type": "source", "symbols": numbered_entries(400)});
    let calls = std::cell::Cell::new(0);
    let finish = |s: serde_json::Value, notes: &[String]| {
        calls.set(calls.get() + 1);
        serde_json::json!({"s": s, "hint": notes.join(" "), "pad": "p".repeat(2_000)})
    };

    let got = fit_envelope(summary, "@file_t", finish);

    assert_eq!(
        calls.get(),
        3,
        "the overhead must be priced in before the first cut"
    );
    assert!(
        ser_len(&got) <= SUMMARY_ENVELOPE_BUDGET,
        "{} B",
        ser_len(&got)
    );
    assert!(got["s"].get("summary_omitted").is_none());
}

#[test]
fn fit_envelope_converges_in_exactly_two_passes_when_the_notes_overshoot_the_slack() {
    // E4 and E7. The hint carries the cut note six times, ~700 B against the ~200 B of slack the
    // 300 B marker reservation leaves, so the FIRST pass overshoots. The second must shrink the
    // budget by exactly that excess and fit: four `finish` calls (whole, overhead, pass 1,
    // pass 2). No shrink repeats pass 1's overshoot until the retries run out; a single pass
    // never gets a second chance. Both end in the minimal summary instead of these symbols.
    let summary = serde_json::json!({"type": "source", "symbols": numbered_entries(400)});
    let calls = std::cell::Cell::new(0);
    let finish = |s: serde_json::Value, notes: &[String]| {
        calls.set(calls.get() + 1);
        serde_json::json!({"s": s, "hint": notes.join(" ").repeat(6)})
    };

    let got = fit_envelope(summary, "@file_t", finish);

    assert_eq!(
        calls.get(),
        4,
        "whole, overhead, pass 1 (overshoots), pass 2 (fits)"
    );
    assert!(
        ser_len(&got) <= SUMMARY_ENVELOPE_BUDGET,
        "{} B",
        ser_len(&got)
    );
    assert!(
        got["s"].get("summary_omitted").is_none(),
        "fell back to the minimal summary"
    );
    assert_eq!(got["s"]["symbols"].as_array().unwrap().len(), 174);
}

#[test]
fn fit_envelope_falls_back_to_the_minimal_summary_while_the_budget_is_still_positive() {
    // E6, and the retry count. Whenever a note exists the envelope is a constant 10,500 B, so no
    // budget fits and each pass shrinks the budget by 1,516. From ~8,990 B that leaves ~2,930 B
    // after four passes: STILL POSITIVE, so the fallback must be the budget-0 minimal summary,
    // not another cut at the leftover budget (which keeps symbols and claims to have fit). Seven
    // `finish` calls: whole, overhead, four passes, the minimal; a fifth pass would make eight.
    let summary =
        serde_json::json!({"type": "source", "line_count": 400, "symbols": numbered_entries(400)});
    let calls = std::cell::Cell::new(0);
    let finish = |s: serde_json::Value, notes: &[String]| {
        calls.set(calls.get() + 1);
        if notes.is_empty() {
            return serde_json::json!({"s": s});
        }
        let bare = ser_len(&serde_json::json!({"s": s, "pad": ""}));
        serde_json::json!({"s": s, "pad": "p".repeat(10_500 - bare)})
    };

    let got = fit_envelope(summary, "@file_t", finish);

    assert_eq!(got["s"]["summary_omitted"], true, "{:.300}", got["s"]);
    assert!(got["s"].get("symbols").is_none());
    assert_eq!(got["s"]["line_count"], 400);
    assert_eq!(calls.get(), 7, "whole, overhead, four passes, the minimal");
}

#[test]
fn bound_summary_shares_the_budget_equally_between_wide_strings() {
    // `head` and `tail` are both informative: neither may be crushed to make room for the
    // other, and together they must fit.
    let s = serde_json::json!({
        "type": "generic",
        "head": "h".repeat(10_000),
        "tail": "t".repeat(10_000),
    });
    let (cut, _) = bound_summary(s, "@file_t");
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
    let (h, t) = (
        cut["head"].as_str().unwrap().len(),
        cut["tail"].as_str().unwrap().len(),
    );
    assert!(
        h > 2_000 && t > 2_000,
        "an unequal split: head {h} B, tail {t} B"
    );
    assert!(
        h.abs_diff(t) < 100,
        "an unequal split: head {h} B, tail {t} B"
    );
}

#[test]
fn bound_summary_cuts_several_mid_sized_strings_that_only_overflow_together() {
    // Four strings of 2,000 B: each is modest, the sum (8 KB) is over the budget.
    let s = serde_json::json!({
        "type": "x", "a": "a".repeat(2_000), "b": "b".repeat(2_000),
        "c": "c".repeat(2_000), "d": "d".repeat(2_000),
    });
    let (cut, _) = bound_summary(s, "@file_t");
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
}

fn numbered_entries(n: usize) -> Vec<serde_json::Value> {
    (1..=n)
        .map(|i| serde_json::json!({"name": format!("sym{i:03}"), "kind": "Function", "line": i}))
        .collect()
}

#[test]
fn bound_summary_cuts_the_largest_array_and_spares_the_small_one() {
    let s = serde_json::json!({
        "type": "source",
        "symbols": numbered_entries(300),
        "keys": numbered_entries(3),
    });
    let (cut, notes) = bound_summary(s, "@file_t");

    assert_eq!(
        cut["keys"].as_array().unwrap().len(),
        3,
        "the small array was cut"
    );
    assert!(cut.get("keys_truncated").is_none());
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));

    let kept = cut["symbols"].as_array().unwrap();
    let after = cut["symbols_omitted"]["after"].as_u64().unwrap() as usize;
    let count = cut["symbols_omitted"]["count"].as_u64().unwrap() as usize;
    assert_eq!(kept[0]["name"], "sym001", "the first entry must survive");
    assert_eq!(
        kept.last().unwrap()["name"],
        "sym300",
        "the last entry must survive"
    );
    assert_eq!(
        kept.len() + count,
        300,
        "kept + omitted must account for every entry"
    );
    assert!(
        after > 0 && after < kept.len(),
        "both halves must be non-empty"
    );
    // The gap is lines [after+1, tail_first-1], each symbol being on its own line number.
    assert_eq!(cut["symbols_omitted"]["from_line"], (after + 1) as u64);
    let tail_first = kept[after]["line"].as_u64().unwrap();
    assert_eq!(cut["symbols_omitted"]["to_line"], tail_first - 1);
    assert_eq!(cut["symbols_truncated"], true);
    assert_eq!(cut["total_symbols"], 300);
}
#[test]
fn bound_summary_cuts_an_array_one_level_down_and_marks_it_beside_the_array() {
    // `summarize_json` keeps its key list under `schema`. The markers must land NEXT TO the
    // array (in `schema`), where the renderer and any consumer already look for `total_keys`
    // and `keys_truncated`, and the note must say where the array lives.
    let keys: Vec<serde_json::Value> = (1..=40)
        .map(|i| serde_json::json!({"path": format!("$.{}{i:02}", "k".repeat(500)), "type": "number"}))
        .collect();
    let s = serde_json::json!({
        "type": "json",
        "line_count": 40,
        "schema": {"root_type": "object", "keys": keys},
    });
    let (cut, notes) = bound_summary(s, "@file_t");

    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
    let schema = &cut["schema"];
    assert_eq!(
        schema["keys_truncated"], true,
        "the marker must sit beside the array: {cut}"
    );
    assert_eq!(schema["total_keys"], 40);
    let after = schema["keys_omitted"]["after"].as_u64().unwrap() as usize;
    let count = schema["keys_omitted"]["count"].as_u64().unwrap() as usize;
    assert_eq!(schema["keys"].as_array().unwrap().len() + count, 40);
    assert!(after > 0, "the first entries must survive");
    assert!(
        schema["keys"].as_array().unwrap().last().unwrap()["path"]
            .as_str()
            .unwrap()
            .ends_with("40"),
        "the last entry must survive"
    );
    assert!(
        cut.get("keys_truncated").is_none(),
        "no stray marker at the top level"
    );
    assert!(notes[0].starts_with("schema.keys: "), "{}", notes[0]);
}
#[test]
fn bound_summary_shares_the_budget_between_two_competing_arrays() {
    // Two arrays in one summary, one of them nested. A SYNTHETIC shape: no summarizer produces
    // two today (a default markdown read marks every heading seen, so `coverage.unread` never
    // rides beside the map). It pins the allocation for the day one does: cutting the larger
    // while the other stood whole once left it no room, and EVERY entry was dropped.
    let headings: Vec<serde_json::Value> = (1..=200)
        .map(|i| serde_json::json!({"h": format!("## S{i:03} {}", "w".repeat(60)), "l": 3 * i}))
        .collect();
    let unread: Vec<serde_json::Value> = (1..=200)
        .map(|i| serde_json::json!(format!("## S{i:03} {}", "w".repeat(60))))
        .collect();
    let s = serde_json::json!({
        "lines": 600,
        "headings": headings,
        "coverage": {"read": 1, "total": 200, "unread": unread},
    });
    let (cut, notes) = bound_summary(s, "@file_t");

    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
    let h = cut["headings"].as_array().unwrap();
    assert!(h.len() > 10, "the heading map was wiped: {} left", h.len());
    assert!(h[0]["h"].as_str().unwrap().contains("S001"));
    assert!(h.last().unwrap()["h"].as_str().unwrap().contains("S200"));
    let u = cut["coverage"]["unread"].as_array().unwrap();
    assert!(u.len() > 10, "the unread list was wiped: {} left", u.len());
    assert!(u[0].as_str().unwrap().contains("S001"));
    assert!(u.last().unwrap().as_str().unwrap().contains("S200"));
    assert_eq!(cut["coverage"]["unread_truncated"], true);
    assert_eq!(notes.len(), 2, "{notes:?}");
}

#[test]
fn bound_summary_spares_an_array_that_fits_its_share_and_gives_its_leftover_away() {
    // `keys` is ~600 B and fits any fair share; `symbols` is huge. The small one must be
    // returned whole AND what it leaves unused must go to the large one: a plain equal split
    // would give `symbols` only half the room.
    let s = serde_json::json!({
        "type": "source",
        "keys": numbered_entries(12),
        "symbols": numbered_entries(400),
    });
    let (cut, _) = bound_summary(s, "@file_t");

    assert_eq!(
        cut["keys"].as_array().unwrap().len(),
        12,
        "a small array was cut"
    );
    assert!(cut.get("keys_truncated").is_none());
    let kept = cut["symbols"].as_array().unwrap().len();
    assert!(
        kept >= 85,
        "the leftover was not handed on: only {kept} symbols kept (an equal split keeps ~55)"
    );
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
}
#[test]
fn bound_summary_leaves_arrays_alone_when_cutting_strings_was_enough() {
    // The string pass leaves the summary under budget, so the small array is NOT touched: if
    // the arrays phase ran anyway it would find no room left (the string took its share) and
    // cut a 3-entry array to nothing.
    let s = serde_json::json!({
        "type": "x",
        "head": "h".repeat(10_000),
        "keys": numbered_entries(3),
    });
    let (cut, notes) = bound_summary(s, "@file_t");
    assert!(cut["head"].as_str().unwrap().contains("bytes shown"));
    assert!(ser_len(&cut) <= SUMMARY_BYTE_BUDGET, "{} B", ser_len(&cut));
    assert_eq!(
        cut["keys"].as_array().unwrap().len(),
        3,
        "an array that fit was cut"
    );
    assert!(cut.get("keys_truncated").is_none());
    assert!(notes.is_empty(), "{notes:?}");
}

/// A summary of two arrays in which `a` (`n` equal-length strings) serializes to EXACTLY
/// `fair + delta` bytes, `fair` being the share `bound_summary` offers it (it is the smaller,
/// so it is allocated first, with `count == 2`). The arithmetic depends on the length of
/// `type`, so that is searched until `a`'s byte count can be hit exactly.
fn two_arrays_with_a_sized_against_its_share(n: usize, delta: i64) -> serde_json::Value {
    for pad in 0..32 {
        let ty = "x".repeat(1 + pad);
        let probe = serde_json::json!({"type": ty, "a": [], "b": []});
        let remaining = SUMMARY_BYTE_BUDGET - ser_len(&probe) - 600;
        let want = (remaining / 2) as i64 + delta;
        // n strings of length L serialize to n * (L + 3) + 1 bytes.
        if want >= 1 && (want - 1) % n as i64 == 0 {
            let len = (want - 1) / n as i64 - 3;
            if len >= 1 {
                let a: Vec<serde_json::Value> = (0..n)
                    .map(|_| serde_json::json!("a".repeat(len as usize)))
                    .collect();
                let s = serde_json::json!({"type": ty, "a": a, "b": numbered_entries(400)});
                let got = ser_len(&serde_json::json!(s["a"]));
                assert_eq!(got as i64, want, "the fixture arithmetic is off");
                return s;
            }
        }
    }
    panic!("no `type` length lands the fixture on fair + {delta} with {n} entries");
}

#[test]
fn bound_summary_returns_an_array_whole_when_it_is_exactly_its_share() {
    // bytes == fair: it fits. With three entries a cut by halves of its own size would still
    // drop one (each half rounds down to one entry), so only the `<=` keeps it whole.
    let s = two_arrays_with_a_sized_against_its_share(3, 0);
    let (cut, _) = bound_summary(s, "@file_t");
    assert_eq!(
        cut["a"].as_array().unwrap().len(),
        3,
        "an array that fit was cut"
    );
    assert!(cut.get("a_truncated").is_none());
    assert!(
        cut["b_truncated"] == true,
        "the large one must still be cut"
    );
}

#[test]
fn bound_summary_marks_no_cut_when_the_halves_still_hold_every_entry() {
    // One byte over its share, but the entries (2 of them) each fit a half: nothing is
    // removed, so nothing may be marked. A marker with `count: 0` claims a cut that did not
    // happen.
    let s = two_arrays_with_a_sized_against_its_share(2, 1);
    let (cut, notes) = bound_summary(s, "@file_t");
    assert_eq!(cut["a"].as_array().unwrap().len(), 2);
    assert!(cut.get("a_truncated").is_none(), "{}", cut["a_omitted"]);
    assert!(cut.get("a_omitted").is_none());
    assert_eq!(notes.len(), 1, "only `b` was cut: {notes:?}");
}

#[test]
fn bound_summary_note_is_a_ready_to_run_call_naming_the_handle_and_the_gap() {
    let s = serde_json::json!({"type": "source", "symbols": numbered_entries(300)});
    let (cut, notes) = bound_summary(s, "@file_t");
    let from = cut["symbols_omitted"]["from_line"].as_u64().unwrap();
    let to = cut["symbols_omitted"]["to_line"].as_u64().unwrap();
    assert_eq!(
        notes[0],
        format!(
            "symbols: {} of 300 entries omitted (lines {from}-{to}); read them with \
             read_file(path=\"@file_t\", start_line={from}, end_line={to}).",
            cut["symbols_omitted"]["count"]
        )
    );
}

#[test]
fn bound_summary_keeps_the_totals_the_summarizer_already_named() {
    // `summarize_toml` truncates to 30 itself and records the FILE's total. This cut works on
    // what was left, so "N of M" must use the file's M, and the key must not be overwritten.
    let s = serde_json::json!({
        "type": "toml",
        "sections": numbered_entries(200),
        "total_sections": 900,
        "sections_truncated": true,
    });
    let (cut, notes) = bound_summary(s, "@file_t");
    assert_eq!(
        cut["total_sections"], 900,
        "the summarizer's own total was overwritten"
    );
    assert!(notes[0].contains("of 900 entries omitted"), "{}", notes[0]);
}

#[test]
fn bound_summary_note_for_entries_with_no_line_numbers_still_says_how_to_read_more() {
    let entries: Vec<serde_json::Value> = (0..300)
        .map(|i| serde_json::json!({"key": format!("key{i:03}-{}", "k".repeat(40))}))
        .collect();
    let s = serde_json::json!({"type": "x", "keys": entries});
    let (cut, notes) = bound_summary(s, "@file_t");
    assert!(cut["keys_omitted"]["from_line"].is_null());
    assert!(
        notes[0].contains("read the file in ranges with"),
        "{}",
        notes[0]
    );
    assert!(notes[0].contains("@file_t"));
}

/// docs/issues/archive/2026-08-27-append-entry-anchor-is-undiscoverable-through-the-surface-its-error-names.md
///
/// Third instance of one defect class, and the one every heading-addressed tool
/// routes through. The window was `take(15)` — head-only — while a
/// heading-addressed append targets the document's LAST stanza, so on a long
/// document the needed heading was exactly the one the list dropped.
#[test]
fn a_missing_heading_lists_both_ends_not_just_the_first_fifteen() {
    let mut body = String::new();
    for i in 0..40 {
        body.push_str(&format!("## H{i}\n\ntext\n\n"));
    }
    body.push_str("## Template for new entries\n\ntext\n");

    let err = resolve_section_range(&body, "## No Such Heading").unwrap_err();
    let hint = err.hint().unwrap_or_default();

    assert!(
        hint.contains("## Template for new entries"),
        "the final heading — the conventional append anchor — must be listed: {hint}"
    );
    assert!(
        hint.contains("## H0"),
        "the head of the document must still be listed: {hint}"
    );
    assert!(
        hint.contains("more"),
        "the elision must be counted, so the gap is legible rather than merely \
             absent: {hint}"
    );
}

/// A short document has no gap, so it must not grow an elision marker.
#[test]
fn a_short_document_lists_every_heading_with_no_elision() {
    let body = "## A\n\nx\n\n## B\n\ny\n";
    let err = resolve_section_range(body, "## Nope").unwrap_err();
    let hint = err.hint().unwrap_or_default();
    assert!(
        hint.contains("## A") && hint.contains("## B"),
        "got: {hint}"
    );
    assert!(
        !hint.contains("more"),
        "nothing was elided, so nothing should be announced: {hint}"
    );
}

#[test]
fn json_summary_signals_truncated_keys() {
    let mut obj = serde_json::Map::new();
    for i in 0..40 {
        obj.insert(format!("k{i}"), serde_json::json!(i));
    }
    let content = serde_json::to_string(&serde_json::Value::Object(obj)).unwrap();
    let s = summarize_json(&content);
    let schema = &s["schema"];
    assert_eq!(schema["keys"].as_array().unwrap().len(), 30, "capped at 30");
    assert_eq!(schema["total_keys"].as_u64().unwrap(), 40);
    assert_eq!(schema["keys_truncated"], serde_json::json!(true));
}

#[test]
fn toml_summary_signals_truncated_flat_keys() {
    // No table headers -> flat top-level key branch, capped at 20.
    let content: String = (0..25)
        .map(|i| format!("k{i} = {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let s = summarize_toml(&content);
    assert_eq!(s["keys"].as_array().unwrap().len(), 20, "capped at 20");
    assert_eq!(s["total_keys"].as_u64().unwrap(), 25);
    assert_eq!(s["keys_truncated"], serde_json::json!(true));
}

#[test]
fn toml_summary_signals_truncated_sections() {
    let content: String = (0..35)
        .map(|i| format!("[t{i}]\nk = {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    let s = summarize_toml(&content);
    assert_eq!(s["sections"].as_array().unwrap().len(), 30, "capped at 30");
    assert_eq!(s["total_sections"].as_u64().unwrap(), 35);
    assert_eq!(s["sections_truncated"], serde_json::json!(true));
}

#[test]
fn toml_summary_handles_malformed() {
    let content = "not valid toml [[[";
    let s = summarize_toml(content);
    assert_eq!(s["type"].as_str().unwrap(), "toml");
    assert!(s["line_count"].as_u64().is_some());
}

#[test]
fn yaml_summary_shows_top_level_keys() {
    let content =
        "database:\n  host: localhost\n  port: 5432\nserver:\n  port: 8080\nlogging:\n  level: debug";
    let s = summarize_yaml(content);
    assert_eq!(s["type"].as_str().unwrap(), "yaml");
    assert_eq!(s["format"].as_str().unwrap(), "yaml");
    let sections = s["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0]["key"].as_str().unwrap(), "database");
    assert_eq!(sections[0]["line"].as_u64().unwrap(), 1);
    assert_eq!(sections[0]["end_line"].as_u64().unwrap(), 3);
    assert_eq!(sections[1]["key"].as_str().unwrap(), "server");
    assert_eq!(sections[2]["key"].as_str().unwrap(), "logging");
}

#[test]
fn yaml_summary_skips_comments_and_directives() {
    let content = "---\n# A comment\nfoo:\n  bar: 1\nbaz:\n  qux: 2\n...";
    let s = summarize_yaml(content);
    let sections = s["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0]["key"].as_str().unwrap(), "foo");
    assert_eq!(sections[1]["key"].as_str().unwrap(), "baz");
}

#[test]
fn yaml_summary_handles_empty_file() {
    let content = "# just a comment\n---";
    let s = summarize_yaml(content);
    assert_eq!(s["type"].as_str().unwrap(), "yaml");
    // Falls back to generic
    assert!(s["head"].is_string());
}

#[test]
fn extract_markdown_section_exact_match() {
    let content = "# Intro\nwelcome\n## Setup\ndo this\nand that\n## Usage\nuse it";
    let result = extract_markdown_section(content, "## Setup").unwrap();
    assert_eq!(result.content, "## Setup\ndo this\nand that");
    assert_eq!(result.line_range, (3, 5));
    assert_eq!(result.breadcrumb, vec!["# Intro", "## Setup"]);
    assert_eq!(result.siblings, vec!["## Usage"]);
}

#[test]
fn extract_markdown_section_prefix_match() {
    let content = "# Title\n## Authentication Guide\ndetails here";
    let result = extract_markdown_section(content, "## Auth").unwrap();
    assert!(result.content.contains("Authentication Guide"));
}

#[test]
fn extract_markdown_section_not_found() {
    let content = "# Title\n## Setup\ntext";
    let result = extract_markdown_section(content, "## Nonexistent");
    assert!(result.is_err());
}

#[test]
fn extract_markdown_section_no_headings() {
    let content = "just some text\nno headings here";
    let result = extract_markdown_section(content, "## Anything");
    assert!(result.is_err());
}

#[test]
fn extract_markdown_section_beyond_30_headings() {
    let mut content = String::from("# Title\n");
    for i in 1..=35 {
        content.push_str(&format!("## Section {i}\ncontent {i}\n"));
    }
    let result = extract_markdown_section(&content, "## Section 35").unwrap();
    assert!(result.content.contains("content 35"));
}

#[test]
fn extract_markdown_section_stripped_match() {
    let content = "# Title\n## The `auth` Module\ndetails here\n";
    let result = extract_markdown_section(content, "## The auth Module").unwrap();
    assert!(result.content.contains("details here"));
}

#[test]
fn extract_json_path_top_level_key() {
    let content = r#"{"name": "test", "deps": {"a": 1, "b": 2}}"#;
    let (result, type_name, count) = extract_json_path(content, "$.deps").unwrap();
    assert!(result.contains("\"a\""));
    assert!(result.contains("\"b\""));
    assert_eq!(type_name, "object");
    assert_eq!(count, Some(2));
}

#[test]
fn extract_json_path_nested() {
    let content = r#"{"db": {"connection": {"host": "localhost", "port": 5432}}}"#;
    let (result, _, _) = extract_json_path(content, "$.db.connection").unwrap();
    assert!(result.contains("localhost"));
}

#[test]
fn extract_json_path_array_index() {
    let content = r#"{"users": [{"name": "alice"}, {"name": "bob"}]}"#;
    let (result, _, _) = extract_json_path(content, "$.users[0]").unwrap();
    assert!(result.contains("alice"));
    assert!(!result.contains("bob"));
}

#[test]
fn extract_json_path_not_found() {
    let content = r#"{"name": "test"}"#;
    let result = extract_json_path(content, "$.nonexistent");
    assert!(result.is_err());
}

/// A capped key list must SAY it was capped, and must keep the TAIL.
///
/// `resolve_json_segment`'s hint was `obj.keys().take(10)` with no marker.
/// `serde_json` is built with `preserve_order`, so `keys()` is insertion order and
/// head-only truncation drops the keys added LAST — the newest, and the ones a
/// caller is most likely reaching for. Measured 2026-09-07 on `doctor`'s
/// `catalog_health`: 13 keys, the three elided were `open_bug_source_citations`,
/// `audit` and `hint`, and a session read the resulting complete-looking list as
/// evidence that the running binary lacked the code inserting them. The list named
/// no key wrongly; it was wrong by being readable as total.
///
/// THREE DIRECTIONS, because each assertion alone is monotone under a different
/// wrong implementation:
///
/// - the elision is stated with its true count -> reds on silent truncation
/// - the tail survives -> reds on a head-only window, even one that marks itself
/// - under the cap: all keys, no marker -> reds on one that always marks
#[test]
fn json_path_key_miss_hint_states_its_elision_and_keeps_the_tail() {
    // 13 keys, insertion-ordered k00..k12. BOTH details are load-bearing: the count
    // must exceed HEAD + TAIL (7 + 3), or the windowing branch never runs and this
    // passes without discriminating anything; and `k12` must be the FINAL insertion,
    // or the tail assertion stops distinguishing a head-only window from a two-ended
    // one. A tidy-up that shrinks the loop or sorts the keys silently guts this test.
    let mut obj = serde_json::Map::new();
    for i in 0..13 {
        obj.insert(format!("k{i:02}"), serde_json::json!(i));
    }
    let content = serde_json::to_string(&serde_json::Value::Object(obj)).unwrap();
    let err = extract_json_path(&content, "$.absent")
        .unwrap_err()
        .to_string();

    // The cut is stated, with the number actually elided (13 - 7 - 3). This pins
    // HEAD/TAIL as a contract rather than an implementation detail: changing the
    // window is a decision, so it should change this number deliberately, not drift.
    assert!(
        err.contains("(+3 more)"),
        "hint must name how many keys it elided: {err}"
    );
    // The tail survives. The bug named k00..k09 and dropped exactly these.
    assert!(
        err.contains("k12") && err.contains("k11") && err.contains("k10"),
        "hint must keep the last-inserted keys: {err}"
    );

    // Under the cap: all three keys named and NO marker. Without this case an
    // implementation that always prints a marker satisfies both assertions above,
    // and one that always truncates is indistinguishable from one that windows.
    let small_err = extract_json_path(r#"{"a": 1, "b": 2, "c": 3}"#, "$.absent")
        .unwrap_err()
        .to_string();
    assert!(
        small_err.contains("a, b, c"),
        "an under-cap object must list every key: {small_err}"
    );
    assert!(
        !small_err.contains("more)"),
        "no elision marker when nothing was elided: {small_err}"
    );
}

#[test]
fn extract_json_path_root() {
    let content = r#"{"a": 1}"#;
    let (result, type_name, count) = extract_json_path(content, "$").unwrap();
    assert!(result.contains("\"a\""));
    assert_eq!(type_name, "object");
    assert_eq!(count, Some(1));
}

#[test]
fn extract_toml_key_table() {
    let content = "[package]\nname = \"foo\"\n\n[dependencies]\nserde = \"1.0\"\ntokio = \"1.0\"";
    let result = extract_toml_key(content, "dependencies").unwrap();
    assert!(result.content.contains("serde"));
    assert!(result.content.contains("tokio"));
    assert_eq!(result.format, "toml");
    assert!(result.siblings.iter().any(|s| s.contains("package")));
}

#[test]
fn extract_toml_key_not_found() {
    let content = "[package]\nname = \"foo\"";
    let result = extract_toml_key(content, "nonexistent");
    assert!(result.is_err());
}

#[test]
fn extract_yaml_key_section() {
    let content = "database:\n  host: localhost\n  port: 5432\nserver:\n  port: 8080";
    let result = extract_yaml_key(content, "database").unwrap();
    assert!(result.content.contains("host"));
    assert!(result.content.contains("localhost"));
    assert_eq!(result.format, "yaml");
    assert!(result.siblings.iter().any(|s| s == "server"));
}

#[test]
fn extract_yaml_key_not_found() {
    let content = "database:\n  host: localhost\nserver:\n  port: 8080";
    let result = extract_yaml_key(content, "nonexistent");
    assert!(result.is_err());
}

#[test]
fn extract_toml_key_table_past_summary_cap() {
    // >30 tables: a table past the 30-section summary cap must still resolve,
    // not false-error. Regression for the summary-cap false-"not found" bug
    // (docs/issues/archive/2026-07-10-toml-yaml-key-false-not-found-past-summary-cap.md).
    let mut content = String::new();
    for i in 0..40 {
        content.push_str(&format!("[table{i:02}]\nval = {i}\n\n"));
    }
    let result = extract_toml_key(&content, "table35").unwrap();
    assert!(
        result.content.contains("35"),
        "table past the summary cap must resolve: {}",
        result.content
    );
}

#[test]
fn extract_toml_key_top_level_scalar_in_mixed_file() {
    // Mixes top-level scalars with tables: summarize_toml emits `sections`
    // (tables exist), so the pre-fix sections-first-then-error path never reached
    // the flat/dotted fallback. The top-level scalar must resolve. Regression for
    // docs/issues/archive/2026-07-10-extract-toml-key-branch-order-mixed-files-unreachable.md.
    let content = "edition = \"2021\"\nname = \"foo\"\n\n[deps]\nserde = \"1\"\n";
    let result = extract_toml_key(content, "edition").unwrap();
    assert!(
        result.content.contains("2021"),
        "top-level scalar must resolve in a mixed file: {}",
        result.content
    );
}

#[test]
fn extract_yaml_key_past_summary_cap() {
    // >30 top-level keys: a key past the 30-key display cap must still resolve.
    let mut content = String::new();
    for i in 0..40 {
        content.push_str(&format!("key{i:02}: value{i}\n"));
    }
    let result = extract_yaml_key(&content, "key35").unwrap();
    assert!(
        result.content.contains("value35"),
        "yaml key past the cap must resolve: {}",
        result.content
    );
}

#[test]
fn parse_all_headings_basic() {
    let content = "# Title\ntext\n## Setup\ndo this\n## Usage\nuse it";
    let headings = parse_all_headings(content);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].text, "# Title");
    assert_eq!(headings[0].level, 1);
    assert_eq!(headings[0].line, 1);
    assert_eq!(headings[0].end_line, 6);
    assert_eq!(headings[1].text, "## Setup");
    assert_eq!(headings[1].line, 3);
    assert_eq!(headings[1].end_line, 4);
    assert_eq!(headings[2].text, "## Usage");
    assert_eq!(headings[2].line, 5);
    assert_eq!(headings[2].end_line, 6);
}

#[test]
fn parse_all_headings_skips_code_blocks() {
    let content = "# Title\n```\n## Not a heading\n```\n## Real heading\ntext";
    let headings = parse_all_headings(content);
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].text, "# Title");
    assert_eq!(headings[1].text, "## Real heading");
}

/// Regression: a nested three-backtick fence must not close the enclosing
/// four-backtick block. When it did, the phantom heading terminated the
/// enclosing section early and scoped edits reported `old_string not found`
/// for text that was byte-present.
/// docs/issues/archive/2026-08-11-artifact-nested-fence-closes-outer-fence.md
#[test]
fn parse_all_headings_respects_nested_fence_run_length() {
    let content = "\
## Task

````markdown
# Page Title

```toml
# .codescout/project.toml
```
````

Prose after the outer fence.

## Next
";
    let got: Vec<String> = parse_all_headings(content)
        .into_iter()
        .map(|h| h.text)
        .collect();
    assert_eq!(got, vec!["## Task", "## Next"]);
}

/// The section must extend past the nested fence to the next real sibling,
/// so text after the outer fence is still inside `## Task`.
#[test]
fn extract_markdown_section_spans_a_nested_fence() {
    let content = "\
## Task

````markdown
# Page Title

```toml
# .codescout/project.toml
```
````

Prose after the outer fence.

## Next
next body
";
    let result = extract_markdown_section(content, "## Task").unwrap();
    assert!(
        result.content.contains("Prose after the outer fence."),
        "section truncated at the phantom heading: {:?}",
        result.content
    );
    assert!(
        !result.content.contains("next body"),
        "section must still stop at the real sibling: {:?}",
        result.content
    );
}

/// A backtick run never closes a tilde block.
#[test]
fn parse_all_headings_does_not_close_a_tilde_fence_with_backticks() {
    let content = "# Real\n~~~\n```\n## Phantom\n~~~\n## Also real\n";
    let got: Vec<String> = parse_all_headings(content)
        .into_iter()
        .map(|h| h.text)
        .collect();
    assert_eq!(got, vec!["# Real", "## Also real"]);
}

#[test]
fn parse_all_headings_no_truncation() {
    let mut content = String::from("# Title\n");
    for i in 1..=35 {
        content.push_str(&format!("## Section {i}\ntext\n"));
    }
    let headings = parse_all_headings(&content);
    assert_eq!(headings.len(), 36); // 1 title + 35 sections
}

#[test]
fn parse_all_headings_empty_doc() {
    let headings = parse_all_headings("no headings here\njust text");
    assert!(headings.is_empty());
}

#[test]
fn strip_inline_formatting_backticks() {
    assert_eq!(
        strip_inline_formatting("## The `auth` Module"),
        "## The auth Module"
    );
}

#[test]
fn strip_inline_formatting_bold() {
    assert_eq!(
        strip_inline_formatting("## **Important** Notes"),
        "## Important Notes"
    );
}

#[test]
fn strip_inline_formatting_italic() {
    assert_eq!(
        strip_inline_formatting("## _Setup_ Guide"),
        "## Setup Guide"
    );
}

#[test]
fn strip_inline_formatting_mixed() {
    assert_eq!(
        strip_inline_formatting("## The `auth` **middleware** _layer_"),
        "## The auth middleware layer"
    );
}

#[test]
fn strip_inline_formatting_no_formatting() {
    assert_eq!(
        strip_inline_formatting("## Plain heading"),
        "## Plain heading"
    );
}

#[test]
fn strip_inline_formatting_collapses_spaces() {
    assert_eq!(
        strip_inline_formatting("##  Extra   spaces "),
        "## Extra spaces"
    );
}

#[test]
fn resolve_section_range_exact_match() {
    let content = "# Title\ntext\n## Setup\ndo this\n## Usage\nuse it";
    let range = resolve_section_range(content, "## Setup").unwrap();
    assert_eq!(range.heading_line, 3);
    assert_eq!(range.body_start_line, 4);
    assert_eq!(range.end_line, 4);
    assert_eq!(range.heading_text, "## Setup");
    assert_eq!(range.level, 2);
}

#[test]
fn resolve_section_range_stripped_match() {
    let content = "# Title\n## The `auth` Module\ndetails";
    let range = resolve_section_range(content, "## The auth Module").unwrap();
    assert_eq!(range.heading_text, "## The `auth` Module");
    assert_eq!(range.heading_line, 2);
}

#[test]
fn resolve_section_range_prefix_match() {
    let content = "# Title\n## Authentication Guide\ndetails";
    let range = resolve_section_range(content, "## Auth").unwrap();
    assert_eq!(range.heading_text, "## Authentication Guide");
}

#[test]
fn resolve_section_range_empty_section() {
    let content = "# Title\n## Empty\n## Next\nstuff";
    let range = resolve_section_range(content, "## Empty").unwrap();
    assert_eq!(range.heading_line, 2);
    assert_eq!(range.body_start_line, 3);
    assert_eq!(range.end_line, 2);
}

#[test]
fn resolve_section_range_last_section() {
    let content = "# Title\n## Last\nfinal content\nmore";
    let range = resolve_section_range(content, "## Last").unwrap();
    assert_eq!(range.end_line, 4);
}

#[test]
fn resolve_section_range_last_h2_in_multi_heading_doc() {
    // Bug-shape: 7 headings (1 H1 + 6 H2), last section is "## Resume" with body
    // and trailing blank line. Regression for
    // docs/issues/archive/2026-05-21-edit-markdown-last-heading-unaddressable.md.
    let content = "# Title\n\
                   intro\n\
                   ## Root cause\n\
                   a\n\
                   ## Classification rule\n\
                   b\n\
                   ## Candidates\n\
                   c\n\
                   ## Proposed guard\n\
                   d\n\
                   ## Done log\n\
                   e\n\
                   ## Resume\n\
                   final body\n";
    let headings = parse_all_headings(content);
    assert_eq!(
        headings.len(),
        7,
        "expected 7 headings, got {}: {:?}",
        headings.len(),
        headings.iter().map(|h| h.text.as_str()).collect::<Vec<_>>()
    );
    let range = resolve_section_range(content, "## Resume").unwrap();
    assert_eq!(range.heading_text, "## Resume");
    assert_eq!(range.level, 2);

    // Same query without the `## ` prefix (the form the bug report used).
    let range_stripped = resolve_section_range(content, "Resume").unwrap();
    assert_eq!(range_stripped.heading_text, "## Resume");
}

#[test]
fn resolve_section_range_last_heading_with_unbalanced_code_fence() {
    // Regression for docs/issues/archive/2026-05-21-edit-markdown-last-heading-unaddressable.md.
    // If an earlier batch edit leaves an unmatched ``` fence open, the naive
    // toggle-on-every-``` parser flips in_code_block and hides every heading
    // after the unclosed fence. parse_all_headings now detects unbalanced
    // fences (odd count) and treats them as plain text instead.
    let content = "# Title\n\
                   ## A\n\
                   ```\n\
                   open fence with no close\n\
                   ## Hidden\n\
                   ## Resume\n\
                   tail\n";
    let headings = parse_all_headings(content);
    assert_eq!(
        headings.len(),
        4,
        "expected all 4 headings to be visible despite unbalanced ``` fence, got {:?}",
        headings.iter().map(|h| h.text.as_str()).collect::<Vec<_>>()
    );
    let range = resolve_section_range(content, "## Resume").unwrap();
    assert_eq!(range.heading_text, "## Resume");
}

#[test]
fn resolve_section_range_balanced_code_fence_still_masks_inner_headings() {
    // Don't regress the balanced-fence case: a properly closed code block
    // must still mask `# heading-looking` lines inside it.
    let content = "# Title\n\
                   ## A\n\
                   ```\n\
                   ## Not a real heading\n\
                   ```\n\
                   ## B\n\
                   body\n";
    let headings = parse_all_headings(content);
    let texts: Vec<&str> = headings.iter().map(|h| h.text.as_str()).collect();
    assert_eq!(texts, vec!["# Title", "## A", "## B"]);
}

#[test]
fn resolve_section_range_not_found() {
    let content = "# Title\n## Setup\ntext";
    let err = resolve_section_range(content, "## Nonexistent").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn resolve_section_range_duplicate_heading_error() {
    let content = "# Title\n## Example\nfirst\n## Other\n## Example\nsecond";
    let err = resolve_section_range(content, "## Example").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("2") || msg.contains("multiple"),
        "should mention duplicate count: {msg}"
    );
}

#[test]
fn occurrence_selects_among_identical_headings() {
    // Two byte-identical headings: the exact tiers match both and return before the
    // fuzzy tiers run, and no query string can separate two equal strings anyway.
    // The 1-indexed selector is the only way to reach either.
    // docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md
    let content = "# Title\n## Fix\nfirst\n## Middle\nm\n## Fix\nsecond";

    let first = resolve_section_range(content, HeadingQuery::new("## Fix", Some(1))).unwrap();
    let second = resolve_section_range(content, HeadingQuery::new("## Fix", Some(2))).unwrap();

    // Mutation control: selecting `indices[0]` regardless of `n` collapses these two.
    assert_eq!(first.heading_line, 2, "occurrence=1 is the first '## Fix'");
    assert_eq!(
        second.heading_line, 6,
        "occurrence=2 is the second '## Fix'"
    );
}

#[test]
fn occurrence_past_the_match_count_errors_naming_the_count() {
    let content = "# Title\n## Fix\nfirst\n## Fix\nsecond";
    let err = resolve_section_range(content, HeadingQuery::new("## Fix", Some(5))).unwrap_err();
    let msg = err.to_string();
    // Mutation control: clamping to the last match would return Ok and edit the
    // wrong section silently -- the exact failure this whole change exists to avoid.
    assert!(msg.contains('5'), "should echo what was asked for: {msg}");
    assert!(msg.contains('2'), "should name how many exist: {msg}");
}

#[test]
fn occurrence_zero_is_rejected_rather_than_read_as_the_first() {
    let content = "# Title\n## Fix\nfirst\n## Fix\nsecond";
    let err = resolve_section_range(content, HeadingQuery::new("## Fix", Some(0))).unwrap_err();
    // Mutation control: 0-indexing would resolve this to the first section, so a
    // caller who counted from zero would edit one section while naming another.
    assert!(err.to_string().contains("1-indexed"), "{err}");
}

#[test]
fn occurrence_on_a_unique_heading_still_resolves() {
    let content = "# Title\n## Only\nbody";
    let range = resolve_section_range(content, HeadingQuery::new("## Only", Some(1))).unwrap();
    assert_eq!(range.heading_line, 2);
}

#[test]
fn an_unqualified_duplicate_query_still_refuses_rather_than_guessing() {
    // Adding `occurrence` must not soften the unqualified case into a silent
    // first-match-wins: that is how a caller edits the plan believing they edited
    // the shipped record. The loud refusal is the correct behaviour, not the bug.
    let content = "# Title\n## Fix\nfirst\n## Fix\nsecond";
    let err = resolve_section_range(content, "## Fix").unwrap_err();
    assert!(err.to_string().contains("found 2 times"), "{err}");
}

#[test]
fn the_duplicate_hint_names_occurrence_and_not_edit_file() {
    // The hint used to prescribe `edit_file` with start_line/end_line -- parameters
    // absent from edit_file's schema, on a tool IL-5 refuses for .md before schema
    // validation, and that every librarian-managed artifact refuses on every path.
    // Following it returned "Use edit_markdown for markdown files": a closed loop.
    let content = "# Title\n## Fix\nfirst\n## Fix\nsecond";
    let err = resolve_section_range(content, "## Fix").unwrap_err();
    let rendered = format!("{err:?}");
    assert!(
        rendered.contains("occurrence"),
        "hint must name the remedy that exists: {rendered}"
    );
    assert!(
        !rendered.contains("edit_file"),
        "hint must not send the caller to a closed door: {rendered}"
    );
}

#[test]
fn resolve_section_range_nested_sections() {
    let content = "# Title\n## Parent\nparent text\n### Child\nchild text\n## Sibling\nsibling";
    let range = resolve_section_range(content, "## Parent").unwrap();
    assert_eq!(range.heading_line, 2);
    assert_eq!(range.end_line, 5);
}

#[test]
fn resolve_section_range_bare_query_reaches_the_exact_tier() {
    // Regression for
    // docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
    //
    // FIXTURE DETAIL IS LOAD-BEARING, both properties: the `###` must come BEFORE
    // `## Index` in document order AND contain "Index" as a substring. Drop either and
    // the bare query resolves correctly through tier 4 by accident, leaving this test
    // green and no longer discriminating. That is precisely why the bare-form assertion
    // in `resolve_section_range_finds_last_heading` does not cover this: its fixture has
    // no earlier heading containing "Resume", so tier 4's first-match-wins happens to be
    // right there and wrong here.
    let content = "# Title\n\
                   ## Alpha\n\
                   ### One slug, two spellings - a pattern cannot see the Index\n\
                   subsection body\n\
                   ## Index\n\
                   INDEX-BODY\n";

    let range = resolve_section_range(content, "Index").unwrap();

    assert_eq!(
        range.heading_text, "## Index",
        "a bare query must reach an EXACT tier and bind `## Index`; binding the earlier \
         `###` that merely CONTAINS the word means tiers 1-2 were unreachable and tier 4 \
         picked by document order"
    );
    assert_eq!(range.level, 2);
}

#[test]
fn resolve_section_range_prefixed_query_keeps_exact_level_semantics() {
    // The guard on what the tier-2 fix must NOT cost. Tier 1 compares raw text, so a
    // caller who writes the markers still selects by level and `### Fix` cannot answer a
    // `## Fix` query.
    //
    // MUTATION-CHECKED, not assumed: deleting the tier-1 early return sends this query to
    // the marker-stripping tier 2, which matches BOTH headings and returns the duplicate
    // error instead — so this test observes a real RED when the property it names is
    // broken. Without tier 1 there is nothing else in the cascade that is level-aware.
    let content = "# Title\n## Fix\ntwo-level\n### Fix\nthree-level\n";

    let range = resolve_section_range(content, "## Fix").unwrap();

    assert_eq!(range.heading_text, "## Fix");
    assert_eq!(
        range.level, 2,
        "a `## ` query must not bind the `### ` heading"
    );
}

#[test]
fn resolve_section_range_bare_query_matching_two_levels_is_ambiguous_not_silent() {
    // The behaviour the tier-2 fix deliberately CHANGES, so it is asserted rather than
    // discovered later. Before the fix this query fell to tier 4, matched both headings on
    // substring, and returned `## Fix` by document order with nothing said. Now both match
    // at an exact tier and the caller is told, with both line numbers and the `occurrence`
    // escape — the same contract two byte-identical headings already get.
    //
    // Asserting on the NAMED discriminant, not on the message text: `heading_ambiguous` is
    // the field a caller one frame up reads, and a message-substring assertion would still
    // pass if the extra were dropped and the error silently collapsed into a plain miss.
    let content = "# Title\n## Fix\ntwo-level\n### Fix\nthree-level\n";

    let err = resolve_section_range(content, "Fix").unwrap_err();

    assert_eq!(
        err.extra.get("heading_ambiguous").and_then(|v| v.as_bool()),
        Some(true),
        "a bare query matching two levels must report ambiguity, not silently pick the \
         first in document order: {}",
        err.message
    );
}

#[test]
fn resolve_section_range_heading_in_code_block() {
    let content = "# Title\n```\n## Not a heading\n```\n## Real\ntext";
    let range = resolve_section_range(content, "## Real").unwrap();
    assert_eq!(range.heading_line, 5);
}

#[test]
fn parse_empty_path_returns_empty_segments() {
    assert_eq!(parse_json_path_segments("").unwrap(), Vec::<Segment>::new());
}

#[test]
fn parse_root_only() {
    assert_eq!(
        parse_json_path_segments("$").unwrap(),
        Vec::<Segment>::new()
    );
}

#[test]
fn parse_negative_single_index() {
    assert_eq!(
        parse_json_path_segments("$.a[-1]").unwrap(),
        vec![Segment::Key("a".into()), Segment::NegIndex(1)]
    );
}

#[test]
fn parse_negative_slice_from() {
    assert_eq!(
        parse_json_path_segments("$.a[-3:]").unwrap(),
        vec![Segment::Key("a".into()), Segment::NegSliceFrom(3)]
    );
}

#[test]
fn parse_chained_negative_after_positive() {
    assert_eq!(
        parse_json_path_segments("$.a[0][-1]").unwrap(),
        vec![
            Segment::Key("a".into()),
            Segment::Index(0),
            Segment::NegIndex(1)
        ]
    );
}

#[test]
fn parse_top_level_negative_index() {
    assert_eq!(
        parse_json_path_segments("$[-1]").unwrap(),
        vec![Segment::NegIndex(1)]
    );
}

#[test]
fn parse_rejects_positive_slice() {
    let err = parse_json_path_segments("$.a[1:3]").unwrap_err();
    assert!(
        err.to_string().contains("unsupported json_path segment"),
        "got: {}",
        err
    );
    assert!(err.to_string().contains("[1:3]"), "got: {}", err);
}

#[test]
fn parse_rejects_slice_with_step() {
    let err = parse_json_path_segments("$.a[::2]").unwrap_err();
    assert!(err.to_string().contains("unsupported json_path segment"));
}

#[test]
fn parse_rejects_open_end_positive() {
    let err = parse_json_path_segments("$.a[1:]").unwrap_err();
    assert!(err.to_string().contains("unsupported json_path segment"));
}

#[test]
fn parse_rejects_negative_zero() {
    let err = parse_json_path_segments("$.a[-0]").unwrap_err();
    assert!(err.to_string().contains("[-0]"), "got: {}", err);
    assert!(err.to_string().contains("[0]"), "got: {}", err);
}

#[test]
fn parse_rejects_non_integer_bracket() {
    let err = parse_json_path_segments("$.a[abc]").unwrap_err();
    assert!(err.to_string().contains("[abc]"));
}

#[test]
fn parse_bracket_quoted_key_with_dots() {
    // Bug 2026-07-01: object keys containing '.' are reachable only via
    // quoted bracket syntax. Both quote styles; a quoted numeric string
    // is a Key, not an array Index.
    assert_eq!(
        parse_json_path_segments("$[\"2.1.5\"]").unwrap(),
        vec![Segment::Key("2.1.5".into())]
    );
    assert_eq!(
        parse_json_path_segments("$['1.1']").unwrap(),
        vec![Segment::Key("1.1".into())]
    );
    assert_eq!(
        parse_json_path_segments("$[\"1\"]").unwrap(),
        vec![Segment::Key("1".into())]
    );
}

#[test]
fn parse_bracket_quoted_key_then_field() {
    // Tokenizer must split on '.' only outside brackets.
    assert_eq!(
        parse_json_path_segments("$[\"2.1.5\"].x").unwrap(),
        vec![Segment::Key("2.1.5".into()), Segment::Key("x".into())]
    );
}

#[test]
fn extract_json_path_dotted_string_key() {
    let content = r#"{"1.1": {"a": 1}, "2.1.5": {"x": 10, "y": 20}}"#;
    let (result, type_name, count) = extract_json_path(content, "$[\"2.1.5\"]").unwrap();
    assert!(result.contains("\"x\""));
    assert!(result.contains("\"y\""));
    assert_eq!(type_name, "object");
    assert_eq!(count, Some(2));
}

#[test]
fn parse_rejects_negative_zero_slice() {
    let err = parse_json_path_segments("$.a[-0:]").unwrap_err();
    assert!(err.to_string().contains("[-0:]"), "got: {}", err);
    assert!(err.to_string().contains("[0]"), "got: {}", err);
}

#[test]
fn parse_rejects_positive_sign() {
    let err = parse_json_path_segments("$.a[+1]").unwrap_err();
    assert!(err.to_string().contains("[+1]"), "got: {}", err);
    assert!(
        err.to_string().contains("unsupported json_path segment"),
        "got: {}",
        err
    );
}

#[test]
fn extract_root_returns_parsed() {
    let (content, ty, count) = extract_json_path(r#"{"a":1}"#, "$").unwrap();
    assert!(content.contains("\"a\""), "got: {}", content);
    assert_eq!(ty, "object");
    assert_eq!(count, Some(1));
}

#[test]
fn extract_top_level_negative_index() {
    let (content, ty, _) = extract_json_path(r#"["a","b","c"]"#, "$[-1]").unwrap();
    assert_eq!(content, "c");
    assert_eq!(ty, "string");
}

#[test]
fn extract_negative_index_returns_last_element() {
    let (content, ty, _) = extract_json_path(r#"{"items":["a","b","c"]}"#, "$.items[-1]").unwrap();
    assert_eq!(content, "c");
    assert_eq!(ty, "string");
}

// ── json_path `[*]` wildcard projection ──────────────────────────────────────
//
// The overflow envelope tells callers to recover a buffered result with
// `read_file("@tool_xyz", json_path="$.field")`. But the results that actually
// overflow are overwhelmingly *arrays of records*, where the useful projection is
// "this field from every element" — and `[*]` was rejected. Measured 2026-08-15
// across 13 usage.db files: `[*]` was 22 of 30 rejected segments (73%), and agents
// fell back to shelling out.
// docs/issues/archive/2026-08-15-jsonpath-subset-defeats-the-overflow-recovery-hint.md

#[test]
fn wildcard_projects_a_field_from_every_element() {
    let content = r#"{"observations": [{"id": "T-1", "n": 1}, {"id": "T-2", "n": 2}]}"#;
    let (result, ty, count) = extract_json_path(content, "$.observations[*].id").unwrap();
    assert_eq!(ty, "array");
    assert_eq!(count, Some(2));
    assert!(result.contains("T-1"), "got: {result}");
    assert!(result.contains("T-2"), "got: {result}");
    assert!(
        !result.contains("\"n\""),
        "only the named field projects: {result}"
    );
}

#[test]
fn wildcard_works_on_a_root_array() {
    let content = r#"[{"name": "a"}, {"name": "b"}]"#;
    let (result, ty, count) = extract_json_path(content, "$[*].name").unwrap();
    assert_eq!(ty, "array");
    assert_eq!(count, Some(2));
    assert!(
        result.contains('a') && result.contains('b'),
        "got: {result}"
    );
}

#[test]
fn trailing_wildcard_yields_the_elements_themselves() {
    let content = r#"{"items": ["x", "y", "z"]}"#;
    let (result, ty, count) = extract_json_path(content, "$.items[*]").unwrap();
    assert_eq!(ty, "array");
    assert_eq!(count, Some(3));
    assert!(
        result.contains('x') && result.contains('z'),
        "got: {result}"
    );
}

#[test]
fn nested_wildcards_project_through_both_levels() {
    let content = r#"{"groups": [{"rows": [{"v": 1}, {"v": 2}]}, {"rows": [{"v": 3}]}]}"#;
    let (result, ty, _) = extract_json_path(content, "$.groups[*].rows[*].v").unwrap();
    assert_eq!(ty, "array");
    // Nesting is preserved rather than flattened: [[1,2],[3]]. Flattening would
    // lose which group a value came from, which is the question a grouped
    // projection is usually asked to answer.
    assert!(
        result.contains('1') && result.contains('3'),
        "got: {result}"
    );
    assert!(
        result.matches('[').count() >= 3,
        "expected nested arrays: {result}"
    );
}

#[test]
fn wildcard_on_a_non_array_says_so() {
    let content = r#"{"cfg": {"a": 1}}"#;
    let err = extract_json_path(content, "$.cfg[*]").expect_err("[*] needs an array");
    let msg = format!("{err}");
    assert!(
        msg.contains("array") || msg.contains("object"),
        "got: {msg}"
    );
}

/// A projection that silently dropped rows would be the same defect class as the
/// self-refuting "Showing N of N" and the unmarked buffered summary: a short result
/// that reads as complete. So a missing key fails loudly and names the element.
#[test]
fn wildcard_names_the_element_when_a_key_is_missing() {
    let content = r#"{"rows": [{"id": "a"}, {"other": 1}]}"#;
    let err = extract_json_path(content, "$.rows[*].id")
        .expect_err("missing key must not be silently dropped");
    let msg = format!("{err}");
    assert!(
        msg.contains('1'),
        "the error must name which element failed: {msg}"
    );
}

#[test]
fn the_unsupported_segment_hint_advertises_the_wildcard() {
    let err = extract_json_path(r#"{"a":[]}"#, "$.a[1:3]").expect_err("slices stay unsupported");
    let msg = format!("{err}");
    assert!(
        msg.contains("[*]"),
        "the rejection hint must name the form that IS supported, since that is \
         where the grammar is discovered: {msg}"
    );
}

#[test]
fn extract_negative_slice_returns_tail() {
    let (content, ty, count) =
        extract_json_path(r#"{"items":["a","b","c","d"]}"#, "$.items[-2:]").unwrap();
    assert!(content.contains("\"c\""));
    assert!(content.contains("\"d\""));
    assert!(!content.contains("\"a\""));
    assert_eq!(ty, "array");
    assert_eq!(count, Some(2));
}

#[test]
fn extract_negative_index_oob_returns_clear_error() {
    let err = extract_json_path(r#"{"items":["a"]}"#, "$.items[-5]").unwrap_err();
    assert!(err.to_string().contains("out of bounds"), "got: {}", err);
    assert!(err.to_string().contains("length 1"), "got: {}", err);
}

#[test]
fn extract_negative_slice_oob_returns_clear_error() {
    let err = extract_json_path(r#"{"items":["a"]}"#, "$.items[-5:]").unwrap_err();
    assert!(err.to_string().contains("out of bounds"));
    assert!(err.to_string().contains("length 1"));
}

#[test]
fn extract_mid_path_slice_then_index() {
    let (content, ty, _) = extract_json_path(
        r#"{"items":[{"v":1},{"v":2},{"v":3}]}"#,
        "$.items[-2:][0].v",
    )
    .unwrap();
    assert_eq!(content, "2");
    assert_eq!(ty, "number");
}

#[test]
fn extract_unsupported_syntax_distinguished_from_not_found() {
    let err = extract_json_path(r#"{"items":["a"]}"#, "$.items[1:3]").unwrap_err();
    assert!(
        err.to_string().contains("unsupported json_path segment"),
        "got: {}",
        err
    );
    assert!(!err.to_string().contains("not found"), "got: {}", err);
}

#[test]
fn extract_markdown_section_reports_the_heading_it_bound() {
    // The read-path half of the disclosure fix:
    // docs/issues/2026-09-03-a-bare-heading-query-cannot-reach-the-exact-match-tiers.md
    // `doc(action="get")` builds body_meta.heading from the caller's REQUEST, so a fuzzy
    // bind is invisible there while `read_file` shows it. Exposing the bound heading on
    // SectionResult is what lets get.rs report the resolved value instead of the query.
    //
    // FIXTURE DETAIL IS LOAD-BEARING: "slug" matches no heading exactly, so it binds
    // through tier 4 and the bound heading DIFFERS from the query. If they coincide the
    // assertion is satisfied by echoing the request and tests nothing.
    let content = "# Title\n## Alpha\n### One slug, two spellings\nbody\n## Index\nindex body\n";

    let r = extract_markdown_section(content, "slug").unwrap();

    assert_eq!(
        r.heading_text, "### One slug, two spellings",
        "the result must name the heading actually bound, so a caller can tell a fuzzy \
         bind from an exact one"
    );
}
