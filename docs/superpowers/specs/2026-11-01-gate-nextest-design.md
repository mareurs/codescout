# Gate and CI test lanes on nextest — design

Date: 2026-11-01. Status: draft for approval. Measurements: codescout memory `research/gate-speed-20261101`.

## Intent

The gate's two test lanes take most of its time: in the last full run, LEAN=272 s and DEFAULT=1517 s of TOTAL=2007 s. The cost is the runner, not the tests: on this 64-core machine the default lib tests take 72–80 s under nextest and 369–575 s under libtest, because libtest's one shared process spends about 89% of its CPU in the kernel. The operator chose (2026-11-01) to move the gate AND CI to nextest, so that both run the tests the same way, and to remove the unoptimised hash that makes about 26 tests cost ~10 s each under nextest.

## Goals

- `scripts/gate.sh` runs both test lanes with `cargo nextest run`, and runs the doctests that nextest does not run.
- Every CI job that runs the Rust test suite does the same.
- `sha2` is optimised in the dev profile.
- The gate's guards keep working: they must still be able to fail for the reason they name.

## Non-goals

- Fewer lanes. The lean lane stays: it catches failures that occur only under `cfg`, and under nextest it costs about 55 s.
- Targeted runs (`scripts/with-slot.sh cargo test --lib -- <filter>`) and `scripts/mutation-probe.sh`. Both keep libtest.
- `TMPDIR` on tmpfs (option C). Not measured; a separate decision.

## Anti-goals

- **Retries.** nextest can retry a failed test. A retry turns a flaky red into a green and hides it. No profile sets `retries`.
- **A green that means less than before.** Doctests must still run, and the feature-lane guard must still see which features' tests run.

## Decisions

1. **Gate commands.** The lean lane becomes `cargo nextest run --workspace --no-default-features --no-fail-fast`, then `cargo test --workspace --doc --no-default-features`. The default lane becomes `cargo nextest run --workspace --no-fail-fast`, then `cargo test --workspace --doc`. A lane is red when either of its two commands is red. The lanes keep their names, their order and the `;` rule (gate-ordering.md).
2. **`--no-fail-fast`.** My call. nextest's default stops at the first failed test. The old `cargo test` stopped only at the first failed binary, and it ran every test in that binary. `--no-fail-fast` keeps every failure visible, and it costs time only on a red run.
3. **nextest is required.** At start, `gate.sh` checks `cargo nextest --version`. If nextest is missing, the gate exits 2 and prints the install command. It never falls back to libtest, because a silent fallback would make two machines run different gates.
4. **CI.** The `test` matrix job (3 OS × 3 configs) installs nextest with `taiki-e/install-action`, version pinned, and runs `cargo nextest run --workspace ${{ matrix.config.flags }}` plus the matching `cargo test --workspace --doc ${{ matrix.config.flags }}`. `test-server-stack` and the `dashboard` test step in `feature-check` change the same way.
5. **Feature-lane guard.** `tests/feature_lanes.rs` counts a feature as tested only on a line that contains `cargo test`. It must also count `cargo nextest run`. Its fixture test gets a `cargo nextest run --features …` line, so the new arm is proved against an input and not only against the live workflows.
6. **Pinned command lists.** `CLAUDE.md`'s gate line, the order test `claude_md_gate_lists_its_four_commands_in_the_load_bearing_order` (`src/prompts/mod.rs`) and `tests/test_python_runner.py` change together. `CLAUDE.md` has 28 bytes free under `CLAUDE_MD_BYTE_BUDGET` (5600), so the new line must cost no more bytes than it adds; the doctest detail goes to gate-ordering.md.
7. **`sha2`.** `[profile.dev.package.sha2] opt-level = 3` in `Cargo.toml`.
8. **No nextest config file at first.** The defaults are enough; a `.config/nextest.toml` comes only with a measured reason.

## Risks and what is known about them

- **Process per test.** A test that depends on state another test left in the same process would pass under libtest and fail under nextest, or the other way round. Evidence: 8192/8192 passed in three nextest runs on Linux. Not known: macOS and Windows; CI's first run is that evidence.
- **CI gain is not measured.** CI runners have few cores, and libtest's contention grows with the thread count. CI may get much less than this machine's 5×. The reason for the CI change is that gate and CI run the same runner.
- **A new tool in the supply chain.** nextest is installed from a pinned version in CI and by the developer locally.

## Left to the builder

The CI install step's exact form and version, the wording of the gate's missing-nextest message, and how the gate-ordering.md text is rewritten.

## Verification

- The full gate on this machine, with the lane times compared to TOTAL=2007 / LEAN=272 / DEFAULT=1517 (state the load).
- The time of `tools::config::tests::project_status_compact_shape` alone under nextest, before (12.0 s) and after the `sha2` change.
- A CI run with every job green, before the change is merged.
