# Wine Windows test suite (set up 2026-11-08)

This machine can run Windows builds of codescout tests and Windows Python tests under Wine. Use it to
reproduce or refute a Windows-only issue before asking for a Windows host. It is NOT Windows: read the
"What Wine can and cannot prove" section before you cite a result.

## What is installed (verified 2026-11-08)

- Wine 11.21 (`/usr/bin/wine`), default prefix `~/.wine`, arch `win64`.
- Rust target `x86_64-pc-windows-gnu` (rustup) and mingw-w64 gcc (`x86_64-w64-mingw32-gcc`).
- Windows Python 3.11.9 (64-bit, MSC 1938) in the prefix at `C:\Python311` (`~/.wine/drive_c/Python311`),
  with `pytest` 9.2.0. `sys.platform == "win32"`, `os.name == "nt"`, `signal.CTRL_C_EVENT == 0`.
  Not on the Wine PATH: call it by full path.
- Installer provenance: `python-3.11.9-amd64.exe` from python.org, 26216840 bytes, GPG signature good,
  key `FC624643487034E5` (Steve Dower, Python Release Signing), fingerprint
  `7ED1 0B65 31D7 C8E1 BC29  6021 FC62 4643 4870 34E5`. python.org lists no MD5 any more. The installer ran
  with `/quiet InstallAllUsers=0 TargetDir=C:\Python311 PrependPath=0 Include_launcher=0
  InstallLauncherAllUsers=0 Include_doc=0 Include_test=0 Shortcuts=0 AssociateFiles=0` and took 14 s.

## Recipes

Rust lib tests (cross-build, then run one test under Wine). The lean feature set builds in ~1m20s:

    scripts/with-slot.sh cargo test --lib --no-default-features --target x86_64-pc-windows-gnu --no-run
    WINEDEBUG=-all wine <slot>/x86_64-pc-windows-gnu/debug/deps/codescout-<hash>.exe <filter> --test-threads=1

Run the exe at once: the binary lives in a pool slot that the next build may reuse. The exe file name hash
does not change when the source changes, so judge a rebuild by the test result, not by the name.

Python tests (run from the repo root; the two env/flags keep the repo clean):

    PYTHONDONTWRITEBYTECODE=1 WINEDEBUG=-all wine 'C:\Python311\python.exe' -m pytest tests/<file>.py -q -p no:cacheprovider

stderr carries `MESA-EGL: warning ... pci id` noise on every run: ignore it.

Controlled red (always do it; a test that is green on the broken code proves nothing): for Rust use a
throwaway `git worktree add --detach <scratchpad>/wt HEAD`, edit there with `edit_file(workspace=<wt>)`,
build and run, then `git worktree remove --force`. For Python copy `scripts/` and the test file into a
scratch tree and mutate the copy. Never mutate the shared checkout.

## What Wine can and cannot prove

| Question | Wine |
|---|---|
| Path-string logic: `\\?\C:\` verbatim vs `C:\` spellings, `strip_verbatim` | Yes. Seen red on the pre-fix line and green with the fix (bug 52e82dd3ddd5d6a0, `6e5c9bffa`). |
| Win32 API calls through ctypes: `OpenProcess`, `GetExitCodeProcess` | Yes for live and dead pids. |
| `os.kill(pid, 0)` console Ctrl+C broadcast (bug 6d3b04bf30e0fcf9) | NO. Wine does not deliver the console event: the "sends a signal to nobody" test is green on the broken code, so it is uninformative under Wine. |
| Windows pid 4 (System) exists, `OpenProcess` gives access denied | NO. Wine returns error 87 (invalid parameter), so `test_a_process_that_exists_but_cannot_be_opened_is_alive` fails under Wine for a reason that is not a defect. |
| A backslash in a file name (bug 2d8475ed5bc831e5 and relatives) | NO. Wine's `Z:` drive is the Linux filesystem, where `\` is legal in a name. NTFS forbids it. |

Rule: a failure under Wine proves the defect only if Wine behaves like Windows for that call. A pass proves
nothing about Windows. Record any such result as `unverified: seen under Wine 11.21 only`.

## Results so far

- 52e82dd3ddd5d6a0: `tag_external_path_returns_project_for_a_uri_*` green with the fix, red (`lib:src` instead of
  `project`) on the pre-fix line.
- 6d3b04bf30e0fcf9: `tests/test_pid_alive.py` with the fix: 7 passed, 1 failed (pid 4, a Wine difference).
  On the pre-fix mutant: the dead-pid case failed (a process that exited read as alive) and the out-of-range pid
  raised `OverflowError`. The Ctrl+C broadcast did not reproduce.

Still needs a real Windows host or CI: the Ctrl+C broadcast, pid 4, backslash file names.
