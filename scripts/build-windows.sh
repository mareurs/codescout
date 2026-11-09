#!/usr/bin/env bash
# Cross-compile / test codescout for Windows (x86_64-pc-windows-gnu) FROM Linux
# via the MinGW-w64 toolchain, optionally executing the test binaries under wine.
#
# This is the LOCAL (off-VDI) Windows verification loop. It targets the *gnu* ABI
# — the same ABI the EDR/VDI deployment uses (that is why the local-embed-dynamic
# feature exists; ort ships no gnu prebuilt) — so a green run here mirrors the
# VDI's artifact, NOT the MSVC `windows-latest` CI runner.
#
# Why env-var overrides instead of .cargo/config.toml: the committed config is
# also read by the native-gnu build ON the VDI. A hardcoded cross-linker / wine
# runner there would break the VDI's native build, so the cross-compile knobs
# live here (machine-local) and stay out of the repo's shared config.
#
# Requirements (this machine):
#   - mingw-w64           (x86_64-w64-mingw32-gcc)        e.g. sudo pacman -S mingw-w64
#   - rustup target       x86_64-pc-windows-gnu           rustup target add x86_64-pc-windows-gnu
#   - wine (test mode only)                               e.g. sudo pacman -S wine
#
# Usage:
#   scripts/build-windows.sh                 # build, default features
#   scripts/build-windows.sh build --edr     # build with runtime-loaded ONNX (local-embed-dynamic)
#   scripts/build-windows.sh check           # fast type-check only (no link)
#   scripts/build-windows.sh clippy --all-targets -- -D warnings   # lint the Windows cfg
#   scripts/build-windows.sh test [FILTER]   # cargo test under wine (optional name filter)
#   scripts/build-windows.sh test --edr win32
#
# Why 'clippy' is its own mode: lints are cfg-sensitive, so a host-only clippy run
# cannot see #[cfg(windows)] code at all, and can even disagree about code it CAN
# see -- a `return` that is redundant only once the #[cfg(unix)] arms around it are
# erased. Two such lints sat unnoticed until someone ran clippy on a Windows host.
# See docs/issues/archive/2026-08-08-clippy-pre-existing-drift-stable-gnu-toolchain.md.
#
# Caveat: wine executes the Win32 API surface (OpenProcess/TerminateProcess/...)
# and the platform logic, but it is NOT EDR. EDR-only behaviors (GPU-probe skip,
# run_command child hangs, AV-mediated kills) reproduce only on the VDI.
#
# Shell-dependent tests need a Windows bash and a few need `git.exe` and python3: codescout
# runs commands through Git Bash, and without one every `run_command`-touching test dies on
# "no POSIX shell available" (8 such failures in `server::guide_hint_tests` alone, enough
# noise to hide a real one behind). `scripts/wine-env.sh setup` builds a dedicated wine
# profile with PortableGit and CPython from pinned, checksummed downloads, and the `test`
# mode below uses it automatically once it exists. The recipe lives there and in
# scripts/wine-env.manifest, not in this comment.
#
# History of the numbers, from the manual recipe this replaces: with CODESCOUT_BASH set,
# `server::guide_hint_tests` went from 9 failures to 1, and the 1 was a genuine Windows
# defect (`docs/issues/archive/2026-08-26-windows-lanes-still-red-on-four-remaining-causes.md`
# group A). With git.exe on WINEPATH as well the suite needed 7 skips instead of 32 (4283
# passed, 0 failed, measured 2026-08-26).
#
# To use your own wine setup instead, export WINEPREFIX or CODESCOUT_BASH yourself: the
# profile is only picked up when you set neither.
#
# Note this is still the gnu ABI, and a green wine run is NOT a green `windows-latest`.
# The sharpest demonstration to date: two `retrieval::index_lock` tests passed here while
# failing on MSVC, because wine implements Windows byte-range locks permissively where real
# Windows makes them MANDATORY — so wine could neither reproduce that defect nor verify its
# fix (`ee9d9844`). Nor is a green run here a green wine LANE — though the gap narrowed on
# 2026-09-02: the lane now PINS wine to a specific WineHQ build matching this box, so
# the two are comparable rather than two majors apart. They can still diverge, because the pin
# is a constant and a dev box tracks its distro; that is why the versions are printed below
# rather than assumed equal. History:
# `docs/issues/archive/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md`.
# Check `wine --version` before trusting a local result against a CI failure.
set -euo pipefail

TARGET="x86_64-pc-windows-gnu"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

# MinGW linker for the gnu target, supplied as a CARGO_TARGET_* override (see header).
export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="x86_64-w64-mingw32-gcc"

require() {
  command -v "$1" >/dev/null 2>&1 || { echo "error: '$1' not found — $2" >&2; exit 1; }
}

require x86_64-w64-mingw32-gcc "install mingw-w64 (e.g. 'sudo pacman -S mingw-w64')"
rustup target list --installed | grep -qx "$TARGET" || {
  echo "error: rustup target '$TARGET' not installed — run 'rustup target add $TARGET'" >&2
  exit 1
}

CMD="${1:-build}"
shift || true

