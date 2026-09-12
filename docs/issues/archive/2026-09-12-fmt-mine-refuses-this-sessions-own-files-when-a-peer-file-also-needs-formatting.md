---
id: d4db8a2d93bdced9
kind: bug
status: fixed
title: 'BUG: fmt-mine.sh refuses this session''s OWN files whenever a peer file also needs formatting, and its remedy text names the command it exists to prevent'
tags:
- cluster/guard-narrower-than-its-name
---

## Summary

`scripts/fmt-mine.sh` is the gate's mandated step 1. When the set of files needing
formatting contains **both** this session's files and a peer's, it refuses **all of them**
and exits 1 — this session's own files included. `$MINE` is computed at `:146` and then
never read on the refusal path (`:150-181`), which returns before the formatting branch at
`:211`.

So on a shared checkout the gate's first command cannot be run at all while any other
session holds an unformatted `.rs` file, and the session's own correctly-attributed files
stay unformatted. Five sessions were live in this checkout when this was observed.

Two further defects sit in the refusal's TEXT, and they are the half that misroutes a
reader rather than merely blocking them.

## Symptom (Effect)

```
$ ./scripts/fmt-mine.sh
fmt-mine: REFUSED — these need formatting and are not this session's to write:
MINE      src/tools/symbol/symbols.rs
          written by THIS session (b80a27d4)
PEER      tests/doc_tool_refs.rs
          written by f3c594ce-…  [LIVE]
...
FMT exit=1
```

**The header asserts the opposite of the row beneath it.** `src/tools/symbol/symbols.rs`
is labelled `MINE` and described in the same breath as *"not this session's to write"*.
The verbatim print is deliberate and documented at `:152` — the scan carries the `uds:`
socket and the `[LIVE]` marker, which is what makes the remedy performable — but the
header was written for a scan containing only not-mine rows.

**The prescribed remedy is the command this script exists to prevent.** The refusal text
says: *"If you have decided it is safe, run `cargo fmt` yourself — that is the same act,
minus the false assurance that a guard sanctioned it."* `cargo fmt` takes no pathspec and
rewrites every `.rs` in the workspace, which is `ce3a628db5fa1168` exactly. Following the
guard's own advice re-admits the defect it was built to close.

**And the narrow remedy is implemented thirty lines below the message that omits it.**
`:211` is `rustfmt --edition 2021 $MINE`, with a comment explaining the choice — *"cargo
has no per-file mode and formatting the whole workspace is the thing being avoided"*. The
script knows the right answer and does not offer it at the refusal site.

## Reproduction

Observed 2026-09-12 on `experiments`, during an ordinary gate run.

1. Have an uncommitted `.rs` file of your own that needs formatting.
2. Have any peer hold an uncommitted `.rs` file that also needs formatting. (Not
   contrived — it was `tests/doc_tool_refs.rs`, mid-edit by a live session.)
3. `./scripts/fmt-mine.sh` → `exit 1`, nothing formatted, message as above.
4. `rustfmt --edition 2021 <your file>` → formats only yours, gate proceeds.

**Precondition that decays:** step 2 requires a peer to be holding an unformatted file at
that instant. Re-running this next week against a clean tree gives the `nothing
attributable to this session needs formatting` branch, which is a different path and is
correct — a reader who meets that will conclude the bug is gone.

## Environment

`scripts/fmt-mine.sh` at `2caf55c5` onwards. Shared checkout, several concurrent sessions.
Not reproducible on a solo checkout, which is most of why it survived its own 29-assertion
suite: the suite stubs attribution via `FMT_MINE_PROVENANCE` and can construct a
PEER-only or a MINE-only scan, but a MIXED scan is the case nobody wrote.

## Root cause

```sh
MINE=$(printf '%s\n' "$PROV" | awk '$1=="MINE"{...}')          # :146
NOT_MINE=$(printf '%s\n' "$PROV" | awk '$1=="SHARED"||...')    # :147

if [ -n "$NOT_MINE" ]; then                                    # :150
    echo "fmt-mine: REFUSED — these need formatting and are not this session's to write:"
    printf '%s\n' "$PROV"                                      # :154  — includes MINE rows
    ...
    exit 1                                                     # :180  — $MINE never read
fi
```

The two sets are computed independently and then only one is consulted. The partition is
correct; the disposition is not a partition at all.

**Whether the wholesale refusal is deliberate is genuinely unclear and this file does not
guess.** Failing closed on a mixed scan is defensible — one decision for the reader instead
of a partial success that leaves the gate red anyway. What is not defensible either way is
the message, which describes a file as not-this-session's while printing its `MINE` verdict
directly below, and which then routes to the wide command while the narrow one is already
in the script. If the behaviour is intended, the text is still wrong.

Its own bug file (`ce3a628db5fa1168` § Fix) describes the design as *"formats only the
files `scripts/file-provenance.py` attributes to this session and refuses the rest"* —
which is the partial-success reading, not the wholesale one. So the documentation and the
code disagree about which of the two this is.


### Resolved 2026-09-12 — the ruling was already written, and the refusal was not arbitrary

**On "is the wholesale refusal deliberate?":** the ruling exists, in a surface this file did
not check. `CLAUDE.md:38` states the contract as *"The script formats what
`scripts/file-provenance.py` attributes to you and **refuses** the rest"* — two actions in
one run, not a choice between them — and it is the document every session is served at
startup. With `ce3a628db5fa1168` saying the same, that is two independent documents against
the code, which makes this a defect rather than an open question. § Fix option 2.

