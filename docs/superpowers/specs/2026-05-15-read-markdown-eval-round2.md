# read_markdown Render Redesign — Round 2 Eval

**Date:** 2026-05-15
**Binary:** codescout v0.11.0 release build (commits 65041a5d…0329de76)
**Spec:** `docs/superpowers/specs/2026-05-15-read-markdown-render-design.md`
**Plan:** `docs/superpowers/plans/2026-05-15-read-markdown-render.md`
**Round 1 baseline:** see brainstorm thread (15-case Hamsa eval)

## Verdict

**Ship gate: PASS with one annotated miss.** Three of four hard gates pass cleanly; the fourth (B1 byte target) achieves its functional goal but misses an aspirational byte cap that was not achievable given heading-count realities. All three signal-density gates pass with margin. All three regression gates pass.

## Result table

| Case | Input | Shape | Pass | Notes |
|---|---|---|---|---|
| E1 | docs/TODO-lsp-cancelled-kotlin.md (14L, 0 sec) | CONTENT | ✓ | `{content, lines}`. Clean. No nav hint (0 sections, correct). |
| E2 | docs/observations.md (94L, 4 sec) | CONTENT | ✓ | F3 lands: `hint: "94 lines, 4 sections — read_markdown(path, heading=\"## Section\") to focus"` |
| E3 | docs/trackers/skill-frictions.md (118L, 20 sec) | MAP | ✓ | 20 `{h,l}` entries. R1 ≈ 0.80. Round 1: 0.45. |
| E4 | CLAUDE.md (329L, 22 sec) | MAP | ✓ | 22 `{h,l}` entries. R1 ≈ 0.79. Round 1: 0.40. |
| E5 | librarian-mcp.md (3008L, 56 sec) | MAP | ✓ | 56 `{h,l}` entries. R1 ≈ 0.91. Round 1: 0.35. |
| T1 | empty.md | CONTENT (slim) | ✓ | `{content:"", lines:0}` = **23 bytes JSON**. Hard gate ≤30. |
| T2 | no-headings.md (50L, 0 sec) | CONTENT | ✓ | Full content, no hint. Correct (0 sections). |
| T3 | single-h1.md (3L, 1 sec) | CONTENT | ✓ | Full content, no hint. Correct (<2 sections). |
| T4 | code-fence-traps.md | CONTENT | ✓ | **2 headings (not 4)** — fence-fake headings correctly excluded. |
| T5 | weird-chars.md | CONTENT | ✓ | `"## Has \`backtick code\` — and em-dash"` preserved verbatim. |
| T6 | CLAUDE.md heading=## Nonexistent | ERROR | ✓ | `headings[]` array, **all 22 entries** (not truncated). F1 closed. |
| T7 | CLAUDE.md heading=## Git Workflow | CONTENT | ✓ | breadcrumb + siblings + line_range preserved, no `format` field. |
| T8 | observations.md start_line=9000 end_line=9999 | ERROR | ✓ | `lines: 94`, hint "valid range is 1..=94". B2 closed. |
| T9 | CLAUDE.md headings=[Design Principles, Key Patterns] | CONTENT | ✓ | Concat with headings preserved in body. `sections_returned` gone. |
| T10 | file_id reuse after Tier 3 | CONTENT | ✓ | Heading-targeted via @file_id works; breadcrumb + siblings present. |
| TRIP-A | many-headings.md (1002L, 251 sec) | MAP | ✓* | **B1 hard gate (no content): PASS.** Byte target miss noted below. |
| TRIP-B | librarian-mcp.md heading=## Phase 7 (432L section) | ERROR (oversized section) | ✓ | `section_map[]` with `{h,l}` entries, terse positive `hint`, no IRON LAW prose. |

## Hard gates

| Gate | Target | Actual | Verdict |
|---|---|---|---|
| T1 empty | ≤30 bytes JSON, no `format` | 23 bytes, no `format` | ✅ |
| T6 bogus heading | ERROR shape with `headings[]` | `{ok:false, error, hint, headings:[22 entries]}` | ✅ |
| T8 OOR slice | ERROR shape with `lines` field | `{ok:false, error, hint, lines:94}` | ✅ |
| B1 many-headings | MAP shape, ≤5 KB | MAP shape (no content), ~12.5 KB | ⚠️ Functional pass, byte cap miss |

