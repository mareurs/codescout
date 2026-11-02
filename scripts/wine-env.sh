#!/usr/bin/env bash
# A dedicated wine profile for codescout's Windows test loop: one prefix, with the tools the
# tests reach for already installed, built from pinned and checksummed downloads.
#
#   scripts/wine-env.sh setup     # build the profile (idempotent; downloads about 70 MB once)
#   scripts/wine-env.sh status    # which profile this wine and manifest select, and its state
#   scripts/wine-env.sh env       # print `export` lines; use as:  eval "$(scripts/wine-env.sh env)"
#
# `scripts/build-windows.sh test` evals `env` by itself when a profile is ready and the caller
# has set neither WINEPREFIX nor CODESCOUT_BASH, so after `setup` nothing else is needed.
#
# WHY IT EXISTS. A local wine run used to differ from the CI lane in ways nobody wrote down:
# the default ~/.wine is shared by every session and carries whatever each one left in it, the
# PortableGit recipe lived twice (a comment in build-windows.sh, YAML in ci.yml), and nothing
# provided python3. That is the class `cluster/repro-env-diverges-from-gate-env`; the Windows
# lib-test bug db5303eb3b9c2781 paid for it. With bash, git and a full CPython in wine, the four
# `attribution` failures reproduced locally and their cause (a `\` where git prints `/`) was
# found in minutes.
#
# WHAT IT CARRIES (versions, URLs and SHA256 sums: scripts/wine-env.manifest)
#   - PortableGit: bash.exe for CODESCOUT_BASH and git.exe for WINEPATH, which the tests that
#     call `git` directly need. It is a 7z self-extractor; `7z x` unpacks it with no installer.
#   - CPython from the NuGet `python` package, with `python3.exe` beside `python.exe`. Not the
#     python.org "embeddable" zip: that one ignores PYTHONPATH and fails two attribution tests.
#
# WHAT IT CANNOT DO. It cannot make wine behave like NTFS. Measured 2026-11-02: wine does not
# expand an 8.3 short name such as `RUNNER~1` the way Windows does, and `canonicalize` there
# does not either, so the failures that come from a short-named temp directory reproduce only
# on a real runner. Wine also takes byte-range locks permissively (see build-windows.sh). A
# green run in this profile is closer to the CI wine lane than a bare ~/.wine is, and it is
# still not a green `windows-latest`.
#
# WHERE IT LIVES. ${CODESCOUT_WINE_HOME:-${XDG_CACHE_HOME:-~/.cache}/codescout/wine}/<key>/,
# where <key> is the wine version plus the first 8 hex digits of the manifest's SHA256. A new
# wine or an edited manifest selects a NEW directory; the old one stays until you delete it.
# Measured 2026-11-02 on a fresh profile: prefix 375 MB, PortableGit 409 MB, Python 41 MB, plus
# 72 MB of downloads, so about 0.9 GB. A prefix grows as tests run in it (one used for a full
# lib run was 996 MB), so expect about 1.3 GB in steady use.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MANIFEST="${CODESCOUT_WINE_MANIFEST:-$SCRIPT_DIR/wine-env.manifest}"
WINE_HOME="${CODESCOUT_WINE_HOME:-${XDG_CACHE_HOME:-$HOME/.cache}/codescout/wine}"

MANIFEST_KEYS=(PORTABLEGIT_VERSION PORTABLEGIT_URL PORTABLEGIT_SHA256 PYTHON_VERSION PYTHON_URL PYTHON_SHA256)

die() { echo "wine-env: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' not found — $2"; }

load_manifest() {
  [ -f "$MANIFEST" ] || die "manifest not found: $MANIFEST"
  # Unset first: a variable inherited from the caller's environment must not stand in for a
  # line the manifest forgot to write.
  local k
  for k in "${MANIFEST_KEYS[@]}"; do unset "$k"; done
  # shellcheck disable=SC1090
  . "$MANIFEST"
  for k in "${MANIFEST_KEYS[@]}"; do
    [ -n "${!k:-}" ] || die "$MANIFEST does not set $k"
  done
}

# The profile's key: this wine's version plus the manifest's content hash.
profile_key() {
  need wine "install wine (e.g. 'sudo pacman -S wine')"
  need sha256sum "part of coreutils"
  local ver mh
  ver="$(wine --version 2>/dev/null)" || die "'wine --version' failed"
  ver="$(printf '%s' "$ver" | tr -c 'A-Za-z0-9.-' '_')"
  mh="$(sha256sum "$MANIFEST" | cut -c1-8)"
  printf '%s-%s' "$ver" "$mh"
}

# A Unix path as wine sees it. Wine maps `/` to `Z:`, and a Unix path in CODESCOUT_BASH or
# WINEPATH fails silently, so every path handed to a wine process goes through here.
winpath() { printf 'Z:%s' "$(printf '%s' "$1" | sed 's|/|\\|g')"; }

# Sets the paths of one profile from its key.
layout() {
  KEY="$(profile_key)"
  BASE="$WINE_HOME/$KEY"
  PREFIX="$BASE/prefix"
  GIT_DIR="$BASE/tools/portable-git"
  PY_PKG="$BASE/tools/python"
  PY_DIR="$PY_PKG/tools"          # the NuGet package keeps the interpreter under tools/
  DOWNLOADS="$BASE/downloads"
  READY="$BASE/ready"
}

is_ready() { [ -f "$READY" ] && [ "$(cat "$READY")" = "$KEY" ]; }

