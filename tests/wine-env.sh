#!/usr/bin/env bash
# Suite for scripts/wine-env.sh — the dedicated wine profile for the local Windows test loop.
#
# No download, no wine, no 7z runs here. The script is driven with FAKE `wine`, `wineboot`,
# `curl` and `7z` on PATH, so the suite checks what the script decides (which directory a
# manifest and a wine version select, whether it refuses a wrong checksum, whether a second
# `setup` repeats work, what `env` prints), not what wine does.
#
# What this suite cannot say: that the real PortableGit and CPython run under a real wine.
# `scripts/wine-env.sh setup` proves that itself, because it runs python3.exe, git.exe and
# bash.exe inside the new prefix before it writes its `ready` marker.
#
# Every REFUSE row below is paired with an ALLOW row that goes through the same code path,
# and each allow row asserts that the work happened. A script that refuses everything would
# pass the refusals alone; "no complaint printed" is also what a script that died early prints.

set -u

PASS=0; FAIL=0
ok()   { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no()   { echo "  FAIL  $1"; echo "        $2"; FAIL=$((FAIL + 1)); }
eq()   { [ "$2" = "$3" ] && ok "$1" || no "$1" "expected [$3], got [$2]"; }
has()  { printf '%s' "$2" | grep -qF -- "$3" && ok "$1" || no "$1" "expected to find: $3 in: $2"; }
hasnt(){ printf '%s' "$2" | grep -qF -- "$3" && no "$1" "should NOT contain: $3" || ok "$1"; }

SCRIPT="$(cd "$(dirname "$0")/.." && pwd)/scripts/wine-env.sh"
[ -f "$SCRIPT" ] || { echo "no scripts/wine-env.sh at $SCRIPT"; exit 2; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/wine-env-XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
FAKE="$WORK/fakebin"; mkdir -p "$FAKE"
export CODESCOUT_WINE_HOME="$WORK/home"
export CODESCOUT_WINE_MANIFEST="$WORK/manifest"
WINE_VERSION="wine-9.9"
CURL_LOG="$WORK/curl.log"; BOOT_LOG="$WORK/wineboot.log"
: > "$CURL_LOG"; : > "$BOOT_LOG"

# The fake wine answers the three checks `setup` makes and reports a version that the test
# can change through a file.
cat > "$FAKE/wine" <<EOF
#!/usr/bin/env bash
case "\${1:-}" in
  --version) cat "$WORK/wine-version" ;;
  python3.exe) echo "Python 3.12.8" ;;
  git.exe) echo "git version 2.55.0.windows.5" ;;
  *bash.exe) echo "bash-ok" ;;
  *) echo "fake wine: unexpected args: \$*" >&2; exit 9 ;;
esac
EOF
echo "$WINE_VERSION" > "$WORK/wine-version"
# The fake wineboot only records that it ran, and with which prefix.
cat > "$FAKE/wineboot" <<EOF
#!/usr/bin/env bash
echo "WINEPREFIX=\${WINEPREFIX:-unset}" >> "$BOOT_LOG"
mkdir -p "\${WINEPREFIX:?}"
EOF
# The fake curl writes "ARCHIVE:<url>" to the -o target: the manifest's sums are computed over
# exactly that, so a right sum and a wrong sum are both under the test's control.
cat > "$FAKE/curl" <<EOF
#!/usr/bin/env bash
url=""; out=""
while [ \$# -gt 0 ]; do
  case "\$1" in -o) out="\$2"; shift 2 ;; -*) shift ;; *) url="\$1"; shift ;; esac
done
echo "\$url" >> "$CURL_LOG"
printf '%s' "ARCHIVE:\$url" > "\$out"
EOF
# The fake 7z lays out the files the script looks for, by archive name.
cat > "$FAKE/7z" <<'EOF'
#!/usr/bin/env bash
dir=""; archive=""
for a in "$@"; do
  case "$a" in -o*) dir="${a#-o}" ;; -*|x) ;; *) archive="$a" ;; esac
done
case "$archive" in
  *PortableGit*) mkdir -p "$dir/bin" "$dir/cmd"; : > "$dir/bin/bash.exe"; : > "$dir/cmd/git.exe" ;;
  *nupkg*)       mkdir -p "$dir/tools"; : > "$dir/tools/python.exe" ;;
  *) echo "fake 7z: unknown archive $archive" >&2; exit 9 ;;
