# Wine Windows test suite (set up 2026-11-08)

This machine can run Windows builds of codescout and Windows Python/Node tools under Wine. Use it to reproduce
or refute a Windows-only issue before asking for a Windows host. It is NOT Windows: read "What Wine can and
cannot prove" before you cite a result. The user asked for this to be a standing reproduction environment.

## What is installed (verified 2026-11-08)

- Wine 11.21, default prefix `~/.wine` (win64, emulating Windows 10.0.19045; `cmd.exe`, `conhost.exe`,
  `wineconsole.exe` exist). A backup of the prefix from before Node/pyright: `~/.wine.bak-20261108` (1.9 GB);
  delete it when the prefix is known good.
- Rust target `x86_64-pc-windows-gnu` and mingw-w64 gcc.
- Windows Python 3.11.9 at `C:\Python311` with `pytest` 9.2.0. Not on the Wine PATH: use the full path.
- Windows Node 22.23.4 at `C:\node` (zip, sha256 `cbc74b5e0982c9dbb41525fd8fb5dcd32fbee237863f7829072f4639a1ce5f20`,
  checked against nodejs.org `SHASUMS256.txt`).
- pyright 1.1.413 (the version of the Windows report in bug 5d2b664a26dfaee3) at `C:\pyright\node_modules`, with
  the `.cmd` shims. Entry: `C:\pyright\node_modules\pyright\langserver.index.js`.
- Windows `codescout.exe` builds in `C:\codescout-bin\`: `codescout-base.exe` = `267d8cd01` (before any fix to
  5d2b664a26dfaee3), `codescout-head.exe` = `5cf72a154` (with the first-call settle wait `ec450d195`). Both built with
  `--no-default-features --features http` (~30 s each in a warm slot).
- Reproduction project `C:\pybig` (2169 `.py` files: the Python 3.11.9 standard library plus a package `pkg`), made by
  `tools/wine-repro/make-project.sh`. `C:\pyproj` is a small 8-file variant, too small to show the defect.
- MSYS2 base at `C:\msys64` (a partial `bash.exe`, see "MSYS2 `bash.exe` under Wine" below).
- Git for Windows 2.56.0.windows.4 (PortableGit) at `C:\PortableGit` (`bin\bash.exe`, `cmd\git.exe`), see "Git for
  Windows (PortableGit) under Wine" below. This is the shell to use, not MSYS2.
- NOT installed: `winetricks`, `wine-mono`, `wine-gecko`. On 2026-11-08 from about 12:54 EET `api.github.com` answered 504
  and `github.com` stalled in TLS: a GitHub incident, over by 13:57 EET (`~/agents/system/GITHUB_TLS_STALL_2026-11-08.md`,
  SYS-022). The user approved
  installing `winetricks` (pacman `extra`, 20260125-2) if it is needed; `sudo -n` printed no error. Not needed so far.
- Python installer provenance: `python-3.11.9-amd64.exe` from python.org, 26216840 bytes, GPG signature good, key
  `FC624643487034E5` (Steve Dower), fingerprint `7ED1 0B65 31D7 C8E1 BC29  6021 FC62 4643 4870 34E5`. Silent options:
  `/quiet InstallAllUsers=0 TargetDir=C:\Python311 PrependPath=0 Include_launcher=0 InstallLauncherAllUsers=0
  Include_doc=0 Include_test=0 Shortcuts=0 AssociateFiles=0`.

## Recipes

Rust lib tests (lean build ~1m20s), then run one test under Wine; run the exe at once, the slot may be reused:

    scripts/with-slot.sh cargo test --lib --no-default-features --target x86_64-pc-windows-gnu --no-run
    WINEDEBUG=-all wine <slot>/x86_64-pc-windows-gnu/debug/deps/codescout-<hash>.exe <filter> --test-threads=1

Python tests (repo root): `PYTHONDONTWRITEBYTECODE=1 WINEDEBUG=-all wine 'C:\Python311\python.exe' -m pytest
tests/<file>.py -q -p no:cacheprovider`. A script outside the prefix is `Z:\home\...`. stderr carries `MESA-EGL ... pci id`
noise: ignore it.

Windows codescout server, driven from Linux (the `tools/wine-repro/` scripts):

    scripts/with-slot.sh cargo build --bin codescout --no-default-features --features http --target x86_64-pc-windows-gnu
    cp <slot>/x86_64-pc-windows-gnu/debug/codescout.exe ~/.wine/drive_c/codescout-bin/<name>.exe
    tools/wine-repro/make-project.sh                      # once; refuses if C:\pybig exists
    tools/wine-repro/cycle.sh <name>.exe <port> <label>   # one cold cycle: 2 references calls, timed

`cycle.sh` runs `wineserver -k` before and after, which stops EVERY process in the default prefix. The server is
`codescout.exe start --project C:\pybig --transport http --port N --auth-token wine-test`, endpoint
`http://127.0.0.1:N/mcp` (needs `Accept: application/json, text/event-stream` and a bearer token).
`tools/wine-repro/mcp_call.py <port> <tool> '<json>' ...` is the client. There is no `Mux` subcommand on Windows
(`cfg(unix)`): the server talks to pyright directly. The HTTP transport needs the `http` feature, which
`--no-default-features` removes.

