---
id: '5d9de3691e208ba2'
kind: bug
status: open
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

## Fix

Not chosen. Three shapes, cheapest first:

1. **Fix only the text.** Keep the wholesale refusal, and (a) head the list with something
   true of every row — *"these files need formatting; the ones below marked PEER / SHARED /
   UNKNOWN are not this session's to write"* — and (b) name `rustfmt --edition 2021 <path>`
   as the narrow escape beside `cargo fmt`, since the script already uses exactly that.
   This is the whole fix if the wholesale refusal is intended.
2. **Format `$MINE`, then refuse the rest.** What `ce3a628db5fa1168` § Fix already claims
   happens. Partial success, and the reader still has to act on the peer files, but the
   gate's step 1 is no longer blocked by a peer.
3. **Do nothing and say so at the refusal site**, per this corpus's standing preference —
   but there IS a refusal site here, so option 1 is strictly better than option 3.

Any of them wants a MIXED-scan assertion in `tests/fmt-mine.sh`, which is the case the
existing suite cannot express: its stub produces a homogeneous verdict.

Fix SHA: *(not yet fixed)*
Patch-id: *(not yet fixed)*

## Workarounds

`rustfmt --edition 2021 <your files>` — narrower than the script's own suggestion, touches
only what you name, and is the exact command the script runs at `:211`. Used on 2026-09-12
to get through the gate.

Do **not** take the suggested `cargo fmt`: on a shared checkout that is
`ce3a628db5fa1168`, live.

## Tests added

None yet. The one that matters is a MIXED scan — a stub emitting one `MINE` row and one
`PEER` row — asserting both on the exit disposition and on the message. The existing
suite's stub cannot produce that shape, which is why 29 assertions pass over this.

## Resume

Unclaimed. The first question is a ruling, not a patch: **is the wholesale refusal
intended?** `ce3a628db5fa1168` § Fix says it formats mine and refuses the rest; the code
refuses everything. Whoever answers that picks between § Fix options 1 and 2; the message
defects are owed under either.

## References

- `ce3a628db5fa1168` — the bug this script mitigates, and the one its own suggested remedy
  re-admits.
- `scripts/fmt-mine.sh:146-181`, `:211`.
