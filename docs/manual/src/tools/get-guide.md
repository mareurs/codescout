# `get_guide`

> Fetch deep guidance for a named topic. The default tool descriptions and
> `server_instructions` surface stay terse; longer rules and worked examples
> live behind `get_guide(topic)` so the prompt budget is spent on what every
> session needs, not what most sessions can ignore.

## When to call it

The system prompt and individual tool errors will point you to a topic
explicitly — e.g. *"see `get_guide('librarian')`"* or *"full guidance:
`get_guide('error-handling')`"*. Call it then. You will also see a one-shot
`_guide_hint` field appended to the first response from a tool whose
discipline lives in one of the guide topics — that is the hint to fetch the
guide before the next call against the same surface.

You do not need to call `get_guide` proactively. The signal is always pulled
from a hint, an error message, or a system-prompt pointer.

## Topics

| Topic                     | Covers                                                                    |
| ------------------------- | ------------------------------------------------------------------------- |
| `librarian`               | artifact model, filter syntax, trackers, augmentations                    |
| `tracker-conventions`     | frontmatter, archive flow, status vocabulary                              |
| `progressive-disclosure`  | `MAX_INLINE_TOKENS`, the `@ref` buffer, overflow patterns                 |
| `error-handling`          | `RecoverableError` vs `anyhow::bail`, `is_error` routing                  |

Each topic returns the full guide body as a single string. Topic bodies are
large enough that the response usually overflows into a `@tool_*` buffer
(progressive disclosure applies — see `get_guide('progressive-disclosure')`).

## Examples

List topics + one-line summaries (no `topic` arg):

```json
{
  "tool": "get_guide",
  "arguments": {}
}
```

→

```json
{
  "topics": ["error-handling", "librarian", "progressive-disclosure", "tracker-conventions"],
  "summaries": {
    "librarian": "artifact model, filter syntax, trackers, augmentations",
    "tracker-conventions": "frontmatter, archive flow, status vocabulary",
    "progressive-disclosure": "MAX_INLINE_TOKENS, @ref buffer, overflow patterns",
    "error-handling": "RecoverableError vs anyhow::bail, is_error routing"
  }
}
```

Fetch a specific topic:

```json
{
  "tool": "get_guide",
  "arguments": { "topic": "librarian" }
}
```

→

```json
{
  "topic": "librarian",
  "body": "..."
}
```

Unknown topics return a `RecoverableError` listing the four valid topics.

## First-call hint mechanism

Some tools declare a `relevant_guide_topic` (currently `librarian` for the
artifact tools). The very first time you call one of those tools in a
session, the response includes a `_guide_hint` field that points you at the
matching topic:

```json
{
  "result": "...",
  "_guide_hint": "First call this session for topic 'librarian'. Run get_guide(\"librarian\") for full guidance."
}
```

The hint is one-shot per topic per session — subsequent calls against the
same topic do not re-emit it. The set is cleared on
`workspace(action="activate")`.

## Why a separate tool instead of inline prompt content

The system-prompt budget is a scarce shared resource. Every tool description
visible at session start eats from that budget, and most tools have rules
that only become load-bearing when the agent first touches a domain
(librarian artifacts, error routing, etc.). Keeping the entry-level
guidance terse and routing the rest behind `get_guide` means the prompt
stays under the cap on initial connect, and the longer rules land only when
the agent has the question they answer.

See `src/prompts/source.md` for the entry-level surface, and
`src/prompts/guides/*.md` for the topic bodies served by this tool.
