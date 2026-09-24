#!/usr/bin/env bash
# Suite for scripts/gate.sh's slot pool — bug 37b251b33adb37eb.
#
# gate.sh used to key CARGO_TARGET_DIR on the session id, so every session that ever ran
# the gate left a 16-33G tree behind (323G across 17 trees on 2026-09-24). The isolation
# the 2026-09-14 race needs lasts one RUN, not one session, so the pool leases a slot for
# the duration of one run and hands it to the next run afterwards. Disk then grows with
# peak concurrency instead of with the number of sessions ever started.
#
# The real gate.sh is driven here, not a copy of its logic: `cargo` is a stub on PATH that
# records the CARGO_TARGET_DIR each lane saw, `./scripts/fmt-mine.sh` is a stub in a fake
# checkout (gate.sh calls it relative to its cwd), and HOME plus CODESCOUT_GATE_POOL point
# into a temp dir, so no run of this suite can write into the real ~/.cache.
#
# Every background process this suite starts is recorded and killed BY PID on exit. A
# `pkill -f` pattern can match a peer's process on a shared machine
# (bug-fix-session-log:F-173).

set -u

PASS=0; FAIL=0
ok()   { echo "  PASS  $1"; PASS=$((PASS + 1)); }
no()   { echo "  FAIL  $1"; echo "        $2"; FAIL=$((FAIL + 1)); }
eq()   { [ "$2" = "$3" ] && ok "$1" || no "$1" "expected $3, got $2"; }

GATE="$(cd "$(dirname "$0")/.." && pwd)/scripts/gate.sh"
[ -f "$GATE" ] || { echo "no scripts/gate.sh at $GATE"; exit 2; }

WORK="$(mktemp -d "${TMPDIR:-/tmp}/gate-slot-XXXXXX")"
PIDS=()
cleanup() {
    rm -f "$WORK"/hold-*
    # Kill each gate by ITS pid: a timed-out gate must never outlive the suite, since a
    # spinning mutant once filled /tmp with ~500k lock files after its wrapper was killed.
    for f in "$WORK"/gatepid-*; do [ -e "$f" ] && kill -9 "$(cat "$f")" 2>/dev/null; done
    for p in "${PIDS[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null; done
    rm -rf "$WORK"
}
trap cleanup EXIT

POOL="$WORK/pool"
mkdir -p "$WORK/repo/scripts" "$WORK/bin" "$WORK/home"
printf '#!/usr/bin/env bash\nexit 0\n' > "$WORK/repo/scripts/fmt-mine.sh"
chmod +x "$WORK/repo/scripts/fmt-mine.sh"

# The stub records one line per lane. With `hold-<tag>` present, its FIRST call parks
# until the file is removed, so a test can act while a gate run is inside its lanes.
cat > "$WORK/bin/cargo" <<'EOF'
#!/usr/bin/env bash
tag="${GATE_TEST_TAG:?}"; w="${GATE_TEST_WORK:?}"
echo "${CARGO_TARGET_DIR:-<unset>}" >> "$w/seen-$tag"
if [ -e "$w/hold-$tag" ] && [ ! -e "$w/started-$tag" ]; then
    echo $$ > "$w/worker-$tag.pid"
    # The stub's parent IS the gate's bash. `$!` in the suite is only a wrapper subshell.
    echo "$PPID" > "$w/gate-$tag.pid"
    : > "$w/started-$tag"
    for _ in $(seq 1 300); do [ -e "$w/hold-$tag" ] || break; sleep 0.1; done
fi
exit 0
EOF
chmod +x "$WORK/bin/cargo"

# `exec` makes the stub's parent the gate's own bash; the backgrounded `$!` of this
# FUNCTION is a wrapper subshell, so case D kills the pid the stub records instead.
run_gate() {
    local tag="$1" sid="$2"; shift 2
    ( echo "$BASHPID" > "$WORK/gatepid-$tag"; cd "$WORK/repo" && exec env -u CARGO_TARGET_DIR HOME="$WORK/home" \
        PATH="$WORK/bin:$PATH" CLAUDE_CODE_SESSION_ID="$sid" CODESCOUT_GATE_POOL="$POOL" \
        GATE_TEST_TAG="$tag" GATE_TEST_WORK="$WORK" "$@" bash "$GATE" ) \
        > "$WORK/out-$tag" 2>&1
}
first_seen() { head -1 "$WORK/seen-$1" 2>/dev/null; }
wait_for()   { for _ in $(seq 1 100); do [ -e "$1" ] && return 0; sleep 0.1; done; return 1; }
slot_dirs()  { find "$POOL" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l | tr -d ' '; }

