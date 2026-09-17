# Onboarding Improvements

Three improvements to the `onboarding` tool that make it safer for large
projects, more resilient to tool API changes, and specific to the machine it
runs on.

---

## Subagent Delegation

Onboarding now offloads project exploration to a **dedicated subagent**
instead of performing it inline. This prevents the exploration phase —
which can involve dozens of tool calls across a large codebase — from
exhausting the main agent's context window.

### How it works

When `onboarding` is called on a project that hasn't been onboarded yet,
it returns a two-part response:

1. **`main_agent_instructions`** (~200 tokens) — short instructions telling
   the calling agent to dispatch a Sonnet subagent.
2. **`subagent_prompt`** — a self-contained prompt the calling agent passes
   verbatim to the subagent. Contains: preamble, step-by-step exploration
   instructions, memory templates, and an epilogue.

The subagent performs all exploration (file reads, symbol scans, language
detection) and writes the project memories. The main agent's context stays
clean.

```
Agent calls onboarding()
  → receives dispatch instructions + subagent_prompt
  → spawns subagent with subagent_prompt
     → subagent explores codebase
     → subagent writes memory files
  → main agent continues with fresh context
```

### Fast path unchanged

If the project is already onboarded, `onboarding()` returns a short status
message as before — no subagent is involved.

---

## Version-Aware System Prompt Refresh

The `onboarding` tool now tracks a version number (`ONBOARDING_VERSION`)
stored in `.codescout/project.toml`. When a project's stored version is
older than the current server's version, onboarding **automatically
dispatches a lightweight refresh subagent** to regenerate the system prompt
from existing memories — without re-exploring the codebase.

### Why it exists

When codescout's tool API changes (renames, new tools, removed parameters),
existing projects carry a system prompt that references old tool names. The
refresh detects the version mismatch and rebuilds the prompt from current
templates, so the agent's guidance stays accurate without requiring a full
re-onboard.

### Behavior

| Stored version | Action |
|---|---|
| Missing (pre-versioning project) | Triggers refresh |
| Lower than `ONBOARDING_VERSION` | Triggers refresh |
| Equal to `ONBOARDING_VERSION` | No-op (already current) |
| Higher (downgrade scenario) | No-op (avoids churn) |

### refresh_prompt parameter

To force a prompt refresh explicitly — for example, after updating memories
manually — pass `refresh_prompt=true`:

```
onboarding(refresh_prompt=true)
```

This regenerates the system prompt from current memories and templates
without re-scanning the project. Useful after bulk memory edits or after
upgrading codescout to a new version that bumps `ONBOARDING_VERSION`.

### What gets refreshed

The refresh subagent reads existing project memories and rewrites the system
prompt section of `.codescout/project.toml`. It does not re-read source
files or re-scan the project structure — only the prompt template is
regenerated.


---

## Hardware-Aware Model Selection

Onboarding picks the starting value for `[embeddings]` from the machine it runs
on, and writes the winner into `.codescout/project.toml`. The full ranked list —
every option, with the reason it was ranked where it was — travels in
`subagent_prompt` under **Model options**, so the agent can offer alternatives
rather than only the one that was chosen.

### What each probe is evidence for

| Probe | Ranks | Why |
|---|---|---|
| `gpu` | the **Ollama** entry, never a `local:` one | The local ONNX path is CPU-only in every shipped build — `codescout-embed` selects `ort`'s CPU backend and registers no execution provider. A GPU accelerates what Ollama serves and does nothing for `local:`. |
| `ram_gb`, `cpu_cores` | which local model leads | Local embedding is CPU-bound and runs over the whole repo at index time, so a ~300 MB code model is a real cost on a small host, not a preference. |
| `ollama_available` | whether the Ollama entry exists at all | A TCP probe of `$OLLAMA_HOST`. |

The local crossover is **16 GB RAM and 8 cores**: at or above both,
`local:JinaEmbeddingsV2BaseCode` leads (768d, 8192-token context,
code-specialized); below either, `local:AllMiniLML6V2Q` leads (384d, 22 MB).

### Compiled features are part of the ranking

A binary built without `local-embed` cannot load a `local:` model — it answers
`Local embedding requires the 'local-embed' feature`. The ranking reads its own
feature set, so such a build never recommends one: it leads with Ollama if a
server is reachable, and otherwise with the external-server option. A build with
neither `local-embed` nor `remote-embed` says so in the option's `reason` rather
than recommending something that cannot run.

### Reading an option

Each entry carries a `target` naming which field to set:

```json
{"target": "model",  "model": "local:AllMiniLML6V2Q"}
{"target": "server", "url": "http://localhost:11434/v1", "model": "nomic-embed-text"}
```

`available: false` means the option needs something first — a key, a url, or a
rebuild. It is why the OpenAI entry is never marked available: no key is probed
for.

> The `openai:` entry exists so the API-key path is *offered* at onboarding
> rather than only documented. See
> [Embedding Backends](../configuration/embedding-backends.md) § Authentication.
