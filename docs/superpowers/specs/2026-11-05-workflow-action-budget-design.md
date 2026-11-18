---
id: '02ff0a616cd02962'
kind: spec
status: active
title: Bound verified workflows by cost and time rather than a pilot action ceiling
tags:
- effort-steering
- evaluation
---

# Verified workflow action budget design

## Intent

Complete the low/xhigh calibration with a client that has an opportunity to repair and verify. The closed recovery cohort contains a complete LOW and an xhigh patch passing private tests but stopped at error_max_turns32 before public checks (issue3a69dda510501d5a). User granted autonomous execution and additional spending latitude; these implementation choices are the controller's.

## Approaches

Raising32to another arbitrary ceiling retains a second unrelated censor. Removing the per-message turn option in opt-in verified workflow mode makes the existing explicit USD4 CLI/proxy ceiling,3600s overall deadline and4continuation limit the controlling bounds. Choose the latter. Legacy one-shot/scripted-without-workflow invocations keep --max-turns32 and CLI0.65 default.

## Scope and constraints

No prompt, onboarding, tools, model, output8192, public command, oracle or cold resource/deadline change. Fresh pair allocation8USD/4perarm inside cumulative60; prior33.571720199999999964 includes all closed/censored attempts. Both completed patch private acceptances from the closed cohort remain preserved; never resume/relabel it. Keep the zero-provider ex-post xhigh public diagnostic separate from model workflow completion.

## Qualification

Through-worker tests inspect actual launch argv for workflow omission and legacy preservation. A real pinned CLI/native scripted workflow must perform more than32tool actions, settle, retain its background log and pass public probes with0provider cost. Reuse unchanged native resource/cold-reference/private oracle proofs by exact hashes; qualify changed worker bytes. Review only this delta with applied mutations, complete required checks, commit exact paths and use a fresh exclusive registered root. No paid call before admission.

## Historical cost control

Add closed_recovery_history(root=None) to reconstruct both closed recovery arm costs from their raw stream/stdout/receipt coverage and verify its final ledger. Register that cost in the new thread prior; changed or unknown history blocks before credentials. Existing old-pilot and aborted-LOW history validations remain unchanged.

## Limits

Still one exposed task and fixed low/xhigh order. A valid completed pair calibrates the harness; it does not prove generic mechanism attribution, authority benefit or external transfer.