echo "A. an empty pool leases slot-0 for the whole run"
run_gate a sid-a
eq "first run leases slot-0" "$(first_seen a)" "$POOL/slot-0"
eq "the stub was reached once per cargo lane" "$(wc -l < "$WORK/seen-a" | tr -d ' ')" "3"
eq "all three cargo lanes share ONE target dir" "$(sort -u "$WORK/seen-a" | wc -l | tr -d ' ')" "1"

echo "B. a later run from a DIFFERENT session reuses the freed slot"
run_gate b sid-b
eq "the second session gets slot-0 again" "$(first_seen b)" "$POOL/slot-0"
eq "two sessions left ONE tree behind, not two" "$(slot_dirs)" "1"

echo "C. a concurrent run never shares a held slot"
: > "$WORK/hold-c"
run_gate c sid-c & PC=$!; PIDS+=("$PC")
if wait_for "$WORK/started-c"; then
    run_gate d sid-d
    eq "a run started while slot-0 is held leases slot-1" "$(first_seen d)" "$POOL/slot-1"
else
    no "holder reached its lanes" "run c never started a cargo lane: $(cat "$WORK/out-c")"
fi
rm -f "$WORK/hold-c"; wait "$PC" 2>/dev/null
eq "the holder kept slot-0 throughout" "$(sort -u "$WORK/seen-c")" "$POOL/slot-0"

echo "D. SIGKILLing a gate while its cargo still runs does NOT free the slot"
: > "$WORK/hold-e"
run_gate e sid-e 2>/dev/null & PE=$!; PIDS+=("$PE")
if wait_for "$WORK/started-e"; then
    W="$(cat "$WORK/worker-e.pid")"; G="$(cat "$WORK/gate-e.pid")"; PIDS+=("$W" "$G")
    kill -9 "$G"; wait "$PE" 2>/dev/null
    # Both preconditions matter: killing a wrapper instead of the gate once made every
    # assertion below pass whatever the lock did.
    if kill -0 "$G" 2>/dev/null; then no "the gate itself is dead" "gate $G survived kill -9"
    else ok "the gate itself is dead"; fi
    if kill -0 "$W" 2>/dev/null; then ok "the worker outlives its SIGKILLed gate"
    else no "the worker outlives its SIGKILLed gate" "worker $W died with the gate"; fi
    run_gate f sid-f
    eq "slot-0 stays leased to the orphaned worker" "$(first_seen f)" "$POOL/slot-1"
    rm -f "$WORK/hold-e"
    for _ in $(seq 1 100); do kill -0 "$W" 2>/dev/null || break; sleep 0.1; done
    run_gate g sid-g
    eq "once the orphaned worker exits, slot-0 is reused" "$(first_seen g)" "$POOL/slot-0"
else
    no "holder reached its lanes" "run e never started a cargo lane: $(cat "$WORK/out-e")"
fi

# Existing behaviour, kept: this case passes against the pre-pool script too.
echo "E. a preset CARGO_TARGET_DIR is honoured"
run_gate h sid-h CARGO_TARGET_DIR="$WORK/mine"
eq "every lane sees the preset dir" "$(sort -u "$WORK/seen-h")" "$WORK/mine"

echo "F. a flock that cannot run stops the gate instead of spinning"
mkdir -p "$WORK/noflock"
printf '#!/usr/bin/env bash\nexit 127\n' > "$WORK/noflock/flock"; chmod +x "$WORK/noflock/flock"
run_gate i sid-i PATH="$WORK/noflock:$WORK/bin:$PATH" & PI=$!; PIDS+=("$PI")
for _ in $(seq 1 100); do kill -0 "$PI" 2>/dev/null || break; sleep 0.1; done
if kill -0 "$PI" 2>/dev/null; then
    no "the gate stops within 10s" "still running; killed"; kill -9 "$(cat "$WORK/gatepid-i")" 2>/dev/null
else
    wait "$PI"; eq "the gate exits 2" "$?" "2"
fi
eq "no cargo lane ran without a lease" "$([ -e "$WORK/seen-i" ] && echo ran || echo none)" "none"

echo
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