To test another revision: `git worktree add --detach <scratchpad>/wt <rev>`, build there with `with-slot.sh`, copy the
exe, `git worktree remove --force`. Never mutate the shared checkout. A controlled red is mandatory: a test that is
green on the broken code proves nothing.

## Git for Windows (PortableGit) under Wine, 2026-11-08

- Installed: `PortableGit-2.56.0.4-64-bit.7z.exe` from the release `v2.56.0.windows.4`, 60087472 bytes, sha256
  `b1f3e52216e2ed6882d1bebfd4d3fdf319bec40d2edbbf9fd26ec4fd3fce7ed1`, read from the release page's own asset list (it
  matched the value a search snippet gave) and checked with `sha256sum -c`. Extracted with `7z x` (no Wine run of the
  self-extractor): `7z x -y -o~/.wine/drive_c/PortableGit <file>`; 9616 files, 407 MB. No GPG signature was checked.
- `git.exe` (native Windows, `C:\PortableGit\cmd\git.exe`) works: `--version`, `-C <dir> status|log|config`, and
  `-c user.name=... -c user.email=... commit -m` (made commit `2850714`).
- `bash.exe` (`C:\PortableGit\bin\bash.exe`), unlike the MSYS2 one, starts external programs without `--login`, from any
  current directory. Same Cygwin warning `Couldn't compute FAST_CWD pointer` on every start. With the default PATH a test
  script gave the right answers: `$(echo hello | tr a-z A-Z)` = HELLO, `( )` subshell, ten `$(echo $i | cat)` summing to 55,
  `ls /usr/bin | wc -l` = 366, `uname -a` = `MINGW64_NT-10.0-19045 ... Msys`, `mkdir`, `git` inside bash.
- Do NOT put `export PATH=/usr/bin` in a test script: after it `ls | wc` printed nothing and `mkdir` returned 1 with no
  message. It looked like a Wine defect and was my own override.
- `/c` is the current directory's drive: with the current directory `C:\`, `pwd` printed `/c`. Use `/c/...` for files in the
  prefix, `/z/...` for Linux paths. In a bash script, a `git commit` without an identity fails with `Author identity
  unknown` (exit 128): set `-c user.name=... -c user.email=...`.
- Unexplained, not chased: in one script run (`git init`, `git add`, `git commit -q`, `git log | cat`, `git status | wc`) the
  stderr of the commands after `git add` was not shown, so the missing identity error was invisible. The same commit run
  through `bash.exe -c '...'` printed the error correctly. If a script under Wine seems to do nothing, rerun the step
  through `-c` and check the exit code before blaming the code under test.
- Not tested: `run_command` of a Windows codescout through this bash, long-running children, signals, `git` with hooks or
  `core.autocrlf` effects (`git add` warned that LF becomes CRLF).

## MSYS2 `bash.exe` under Wine (instead of Git for Windows), 2026-11-08

Git for Windows could not be downloaded while `github.com:443` stalled in the TLS handshake (TCP connected, no ServerHello;
`--curves X25519` changed nothing; `api.github.com` the same; `git-scm.com` answered but links to `github.com`). That was a
GitHub incident ("Increase in timeouts for some unauthenticated users", 10:36Z to 11:56Z), not a fault of this machine, and
GitHub answered again at 13:57 EET. `repo.msys2.org` answered throughout, so MSYS2 was used meanwhile. Do not assume the
block is permanent: retry the PortableGit download first, and treat MSYS2 as the fallback.

- Installed: `msys2-base-x86_64-20260927.tar.xz` from `repo.msys2.org/distrib/x86_64/`, extracted to `C:\msys64` (no
  `pacman` run to install it). The signature is good: key `5A62D0CAB6264964`, primary fingerprint
  `0EBF 782C 5D53 F7E5 FB02  A667 46BD 761F 7A49 B0EC` (Christoph Reiter), the same fingerprint that `msys2.org/docs/installer/`
  publishes. `C:\drivers` (a copy of `system32\drivers`) and `C:\fork-test.sh` were also added to the prefix.
- It is NOT a faithful shell. Measured, and the same on every run:
  - `bash.exe -c` without `--login`: builtins and `( )` subshells work, but every external program it starts fails with
    `Bad address` (rc 126), with or without `MSYS2_PATH_TYPE=inherit` and `MSYS2_ARG_CONV_EXCL='*'`.
  - `--login` fails with `Directory \drivers\etc does not exist` when the current directory is on `Z:` (any Linux path such
    as `/tmp`). Run it with the current directory INSIDE `~/.wine/drive_c`, so Wine maps it to `C:`. The first `--login`
    run does the MSYS2 first-run setup and generates the pacman keyring; it did not hang here (WineHQ bug 53574 reports
    that it can).
  - With `--login` from `C:`: `echo hi | /usr/bin/cat` prints `hi`, `uname -a` once printed `MSYS_NT-10.0-19045 ... 3.6.10 ...
    Msys`, and `$(echo hello | tr a-z A-Z)` works. But in one script `uname -a` and `ls /usr/bin | wc -l` printed nothing,
    and `x=$(echo $i | cat)` came back empty ten times. So some child programs run and some lose their output.
  - Every start prints a Cygwin warning `Couldn't compute FAST_CWD pointer`.