# FEATURES for the windows-gnu target: cargo's default set PLUS `local-embed-dynamic`.
#
# `ort` publishes no prebuilt binary for `x86_64-pc-windows-gnu`, so the static ONNX
# backend (`local-embed`) cannot build for this target, and that is why
# `local-embed-dynamic` (runtime-loaded onnxruntime.dll) exists. `local-embed` is NOT in
# cargo's `default` (it left on 2026-11-08; it was there from 2026-09-17, when this
# script had to hand-list the default set with `local-embed` swapped out, because
# taking cargo's default here failed inside the ort build script:
#
#   error: ort-sys@2.0.0-rc.11: ort does not provide prebuilt binaries for the
#          target `x86_64-pc-windows-gnu` with feature set (no features).
# ).
#
# So this is now a plain `--features`, with no `--no-default-features` and no hand-copied
# default list: the two ONNX backends are mutually exclusive, and with `local-embed` out of
# `default` adding `local-embed-dynamic` on top cannot enable both. It also picks up
# features added to `default` later, which the hand-written list did not.
#
# The set is unchanged by this: `remote-embed,http,librarian,local-embed-dynamic`. It is
# kept as the DEFAULT here, not behind `--edr`, so CI's three calls (`build`, `clippy`,
# `test`) keep compiling the windows-gnu dlopen path that no Linux lane can see.
# `tests/feature_lanes.rs::the_onnx_backends_are_opt_in_not_default` is what keeps cargo's
# own default from regaining the static backend and breaking this target again.
FEATURES=(--features "local-embed-dynamic")
ARGS=()
for a in "$@"; do
  case "$a" in
    # Retained as a no-op alias: it named the shape that is now unconditional, and
    # CI/docs/muscle memory still pass it. Accepting it silently is better than
    # failing on a flag whose request is already satisfied.
    --edr) : ;;
    *)     ARGS+=("$a") ;;
  esac
done

case "$CMD" in
  build) set -x; exec cargo build --target "$TARGET" "${FEATURES[@]}" "${ARGS[@]}" ;;
  check) set -x; exec cargo check --target "$TARGET" "${FEATURES[@]}" "${ARGS[@]}" ;;
  clippy) set -x; exec cargo clippy --target "$TARGET" "${FEATURES[@]}" "${ARGS[@]}" ;;
  test)
    require wine "install wine to execute the test binaries (e.g. 'sudo pacman -S wine')"
    # IC-5 — the reproduction environment is not the gating environment. CI's
    # windows-gnu lane installs Ubuntu's `wine` package (.github/workflows/ci.yml,
    # step "Install MinGW + wine"); a dev box tracks its own distro and is
    # typically two majors ahead. The two have already diverged twice, and the
    # divergence is SILENT — both runs print the same "test result: ok", so a
    # green here gets read as a green lane. Naming both ends at the top of every
    # run is what makes a local result comparable to a CI one.
    #
    # Deliberately NOT asserting a specific CI version. ubuntu-latest's wine
    # moves, so a hardcoded "9.0" would be a constant that decays while still
    # reading as fact — the shape docs/trackers/issue-clusters.md files as IC-11.
    # Print what THIS box runs, name where CI's is decided, and let the reader
    # compare two live values rather than one live and one remembered.
    #
    # CI's half is now READ FROM ci.yml rather than restated here, and the reason is
    # that this block previously did the thing its own comment forbids: it hardcoded
    # `11.16` on the line directly below the warning against hardcoding, and by
    # 2026-09-08 the pin had moved to 11.17 and this line was printing a stale number
    # into every lane log — the exact "one live and one remembered" it argues against.
    # A grep against the single source of truth cannot decay; if it ever prints
    # `unreadable`, the pin's spelling in ci.yml changed and THAT is the thing to fix.
    ci_pin="$(grep -m1 -oE 'WINE_PIN:[[:space:]]*"[0-9.]+"' \
        "$PROJECT_ROOT/.github/workflows/ci.yml" 2>/dev/null \
        | grep -oE '[0-9]+\.[0-9]+' || true)"
    echo ">>> wine here:  $(wine --version 2>/dev/null || echo unknown)" >&2
    echo ">>> wine in CI: pinned to ${ci_pin:-unreadable} (WineHQ devel), .github/workflows/ci.yml WINE_PIN" >&2
    echo ">>>             — a constant matched to this box, read live from ci.yml, not a live read of CI." >&2
    echo ">>> A green run here is NOT a green wine LANE — nor a green windows-latest." >&2
    echo ">>>   docs/issues/archive/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md" >&2
    # The dedicated profile (scripts/wine-env.sh), when one is ready for THIS wine and manifest.
    # Never over a setup the caller made: WINEPREFIX or CODESCOUT_BASH already set means they
    # chose their own, and a silent swap would make their result incomparable to their setup.
    # `env` exits 3 when no profile is ready; that is not an error here, only a missing tool.
    if [ -z "${WINEPREFIX:-}" ] && [ -z "${CODESCOUT_BASH:-}" ]; then
      if profile_env="$("$PROJECT_ROOT/scripts/wine-env.sh" env 2>/dev/null)"; then
        eval "$profile_env"
        echo ">>> wine profile: $WINEPREFIX (scripts/wine-env.sh)" >&2
      else
        echo ">>> wine profile: none — bare ~/.wine, no bash/git/python3. Run scripts/wine-env.sh setup." >&2
      fi
    else
      echo ">>> wine profile: yours (WINEPREFIX/CODESCOUT_BASH already set)" >&2
    fi
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER="wine"
    export WINEDEBUG="${WINEDEBUG:--all}"   # silence wine's GL/pci-id probe noise
    set -x; exec cargo test --target "$TARGET" "${FEATURES[@]}" "${ARGS[@]}" ;;
  *)
    echo "usage: $0 {build|check|clippy|test} [--edr] [cargo args... | test filter]" >&2
    exit 2 ;;
esac
