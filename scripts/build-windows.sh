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
# (`docs/issues/2026-08-26-windows-lanes-still-red-on-four-remaining-causes.md` group A).
#
# Note this is still the gnu ABI. Two `retrieval::index_lock` tests PASS here and
# FAIL on MSVC, so a green wine run is not a green `windows-latest`.
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
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER="wine"
    export WINEDEBUG="${WINEDEBUG:--all}"   # silence wine's GL/pci-id probe noise
    set -x; exec cargo test --target "$TARGET" "${FEATURES[@]}" "${ARGS[@]}" ;;
  *)
    echo "usage: $0 {build|check|clippy|test} [--edr] [cargo args... | test filter]" >&2
    exit 2 ;;
esac
