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
# Shell-dependent tests need a Windows bash: codescout runs commands through Git
# Bash, and without one every `run_command`-touching test dies on "no POSIX shell
# available" — 8 such failures in `server::guide_hint_tests` alone, which is enough
# noise to hide a real one behind. Extract PortableGit (a 7z SFX; `7z x` unpacks it
# on Linux, no installer) and point the resolver at it:
#
#   CODESCOUT_BASH='Z:\path\to\PortableGit\bin\bash.exe' scripts/build-windows.sh test --lib
#
# The value must be a WINDOWS path — wine maps `/` to `Z:`. With it set, that module
# went 9 failures -> 1, and the 1 was a genuine Windows defect
# (`docs/issues/archive/2026-08-26-windows-lanes-still-red-on-four-remaining-causes.md`
# group A).
#
# A few tests shell out to `git` DIRECTLY rather than through Git Bash, so CODESCOUT_BASH
# never reaches them and they die on "program not found". Put PortableGit's `cmd/` on the
# WINDOWS path too — a Unix PATH entry does nothing for a wine process:
#
#   WINEPATH='Z:\path\to\PortableGit\cmd' CODESCOUT_BASH='...' scripts/build-windows.sh test --lib
#
# With both set the suite needs 7 skips instead of 32 — 4283 passed, 0 failed, measured
# 2026-08-26.
#
# Note this is still the gnu ABI, and a green wine run is NOT a green `windows-latest`.
# The sharpest demonstration to date: two `retrieval::index_lock` tests passed here while
# failing on MSVC, because wine implements Windows byte-range locks permissively where real
# Windows makes them MANDATORY — so wine could neither reproduce that defect nor verify its
# fix (`ee9d9844`). Nor is a green run here a green wine LANE — though the gap narrowed on
# 2026-09-02: the lane now PINS wine to 11.16 via the WineHQ apt repo, matching this box, so
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

# --edr swaps default features for the runtime-loaded-ONNX shape used on windows-gnu.
FEATURES=()
ARGS=()
for a in "$@"; do
  case "$a" in
    --edr) FEATURES=(--no-default-features --features "remote-embed,http,librarian,local-embed-dynamic") ;;
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
    echo ">>> wine here:  $(wine --version 2>/dev/null || echo unknown)" >&2
    echo ">>> wine in CI: pinned to 11.16 (WineHQ devel), .github/workflows/ci.yml WINE_PIN" >&2
    echo ">>>             — a constant matched to this box on 2026-09-02, not a live read." >&2
    echo ">>> A green run here is NOT a green wine LANE — nor a green windows-latest." >&2
    echo ">>>   docs/issues/archive/2026-08-26-wine-lane-runs-wine-9-and-diverges-from-the-local-loop.md" >&2
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER="wine"
    export WINEDEBUG="${WINEDEBUG:--all}"   # silence wine's GL/pci-id probe noise
    set -x; exec cargo test --target "$TARGET" "${FEATURES[@]}" "${ARGS[@]}" ;;
  *)
    echo "usage: $0 {build|check|clippy|test} [--edr] [cargo args... | test filter]" >&2
    exit 2 ;;
esac
