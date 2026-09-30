---
kind: bug
status: wontfix
tags:
- cluster/unclassified
closed: 2026-09-24
opened: 2026-09-04
owner: marius
related: []
severity: low
---

# BUG: two `doc(update)` calls were refused with "patch must be a JSON object" while the patch was one — five controlled variants failed to reproduce it

## Summary

Two consecutive `doc(action="update", patch={body_edits: [...]})` calls were refused with
`patch must be a JSON object mapping field names to new values`. The patch in both was a JSON
object with a single `body_edits` key. Splitting the same work into two smaller calls succeeded
immediately afterwards against the same artifact. A five-variant isolation run **failed to
reproduce** the refusal, which eliminates the four obvious causes and leaves transport as the
standing lead.

**The value of this file is the elimination, not the diagnosis.** A later session hitting this
should not re-run the matrix below.

## Symptom (Effect)

```
{"ok": false,
 "error": "doc(action=\"update\") patch must be a JSON object mapping field names to new values",
 "hint": "e.g. patch={\"status\": \"fixed\"}. A patch that is an array or scalar is not a valid
          RFC 7396 merge document."}
```

Emitted twice, for `doc(action="update", id="cc4843e5c1a020bd", …)`, 2026-09-04 ~02:2x EEST.
Both payloads carried `patch = {"body_edits": [ … ]}` — an object, one key.

## Reproduction

**Not yet reproducible — best lead: MCP transport mangling of a large payload containing literal
non-ASCII, such that `patch` arrived as something other than an object.** If that is what happened,
the error text is *correct about what it received* and the defect is upstream of `doc` entirely,
which is why this file claims no defect in the librarian.

The two failing payloads differed from the succeeding ones in four candidate ways. All four were
tested against `doc(action="update", id="154848bbd55e7768")` using a probe that discriminates
**parse** from **application**: a patch that parses fails on a deliberately absent `old_string`
(`body_edits[0]: old_string not found…`), while a patch that does not parse fails on the patch. No
write can occur either way, because `old_string` matches nothing.

| # | variant | result |
|---|---|---|
| 1 | single `edit`, ASCII only (control) | parsed |
| 2 | single `edit`, `⚠` (`⚠`) in `new_string` | parsed |
| 3 | two `edit` items in the array | parsed |
| 4 | single `edit`, em-dashes in `new_string` | parsed |
| 5 | two `edit` items, ~350-char padded strings each | parsed |

**So the trigger is none of: array arity, the `⚠` codepoint, em-dash content, or payload size.**

**A limit of the instrument, stated because it is the reason the matrix could not close.** The
original failures were typed with *literal* `⚠` and `—` characters; every probe above and every
succeeding call used `\uXXXX` escapes. Whether a literal multi-byte character survives to the
server as a literal is decided by the harness's JSON serialisation, which the caller does not
control — so variant 4 may have tested the escaped form under both labels. That is the one axis
this matrix does not actually vary, and it is also the lead.

## Environment

codescout `experiments` @ `72311ef5`, main checkout, MCP over the release binary rebuilt earlier
the same session (`cargo rb` + `/mcp`). Target artifacts: `cc4843e5c1a020bd`
(`docs/trackers/retrieval-benchmark.md`, augmented) for the failures,
`154848bbd55e7768` (an archived bug) for the probes.

## Root cause

Unknown. Two readings survive the matrix and they assign the defect to different systems:

1. **Transport mangling.** A large payload with literal multi-byte characters is truncated or
   re-encoded en route, so `patch` genuinely arrives as a non-object. The error is then accurate,
   the librarian is behaving correctly, and the bug is in the MCP layer.
2. **A parse path in `doc(update)`** that rejects some payload shape and reports the wrong reason.

Nothing observed distinguishes them, and reading the deserialiser cannot: it would show what the
code does with what it *receives*, and the open question is what it received.

## Evidence

The succeeding split, moments after the second refusal and against the same artifact, is what makes
this a defect report rather than a note about a malformed call: the identical two operations landed
as `{"updated": true}` when sent as two single-edit calls with escaped characters. Same session,
same id, same intent, no intervening change to the artifact.

