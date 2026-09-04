---
id: '87c41034e7bd8b4d'
kind: bug
status: open
title: 'BUG: grep served pre-edit content immediately after four successful edit_file writes, reporting the old bytes as current'
tags:
- cluster/transient-shared-state-lies-to-readers
- grep
- edit_file
- silent-wrong-answer
- read-after-write
---

## Summary

Four consecutive `edit_file` calls against `src/librarian/prompts/companion_hint.md` each returned
`{"status": "ok"}`. A `grep` issued immediately afterwards, same path, returned **the pre-edit
content** — showing all four replaced strings as still present. The writes had in fact landed. A
second identical `grep` moments later returned the correct (post-edit) result.

The failure is silent and returns a **plausible** answer rather than an error, which is the
admission test for this class: nothing downstream fires, and the natural reading of the output is
*"my four edits silently failed"*.

## Symptom (Effect)

```
edit_file(...replace_all `artifact` with -> `doc` with ...)   -> {"status":"ok"}
edit_file(...librarian_context -> librarian action=context)   -> {"status":"ok"}
edit_file(...artifact_event -> doc action=event_create)       -> {"status":"ok"}
edit_file(...artifact_link -> doc action=link)                -> {"status":"ok"}

grep("artifact_event|artifact_link|librarian_context|`artifact` with",
     path="src/librarian/prompts/companion_hint.md")
  -> 17 matches, INCLUDING all four supposedly-replaced strings
```

Ground truth at that same moment, from three independent readers:

| instrument | result |
|---|---|
| `stat -c '%y %s'` | mtime `17:00:19`, i.e. after the edits |
| `git diff --stat` | `1 file changed, 15 insertions(+), 15 deletions(-)` |
| `python3` reading the file | `` `artifact` with `` → **0**, `artifact_event` → **0**, `artifact_link` → **0**, `librarian_context` → **0**, `` `doc` with `` → **18** |

Re-running the *same* `grep` call afterwards returned `0 matches`, correctly.

## Reproduction

Not reduced to a minimal recipe. Observed once, in this sequence: four `edit_file` writes to one
markdown file under `src/` in immediate succession (the first with `replace_all: true`), then a
`grep` on that path with no intervening call. The window appears to be short — the next `grep`,
after a few unrelated tool calls, was correct.

**What would make this reducible:** whether the stale read is a function of elapsed time, of
intervening calls, or of `replace_all` specifically. A loop of `edit_file` → immediate `grep` on a
scratch file, N iterations, counting how often the read is stale, would settle it and would also
give the window a number instead of "short".

## Root cause

Not established. `grep` on a path inside the active project appears to serve from an index or cache
rather than the file's current bytes, and that layer had not observed the writes `edit_file` had
just completed. Candidate mechanisms, none verified: a debounced index refresh; a mmap/stat cache;
`edit_file` marking the file dirty asynchronously (`mark_file_dirty_for` is `.await`ed on the write
path for `create_file`, so the ordering there is worth reading).

### Classification, and where it is a stretch

Tagged `cluster/transient-shared-state-lies-to-readers` (`IC-12`), whose claim is *"one session's
tooling mutates shared state for the duration of an operation; every other session's read is wrong
for that window, and the standard diagnostic reports the lie as truth rather than as an outage."*
The second half is an exact match — `grep` is the sanctioned diagnostic here and it reported the
lie as truth.

**Where this instance diverges from the claim, stated so a reader can downgrade it rather than
inherit it:** `IC-12`'s existing members are all *cross-session* (a peer's pre-commit stash, a
gate reading another session's half-written file). This was observed **same-session** — my own
write, my own read. It belongs in the class only via the fact that codescout's index lives in a
per-workspace server process shared by every session in the checkout, so the stale window is
genuinely visible to other readers and not merely to the writer. That inference is untested here:
nobody checked whether a peer session's `grep` was also stale during the window.

If that turns out to be wrong — if the staleness is per-connection rather than per-server — this is
not `IC-12` and should move to `cluster/unclassified` rather than be argued into the class. One
probe settles it: two sessions grepping the same path immediately after one of them writes.
## Evidence

### Why this is more than a cosmetic lag

The wrong answer arrives in the exact shape of a *different* real defect — "your edit did not
apply" — and the obvious response to that reading is **to re-apply the edits**. Here that would
have been harmless (the `old_string`s were gone, so the retries would have errored). It is not
harmless in general: an `insert`-style or append-style edit re-applied on a false "it didn't land"
signal duplicates content, and a `replace_all` whose new string still contains the old one would
compound.

### It defeats the standard remedy

The reconnaissance discipline for this is *"verify at the bytes"* — but `grep` **is** one of the
byte-verification tools, and it is the one the Iron Laws route markdown and source reads toward
(`read_file` on `.md` is refused in favour of heading-addressed reads; shell `grep` on source files
is blocked outright). So the sanctioned instrument is the one that lied, and the reader who follows
the rules most closely is the most exposed. What caught it here was reaching for three *unsanctioned*
readers (`stat`, `git diff`, `python3`) after the result disagreed with a strongly-held expectation.

### Same family as an already-recorded law

`reconnaissance-patterns:R-174` — *when two tools disagree, the question is not whose logic is wrong
but which world each one read*. This is that, with the two worlds separated by milliseconds rather
than by a retired datastore.

## Fix

Not implemented; root cause not established, and a fix chosen before the mechanism is known would
be a guess.

1. **Reduce it first** (see *Reproduction*) — the loop is cheap and turns "short window" into a
   number. Until then there is no way to tell a fix from a coincidence.
2. If it is a stale index: make `grep` on a single explicit `path` read the file directly rather
   than consult the index. A one-path `grep` has no reason to go through a corpus index, and that
   removes the class for the case where it matters most (verifying an edit you just made).
3. If it is write-ordering inside `edit_file`: ensure the dirty-marking / invalidation completes
   before the tool returns `ok`, so `ok` means "a subsequent read will see this".
4. Whatever the mechanism, prefer making the correct path end in a consistent state over documenting
   the hazard — a caching rule the reader must remember is a policy, not a mechanism.

## Resume

Unclaimed. Noticed 2026-09-04 while fixing
`docs/issues/archive/2026-09-02-a-test-file-in-no-cargo-target-asserts-nothing-and-is-a-tautology-anyway.md`:
the four prose edits it required were followed by a verification `grep` that showed none of them
had applied. Filed on notice rather than at task end, because the interesting half — that the
sanctioned verification instrument is the one that returned the wrong answer — is the half that
would have been dropped from a summary written later.
