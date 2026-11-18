# Nushell semantic contract: offline diagnosis

Read-only analysis of preserved source, patches and saved logs. No new model, Docker, native test or grading run. No source change. Parent owns experiment/process interpretation and issue filing.

## Finding

All three candidates reject a stored stream if **any** row is an error. The reference contract rejects it only when the retained result is **entirely** errors. This single quantifier difference explains the same saved private-test failure in every arm. It does not require a new execution to diagnose: the saved logs expose the failing value, and each patch visibly disables the only store call when the mixed result contains its error row.

Exact minimal preserved witness (not an invented example):

```nu
# Interactive session; capture budget 1 MiB
[{name: 'ab'} {name: 1}]
$ans.last | str length name
```

Expected retained payload is a two-element list: first row `{name: 2}`, second row `Value::Error`. All candidates retain the original two records instead. The first disputed value is observed directly in every saved log: `left: String("ab")`, `right: Int(2)`, assertion at injected `crates/nu-cli/tests/last_result.rs:900:5`. This is not a count/parser-only inference.

Evidence:

- `/home/marius/work/claude/setup-ab-data/fresh-tasks/nushell__nushell-18863/build/record.json`, field `test_patch`, added `mixed_str_length_result_does_replace_last` (target source lines 883-907; assertion line 900), and field `PASS_TO_PASS` containing that test.
- `/home/marius/.cache/codescout-resident/codescout/semantic-contract-policy-v2-20261107/saved-grades/nushell__nushell-18863--low-control/grader/low-control-ebdab4c5/grade.log:3729-3734`.
- `/home/marius/.cache/codescout-resident/codescout/semantic-contract-policy-v2-20261107/saved-grades/nushell__nushell-18863--low-contract/grader/low-contract-06a66648/grade.log:3729-3734`.
- `/home/marius/.cache/codescout-resident/codescout/semantic-contract-policy-v2-20261107/saved-grades/nushell__nushell-18863--xhigh-control/grader/xhigh-control-8cdfae4a/grade.log:3729-3734`.

## Exact causal path

Below, B is `/home/marius/work/claude/setup-ab-data/fresh-tasks/nushell__nushell-18863/build/baseline-tree/testbed`.

1. `B/crates/nu-command/src/strings/str_/length.rs:137-176`: `str length` passes its cell paths to `operate`; the string `ab` maps to integer 2; a non-string produces `Value::Error`.
2. `B/crates/nu-cmd-base/src/input_handler.rs:38-86`: with cell paths, `operate` updates the specified cell; update failure becomes an error row. `B/crates/nu-protocol/src/pipeline/pipeline_data.rs:480-529` maps a list through an iterator into pipeline data; scalar mapping errors can instead return `Err`. This explains why the mixed-list case reaches stream storage, rather than the thrown-error path.
3. `B/crates/nu-cli/src/util.rs:371-414` captures before display. Its baseline `store_list_stream_prefix` at 465-527 collects the list and unconditionally calls `store_last_result_raw` at 518. Thus the mixed transformed payload was already stored by the base, even when display encountered the error row.
4. Candidate root C is `/home/marius/.cache/codescout-resident/codescout/semantic-contract-runtime-20261106/prepared/arms`:
   - `C/nushell__nushell-18863--low-control/candidate.patch:40-66`: set `has_error` upon any error and skip store if set. It continues collecting until budget exhaustion rather than breaking at the first error.
   - `C/nushell__nushell-18863--low-contract/candidate.patch:39-68`: set `has_error`, retain that item for printing, break, then skip store.
   - `C/nushell__nushell-18863--xhigh-control/candidate.patch:29-59`: equivalent early break with `failed` and `item.is_error()`.
   - Low-control and low-contract also reject a direct `PipelineData::Value(Value::Error)`; xhigh does not change that branch. This difference is not the cause of the common mixed-stream failure.

## Is the failing private test legitimate?

Yes as a preservation test of behavior already present in base source, and it agrees with gold. It is not explicitly specified by the public issue alone. The issue reports all-invalid record input and says fetching `$ans` should not be broken; it never discusses partially successful transformations. The hidden witness therefore supplies a sharper boundary than the prose, but not a new storage behavior invented after the repair.

The supplied two F2P tests assert preserving the original all-invalid payload and keeping `$ans` printable, with and without the expanded display hook. These do not imply that partially successful data must be discarded. The mixed P2P test asks to retain a meaningful transformed row along with an error row, a compatible requirement. Gold also makes nested errors printable, resolving the apparent tension between retaining partial results and readable `$ans`.

