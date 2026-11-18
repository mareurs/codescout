---
id: c34f5b7ed55241d0
kind: report
status: active
title: 'Immutable real-project execution profiles: runtime qualification and guard review'
tags:
- effort-steering
- runtime
- measurement
topic: effort-steering
---

# Immutable real-project execution profiles — 2026-11-09

## Result and scope

The generic protocol now has host-selected immutable execution profiles. On tracked public rust-analyzer source, the actual executor ran offline metadata, compiled public stdx library tests, a source-write rejection probe, and an intentionally failing public integration-test overlay. Their exit codes were 0, 0, 0 and 101; all four observations were complete, resource-valid, independently reverified receipts, with the baseline source unchanged. No actor, provider, private grader, onboarding author, image build or image pull was used.

The final local protocol suite discovered 156 tests: 154 passed, two explicit native CLI controls skipped, 81.559s. Native P/R synthetic results belong to the earlier [native qualification](2026-11-09-native-phase-cli-qualification.md), not a fresh real-project four-arm comparison. Generic repair efficacy is unproven and paid admission remains closed.

## Generic execution contract

fixture-v1 keeps the previous default: 256 MiB, 1 CPU, 128 pids, executable 64 MiB /tmp, source /work. cargo-v1 uses an 8 GiB container memory/swap cap, 4 CPUs, 512 pids, executable 1 GiB /tmp, source /testbed, offline Cargo and a fresh /build/target. This is a per-container cap, not an assertion about host RAM.

Both retain a read-only root and source, no network, dropped capabilities, exact existing image and independent writable build tree. The image declares VOLUME /mnt_tmp; cargo-v1 explicitly maps it to build/scratch, with TMPDIR=/mnt_tmp. Other unexpected mounts are still rejected. The build digest includes scratch. Actor command fields cannot choose or override the host profile.

Fresh receipts bind the canonical profile digest and raw before/after runtime inspections. Readback revalidates image/worker identity, ownership, limits, mounts and environment. Raw owned inspections are preserved before profile validation so rejected profiles remain diagnosable. Cleanup uses --volumes for the exact owned container, verifies its absence and the absence of its observed anonymous volumes. It neither prunes dangling volumes nor deletes other sessions' resources.

## Real-source controls and exact inputs

Pinned existing image: sha256:a7108935ae3cb9c6573af9f0e371dbaaffb4c9a88a92038e334184bf8672cc32. Source commit: 3678da48aebe17797d2f792bde07a46f5bf4a827. The public export contains exactly 2332 tracked paths, with tar member names checked against the complete git inventory. Tar bytes: 26183680; SHA-256: e567df647118146f0eea7974019b3b3d3f5479c0e0733ece3844075d803e23d9.

Baseline input digest: 7615f687a70e5fd0b2846c962ba4cbdc177b81e94564046c38dcbd2e24375c94. cargo-v1 digest: 847ebb408fd0debfc249345cd9a84d146fa53c6eecf8a4e5add1201a3259abb8.

| Corrected V3 control | Declared regular files in test role | Exit | Interpretation |
|---|---:|---:|---|
| cargo metadata --offline --locked --no-deps | 2 | 0 | Workspace /testbed, target /build/target |
| cargo test --offline --locked -p stdx --lib | 15 | 0 | Public leaf-crate compilation/test command succeeds |
| Attempt append to /testbed/Cargo.toml | 1 | 0 | Probe observed expected read-only-filesystem error |
| Deliberately failing stdx integration test | 16 | 101 | A functional failure remains a valid nonzero observation |

Public/negative test digests are d6703ca848bae49be16bdda5d2332fd06c6a480214acbed4d4a9f2fd84cb1a41 and c5100f44f01352cccb5d7365bc89ee114f3fcbf8c20e904681d0f34221be6304. The negative full source digest is ee13ef5bacd1a469721fb8c34c76fcb0b2744706889afce862bc86a226428ae9. All four receipts were independently read with expected_profile=cargo-v1.

These inventory counts measure regular paths explicitly bound to the test role, not the number of Rust tests or the complete transitive build closure. The full source digest binds all source inputs; the exact image binds installed tools/dependencies. This leaf-crate control does not qualify a full rust-analyzer repair, another language, or substantive onboarding.

## Failed controls kept visible

The preliminary direct runtime probe compiled successfully, but copying its export after container stop failed because the export lived on tmpfs. The later export streamed directly to the host; the failed copy remains failed.

The first actual executor attempt rejected the image's undeclared /mnt_tmp volume before executing Cargo. Raw inspection publication followed validation, so its raw before row was lost. Two consuming rejection cases and an anonymous-volume cleanup case then reproduced the gaps. Their corrections have passing focused controls.

