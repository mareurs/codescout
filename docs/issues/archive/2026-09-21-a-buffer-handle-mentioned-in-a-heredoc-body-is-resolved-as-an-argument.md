---
id: fbdd9b22aaf0fb44
kind: bug
status: archived
title: 'BUG: a buffer handle mentioned inside a quoted heredoc body is resolved as if it were an argument'
tags:
- cluster/addressing-without-an-escape-hatch
claimed_at: 2026-09-27
claimed_by: 48d1f0c8-9f60-43bb-a15e-17ec7995813a
closed: 2026-09-27
opened: 2026-09-21
owner: marius
related: []
severity: medium
---

# BUG: a buffer handle mentioned inside a quoted heredoc body is resolved as if it were an argument

## Summary

`OutputBuffer::resolve_refs` scans the **entire** `run_command` argument with an unanchored
regex and acts on every `@cmd_`/`@file_`/`@tool_`/`@bg_` token it finds, with no notion of
shell structure. A quoted heredoc body — the construct that exists precisely to mean *"this
is data, not syntax"* — is not distinguished from command text. Two consequences, opposite in
loudness: an **expired** handle refuses the whole call, and a **live** one is silently
substituted into the heredoc's content. There is no way to write a handle literally.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — still live, all four modes.** A
quoted-heredoc body mentioning `@cmd_deadbeef` is refused `buffer reference not found` (reproduced
independently by the coordinator as well); `echo harmless @cmd_deadbeef1` is refused naming the 8-hex prefix
(the regex is not right-anchored); a single-quoted `'@cmd_deadbeef'` is refused the same way; and a LIVE
handle inside a quoted heredoc is silently **substituted** with its temp path (exit 0). At the bytes:
unanchored `REF_RE` at `output_buffer.rs:901-903`, refusal at the `ok_or_else` near `:965`,
`result.replace(token, …)` near `:1025`; `mask_heredoc_bodies`' only caller is `run_command/inner.rs:175`; no
fix commit has touched these files since 2026-09-21.

## Symptom (Effect)

**Expired handle, loud.** Measured 2026-09-21, this session, using a fabricated handle:

```
run_command("cat <<'MSG'\nprose mentioning the handle @cmd_deadbeef as a measurement citation\nMSG")
→ {"ok": false,
   "error": "buffer reference not found: @cmd_deadbeef",
   "hint": "Buffer refs expire when the session resets. Re-run the command to get a fresh ref."}
```

The parent session hit this for real on a `git commit -q -F - -- <path> <<'MSG' … MSG` whose
**prose** quoted a handle that had expired at an MCP reconnect. The commit was blocked; the
workaround was to write the message to a file and use `git commit -F <path>`.

**Live handle, silent — the worse half.** Same shape, with a handle that still resolves:

```
run_command("cat <<'MSG'\nprose citing the live handle @cmd_c4b58497 inside a quoted heredoc body\nMSG")
→ {"exit_code": 0,
   "stdout": "prose citing the live handle /tmp/.tmpViSzEE inside a quoted heredoc body\n"}
```

Exit code 0, no warning, no `refreshed_handles` entry. Had that heredoc been a commit
message, the committed text would name a temp path that no longer exists.

## Reproduction

Tree `d155a8f6bb938b769a0a6fd0242ffc7e60a25f77`, branch `experiments`, 2026-09-21T16:03Z–16:40Z.

1. `run_command("echo harmless @cmd_deadbeef1")` → refused, `buffer reference not found: @cmd_deadbeef`.
   Note the reported token is **shorter than the one typed**: the regex takes exactly 8 hex
   characters and is unanchored on the right too.
2. `run_command("echo '@cmd_deadbeef'")` → refused identically. Single quotes are invisible to it.
3. The quoted-heredoc form above → refused identically.
4. Any `run_command` whose output exceeds the inline budget mints a live `@cmd_*` handle; put
   that handle in a `<<'MSG'` body and it is replaced by a temp path, exit 0.