**But the naive option 2 is unsafe, and the reason retroactively justifies the behaviour
this file was questioning.** `rustfmt <file>` is not per-file: it parses the file and
descends into every `mod` the file declares. Verified at the bytes on rustfmt 1.9.0-stable
— `rustfmt --edition 2021 src/lib.rs` rewrote `src/peer.rs` through a `pub mod peer;` line —
and `--skip-children` is not available outside nightly.

So a peer's child module can only be **damaged** if it also needs formatting, and a file
needing formatting is by construction a row in this same scan. Refusing every scan
containing a not-mine row was therefore *accidentally sufficient* to close that door. It
read as over-caution and was load-bearing; nothing in the script said so.

**How it was caught, which is the part worth keeping.** The first draft of the fix did
exactly what § Fix option 2 says and formatted `$MINE` on a mixed scan. The new case's
control assertion — *"THEIR file was left byte-untouched"*, written because an assertion
that only checks *my* file is satisfied by a fix that simply runs the formatter over
everything — fired immediately. Without it the fix would have re-admitted
`ce3a628db5fa1168`, the defect the whole script exists to close, **through the regression
test written to close a different bug in it**, and every assertion about the refusal message
would have stayed green.
## Fix

**Chosen: option 2, plus both message defects, plus a guard option 2 turns out to need.**

1. **`$MINE` is formatted on a mixed scan**, then the rest refused, per the contract in
   `CLAUDE.md:38`. Exit stays **1** — this session's half is done, the gate is still
   blocked, and the reader's remaining action is unchanged. Returning 0 would let a caller
   chaining on success walk past unformatted files it does not own.
2. **Unless formatting `$MINE` would REACH a not-mine file.** `rustfmt --check` is run over
   `$MINE` alone and the files it would rewrite are intersected with `$NOT_MINE` (`grep -Fx`,
   so `src/a.rs` cannot pair with `src/ab.rs`). A non-empty intersection refuses wholesale
   and names mod-descent as the reason rather than repeating a bare refusal; an exit >1 from
   that check refuses as *"cannot tell"*, matching the discipline already applied to the
   provenance scan two stages above.
3. **The refusal lists only not-mine rows.** Filtering them is what makes the *existing*
   header true, so the message needed no rewording — a smaller change than option 1(a)
   proposed, and one that cannot drift from the verdict it describes.
4. **The narrow command is printed with the actual paths**, beside `cargo fmt` rather than
   replacing it: `CLAUDE.md` names `cargo fmt` as the escape, so removing it would put the
   script and the served documentation back into disagreement — which is the shape of this
   bug.

Fix SHA: `d79d95e5`
Patch-id: `91afee870c9aed0cb1d23d85fcd7aa2a3ffaade4`
## Workarounds

`rustfmt --edition 2021 <your files>` — narrower than the script's own suggestion, touches
only what you name, and is the exact command the script runs at `:211`. Used on 2026-09-12
to get through the gate.

Do **not** take the suggested `cargo fmt`: on a shared checkout that is
`ce3a628db5fa1168`, live.

## Tests added

Two cases in `tests/fmt-mine.sh`, and they need a **per-PATH** verdict stub (`mkmixed`) that
`mkstub` cannot express: all four existing stubs answer one verdict for every file handed to
them, so a suite covering each of MINE / PEER / SHARED / UNKNOWN individually could not
express a mixed scan at all. The missing axis was per-path, not per-verdict — a fifth
homogeneous stub would have added nothing.

- **Case 8 — disjoint trees.** Mine under `src/`, theirs under `tests/`, which `cargo fmt`
  covers (verified) but `rustfmt src/lib.rs` cannot reach. Asserts mine written, theirs
  byte-untouched, exit still 1, the refusal no longer listing a file it just formatted, the
  `uds:` socket still named, and the narrow command offered. Mirrors the real observation:
  `src/tools/symbol/symbols.rs` against `tests/doc_tool_refs.rs`, two targets sharing no
  module root.
- **Case 9 — the module tree reaches theirs.** `pub mod peer;` makes `src/lib.rs` the root of
  `src/peer.rs`. Asserts the wholesale refusal, both files untouched, and that the message
  names mod-descent and the file it would have collaterally rewritten.

Three assertions were observed **red** before the fix, each printing the defect verbatim;
case 9's control then fired against the fix's own first draft. Both fixtures carry an
inline note saying which detail is load-bearing — case 8 degenerates into case 9 if the
peer's file moves under `src/`, and case 9 degenerates into case 8 if the `mod` line is
tidied away, both silently and both still passing.

**Verified:** `bash tests/fmt-mine.sh` — **42 passed, 0 failed**, all 7 pre-existing cases
included; CI runs this suite as its own `fmt-mine-tests` job. Full gate: clippy `0`, lean
`0`, default `0` (5832 passed, 0 failures), with
`committed_paths::the_tracked_population_is_not_vacuous` — the one Rust test that names this
script — read green **by name**. Step 1 itself exits 1 on this tree, which is the fixed
message working: it named a live peer's unformatted
`src/librarian/catalog/augmentation.rs`, listed no `MINE` row, and printed
`rustfmt --edition 2021 src/librarian/catalog/augmentation.rs`.
## Resume

Fixed and archived. No further action.

One thing deliberately **not** done: `CLAUDE.md` still names `cargo fmt` as the escape when
the guard refuses, and that stays accurate — the script prints it too, now beside the narrow
form. Adding the narrow form to `CLAUDE.md` as well was considered and dropped as
out-of-scope for a bug about the script's own text.
## References

- `ce3a628db5fa1168` — the bug this script mitigates, and the one its own suggested remedy
  re-admits.
- `scripts/fmt-mine.sh:146-181`, `:211`.