esac
EOF
chmod +x "$FAKE"/*
export PATH="$FAKE:$PATH"

sha() { printf '%s' "$1" | sha256sum | cut -d' ' -f1; }
GIT_URL="https://example.invalid/PortableGit-1.7z.exe"
PY_URL="https://example.invalid/python/3.12.8"
# mk_manifest <git sum> <extra line>: a complete manifest unless a key is left out on purpose.
mk_manifest() {
  {
    echo "PORTABLEGIT_VERSION=1"
    echo "PORTABLEGIT_URL=$GIT_URL"
    echo "PORTABLEGIT_SHA256=${1:-$(sha "ARCHIVE:$GIT_URL")}"
    echo "PYTHON_VERSION=3.12.8"
    echo "PYTHON_URL=$PY_URL"
    echo "PYTHON_SHA256=$(sha "ARCHIVE:$PY_URL")"
    [ -n "${2:-}" ] && echo "$2"
  } > "$CODESCOUT_WINE_MANIFEST"
  return 0
}
profiles() { ls "$CODESCOUT_WINE_HOME" 2>/dev/null | wc -l | tr -d ' '; }

echo "== before setup =="
mk_manifest
out="$("$SCRIPT" status 2>&1)"
has "status says a profile is not set up" "$out" "not set up"
has "status names the wine version in the key" "$out" "$WINE_VERSION-"
"$SCRIPT" env > "$WORK/env.out" 2> "$WORK/env.err"; code=$?
eq "env REFUSES with exit 3 when no profile is ready" "$code" 3
has "the refusal names the command that fixes it" "$(cat "$WORK/env.err")" "scripts/wine-env.sh setup"
eq "env printed nothing on stdout when it refused" "$(cat "$WORK/env.out")" ""

echo "== setup =="
"$SCRIPT" setup > "$WORK/setup.out" 2>&1; code=$?
eq "setup succeeds with a right manifest (the ALLOW row for the checksum refusal below)" "$code" 0
eq "setup downloaded both archives" "$(wc -l < "$CURL_LOG" | tr -d ' ')" 2
eq "setup started the wine prefix exactly once" "$(wc -l < "$BOOT_LOG" | tr -d ' ')" 1
has "the prefix lives under the configured home" "$(cat "$BOOT_LOG")" "WINEPREFIX=$CODESCOUT_WINE_HOME/"
KEYDIR="$(ls -d "$CODESCOUT_WINE_HOME"/*/ | head -1)"
[ -f "${KEYDIR}tools/python/tools/python3.exe" ] && ok "python3.exe sits beside python.exe" || no "python3.exe sits beside python.exe" "missing in ${KEYDIR}tools/python/tools"
out="$("$SCRIPT" status 2>&1)"
has "status says ready after setup" "$out" "ready"

echo "== env =="
"$SCRIPT" env > "$WORK/env.out" 2>&1; code=$?
eq "env succeeds once the profile is ready" "$code" 0
# Evaluate the output the way build-windows.sh does, in a clean subshell, and look at the result.
res="$(unset WINEPREFIX CODESCOUT_BASH WINEPATH; eval "$(cat "$WORK/env.out")"; printf '%s|%s|%s' "$WINEPREFIX" "$CODESCOUT_BASH" "$WINEPATH")"
has "WINEPREFIX is the profile's prefix" "$res" "$CODESCOUT_WINE_HOME/"
has "CODESCOUT_BASH is a WINDOWS path (Z:\\, backslashes)" "$res" 'Z:\home'
has "CODESCOUT_BASH names bash.exe" "$res" 'bin\bash.exe'
has "WINEPATH carries git's cmd directory" "$res" 'cmd'
has "WINEPATH carries the python directory" "$res" 'python\tools'
hasnt "no Unix separator survives in the Windows paths" "$(printf '%s' "$res" | cut -d'|' -f2)" "bash.exe/"
pre="$(unset WINEPREFIX CODESCOUT_BASH; WINEPATH='Z:\pre' "$SCRIPT" env > "$WORK/env2.out"; unset WINEPATH; WINEPATH='Z:\pre' bash -c 'eval "$(cat '"$WORK"'/env2.out)"; printf "%s" "$WINEPATH"')"
case "$pre" in 'Z:\pre;'*) ok "a WINEPATH the caller already had is kept in front" ;; *) no "a WINEPATH the caller already had is kept in front" "got [$pre]" ;; esac