**No escape exists for the case that matters.** Inside `<<'MSG'` the shell performs no
expansion, so every candidate escape changes the emitted bytes rather than escaping the token
(measured, same session):

```
input  : split attempt: @cmd_dead''beef        output: split attempt: @cmd_dead''beef
input  : backslash attempt: @cmd_\deadbeef     output: backslash attempt: @cmd_\deadbeef
```

Byte-splitting does work in a *command* position (`printf '%s' '@cmd_dead''beef'` prints
`@cmd_deadbeef` and passes the scanner), but that is a shell reassembly the heredoc body
cannot perform. Uppercase hex also passes the scanner — and is not the same text.

## Environment

codescout MCP over stdio, profile `~/.claude-kat`, branch `experiments`, Linux.
Session `571eb3d6-c879-43f6-b3f9-5a51e744e1af`.

## Root cause

**Measured** (the two runtime observations above) plus **read at the bytes**:

- `src/tools/output_buffer.rs:901-903` — `REF_RE` is
  `@(?:cmd|file|tool|bg)_[0-9a-f]{8}(\.err)?`, unanchored, applied with `find_iter` to the
  whole `command` string. No shell parse, no quote awareness, no heredoc awareness.
- `src/tools/output_buffer.rs:965-971` — a token that matches the shape and does **not**
  resolve returns `RecoverableError("buffer reference not found: {token}")`, refusing the
  entire call. That is the loud direction.
- `src/tools/output_buffer.rs:1025` (inside the same loop) —
  `result = result.replace(token, &path_str)` replaces **every** occurrence of the token in
  the command string, including occurrences inside a heredoc body. That is the silent
  direction. (The `@bg_` branch does the same at `:961`.)

**The fix for this exact defect already exists 60 lines above, for a sibling namespace.**
`src/tools/output_buffer.rs:864-898` is the `@ack_` arm, and its own comment names this bug:

> Gated on a LIVE handle lookup rather than on the token's SHAPE. `@ack_<8hex>` is also a
> legal filename, and this check runs before shell parsing, so quoting, `./` and backslashes
> are all invisible to it — a shape-only refusal made such a file unreachable through this
> tool with no way to name it, and rejected the whole command over one mention anywhere in it
> (**a heredoc body, a commit message, a comment**).

That arm falls through harmlessly when the handle is not live
(`docs/issues/archive/2026-09-02-a-filename-matching-an-ack-handle-is-unreachable.md`). The
`@cmd_`/`@file_`/`@tool_`/`@bg_` arm did not get the same treatment, and its live-lookup
*failure* is a hard error rather than a pass-through.

## Evidence

### The heredoc masking utilities are already in-tree, `pub`, and unused by this call site

`src/util/path_security.rs` holds two siblings over one shared opener
(`heredoc_opener`, :975):

- `strip_heredoc_bodies` (:934) — removes body lines; correct for a yes/no gate.
- `mask_heredoc_bodies` (:1010) — blanks body bytes **in place, preserving every offset**;
  written for a caller that must then splice at an offset the scan returned.

`mask_heredoc_bodies` exists because of the identical defect at a different scanner —
`docs/issues/archive/2026-08-19-run-command-rewrites-pipes-inside-heredoc-content.md`, where
`detect_terminal_filter` read a `|` in a heredoc body as a pipeline stage and spliced
`| tee '/tmp/codescout-unfiltered-hUMfFa' |` into written content. Exit 0, no warning. That
is the same failure mode as the live-handle substitution above, one scanner over.

### This is the sixth scanner in the family, and the first with the utility available

`CLAUDE.md` § *Parsers Over a Namespace* names "the heredoc tell" at four gates. The
in-tree count is now five heredoc-aware call sites:

| call site | helper |
|---|---|
| `is_dangerous_command` — `src/util/path_security.rs:843` | `strip_heredoc_bodies` |
| `commit_message_backtick_hazard` — `src/util/path_security.rs:1088` | `strip_heredoc_bodies` |
| `detect_il3_violation` — `src/util/path_security.rs:1198` | `strip_heredoc_bodies` |
| `check_source_file_access` — `src/util/path_security.rs:1705` | `strip_heredoc_bodies` |
| `detect_terminal_filter` — `src/tools/run_command/inner.rs:175` | `mask_heredoc_bodies` |

