# Setup A/B, Stage 5: fresh-task builder and control (design)

Date: 2026-11-04. Extends `2026-11-04-setup-ab-daily-setup-arm-design.md` (operator validated D1-D5 and approved
"build and control" on 2026-11-04; cost is a soft cap, the run takes priority) and
`2026-10-31-setup-ab-live-tasks-design.md` (the control gate). Evidence for every fact below:
memory `research/setup-ab-eval-fresh-builder-pilot` and `/home/marius/work/claude/setup-ab-data/fresh-pilot-2026-11-04/`.

## Why

No prebuilt image exists for tasks created after 2026-07-29 (`SWE-bench-Live/MultiLang` Rust split: 171
records, unchanged). The controlled tasks that remain are one-file fixes. A throwaway pilot turned
rust-analyzer PR #23096 into a controlled task with no prebuilt image: strict gate passed at the frozen
8 GB limit, F2P 3, P2P 8,146, about 5 minutes of build and 10 GB of disk per task.

## Scope

One stdlib-only module, `tools/setup-ab-eval/fresh_build.py`, with tests. It turns (repository, PR number,
issue numbers, base image) into: a record in the live-record schema, a local image, and a `control.json`
whose verdict comes from the strict gate. It spends no model tokens. It does not touch `run.py`, `arms.py`,
`multilang_control.py`, `multilang_tasks.py` or any frozen registration; it imports from them.

## Decisions

- **Base commit.** The first parent of the PR's merge commit (a merge commit has two parents). The diff
  from that parent to the merge commit must equal the PR's own changed-file list, or the builder refuses.
- **Patch split.** Paths with any component containing `test` or `e2e` (case-insensitive: `(^|/)[^/]*(test|e2e)[^/]*(/|$)`) form `test_patch`; the rest form `patch`. Ruling 2026-11-04 after the Task 1 review: this reproduces the dataset's own patch/test split on all 1,259 files of the 171 saved records with 0 mismatches, while the first draft (`tests?/`, `testdata/`, `fixtures/`, `_tests?.rs`) misplaced 68 files in 38 records, including 4 of 12 rust-analyzer records (`test_data/`, `macro_expansion_tests/`), whose misplaced test code would have made F2P tests impossible or P2P tests fail for correct fixes. The rule is applied as is to every repository; there is no per-repository fallback or exception list (a fresh PR has no dataset split to compare with, so the 0-mismatch figure above is the only evidence, and a wrong split shows up later as an F2P test that cannot pass or a P2P test that fails for a correct fix, which the control gate and the specification verdict then catch). Line endings are never altered (CRLF blobs exist).
- **Image.** Built from an existing task image of the same repository (warm `target/`, toolchain). The
  tree is swapped to the upstream base; leftovers of crates absent from the new tree are removed; every
  tracked file is touched so cargo cannot run stale binaries; the workspace must load
  (`cargo metadata`); the warm build runs WITH network (the image bakes `CARGO_NET_OFFLINE=true`; override
  it for that step only), its true exit code is recorded, and it must print `Compiling` lines. The
  synthetic base commit's SHA is the record's `base_commit`; the upstream SHA is kept as
  `upstream_base_commit`. A build refuses to start under 60 GB free disk.
- **Derived sets.** F2P = tests that pass with gold+test patch and do not pass at base+test patch
  (failing or absent); P2P = tests that pass in both. Three offline states: base+test, gold+test, and a
  gold repeat. A test whose status differs between the two gold runs is re-run alone; only a test that
  still differs fails the gate (the `--nocapture` interleaving of two tests loses a line of the parser, and
  one such test failed the strict repeat at 24 GB while passing).
- **Gate memory.** The gate runs at `--memory 8g`, the frozen grader limit, `--network none`,
  `--cpus 4`, because a gate passed at a larger limit proves nothing about the grader.
- **Specification rule (operator approved 2026-11-04, decided before any model run).** An F2P test is kept
  only if the issue states the behaviour the test asserts. A pretty-printer or snapshot assertion the issue
  never mentions is dropped (pilot: 2 of 3 F2P of rust-analyzer#23096). The task is usable only if at least
  one F2P test remains. The verdict per test is made outside the builder (an LLM read plus a manual read of a
  sample), cached on `sha256(issue text, test patch, gold patch)`, and consumed from a `spec-verdicts.json`
  file; an F2P test with no verdict is an error, never a silent keep. The builder records the dropped tests
  and the rule in the record.
- **Output layout** (so the existing loaders can later be pointed at it): `<data>/records.jsonl` (one JSON
  line per task, the schema of `multilang-rust.jsonl`), `<data>/controls/<id>/control.json`, and
  `<data>/evidence/<id>/` for logs, patches and the issue JSON.
- **Repositories (operator approved).** rust-analyzer first (base image reused, about 10 GB per task), then
  nushell; zellij or uv only with disk to spare. Disk, not money, is the limit (207 GB free, 89% used).
- **Never rebuild.** Like onboarding, a built image and its control are cached on
  `(repo, upstream base SHA, base image id, builder version)`; a matching key skips the work.

## Out of scope

Onboarding generation, arms, the leak scan, the registration, any model call, and the spec-verdict reader
itself. The pool choice and the registration are later stages.
