# Effort-steering archive file locations

Recorded on 2026-11-18. Archive branch: `archive/effort-steering-20261118`.

## Canonical locations

- Git entry point: `archive/effort-steering-20261118/README.md` on the archive branch.
- Historical experiment index: `docs/trackers/effort-steering/README.md` on that branch.
- External evidence root on this machine: `/home/marius/archives/codescout/effort-steering-20261118`.
- Local archive worktree: `/home/marius/work/claude/codescout.worktrees/effort-steering-archive-20261118` (convenience checkout; the branch is the Git reference).

The external root is retained evidence, not a disposable cache. Its bytes are local only: pushing this branch does not upload them or create an independent backup.

## External evidence capsules

Paths below are relative to the external evidence root above. Sizes and SHA-256 values are copied from `retirement-complete.json`; existence and byte sizes were checked again for this locator. This locator update did not recompute the capsule hashes.

| Relative path | Bytes | SHA-256 |
|---|---:|---|
| `capsules/docs/trackers/effort-steering/evidence/closed-paid-pilot-20261111/evidence.tar.gz` | 2584835393 | `42d8f9a0ca4d95eccd177761d9aeb7a3db1d289644e06787b471aa9f977b4fb8` |
| `capsules/docs/trackers/effort-steering/evidence/contract-resources-20261111/evidence.tar.gz` | 301383544 | `07fca37fce6f3afc7976d1d3d8defc4c07996bb7cc0e26722b6fd7081dfe8860` |
| `capsules/docs/trackers/effort-steering/evidence/disk-runtime-checkpoint-20261111/evidence.tar.gz` | 1330356694 | `d5c81f93a75b644505329ae9c65c2a03213d95cd5ce327d0ffeedae1ed6759ce` |
| `capsules/docs/trackers/effort-steering/evidence/headroom-admission-20261111/evidence.tar.gz` | 4951515050 | `2d115e5e6092fe3dcceaa146a82ef567b608e6876ecbbecee86ecb01e8be7948` |
| `capsules/docs/trackers/effort-steering/evidence/paid-budget-stop-20261111/evidence.tar.gz` | 503072529 | `18c1665382759b1e8cb27c7663aa6da63f4b9c40a1c43588b0c54d2afe82a014` |
| `capsules/docs/trackers/effort-steering/evidence/persistent-native-admission-20261111/evidence.tar.gz` | 7041278551 | `ccdc360d73de3bac91c53c908d770837b618e1a385da5a1cabe62032e12a7ca4` |
| `capsules/docs/trackers/effort-steering/evidence/shared-public-runtime-20261110/final-verification-v2.tar.gz` | 254750617 | `2dd6bb3d81257f768aa2cfd9e3c978e3bc97608f095678ef84e441cd5efbb5da` |
| `capsules/docs/research/data/2026-11-05-external-onboarding-admission-evidence.tar.gz` | 73605618 | `8ccfd8fecbcc4d4b839e6d41d9e505abd62ea52843372cfabf2f205752fac315` |
| `legacy-resident.tar.gz` | 51992483 | `71243f3a730cb59fd71850d389fe6f006574ce99dc418951e8dd694ee2570c77` |

## Other retained external files

These files are also recorded in the completion receipt. Their existence and byte sizes were checked for this locator.

| Relative path | Bytes | SHA-256 |
|---|---:|---|
| `main-index-before.patch` | 506980 | `033f5b1f56779fd9895b1d7e3a8f9cd1f219cca00cc526bbea0a2de66354dd8b` |
| `main-index-before.z` | 840345 | `1ea4f0b56357add9b66ca4a7c20f36146be58348078a9de33fe00b3183be2273` |
| `main-retirement-progress.jsonl` | 366658 | `895ae9cbed867bfe2e2d2270fd179f78ec6ec165ed4362aa3e70d3d6867105af` |
| `supplement-retirement-progress.jsonl` | 45484 | `2e29eb745c84cb431e149cf7db0b0e3b2fcbf9029eb94f39202342856b28f9b0` |
| `research-history.bundle` | 40016909 | `4a17be94e97b8e73c2199511fba516d7da11fa835785d8a4a95e8276f72b52c4` |
| `key-scope-before.json.full.json` | 40314 | `d8f76fbfa06606d28f1025be9e4df02fbccc2ab4438d19d0ea7433ad41d1a222` |
| `key-scope-probe-after.json.full.json` | 29942 | `acc9e9d5f8d6a4e39617217e584e769f51cf99db1137ea514f2bf40f04b2fba8` |
| `key-scope-final.json.full.json` | 29942 | `acc9e9d5f8d6a4e39617217e584e769f51cf99db1137ea514f2bf40f04b2fba8` |

## Original files and recovery map

- `main-original-files/<original relative path>` holds individual originals moved out of the main checkout.
- `main-original-directories/<original relative path>` holds the retired SDD recipe directories and the original `.codescout/resident-private/blind-challenger-20261101` directory.
- `custody-manifest.json` and `supplemental-custody-manifest.json` on the branch map source paths to retained files, tar members, sizes and hashes.
- `main-retirement-primary.json` and `supplement-retirement.json` on the branch record retirement operations. The external root retains their local receipts and progress journals.
- `research-history.bundle` preserves research HEAD `d7236b843a231b63f39e191ee6d85b27619d75a3`; its prerequisite is `32d0c713de272738ae7ccba9e1e4fccb004600fe`, reachable from archive base `034e4783f9ba65d1b8da735263c8bfed61e2ae58`.
- Small results, experimental code snapshots, manifests and verification reports are in Git; the README links their exact files.

## If the evidence root moves

Copy and verify the retained bytes before retiring the old location. Commit the new locator and custody mapping to the archive branch, and update the README beside the external files. Keep the previous mapping as history. Preserve sealed capsule bytes and symlink targets; a new storage location does not make old runtime registrations portable.

Recover into a separate checkout. The research remains paused and its generic mechanism unvalidated.
