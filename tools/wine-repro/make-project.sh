#!/usr/bin/env bash
# Create the Wine reproduction project at C:\pybig: the standard library of the Wine Python plus a
# package `pkg` that holds one function (`build_chain`) used by six files.
#
# Why this shape: a project of ~2000 files makes a cold pyright slow enough that a first
# `references` call can come back partial. A symbol whose module name is a standard-library name
# (`textwrap.dedent`) is a trap: pyright resolves `from textwrap import dedent` to the typeshed stub,
# so the copy in this project never receives a reference. `pkg` is not a standard-library name.
#
# Usage: make-project.sh            (WINEPREFIX defaults to ~/.wine; Python must be at C:\Python311)
set -eu
prefix="${WINEPREFIX:-$HOME/.wine}"
lib="$prefix/drive_c/Python311/Lib"
dest="$prefix/drive_c/pybig"
[ -d "$lib" ] || { echo "no $lib: install Windows Python 3.11 into the prefix first" >&2; exit 1; }
[ ! -e "$dest" ] || { echo "$dest exists: remove it yourself to rebuild" >&2; exit 1; }
mkdir -p "$dest"
cp -r "$lib/." "$dest/"
mkdir -p "$dest/pkg"
: > "$dest/pkg/__init__.py"
printf 'def build_chain(x):\n    return [x, x + 1]\n' > "$dest/pkg/core.py"
for i in 1 2 3 4 5 6; do
    printf 'from pkg.core import build_chain\n\n\ndef use_%s(v):\n    return build_chain(v) + build_chain(v + %s)\n' "$i" "$i" > "$dest/pkg/user_$i.py"
done
echo "created $dest ($(find "$dest" -name '*.py' | wc -l) .py files)"