Evidence: record fields `problem_statement`, `FAIL_TO_PASS`, `PASS_TO_PASS`, `test_patch`, `spec_rule`; baseline storage and gold spans above/below. `spec_rule` is only `f2p kept only if the issue states the asserted behaviour`; it does not establish an enforcement mechanism for every preservation requirement.

## Test visibility

The exact mixed witness was **not an actor-visible upstream test**. It is an added test in the grader-injected `record.json.test_patch`. It is P2P because it passes on the unfixed baseline; P2P does not mean its source existed in the actor checkout.

The host baseline test file ends at line 834 and has no mixed witness. The prescan export also has no symbol of that name. Prescan export HEAD is `5ec462b99bb0773843538cb545a741db84d0da92`, equal to all actors' `candidate-capture.json.baseline_commit`. The baseline and prescan copies of `crates/nu-cli/tests/last_result.rs` hash identically (`ca6c83db42f9c193cfce39000f2064066559456612b0d00620eba66719f78344`); their util.rs copies also match (`80d07517c078829f1b354154767f78683eb433ccde16b8bf6cbed355eb8ef00f`). Task `build/image-ctx/Dockerfile:7-34` resets to record base `b1341d87305292decafa604a4eee8a153d013f8f`, creates an orphan benchmark commit and checks tree identity. This accounts for differing task/actor commit IDs without a semantic source difference.

Visible nearby tests establish ordinary transformed-result replacement (`B/crates/nu-cli/tests/last_result.rs:206-212`, `[10 20 30]` then `$ans.last | first`) and thrown-error preservation (`224-230`, `99` then `error make {msg: boom}`). They do not pin their mixed boundary. Candidate-added tests also miss that boundary: low-contract patch 86-103 checks all-invalid input plus a wholly successful `length`; xhigh patch 78-110 checks all-invalid lists and a single invalid record.

## Truth table

Each row is a fresh Interactive test session with 1 MiB capture. “Prior” means the payload from the seed command. E is a `Value::Error`. Candidate columns concern retained `$ans.last`, not whether the current command prints an error. Except the mixed first-field mismatch (directly observed in all saved logs), entries are source-derived behavior for preserved test inputs, not fresh executions.

| Preserved test input, in order | Desired / gold retained payload | Low control | Low contract | Xhigh control |
|---|---|---|---|---|
| `99`; `error make {msg: boom}` | `99` | `99` | `99` | `99` |
| `[{name: a}]`; `$ans.last \| str length` | Prior `[{name: a}]` | Prior | Prior | Prior |
| `[{name: a}]`; `1..2 \| each {\|i\| {name: $i}} \| str length` | Prior `[{name: a}]` | Prior | Prior | Prior |
| `[{name: 'ab'} {name: 1}]`; `$ans.last \| str length name` | `[{name: 2}, E]` | Prior: `[{name: 'ab'} {name: 1}]` | Same prior | Same prior |
| `[10 20 30]`; `$ans.last \| first` | `10` | `10` | `10` | `10` |
| `42`; `null` | `nothing` | `nothing` | `nothing` | `nothing` |

Inputs come from record.test_patch and `B/crates/nu-cli/tests/last_result.rs:206-230`. The stream-only error test remains in test_patch but is listed in record.dropped_f2p, so do not count it among the two asserted F2P acceptance tests.

## What gold does differently

Full gold patch read: `/home/marius/work/claude/setup-ab-data/fresh-tasks/nushell__nushell-18863/build/gold.patch`, all 220 lines.

- At patch lines 43-56 it introduces `value_is_error_only`: true for an error value or a **nonempty list whose every row is an error**; false for mixed lists and empty lists. At patch lines 19-21 and 30-32 it checks this predicate for direct values and retained stream prefixes respectively. It inspects the retained prefix after budget handling, not arbitrary unseen remainder.
- At patch lines 117-153 and 204-220 it distinguishes top-level expanded lists (raise row errors) from nested values/maps (render error cells). That allows an `$ans` record containing a mixed retained result to remain printable without suppressing top-level command errors. All three candidates omit this table-rendering change.

## Limits

The same P2P failure is explained conclusively at the semantic/code-path level. This report does not attribute why each model selected that contract, whether an instruction caused it, or what the models saw in their transcripts; another investigator owns that evidence. No claim that stronger effort could never fix the task follows from these three patches. Budget/truncation edge cases were not re-executed or exhaustively adjudicated. The frozen saved audit, not this report, owns population counts and grade validity.

Ledger checked: 62 active bug titles inspected; no prior Nushell mixed-result candidate issue found. No new codescout product bug confirmed within this semantic-only scope.
