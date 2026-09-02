---
kind: tracker
status: active
title: 'an assertion that cannot fail is zero coverage wearing a passing test''s clothes'
owners:
- marius
tags:
- defect-classes
- clusters
- assertion-that-cannot-fail
topic: issue clusters and rule promotion
---

## IC-16 — an assertion that cannot fail is zero coverage wearing a passing test's clothes

**Slug:** `cluster/assertion-that-cannot-fail`
**Claim:** An assertion has **no input that would make it fail**. It is not weak coverage — it is zero coverage wearing a passing test's clothes, and it is added most often in the very commit that closes a missing-guard finding.
**Members:** `filter={"tags": {"contains": "cluster/assertion-that-cannot-fail"}}` **New member 2026-09-02: `a-byte-ceiling-test-cannot-see-a-member-stop-delivering`** — an ADJACENT fit, recorded rather than stretched. This clause's headline (*no input would make it fail*) is strictly false of it: `total > 0` over six addends fails if all six are zero. The fit is to the gloss — *zero coverage wearing a passing test's clothes* — with the difference being **no input within the assertion's CLAIMED SCOPE**. The per-member claim is unfalsifiable while the population claim is not; that variant is a law in `CLAUDE.md` § *Testing Discipline* rather than a class here, deliberately, because it is a property of assertions and not a defect class instantiated by bug files. — **`n=3`, 2026-09-01, by query.** Third instance filed 2026-09-01: `docs/issues/archive/2026-09-01-pinnable-assertion-vacuous-for-an-unregistered-tool.md` — `server_advertises_workspace_param_only_for_pinnable_tools` asserted `!pinnable.contains("get_usage_stats")` where `pinnable` is built from the **registry** and the tool was never registered, so no input could fail it. The other two are `ollama_large_batch_exceeding_batch_size` (vacuous the day it was written) and `cross-process-write-lock-test-passes-when-it-does-not-run` (vacuous when skipped). `CLAUDE.md` records four more from a single SDD run, untagged.
**Blind party:** the reviewer, structurally — a passing test is the evidence they are given, and vacuity is invisible in exactly that evidence. `CLAUDE.md` measures it: of four found in one run, *"the fourth only because the final reviewer was told to hunt for one."* Care does not find these; a changed question does. **The third instance is a clean confirmation:** it was not found by reading the test, but while resolving whether a *tool* was reachable — a different question that happened to pass through the same three lines.
**Promotes to:** **clears both bars as of 2026-09-01** — `n=3` across three subsystems (embeddings transport, cross-process locking, MCP tool registry). What the third instance buys is **measurability, not a rule**: `CLAUDE.md` § *Testing Discipline* and § *SDD Rulings* already carry the substance (*"Ask 'what mutation would make this test fail?', never 'does it pass?'"*, and *demand a deliberate break*), so no rule is owed. The open item is the **mechanism**. This field previously read *"below threshold at `n=2`, which is now the only bar it fails"*; that bar is passed, and the sentence is superseded rather than deleted because the count is what moved and nothing else did.
**Mechanism status:** `designed` — the rule exists and is written down; nothing enforces it. Mutation testing per guarded site is the mechanism, applied by hand today. **The third instance names a narrower, buildable one:** an absence assertion over a name list should first assert the **positive** — that each listed name is actually produced by something in the population being searched — and only then that it is absent from the filtered subset. Without that, `!contains` cannot distinguish *correctly excluded* from *never present*. Not built; it would have caught this instance on the day it was written.
**Valid:** dated 2026-09-01

**Boundary against `IC-9`, which is a strict sub-case and must not absorb this.** `IC-9`'s assertion *can* fail — roughly 1-in-800, when a random tempdir name happens to contain the needle. Its mechanism is environment-controlled text in the haystack. This class is the harder one: **no input fails it at all**, so no run frequency, no environment and no amount of CI time will ever surface it. An `IC-9` member is a flake; a member here is a permanent zero.

That distinction is why the two withdrawn tags were withdrawn rather than left. Both read from their titles as *"a test that passes when it shouldn't"* — true of `IC-9` and true of this class and true of several others — and title-matching is what produced the misfit. The claim, not the title, is the admission test.

**This one is deliberately opened despite the rule already existing**, which reverses the usual direction: normally a cluster accumulates until it earns a rule. Here `CLAUDE.md` got the rule first, from an SDD run, and the *corpus* was never indexed against it — so the question *"which of our bugs are instances of the vacuous-assertion rule?"* has no answer, and nobody can tell whether the rule is working. Opening the class is what makes the existing rule measurable rather than merely stated.

**Falsified by** the identified members turning out to have a failing input after all, which would move each of them to `IC-9` or to an ordinary coverage gap.
