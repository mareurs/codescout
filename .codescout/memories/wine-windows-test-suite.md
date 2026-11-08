# Wine Windows test suite (set up 2026-11-08)

This machine can run Windows builds of codescout tests and Windows Python/Node tools under Wine. Use it to
reproduce or refute a Windows-only issue before asking for a Windows host. It is NOT Windows: read the
"What Wine can and cannot prove" section before you cite a result.

## What is installed (verified 2026-11-08)

- Wine 11.21 (`/usr/bin/wine`), default prefix `~/.wine`, arch `win64`, emulating Windows 10.0.19045.
  `cmd.exe`, `conhost.exe`, `wineconsole.exe` exist in `system32`.
- Rust target `x86_64-pc-windows-gnu` (rustup) and mingw-w64 gcc (`x86_64-w64-mingw32-gcc`).
- Windows Python 3.11.9 (64-bit, MSC 1938) at `C:\Python311` (`~/.wine/drive_c/Python311`), with `pytest` 9.2.0.
  `sys.platform == "win32"`, `os.name == "nt"`, `signal.CTRL_C_EVENT == 0`. Not on the Wine PATH: call it by full path.
- Windows Node 22.23.4 (zip, sha256 `cbc74b5e0982c9dbb41525fd8fb5dcd32fbee237863f7829072f4639a1ce5f20`, checked
  against nodejs.org `SHASUMS256.txt`) at `C:\node`. Not on the Wine PATH.
- pyright 1.1.413 (the version of the Windows report in bug 5d2b664a26dfaee3) at `C:\pyright\node_modules`, with the
  `pyright.cmd` and `pyright-langserver.cmd` shims. Entry point: `C:\pyright\node_modules\pyright\langserver.index.js`.
- Backup of the prefix before the Node/pyright install: `~/.wine.bak-20261108` (1.9 GB). Delete it when the prefix is
  known good.
- NOT installed: Git for Windows (`bash.exe`), `winetricks`, `wine-mono`, `wine-gecko`. `winetricks` and `wine-mono`
  are in the pacman `extra` repo (installing needs the user's approval and root). Git for Windows is blocked: on
  2026-11-08 `api.github.com` answered 504 four times and `github.com` timed out. Retry later.
- Installer provenance, Python: `python-3.11.9-amd64.exe` from python.org, 26216840 bytes, GPG signature good, key
  `FC624643487034E5` (Steve Dower, Python Release Signing), fingerprint
  `7ED1 0B65 31D7 C8E1 BC29  6021 FC62 4643 4870 34E5`. python.org lists no MD5 any more. Silent install options:
  `/quiet InstallAllUsers=0 TargetDir=C:\Python311 PrependPath=0 Include_launcher=0 InstallLauncherAllUsers=0
  Include_doc=0 Include_test=0 Shortcuts=0 AssociateFiles=0` (14 s).

## Recipes

Rust lib tests (cross-build, then run one test under Wine). The lean feature set builds in ~1m20s:

    scripts/with-slot.sh cargo test --lib --no-default-features --target x86_64-pc-windows-gnu --no-run
    WINEDEBUG=-all wine <slot>/x86_64-pc-windows-gnu/debug/deps/codescout-<hash>.exe <filter> --test-threads=1

Run the exe at once: the binary lives in a pool slot that the next build may reuse. The exe file name hash does not
change when the source changes, so judge a rebuild by the test result, not by the name.

Python tests (run from the repo root; the flags keep the repo clean):

    PYTHONDONTWRITEBYTECODE=1 WINEDEBUG=-all wine 'C:\Python311\python.exe' -m pytest tests/<file>.py -q -p no:cacheprovider

A script outside the prefix is reached as `Z:\home\...`. stderr carries `MESA-EGL: warning ... pci id` noise on every
run: ignore it.

Node and npm: run `wine 'C:\node\node.exe' ...` with stdout and stderr redirected to a FILE, never to a pipe (see the
pipe finding below). Install a package: `wine 'C:\node\node.exe' 'C:\node\node_modules\npm\bin\npm-cli.js' install
--no-audit --no-fund --prefix 'C:\<dir>' <pkg> > out.txt 2>&1 < /dev/null`.

pyright language server probe: `scratchpad/lsp_probe.py`-style, run with Wine Python: spawn `C:\node\node.exe
C:\pyright\node_modules\pyright\langserver.index.js --stdio` with `subprocess.PIPE` from inside Wine, send
`initialize`. Result 2026-11-08: answered in 0.3 s, exit code 0, stderr empty.

Controlled red (always do it; a test that is green on the broken code proves nothing): for Rust use a throwaway
`git worktree add --detach <scratchpad>/wt HEAD`, edit there with `edit_file(workspace=<wt>)`, build and run, then
`git worktree remove --force`. For Python copy `scripts/` and the test file into a scratch tree and mutate the copy.
Never mutate the shared checkout.

## What Wine can and cannot prove

| Question | Wine |
|---|---|
| Path-string logic: `\\?\C:\` verbatim vs `C:\` spellings, `strip_verbatim` | Yes. Red on the pre-fix line, green with the fix (bug 52e82dd3ddd5d6a0, `6e5c9bffa`). |
| Win32 API calls through ctypes: `OpenProcess`, `GetExitCodeProcess` | Yes for live and dead pids. |
| `os.kill(pid, 0)` console Ctrl+C broadcast (bug 6d3b04bf30e0fcf9) | NO. Wine does not deliver the console event: the "sends a signal to nobody" test is green on the broken code, so it is uninformative under Wine. |
| Windows pid 4 (System) exists, `OpenProcess` gives access denied | NO. Wine returns error 87, so `test_a_process_that_exists_but_cannot_be_opened_is_alive` fails under Wine for a reason that is not a defect. |
| A backslash in a file name (bug 2d8475ed5bc831e5 and relatives) | NO. Wine's `Z:` drive is the Linux filesystem, where `\` is legal in a name. NTFS forbids it. |
| Node `process.stdout` on a pipe created by a LINUX process | NO. `open EBADF` in `new Socket` (`createWritableStdioStream`). It works when stdout is a file, and the pyright language server works when a Wine process creates the pipes. A harness that puts a Linux pipe on a Wine Node process will fail for a Wine reason. |

Rule: a failure under Wine proves the defect only if Wine behaves like Windows for that call. A pass proves nothing
about Windows. Record any such result as `unverified: seen under Wine 11.21 only`.

## Results so far

- 52e82dd3ddd5d6a0: `tag_external_path_returns_project_for_a_uri_*` green with the fix, red (`lib:src` instead of
  `project`) on the pre-fix line.
- 6d3b04bf30e0fcf9: `tests/test_pid_alive.py` with the fix: 7 passed, 1 failed (pid 4, a Wine difference). On the
  pre-fix mutant: the dead-pid case failed and the out-of-range pid raised `OverflowError`. The Ctrl+C broadcast did
  not reproduce.
- 5d2b664a26dfaee3: NOT reproduced yet. Windows Node and pyright 1.1.413 are in place and the language server answers.
  Still missing: a Windows build of codescout driving `references` on a Python project under Wine, with a cold server.

Still needs a real Windows host or CI: the Ctrl+C broadcast, pid 4, backslash file names.
