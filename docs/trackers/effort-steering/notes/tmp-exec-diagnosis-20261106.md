---
id: ebf03d9ed7c7e3c6
kind: research-note
status: archived
title: Temporary execution diagnosis — 2026-11-06
tags:
- effort-steering
- migrated-memory
- historical
topic: effort-steering
interpretation: Historical snapshot; consult the experiment index for current state.
migrated_on: 2026-11-09
original_memory_topic: research/effort-steering/tmp-exec-diagnosis-20261106
original_sha256: 84fd0565e5bb38c7787bf4f55df890249352a23585290032efe74b08eb73c93e
---

# Temporary execution diagnosis — 2026-11-06

See MAIN bug4d66b80cca2e6390 docs/issues/2026-11-06-external-public-verification-temporary-execution-failures.md, now supplemented with actual same-image paired controls. Evidence docs/research/data/2026-11-06-tmp-exec-diagnosis.json SHA25618243378597a8e1c186ff2ee0d83034f5621112c804cd7be38b6b26c6b7ee578,21492bytes independently reopened as JSON. User suggested disk exhaustion as permission-error cause; tested rather than assumed.

RA closed native argv specifies1GiB /tmp tmpfs with rw,size=1g,uid1000,gid1000; exec unspecified. Read-only live NUxhigh actor mountinfo actually shows noexec, /tmp available1071300608bytes and /testbed96826019840bytes (point-in-time, not failed-run historic telemetry). Host /tmp and actor tmpfs separate.

Disposable RA exact-image UID1000, networknone, readonlyroot paired controls: identical copied /bin/true mode0755, tmpfs registered flags failerrno13 while1073696768bytes free; adding only exec passes0. Separate trivial rustdoc1.90.0 doctest: noexec fails101 explicitly Permission denied/maybe tempdir noexec, exec passes1/1. All four diagnostic containers independently absent. No model calls, frozen inputs/actors unchanged. This proves harness mount policy prevents temporary executable fixtures; no disk shortage required. It does not prove every paid doctest/tilde/slow failure same cause or qualify complete baseline suite.

Full available verification/background/stream-stderr logs scanned: NUlow16files, RAxhigh19. Literal disk-space error matching lines0each; permission-error18RA/0NU. These are text predicates, not disk telemetry. Candidate/private correctness still embargoed pending closure of all4. Do not mutate running cohort or retry selectively. After closure finish private grading, then full unchanged-baseline public qualification with developer-compatible exec policy under new registered harness profile.