# Download to a .part file, check the SHA256, then move into place. A file that is already
# there with the right sum is not fetched again.
fetch() {
  local url="$1" sum="$2" dest="$3" got
  if [ -f "$dest" ] && [ "$(sha256sum "$dest" | cut -d' ' -f1)" = "$sum" ]; then return 0; fi
  need curl "needed to download $url"
  rm -f "$dest.part"
  curl -fsSL "$url" -o "$dest.part" || { rm -f "$dest.part"; die "download failed: $url"; }
  got="$(sha256sum "$dest.part" | cut -d' ' -f1)"
  if [ "$got" != "$sum" ]; then
    rm -f "$dest.part"
    die "sha256 mismatch for $url: expected $sum, got $got (the manifest names a different file than the one served)"
  fi
  mv "$dest.part" "$dest"
}

# Unpack into a .part directory and move it into place, so an interrupted unpack never leaves
# a half-filled directory that a later run would take for a finished one.
unpack() {
  local archive="$1" dest="$2"
  [ -d "$dest" ] && return 0
  rm -rf "$dest.part"; mkdir -p "$dest.part"
  7z x -y -o"$dest.part" "$archive" >/dev/null || { rm -rf "$dest.part"; die "7z could not unpack $archive"; }
  mv "$dest.part" "$dest"
}

# Run a Windows program in this profile, from outside any caller's wine environment.
in_profile() {
  WINEPREFIX="$PREFIX" WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml=" \
    WINEPATH="$(winpath "$GIT_DIR/cmd");$(winpath "$PY_DIR")" "$@"
}

cmd_setup() {
  load_manifest
  need 7z "install p7zip (e.g. 'sudo pacman -S p7zip')"
  need flock "part of util-linux"
  layout
  mkdir -p "$BASE" "$DOWNLOADS" "$BASE/tools"
  # Two sessions running `setup` at once must not unpack into the same directories.
  exec 9>"$BASE/.lock"
  flock 9
  if is_ready; then
    echo "wine-env: profile $KEY is ready at $BASE" >&2
    return 0
  fi

  fetch "$PORTABLEGIT_URL" "$PORTABLEGIT_SHA256" "$DOWNLOADS/PortableGit-$PORTABLEGIT_VERSION.7z.exe"
  fetch "$PYTHON_URL" "$PYTHON_SHA256" "$DOWNLOADS/python-$PYTHON_VERSION.nupkg"
  unpack "$DOWNLOADS/PortableGit-$PORTABLEGIT_VERSION.7z.exe" "$GIT_DIR"
  unpack "$DOWNLOADS/python-$PYTHON_VERSION.nupkg" "$PY_PKG"

  [ -f "$GIT_DIR/bin/bash.exe" ] || die "PortableGit unpacked without bin/bash.exe"
  [ -f "$GIT_DIR/cmd/git.exe" ] || die "PortableGit unpacked without cmd/git.exe"
  [ -f "$PY_DIR/python.exe" ] || die "the Python package unpacked without tools/python.exe"
  # The code starts `python3`, the name a Unix box has; Windows installs ship only python.exe.
  cp -f "$PY_DIR/python.exe" "$PY_DIR/python3.exe"

  echo "wine-env: creating the wine prefix (the first start takes a while)" >&2
  in_profile wineboot --init >/dev/null 2>&1 || die "wineboot --init failed for $PREFIX"

  # Prove each tool runs INSIDE the profile; a file that exists proves nothing about that.
  local out
  out="$(in_profile wine python3.exe --version 2>&1 || true)"
  case "$out" in *"Python $PYTHON_VERSION"*) ;; *) die "python3.exe did not answer 'Python $PYTHON_VERSION' under wine: $out" ;; esac
  out="$(in_profile wine git.exe --version 2>&1 || true)"
  case "$out" in *"git version"*) ;; *) die "git.exe did not answer 'git version' under wine: $out" ;; esac
  out="$(in_profile wine "$(winpath "$GIT_DIR/bin/bash.exe")" -c 'echo bash-ok' 2>&1 || true)"
  case "$out" in *bash-ok*) ;; *) die "bash.exe did not run under wine: $out" ;; esac

  printf '%s' "$KEY" > "$READY"
  echo "wine-env: profile $KEY is ready at $BASE" >&2
}

cmd_status() {
  load_manifest
  layout
  echo "key:      $KEY"
  echo "location: $BASE"
  if is_ready; then
    echo "state:    ready"
    du -sh "$PREFIX" "$GIT_DIR" "$PY_PKG" 2>/dev/null | sed 's/^/size:     /' || true
  else
    echo "state:    not set up (run: scripts/wine-env.sh setup)"
  fi
}

cmd_env() {
  load_manifest
  layout
  if ! is_ready; then
    echo "wine-env: no ready profile for $KEY (run: scripts/wine-env.sh setup)" >&2
    exit 3
  fi
  printf 'export WINEPREFIX=%q\n' "$PREFIX"
  printf 'export WINEDLLOVERRIDES=%q\n' "mscoree,mshtml="
  printf 'export CODESCOUT_BASH=%q\n' "$(winpath "$GIT_DIR/bin/bash.exe")"
  printf 'export WINEPATH=%q\n' "${WINEPATH:+$WINEPATH;}$(winpath "$GIT_DIR/cmd");$(winpath "$PY_DIR")"
}

case "${1:-}" in
  setup)  cmd_setup ;;
  status) cmd_status ;;
  env)    cmd_env ;;
  *) echo "usage: $0 {setup|status|env}" >&2; exit 2 ;;
esac
