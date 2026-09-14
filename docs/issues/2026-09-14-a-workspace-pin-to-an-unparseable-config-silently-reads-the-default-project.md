---
status: open
opened: 2026-09-14
closed:
severity: medium
owner: marius
related: []
tags:
- cluster/accepted-parameter-silently-dropped
kind: bug
---

# BUG: a `workspace=` pin at a project whose config fails to parse silently reads the DEFAULT project instead

## Summary

`workspace=<abs path>` is documented as "resolve this call against this workspace". When the
pinned project's `.codescout/project.toml` fails to parse, a **read** does not error and does not
return empty — it returns the **default project's** data, with nothing in the response naming the
substitution. The caller asked about project A and is answered about project B.

The write path on the identical config fails **closed** with an explicit error, so the two halves
of the same pin disagree about what an unparseable config means.

## Symptom (Effect)

Two project roots, structurally identical — same layout, same single marker memory on disk. The
**only** difference is that one `project.toml` omits the `[project]` table.

```
memory(action="list", workspace=<probe4, valid config>)
  1 topics
    probe4-unique-marker                 <- correct

memory(action="list", workspace=<probe3, malformed config>)
  23 topics
    architecture
    cargo-test-lib-skips-integration
    ...                                  <- CODESCOUT's memories, not probe3's
```

probe3's own `probe3-unique-marker` does not appear anywhere in the 23. There is no error, no
warning, and no `scope` note saying which project answered.

A write through the same malformed pin behaves correctly:

```
memory(action="write", topic=..., workspace=<malformed>)
  Error: write gate: missing field `project`
```

## Reproduction

`HEAD = 9045c56a88899d46c16387f817ca440d110e0f72` (branch `experiments`), live MCP.

1. `mkdir -p probeA/.codescout/memories`; write a valid `project.toml` (a `[project]` table with
   `name` and `languages`); drop one uniquely-named `.md` in `.codescout/memories/`.
2. `probeB` identical, except `project.toml` contains only a `[security]` table.
3. `memory(action="list", workspace=<probeA>)` → `1 topics`, the marker.
4. `memory(action="list", workspace=<probeB>)` → the **active** project's topics.

Step 3 is the control; without it, step 4 is indistinguishable from "the memory dir was unreadable".

**Each probe must be a FRESH root.** A root already resident carries a cached config, and
breaking its `project.toml` afterwards changes nothing — see
`docs/issues/2026-09-14-an-out-of-band-project-toml-edit-never-invalidates-the-cached-config.md`,
which will mask this one if you reuse a root.

## Environment

Linux, `experiments`, live MCP (stdio), release binary built 2026-09-14 11:49:52. Probe roots
under the session scratchpad, each its own git repo.

## Root cause

Unknown — not traced to a line. The observable contract is established in both directions by the
probe above, but the resolution path that turns a failed `ProjectConfig` load into "use the
default project" rather than an error has not been read.

Best lead: the write path refuses via the gate with `missing field 'project'`, so the parse
failure IS surfaced there. The read path reaches the memory store without that gate, which points
at the resolution happening before or around `Agent::ensure_resident` (`src/agent/mod.rs:668`)
with a fallback to `default_workspace_root` on load failure.

*measured 2026-09-14 by the four-step probe with its control; mechanism NOT measured — the
sentence above is a lead, not a finding.*

## Hypotheses tried

1. **Hypothesis:** probe3 returned the default project's topics because its memories directory
   was unreadable or its marker file malformed, not because of the config.
   **Test:** probe4, byte-identical in structure and marker, differing only in `project.toml`
   validity.
   **Verdict:** rejected — probe4 returned its own single topic.

## Fix

Not implemented, and the direction is a judgement rather than a detail: a pin that cannot be
honoured should either **error** (matching the write path, which already does) or **answer with
its scope stated** so the substitution is visible. Silently answering about a different project
is the one option that lets a caller act on another project's data believing it is their own.

Matching the write path is the smaller change and makes the two halves of one parameter agree.

## Tests added

None — filed, not fixed. A regression test needs two fresh roots differing only in config
validity, and must assert on the *identity* of what comes back (the marker topic), never on a
count: both projects having some memories makes a count assertion pass in the failing world.

## Workarounds

Validate `project.toml` before relying on a pin. A pinned **write** to the same root surfaces the
parse error, so it can be used as a probe for whether a pinned read can be trusted.

## Resume

Read the resolution path from the `memory` read handler down to the store, and find where a
failed `ProjectConfig::load_or_default` falls back to the default workspace instead of
propagating. Start at `Agent::ensure_resident` (`src/agent/mod.rs:668`) and
`src/config/project.rs:498` (`load_with_global_base`, which is where the `[project]` requirement
bites). Confirm whether the fallback is in the pin resolution or in the memory tool's own
project lookup — those need different fixes.

## References

- `src/agent/mod.rs:668` — `ensure_resident`
- `src/config/project.rs:498` — `load_with_global_base`
- `docs/issues/2026-09-14-an-out-of-band-project-toml-edit-never-invalidates-the-cached-config.md`
  — found in the same probe run; masks this one if a root is reused
