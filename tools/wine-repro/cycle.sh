#!/usr/bin/env bash
# One COLD cycle of the references reproduction: stop every Wine process, start a Windows
# codescout.exe under Wine on HTTP, call `references` twice on pkg.build_chain, print the timings.
#
# Usage: cycle.sh <exe name in C:\codescout-bin> <port> <label>
# Needs: C:\node, C:\pyright (pyright 1.1.413), C:\pybig (make-project.sh), the exe copied to
#        C:\codescout-bin (built with `--no-default-features --features http` for
#        x86_64-pc-windows-gnu). See the codescout memory `wine-windows-test-suite`.
#
# `wineserver -k` stops EVERY process in the default prefix. Do not run it while something else
# uses that prefix.
set -u
exe="$1"; port="$2"; label="$3"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out="${TMPDIR:-/tmp}"
wineserver -k 2>/dev/null; sleep 2
WINEPATH='C:\node;C:\pyright\node_modules\.bin' WINEDEBUG=-all \
  wine "C:\\codescout-bin\\$exe" start --project 'C:\pybig' --transport http --host 127.0.0.1 \
  --port "$port" --auth-token wine-test > "$out/wine-cycle-$label.log" 2>&1 &
sleep 7
python3 "$here/mcp_call.py" "$port" references '{"symbol":"build_chain","path":"pkg/core.py"}' \
  references '{"symbol":"build_chain","path":"pkg/core.py"}' > "$out/wine-cycle-$label.out" 2>&1
echo "== $label ($exe)"
grep -E '^---|references in|^warning' "$out/wine-cycle-$label.out" | cut -c1-170
wineserver -k 2>/dev/null