`resolve_refs` is the sixth scanner over the same argument string and calls neither
(`grep -i heredoc src/tools/output_buffer.rs` → **1** hit, and it is the `@ack_` comment
quoted above, not a call). Each of the five was fixed separately, on its own schedule, by its
own bug file.

### Ordering

`src/tools/run_command/mod.rs:209-216` — the IL-3 gate runs *before* `resolve_refs`, so the
two scanners see the same string and disagree about what a heredoc body is: IL-3 masks it,
`resolve_refs` does not.

## Hypotheses tried

1. **Hypothesis** — quoting escapes the token. **Test** — `echo '@cmd_deadbeef'`.
   **Verdict** rejected: the regex runs before any shell parse.
2. **Hypothesis** — a quoted heredoc is distinguished from command text. **Test** —
   `cat <<'MSG' … MSG`. **Verdict** rejected: identical refusal, and a live handle is
   substituted into the body.
3. **Hypothesis** — some escape exists for *mentioning* a handle inside a quoted heredoc.
   **Test** — byte-splitting with `''`, backslash. **Verdict** rejected: `<<'MSG'` performs
   no expansion, so both emit the escape characters literally.
4. **Hypothesis** — the regex is at least right-anchored, so a longer token is left alone.
   **Test** — `echo harmless @cmd_deadbeef1`. **Verdict** rejected: refused, naming the
   8-hex prefix.

## Fix

Not fixed. Two candidate shapes, neither applied:

1. **Mask first.** Call `crate::util::path_security::mask_heredoc_bodies` on the command
   before `REF_RE.find_iter`, collect the token *offsets* from the masked copy (offsets are
   preserved by construction — that is why the function exists), and substitute only at those
   offsets rather than via `String::replace`, which has no way to skip an occurrence. This
   closes both directions at once.
2. **Adopt the `@ack_` arm's disambiguator for the failure case.** A shape match whose
   lookup fails is currently fatal; making it a pass-through would make an expired handle
   mentionable and would not, by itself, stop a *live* handle being rewritten inside a
   heredoc body. Necessary but not sufficient.

Both need a word at the refusal site about what a caller can do, since today the answer is
"nothing" — per `CLAUDE.md` § *Parsers Over a Namespace*, a documented limitation and a
silent reinterpretation cost a reader very different amounts.

**Shape (1) implemented 2026-09-27, plus a right boundary. Shape (2) declined, with reasons.**

All four modes were re-verified live at HEAD before any code was written — none had been fixed — both at the bytes and through the running MCP `run_command`. Mode 4's reproduction minted a real handle and watched `/tmp/.tmpABeTwL` appear inside a quoted heredoc body at exit 0.

**The substitution is offset-based, which this file said was the substance.** `REF_RE.find_iter(&masked)` collects spans (`output_buffer.rs:939`) and the rewrite walks them with a forward cursor (`:1099-1103`). **Every `result.replace(token, …)` is gone** — both the `@bg_` one and the main one; verified by grep, the only surviving `.replace(` in the file are a doc comment and an unrelated JSON-pretty call. The mask is computed once and feeds **both** scans, the `@cmd_/@file_/@tool_/@bg_` arm and the `@ack_` arm, whose own comment already listed *"a heredoc body"* among what it must not refuse and could not deliver it.

| mode | closed? | by what |
|---|---|---|
| 1 — expired handle in a quoted heredoc body | **closed** | the mask |
| 2 — `@cmd_deadbeef1` refused on its 8-hex prefix | **closed** | right boundary — and it closes an **unreported silent half**: a *live* prefix was being spliced into the middle of a longer word |
| 3 — single-quoted `'@cmd_deadbeef'` | **open, deliberately** | see below |
| 4 — live handle in a body, silently substituted | **closed** | mask **+** offset splice; the mask alone does not close it when the same handle also appears in command text |