## Hypotheses tried

1. **The `⚠` character breaks the patch parse.** Test: variant 2. **Verdict:** rejected.
2. **Two elements in `body_edits` break it.** Test: variant 3. **Verdict:** rejected.
3. **Em-dash content breaks it.** Test: variant 4. **Verdict:** rejected — with the caveat above
   that the escaped form may have been what was sent.
4. **Payload size breaks it.** Test: variant 5, and separately a *successful* ~1.5 KB single-edit
   `insert_after` that landed the banner this session. **Verdict:** rejected.

5. **(Added 2026-09-24, untested at the transport.) The harness sent `patch` as a STRING holding
   JSON, and codescout rescues that shape for arrays but not for objects.** Read at the source:
   `optional_array_param` (`src/tools/core/params.rs:317`) re-parses a string-encoded array
   "for MCP clients that stringify arrays", while `doc(update)`'s guard
   (`src/librarian/tools/update.rs:449`, `!p.is_object()`) refuses a string-encoded object with
   **exactly** the observed message. So if the payload was stringified, a `body_edits` ARRAY
   elsewhere would have survived and this OBJECT would not — which fits "splitting into smaller
   calls succeeded" only if stringification depends on payload shape or size, and nothing measured
   shows that. **Test attempted, inconclusive:** two `doc(update, id=674c1e24c2d421fa)` calls
   meant to send `patch` as a string both arrived as an object (`old_string not found`) — the
   client normalizes object-typed parameters before sending, so it cannot produce the string form.
   Both probes were no-ops by construction. **Verdict:** deferred; the Resume step (log the raw
   `patch` on the rejection path) decides this hypothesis too — a logged `Value::String` confirms
   it, and the fix would then be an object twin of `optional_array_param`.
   Found while chasing a sibling report (`codescout-lessons.md` § 6.5, non-ASCII escapes through
   the edit tools), filed as
   `docs/issues/2026-09-24-edit-tools-collapse-unicode-escapes-in-tool-arguments.md`.
## Fix
**WONTFIX — not a code defect; root cause found 2026-09-24** (open-bug sweep, `deep-agent-workflow-observations:DWF-7`). The refusal was accurate. In `usage.db`, 15 `doc`/`artifact` calls since this file was opened carry a `patch` whose `json_type` is `text` — a string, not an object. All 15 failed, and **14 of the 15 strings are not valid JSON**. One of the two calls this file reports (row 104000, 00:39:45 UTC) ends in `…"}]` with its closing `}` missing. Against that, 1893 object-typed patches went through in the same window. So the caller produced malformed JSON, the client passed it on as a string, and `update.rs:448-454` correctly refused it. "Transport mangling" in this file was close, but the origin is the caller, not the server.

One optional, cheap improvement, not tracked as work: when `patch` arrives as a *string*, the refusal could say so. Today its hint only mentions "array or scalar", so a caller cannot tell it sent a string.

None proposed. Deciding between the two readings needs the bytes the server received, so the next
step is instrumentation rather than a code change: log the raw `patch` value on the rejection path,
or capture the MCP frame. A fix written now would be a guess about which system is at fault.

## Tests added

None. There is nothing verified to regress against — a test asserting today's behaviour would pin
the five passing variants, which already pass.

## Workarounds

Split a `body_edits` patch into single-edit calls, and prefer `\uXXXX` escapes over literal
non-ASCII in tool arguments. Both were sufficient here.

## Resume

Add raw-payload logging to the `patch`-is-not-an-object rejection in `doc(action="update")`, then
retry a large `body_edits` patch containing literal `⚠` and `—` in both `heading` and `content`.
If the logged value is not an object, close this against the transport and re-file there; if it is
an object, the parse path is the defect. **Do not re-run the five-variant matrix** — it is above and
it eliminated its four hypotheses.

## References

- Succeeding split: `doc(action="update", id="cc4843e5c1a020bd")` ×2, landing the superseded banner
  and the citation re-point committed at `72311ef5`.