- Correction, same day, after Git for Windows was tested: the lost `ls /usr/bin | wc -l` output above was caused by my own
  `export PATH=/usr/bin` in the test script. Without it `ls | wc -l` works on MSYS2 too (458). Still wrong on MSYS2 with
  the default PATH: `x=$(echo $i | cat)` in a loop returns empty (sum 0), and `uname -a` prints nothing in a script.
- So: do not use this to judge a shell-dependent Windows bug. It shows only that a `bash.exe` process can exist in the
  process tree. Run as: `cd ~/.wine/drive_c && MSYS2_PATH_TYPE=inherit WINEDEBUG=-all wine 'C:\msys64\usr\bin\bash.exe' --login
  /c/<script>`, with MSYS paths (`/c/...`, `/z/...`).
- Sources: the WineHQ forum thread `viewtopic.php?t=42551` (a workaround for bug 53574, not readable with curl: 403) says to
  set `MSYS2_PATH_TYPE=inherit` and run `bash.exe --login`; Wine 11.0 and 11.2/3-staging are named as tested there.

## Traps found

- Node `process.stdout` on a pipe made by a LINUX process fails with `open EBADF` under Wine. A file works, and so do
  pipes made by a Wine process (the pyright language server answers `initialize` in 0.3 s from a Wine Python). So
  redirect Node's output to a file, and drive codescout over HTTP, not stdio.
- A symbol in a module whose name is a standard-library name gives a false "partial" result: `from textwrap import
  dedent` resolves to the typeshed stub, so pyright reports 3 references for `textwrap.dedent` on Linux and under Wine,
  however long it runs. Not a warm-up effect. Use `pkg.build_chain`.
- `rm -rf` is blocked by the run_command guard; use fresh directory names instead.
- `git add -- <directory>` makes the stage log record the new files with owner `-` and route `unnamed`, and the
  pathspec-commit guard then refuses them as another session's. Fix, with no ack: `git reset -q -- <files>` (index
  only), then `git add -- <file> <file> ...` naming each file; the log then records your session id with route
  `named`. Only for files you wrote yourself.
- Another session may archive a bug file while you edit it: `doc(move)` carries the working-tree content, so your
  edits land in THEIR commit and the id changes (5d2b664a26dfaee3 became 659e1b6e59f10840). Look the file up by path
  before you try to commit it.

## What Wine can and cannot prove

| Question | Wine |
|---|---|
| Path-string logic (`\\?\C:\` vs `C:\`, `strip_verbatim`) | Yes. Bug 52e82dd3ddd5d6a0: red on the pre-fix line, green with the fix. |
| ctypes Win32 calls (`OpenProcess`, `GetExitCodeProcess`) | Yes for live and dead pids. |
| `os.kill(pid, 0)` console Ctrl+C broadcast (bug 6d3b04bf30e0fcf9) | NO. Wine does not deliver the event, so that test is green on the broken code. |
| Windows pid 4 (System): `OpenProcess` gives access denied | NO. Wine returns error 87. |
| A backslash in a file name (bug 2d8475ed5bc831e5 and relatives) | NO. `Z:` is the Linux filesystem, where `\` is legal; NTFS forbids it. |
| Cold-start timing of a real language server (bug 5d2b664a26dfaee3) | YES as evidence of the mechanism: reproduced, see below. Timings are Wine's, not Windows'. |
| Node stdout on a Linux-made pipe | NO (`EBADF`, a Wine difference). |

Rule: a failure under Wine proves a defect only if Wine behaves like Windows for that call. A pass proves nothing about
Windows. Record such results as `unverified: seen under Wine 11.21 only`.

## Results so far

- 52e82dd3ddd5d6a0: `tag_external_path_returns_project_for_a_uri_*` green with the fix, red (`lib:src` for `project`) on
  the pre-fix line.
- 6d3b04bf30e0fcf9: `tests/test_pid_alive.py` with the fix 7 passed, 1 failed (pid 4, a Wine difference). On a mutant
  that skips `_pid_alive_windows`: the dead-pid case failed, the out-of-range pid raised `OverflowError`; the Ctrl+C
  broadcast did not reproduce.
- 5d2b664a26dfaee3 (archived as 659e1b6e59f10840) REPRODUCED (pyright 1.1.413, `C:\pybig`, cold server, `references` on `pkg.build_chain`, twice):
  base `267d8cd01`, 4 runs: first call partial 4 of 4 (1.0-1.8 s, definition file only, with the warning); the
  immediate second call partial in 2 (0.4 s, 0.5 s) and complete in 2 (after 12.7 s and 11.2 s). HEAD `5cf72a154`,
  3 runs: first call complete 3 of 3 (8.0, 8.3, 8.3 s), second complete (4.3, 6.7, 4.6 s). Recorded in the bug file.

Still needs a real Windows host or CI: the Ctrl+C broadcast, pid 4, backslash file names, and the reporter's project.
