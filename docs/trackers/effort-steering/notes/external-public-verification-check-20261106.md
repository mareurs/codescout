---
id: '2b91b4ffb76a7f09'
kind: research-note
status: archived
title: External public verification checkpoint — 2026-11-06
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/external-public-verification-check-20261106
original_sha256: 371cb96369c4101fd1b55a3296e06bc6eebd3880929b0924fa38cc745043648e
---

# External public verification checkpoint — 2026-11-06

Latest observed controller /tmp/codescout-bg/455879-a720241d-bg_000000cc.json remains running, PID1707518. NUlow and RAxhigh candidates/costs closed, NUxhigh active, RAlow pending. results.json absent; private outcomes embargoed. Read this alongside research/effort-steering/current and external-matched-pilot-live-20261105; do not duplicate the campaign.

NUlow cost0.4412445USD, RAxhigh1.7324704000000006USD. Both context_valid true, no transport errors, builds exit0, public tests101. Both lifecycle incomplete specifically continuation limit reached with failed public verification (8continuations). CLI success is only successful actor turn, not public workflow completion. Private correctness unmeasured until all four close.

Fresh raw log read: NU verification-5/1.log contains three external_command_expand_tilde failures and multiple doctest failures; RAxhigh verification-7/1.log contains three slow-test failures plus three doctest targets. RA custom build script under /tmp/testdir/.../target/debug/build/... never executed due Permission denied. Full causes not yet attributed; prior probe issue6fe133b675285980 measured native tmpfs noexec, but current actor mount flags and same-profile unmodified baseline reproduction still required. Known selected private controls and synthetic wire validation did not qualify all these public runtime behaviors. New MAIN bug4d66b80cca2e6390 docs/issues/2026-11-06-external-public-verification-temporary-execution-failures.md captures evidence and next checks immediately.

Preserve fixed order/caps/all outcomes; do not modify frozen worker or send diagnosis to actors, retry selectively, or privately grade early. After closure, grade all candidates, archive results/costs, then reproduce public environment failures on unchanged source without model calls. Admit amended environment only as a new profile after full baseline public qualification. Separate environment/public workflow failures from private repair quality; no generic low=xhigh mechanism proved.

Disk fresh statvfs available93.36GiB after safe cleanup. All backups/SAB images preserved; reservation reply still absent. Do not attribute available-space delta solely to own deletions or wall-time differences solely to effort amid concurrent cleanup.