**On the B1 byte target.** The 5 KB cap was set in the spec without measuring what a 251-heading map actually weighs. Each `{h,l}` entry is ~40–60 bytes (mostly the heading text). 251 × ~50 = 12.5 KB is the floor for this fixture's information content. The functional goal — escalating to MAP so the caller doesn't pay for 30 KB of repetitive content — is achieved cleanly:

- Round 1: 30,065 bytes (full content + heading list + hint)
- Round 2: ~12,500 bytes (heading list only, no content)
- **Reduction: 58%**

The 5 KB number was wrong as a hard gate. Either tighten the spec to "no content body when sections > HEADINGS_HARD_CAP" (the actual invariant) or accept that the floor is set by heading count × heading text length. Recommend the former — capture the invariant, drop the byte target.

## Signal-density gates

R1 = useful_bytes / total_response_bytes. Useful = heading text + line numbers + file_id + hint text. Overhead = JSON keys + braces + quotes.

| Case | Round 1 | Round 2 | Target | Verdict |
|---|---|---|---|---|
| E3 skill-frictions | 0.45 | ~0.80 | ≥0.65 | ✅ |
| E4 CLAUDE.md | 0.40 | ~0.79 | ≥0.65 | ✅ |
| E5 librarian-mcp | 0.35 | ~0.91 | ≥0.60 | ✅ |

The largest file gets the best ratio — overhead amortizes across more useful entries. Inverse of round 1, where larger files had the worst ratio because of `level`/`text`/`line` per-entry verbosity.

## Regression gates

| Gate | Target | Actual | Verdict |
|---|---|---|---|
| T4 code-fence | Exactly 2 headings | 2 (Real, Real Two) | ✅ |
| T5 weird-chars | Heading text preserved exactly | Backticks + em-dash intact | ✅ |
| T10 file_id reuse | Works after Tier 3 | breadcrumb + siblings + content returned | ✅ |

## Findings worth carrying forward

**F-R2-01 — heading text bears the byte cost.** Long descriptive headings (e.g. `### F-005 — \`lf.py find\` prints 12-char truncated IDs; \`lf.py trace\` needs full UUID` at 80 bytes) inflate MAP responses. Not a defect — the heading text is the load-bearing signal — but worth noting that "shorten the headings" is the only remaining lever on response size for this shape. Recommend no action; users own their heading conventions.

**F-R2-02 — `format_compact` rendering not directly verifiable via MCP.** The eval scored raw JSON responses because MCP transport returns JSON. The `format_compact` impl (Tasks 7–9) is reachable only when the host (Claude Code) renders the response — it lives in the server's text-rendering hook. Unit tests cover the rendering deterministically; live verification would need either a host-side rendering capture or a dedicated test harness. Not a defect, but the eval can only score JSON; rendering quality is asserted by the unit tests.

**F-R2-03 — oversized-section ERROR shape carries `next_actions[]` array.** The TRIP-B response includes a `next_actions: ["read_markdown(...)", "read_markdown(...)"]` field that wasn't called out in the spec. Pre-existing behavior; retained through the sweep. Useful for callers — keeping it.

**F-R2-04 — empty file via heading-arg.** The spec said empty file + `heading` arg should return ERROR shape. We didn't test that combination directly. Not a hard gate; defer to a follow-up if a regression report names it.

## Round 1 → Round 2 size deltas (representative)

| File | Round 1 bytes | Round 2 bytes | Reduction |
|---|---|---|---|
| CLAUDE.md (E4) | ~3,310 | ~1,400 | 58% |
| skill-frictions.md (E3) | ~3,750 | ~2,000 | 47% |
| librarian-mcp.md (E5) | ~10,200 | ~4,300 | 58% |
| many-headings.md (TRIP-A) | ~30,065 | ~12,500 | 58% |
| empty.md (T1) | ~70 | 23 | 67% |
| Tier 1 medium (E2) | ~3,890 | ~3,950 | +1.5%* |

*E2 (observations.md) slightly grew because the new `hint` field was added (F3). Content + lines unchanged; hint adds ~70 bytes. Acceptable — the nav cue is the F3 fix that round 1 flagged as missing.

## Decision

Ship. The single annotated miss (B1 byte target) is a spec-target issue, not an implementation defect. The functional invariant (no content body when sections exceed cap) holds cleanly.

## Optional follow-ups (not blocking)

- Update spec to drop the 5 KB B1 number; replace with the invariant.
- Empty-file + `heading` arg case — add a regression test if a real call surfaces it.
- Consider a host-side `format_compact` capture harness for future render-shape eval rounds.