**Mode 3 stays open by design and is pinned as such.** Honouring quotes would be a silent regression the other way — `cat '@cmd_1a2b3c4d'` is an ordinary, correct way to name a buffer, and a quote-aware scan would quietly stop resolving it. So it is a documented limitation **stated at the refusal site**, which is what `IC-6` asks when no escape is affordable. `single_quoting_is_deliberately_not_an_escape_and_the_refusal_says_so` (`:1971`) pins **both halves**: the quoted LIVE handle still resolves, and the quoted expired one is still refused with the working escape named. Its comment says why it exists — *"so nobody credits the fix with coverage it does not provide"* — which is the annotate-inert-as-inert law applied to a deliberate non-closure.

**Shape (2) was declined although this section calls it "necessary", and the ruling is ratified here rather than silently followed or silently ignored.** The reasoning rests on this file's own next clause — (2) *"would not, by itself, stop a live handle being rewritten"*. So (2) buys mentionability **only for expired handles**: a liveness-dependent, partial escape, whose price is deleting an alarm that is genuinely reached and acted on (an agent re-runs the command). Shape (1) already yields a **complete** escape — heredoc body, live or expired — so the refusal keeps its loudness *and* gains a real remedy, which is strictly better than trading one for the other. Preserving the diagnostic *and* passing through would have required `run_command/mod.rs`, outside the dispatched file set, so the combined form was structurally unavailable without stopping. **Recorded as a reasoned departure, not an omission.**

## Fix provenance

- **SHA:** `2219125f` (experiments-only) — positional; dies on a rebase of `experiments`.
- **patch-id:** `bf63b03409996d54bfb6089c700e28e05484ca35` — content hash of the diff; survives rebase and cherry-pick. Derived through a file, never a pipe from `git show`.

**Six mutation sites, all KILLED, none survived**, via `./scripts/mutation-probe.sh` in an isolated worktree — the shared tree was never mutated: the REF-scan mask, the right-boundary predicate, the offset splice, the ACK-scan mask, and the remedy text's two halves separately.

**S3 is the load-bearing result and it settles this file's own ruling.** Keeping the mask but restoring a `String::replace` pass still rewrites the body occurrence, and **exactly one** test catches it — `the_same_handle_is_substituted_in_command_text_and_left_literal_in_the_body`. That is the measurement behind *"the offset-based rewrite is the substantive part of the change, not the masking call"*: the two halves are separable, and only the pair closes mode 4.

**Operational note, because it will otherwise read as a failed fix.** `cargo rb` was not run, so the **live MCP binary still carries the old behaviour** — re-running the four reproductions through `run_command` will still show the bug until someone rebuilds the release binary and reconnects `/mcp`. All post-fix evidence is at the `resolve_refs` level, which is where the entire defect lives.

**The operational note above is DISCHARGED.** The binary was rebuilt and `/mcp` reconnected on 2026-09-28, and all four modes were re-run through the live server. This is the end-to-end evidence that note said was pending; the unit-level results are no longer the only ones.

| mode | before, live | after, live |
|---|---|---|
| 1 — expired handle in a quoted heredoc body | refused `buffer reference not found` | **`prose mentioning the handle @cmd_deadbeef as a measurement citation`**, exit 0 |
| 2 — `echo harmless @cmd_deadbeef1` | refused on the 8-hex prefix | **`harmless @cmd_deadbeef1`**, exit 0 |
| 3 — `echo '@cmd_deadbeef'` | refused, no escape offered | **still refused — and the hint now names the escape** |
| 4 — LIVE handle in a quoted heredoc body | silently substituted to a temp path, exit 0 | **`prose citing the live handle @cmd_e6476ef7 inside a quoted heredoc body`**, exit 0 |

**Mode 4's handle was minted in the same session** (`seq 1 4000` → `@cmd_e6476ef7`) and was live at the instant of the test — so this is the substitution path **declining**, not a lookup failing. That is the distinction the unit tests draw, now observed on the wire, and it is the one a reader would otherwise have to take on trust.

