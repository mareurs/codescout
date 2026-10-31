# Setup A/B, Stage 4: Live Rust tasks, loader and control gate (design)

Date: 2026-10-31. Extends `2026-10-27-codescout-setup-ab-design.md`, which stays the binding authority for the arms, the integrity rules, the cost rules and the strata. This document adds one task source.

## Why

The first study used 12 tasks. Every agent passed almost every task, so the pass rate could not separate the arms (see `docs/research/2026-10-31-setup-ab-stage3.md`). The SWE-bench Multilingual Rust pool has no large tasks: its largest gold patch is 158 lines. A harder pool is needed.

## Source

`SWE-bench-Live/MultiLang`, Rust split: 171 records, 55 repositories, creation dates 2025-06 to 2026-07. The records are saved at `/home/marius/work/claude/setup-ab-data/live/multilang-rust.jsonl` (sha256 `bf2dd8252a07d0f275663ef8f6ab87571dd9bc4ce386abf9ec98d3cd6bb51a81`).

Each record carries a prebuilt Docker image (`docker_image`, registry `starryzhang/`), `base_commit`, `patch`, `test_patch`, `FAIL_TO_PASS`, `PASS_TO_PASS`, `test_cmds`, `print_cmds` and `log_parser` (Python source that maps a log to `{test name: "pass"|"fail"|...}`).

## Decisions

- **After the cutoff.** Only records with `created_at >= 2026-07-01` are eligible. The models' knowledge cutoff is June 2026.
- **Metadata-only rule.** The selection rule reads repository, patch size, file count and creation date. It never reads an outcome. It lives in `live-selection-rule.json` and is committed before any control or run is read.
- **The control gate decides, not belief.** A task is usable only if, in its own image reset to `base_commit`: every `FAIL_TO_PASS` test fails with the test patch alone, and every `FAIL_TO_PASS` and `PASS_TO_PASS` test passes with the gold patch and the test patch. This is an instrument gate, not a model outcome.
- **The image is not trusted.** Probe of `h4ckf0r0day__obscura-426` (2026-10-31): the image tree held the gold fix uncommitted, an old `reports/nextest-results.jsonl`, and extra uncommitted test-code edits. With the record's patches alone, 71 of 330 expected tests did not pass. The loader resets to `base_commit`, cleans, and rejects such tasks at the control gate.
- **The parser is untrusted code.** `log_parser` runs in a subprocess with a timeout. A failure is a failed grade.
- **Disk.** One image is about 14 GB unpacked, with about 97 GB free. Controls pull one image at a time, refuse to pull below 40 GB free, and remove an image they pulled.
- **Scope of this stage.** The loader and the control gate only. The agent image, the prompt, the leak scan, the registration and the screening runs are a separate stage, planned after the controls show which tasks survive.

## Out of scope

Any model call, any spend, any change to `run.py`, `arms.py`, `freeze_stage2.py` or the frozen registration.