echo "== idempotence =="
"$SCRIPT" setup > /dev/null 2>&1; code=$?
eq "a second setup succeeds" "$code" 0
eq "a second setup downloads nothing" "$(wc -l < "$CURL_LOG" | tr -d ' ')" 2
eq "a second setup does not start wine again" "$(wc -l < "$BOOT_LOG" | tr -d ' ')" 1

echo "== a wrong checksum is refused =="
rm -rf "$CODESCOUT_WINE_HOME"; : > "$CURL_LOG"; : > "$BOOT_LOG"
mk_manifest "0000000000000000000000000000000000000000000000000000000000000000"
"$SCRIPT" setup > "$WORK/bad.out" 2>&1; code=$?
[ "$code" -ne 0 ] && ok "setup REFUSES a download whose sha256 is not the manifest's" || no "setup REFUSES a download whose sha256 is not the manifest's" "exit was 0"
has "the refusal says sha256 mismatch" "$(cat "$WORK/bad.out")" "sha256 mismatch"
eq "nothing was unpacked after the refusal" "$(find "$CODESCOUT_WINE_HOME" -name bash.exe 2>/dev/null | wc -l | tr -d ' ')" 0
eq "no wine prefix was started after the refusal" "$(wc -l < "$BOOT_LOG" | tr -d ' ')" 0
eq "no ready marker was written after the refusal" "$(find "$CODESCOUT_WINE_HOME" -name ready 2>/dev/null | wc -l | tr -d ' ')" 0
eq "the rejected download is not left under its final name" "$(find "$CODESCOUT_WINE_HOME" -name 'PortableGit-*' 2>/dev/null | wc -l | tr -d ' ')" 0

echo "== the key follows the manifest and the wine version =="
rm -rf "$CODESCOUT_WINE_HOME"
mk_manifest
"$SCRIPT" setup > /dev/null 2>&1
eq "one profile exists after setup" "$(profiles)" 1
mk_manifest "" "# an edited manifest"
"$SCRIPT" setup > /dev/null 2>&1; code=$?
eq "setup succeeds for an edited manifest" "$code" 0
eq "an edited manifest builds a SECOND profile beside the first" "$(profiles)" 2
echo "wine-10.0" > "$WORK/wine-version"
"$SCRIPT" setup > /dev/null 2>&1
eq "another wine version builds a THIRD profile" "$(profiles)" 3
echo "$WINE_VERSION" > "$WORK/wine-version"
out="$("$SCRIPT" status 2>&1)"
has "going back to the first wine selects the first profile again, ready" "$out" "ready"
eq "no profile was deleted on the way" "$(profiles)" 3

echo "== a manifest missing a key =="
rm -rf "$CODESCOUT_WINE_HOME"
grep -v '^PYTHON_URL=' "$CODESCOUT_WINE_MANIFEST" > "$WORK/manifest.short"
cp "$WORK/manifest.short" "$CODESCOUT_WINE_MANIFEST"
"$SCRIPT" setup > "$WORK/missing.out" 2>&1; code=$?
[ "$code" -ne 0 ] && ok "setup REFUSES a manifest without PYTHON_URL" || no "setup REFUSES a manifest without PYTHON_URL" "exit was 0"
has "the refusal names the missing key" "$(cat "$WORK/missing.out")" "PYTHON_URL"
PYTHON_URL="https://example.invalid/from-the-environment" "$SCRIPT" setup > "$WORK/missing2.out" 2>&1; code=$?
[ "$code" -ne 0 ] && ok "an inherited PYTHON_URL does not stand in for the missing line" || no "an inherited PYTHON_URL does not stand in for the missing line" "exit was 0"
# The exit code alone is not enough: with the inherited value accepted, setup would still fail,
# on a checksum mismatch, and the row above would pass for the wrong reason (a mutation run
# showed exactly that). The refusal must be the one about the missing key.
has "the refusal is about the missing key, not about a later checksum" "$(cat "$WORK/missing2.out")" "does not set PYTHON_URL"
mk_manifest
"$SCRIPT" setup > /dev/null 2>&1; code=$?
eq "the same script with the key restored succeeds" "$code" 0

echo "== usage =="
"$SCRIPT" > /dev/null 2>&1; code=$?
eq "no argument exits 2" "$code" 2
"$SCRIPT" nonsense > /dev/null 2>&1; code=$?
eq "an unknown subcommand exits 2" "$code" 2

echo
echo "wine-env suite: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