**And the shared helper is confirmed at the SECOND refusal site, unplanned.** A stale `@bg_` handle refused in the same session returned the identical escape paragraph under `background job ref not found`. No unit test asserts about the two sites *together*; this is the only evidence that `mention_escape_hint` reaches both in the shipped binary, and it arrived by accident — the `@bg_` handle had gone stale because the rebuild replaced the server process that owned it.

## Tests added

None — status is `open`, nothing is fixed. A regression test would assert both directions at
one site: an expired handle inside `<<'EOF'` is not refused, and a live handle inside
`<<'EOF'` is not substituted. Note the second is the one that would have caught this: the
first was already the `@ack_` arm's test and did not generalise.

## Workarounds

- Write the text to a file and pass it by path — `git commit -F <path>` rather than
  `-F -` with a heredoc. This is what unblocked the parent session.
- Prefer codescout's own write tools (`doc`, `edit_file`, `create_file`) over a shell heredoc
  when the content mentions a handle; they do not route through `resolve_refs`.
- Native `Bash` bypasses `resolve_refs` entirely, but also bypasses the IL-3 and
  dangerous-command gates.

## Resume

Decide between the two fix shapes above. If (1): `resolve_refs`
(`src/tools/output_buffer.rs:864`) currently builds its substitution with
`result.replace(token, &path_str)`, which cannot skip an occurrence — the offset-based rewrite
is the substantive part of the change, not the masking call. Check
`src/tools/output_buffer.rs`'s `is_buffer_only` classifier at the same time: it consumes the
*resolved* string, so a token masked out of substitution must also not count as a buffer temp
path.

## References

- `src/tools/output_buffer.rs` — `resolve_refs` (:864), `REF_RE` (:901), the refusal (:965).
- `src/util/path_security.rs` — `heredoc_opener` (:975), `strip_heredoc_bodies` (:934),
  `mask_heredoc_bodies` (:1010).
- `src/tools/run_command/inner.rs:175` — the masked-copy precedent.
- `docs/issues/archive/2026-08-19-run-command-rewrites-pipes-inside-heredoc-content.md` — the
  silent-substitution twin, fixed.
- `docs/issues/archive/2026-09-02-a-filename-matching-an-ack-handle-is-unreachable.md` — the
  `@ack_` arm's fix, the one this call site did not receive.
- `docs/issues/archive/2026-08-31-dangerous-command-gate-scans-heredoc-body.md`,
  `docs/issues/archive/2026-07-28-il3-gate-matches-pipes-inside-heredoc-text.md`,
  `docs/issues/archive/2026-08-17-heredoc-carve-out-defeated-by-a-pipe-in-the-body.md`,
  `docs/issues/archive/2026-09-01-source-gate-refuses-the-whole-compound-command.md` — the
  four `CLAUDE.md` cites, each fixed separately.
- `CLAUDE.md` § *Parsers Over a Namespace* — the class statement and the heredoc tell.

## Cluster

`cluster/addressing-without-an-escape-hatch` (`IC-6`). The class claim is *"an addressing
scheme interprets every token in its namespace and provides no way to write one literally"* —
which is this defect verbatim: the `@cmd_<8hex>` handle namespace has no escape for
**mention**, and hypothesis 3 above measures that the one place an author most needs it (a
quoted heredoc carrying prose) is the one place no escape can be constructed.

**Asymmetry worth recording, because it cuts against the class's usual severity argument.**
The expired-handle direction **refuses loudly** — better than the silent records of
`read_output_ids`, and it still blocks writing *about* a measurement without re-running it.
But this file's second measurement shows the class holds in the silent direction too at the
same site: a live handle is rewritten into content with exit 0. So loudness here is a property
of *which half of the input space you land in*, not of the site — and per
`CLAUDE.md` § *Testing Discipline* (*loudness is a property of a PATH*), the loud path is the
one that cannot lose data and the quiet one is the one that can.