V2's four commands ran, but its test role named crates/stdx as a directory. The old selected-path digest bound directory metadata, so positive and negative test digests matched despite differing complete source digests. V2 is retained as limited evidence. The executor and receipt reader now require explicit regular test files, rejecting directories and symbolic aliases. Two consuming tests first produced four expected failing subcases and then passed. V3 enumerates the actual 15/16 file inventories above.

Earlier prototype attempts may have left anonymous volumes whose ownership cannot now be established. They have not been deleted by age, dangling status or guesswork.

## Applied review

An independent stronger reviewer applied six concrete mutations to a frozen snapshot. All six mutation runs were valid, each executing the same 12 selected profile controls without skips. One actor override mutant was killed; five survived: runtime environment equality, raw worker identity, raw inspection hash, cleanup ownership and anonymous-volume absence.

Only after observing those survivors, the reviewer wrote five discriminating edge probes: original five passed, and each exact surviving mutant failed its matching probe. Equivalent regressions were integrated into the parent source. Final-source replay observed baseline17/17 and inventory baseline2/2 passing. Each of the six exact original mutants failed one of17 tests; each of the two test-inventory call-site mutants failed two subcases in its2-test suite. All eight final applied candidates were killed, zero survived, zero invalid, with no errors/skips. Every restored copy passed: M1–M6 each17 executed, M7/M8 each2 executed, zero skips/errors/failures. Source hashes match the parent final snapshot. Reviewer scoped approval is complete; raw copies, exact patches, per-run counts and the summary are quiescent for preservation. Selected guard mutation checks are not exhaustive verification. This is a test-rigor finding, not evidence that the production guards had been removed.

## Issue records and remaining work

[Raw inspection loss](../../../issues/2026-11-09-rejected-execution-profile-loses-runtime-inspection.md), [image-declared volume](../../../issues/2026-11-09-cargo-profile-image-declared-volume.md), [anonymous-volume cleanup](../../../issues/2026-11-09-executor-cleanup-leaves-anonymous-image-volume.md), [test inventory](../../../issues/2026-11-09-test-input-inventory-accepts-directory-only-digest.md), [guard regression gaps](../../../issues/2026-11-09-execution-profile-guard-test-gaps.md).

Current source is uncommitted in cf66 on research/evidence-protocol-20261109, base fe8bb4b34f5e7912f9b16c4c0bf1ff73a3a10a23. No staging, merge or push. The earlier public-handoff archive is an earlier source snapshot. Current code and evidence are preserved in the [manifest](../evidence/execution-profiles-20261109/manifest.json), [compact source/review/log archive](../evidence/execution-profiles-20261109/source-and-review.tar.gz) and [readable qualification summary](../evidence/execution-profiles-20261109/qualification-summary.json). The compact archive is1484875 bytes, SHA-2562726b3d6baeff6e1d3f4e53ce9eadbbebb5d5d256841d8348dd805d290763927.

The full source/build closure stays outside the repository at /home/marius/.cache/codescout-resident/codescout/execution-profiles-20261109/execution-profiles.tar.gz:115587963 bytes, SHA-25605ef21706334c7ff72340079fce6a0177ab8b986b166177cdb4e638ce173dcaf. Full readback verified33044 logical entries:29781 regular-file paths (29771 regular headers and10 hard-link headers),36 symbolic links and3227 directories. Bytes, sizes, modes, link targets and complete membership were checked, then original paths rechecked unchanged. The exact dependency image is not exported; it remains required. Raw receipts contain original absolute host mount paths, so archive preservation alone does not establish replay from another location.

The [initial archive-verifier failure](../../../issues/2026-11-09-evidence-archive-verifier-rejects-hardlinks.md) remains recorded with its log. It confused logical regular files with tar hard-link headers. The [corrected verifier](../evidence/execution-profiles-20261109/verify_archive.py) reads and hashes both representations. Its [known-fixture calibration](../evidence/execution-profiles-20261109/verifier-calibration.json) accepted one positive case and rejected four deliberate byte/target/population/mode defects.

The previous whole-product gate remains FAILED on its earlier frozen source and environment. This scoped Python suite does not turn it green. After this adapter checkpoint: substantive source-only onboarding and common L/R/P/X preparation, context/tool readiness in all relevant arms, outcome-blind cohort registration and complete cost/accounting admission. Policy must remain generic; this exposed public source is development input.

## Documentation checks

Scoped CommonMark local-path checks and their exact file hashes are retained in [documentation QA](../evidence/execution-profiles-20261109/documentation-qa.json). The separate [Codescout reference audit summary](../evidence/execution-profiles-20261109/librarian-qa.json) records its full counters, scan degradation state and output limits. Neither instrument checks external links or semantic truth. These checks apply to explicitly selected current documents, not every historical note.
