---
id: d3684a0f0e798b39
kind: bug
status: open
title: 'BUG: Tool::pinnable excludes activate_project, a name no registered tool produces'
tags:
- tools
- workspace-pin
- dead-code
- cluster/declared-not-wired
opened: 2026-10-06
owner: marius
related:
- docs/issues/archive/2026-09-24-residual-pinnable-positive-form-assertion.md
severity: low
---

# BUG: `Tool::pinnable` excludes `activate_project`, a name no registered tool produces

## Summary

The exclusion arm of `Tool::pinnable` lists `"activate_project"`. No tool is registered under that name, so the arm can never match. A test exempts it by name instead of removing it.

## Symptom (Effect)

`src/tools/core/types.rs:1315-1324`:

```
fn pinnable(&self) -> bool {
    !matches!(
        self.name(),
        "workspace"
            | "activate_project"
            | "onboarding"
            | "get_guide"
            | "__probe_description_cap__"
    )
}
```

`src/server.rs:6720-6725`, in `every_pinnable_exclusion_names_a_registered_tool_or_a_named_exemption`, says:

```
//  * `activate_project`: `ActivateProject` is dispatched to by `Workspace`
//    (`action="activate"`), never registered on its own. The arm is dead for the
//    wire surface — filed as a follow-up, not deleted here.
```

## Reproduction

```
git rev-parse --short HEAD    # 10e935e3, branch experiments
```

1. Read `src/tools/config/mod.rs:103`. `Workspace::call` dispatches `"activate"` to `ActivateProject.call(...)`.
2. Run `grep -n ActivateProject src/server.rs`. Three lines match: one doc comment and two lines in the test above. There is no registration.
3. Read the test at `src/server.rs:6716`. It exempts `crate::tools::config::ActivateProject.name()` by name.

## Environment

Linux, `experiments` at `10e935e3`.

## Root cause

`ActivateProject` appears to be a leftover from before `workspace(action="activate")` took its place. The struct remains because `Workspace` calls it. The arm in `pinnable` was not removed. The new positive-form test (`4c37ba0d`) found the dead name and exempted it instead of deleting it.

Read from the source; not measured at runtime. The history is inferred from the type's remaining role and the test comment, not from `git log`.

## Evidence

Quoted above. The archived record `docs/issues/archive/2026-09-24-residual-pinnable-positive-form-assertion.md` says in its Resume: "the `"activate_project"` arm in `Tool::pinnable` ... is dead for the wire surface, since no tool registers under that name; the new test exempts it by name rather than deleting it." It listed this as a follow-up and did not file it.

## Hypotheses tried

None needed.

## Fix

Not started. Remove `"activate_project"` from the arm and the matching entry from the exemption array and its assertion at `src/server.rs:6730-6734`. Keep `ActivateProject` itself, because `Workspace` calls it. Check that nothing relies on `ActivateProject.pinnable()` being false when it is called directly.

## Tests added

N/A — not fixed. The existing exemption test will fail on the extra exemption, which is the intended prompt.

## Workarounds

None needed. The arm is harmless.

## Resume

Delete the arm and the exemption in one change.

## References

- `docs/issues/archive/2026-09-24-residual-pinnable-positive-form-assertion.md`, Resume (1).
- `docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md`: the earlier case of the same shape.
- Cluster `IC-3`, read loosely: a surface declares something production never reaches. Here the declaration names a tool that no longer exists, which is the reverse direction.
