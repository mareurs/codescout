#!/usr/bin/env bash
# Suite for scripts/gate.sh's slot pool — bug 1da62c896d649aa6.
#
# gate.sh used to key CARGO_TARGET_DIR on the session id, so every session that ever ran
# the gate left a 16-33G tree behind (323G across 17 trees on 2026-09-24). The isolation
# the 2026-09-14 race needs lasts one RUN, not one session, so the pool leases a slot for
# the duration of one run and hands it to the next run afterwards. Disk then grows with
# peak concurrency instead of with the number of sessions ever started.
#
# A pool of long-lived trees then needs two more bounds, and cases G-J pin both. Cargo
# never garbage-collects a target dir, so one slot grows toward the longest-lived tree on
# the machine (104G measured beside a 17G slot): a slot over CODESCOUT_SLOT_CEILING_MB is
# emptied under its own lock. And a burst of concurrent runs leaves high-numbered slots
# behind forever, so free slots numbered CODESCOUT_GATE_POOL_KEEP or higher are removed
# under THEIR locks by whichever run comes next (bug 503fa887ab0ec144). Case K covers
# scripts/with-slot.sh, the leased way to run one ad-hoc command, which exists because a
# session reused a path the gate printed and grew a tree outside the lease (bug
# 097aa5ca2222a91d).
#
# Cases L-N cover the last bound, by cause rather than size: a lease evicts a compiled build
# script recorded as built for another checkout (bug f1162428494d0ad1). L and N read fixtures
# and pin which scripts go and which stay; M is the half only cargo can answer, that the
# eviction makes cargo compile the script again for the lessee.
#
# Cases P-R cover the fourth bound, the same cause for the workspace crates (2026-10-16, E0432
# on a module the source declares, in a slot another checkout last built): a lease stamps the
# slot with its checkout and, when the stamp changes, evicts every workspace crate's units and
# their build-script units. P reads fixtures, R calls the function directly for the edges the
# gate cannot reach (an interrupted eviction, a nested checkout, an empty cargo-home, no
# checkout), and Q is the half only cargo can answer, in three shapes: a member whose output
# differs, a build script whose OUT_DIR differs, and the E0432 itself.
#
# The suite sets no CARGO_HOME of its own for the scripts it drives: `run_gate` and `run_with`
# unset it, so an exported one cannot move the registry exemption the fixtures assume. They
# likewise unset XDG_CONFIG_HOME and every CODESCOUT_GATE_RAM_* variable, so an operator's real
# RAM-tier config cannot move cases A-R off the disk pool.
#
# Case S covers the admission half of the opt-in RAM tier (docs/superpowers/specs/2026-10-31-gate-ram-tier-design.md):
# the config file is parsed, never sourced, and a pool is admitted only if it is a capped zram
# mount on a machine with memory to spare. `findmnt` is a stub on PATH and the two kernel files
# are fixtures, so nothing here reads this machine's real zram state.
#
# Case T covers the lease itself: a gate run takes a RAM slot before the disk pool, bounded to
# CODESCOUT_GATE_RAM_SLOTS, and `with-slot.sh --slot ram-N` names exactly one RAM slot and never
# falls back to the other tier. It reuses case S's stubs and adds a `df` stub that models a filesystem.
#
# Case W covers the reporting (spec 7): a RAM run names its tier near the top and prints the zram
# figures after the tree size, a red lane in a RAM slot adds one line that names the tier, and a disk
# run prints none of it. The stub cargo exits with GATE_TEST_EXIT, so a lane can be red.
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
git -C "$WORK/repo" init -q
cp "$(dirname "$GATE")/session_context.py" "$(dirname "$GATE")/session-context-v1.schema.json" "$WORK/repo/scripts/"
# Both exit 0 unless the case sets GATE_TEST_PY_EXIT / GATE_TEST_FMT_EXIT (case W).
printf '# Fixture Python lane.\nimport os, sys\nsys.exit(int(os.environ.get("GATE_TEST_PY_EXIT", "0")))\n' > "$WORK/repo/scripts/python-tests.py"
printf '#!/usr/bin/env bash\nexit "${GATE_TEST_FMT_EXIT:-0}"\n' > "$WORK/repo/scripts/fmt-mine.sh"
chmod +x "$WORK/repo/scripts/fmt-mine.sh"

# The stub records one line per lane. With `hold-<tag>` present, its FIRST call parks
# until the file is removed, so a test can act while a gate run is inside its lanes.
cat > "$WORK/bin/cargo" <<'EOF'
#!/usr/bin/env bash
tag="${GATE_TEST_TAG:?}"; w="${GATE_TEST_WORK:?}"
echo "${CARGO_TARGET_DIR:-<unset>}" >> "$w/seen-$tag"
if [ -e "$w/hold-$tag" ] && [ ! -e "$w/started-$tag" ] && { [ "${GATE_TEST_HOLD_DEFAULT:-0}" != 1 ] || [ "$*" = "test --workspace" ]; }; then
    echo $$ > "$w/worker-$tag.pid"
    # The stub's parent IS the gate's bash. `$!` in the suite is only a wrapper subshell.
    echo "$PPID" > "$w/gate-$tag.pid"
    : > "$w/started-$tag"
    for _ in $(seq 1 300); do [ -e "$w/hold-$tag" ] || break; sleep 0.1; done
fi
exit "${GATE_TEST_EXIT:-0}"
EOF
chmod +x "$WORK/bin/cargo"

# `exec` makes the stub's parent the gate's own bash; the backgrounded `$!` of this
# FUNCTION is a wrapper subshell, so case D kills the pid the stub records instead.
run_gate() {
    local tag="$1" sid="$2"; shift 2
    local session_env=(CLAUDE_CODE_SESSION_ID="$sid") mode=()
    case "$sid" in
        codex:*) session_env=(CODEX_SESSION_ID="${sid#codex:}") ;;
        "") session_env=(); mode=(--check) ;;
    esac
    scrub_ram_env
    ( echo "$BASHPID" > "$WORK/gatepid-$tag"; cd "$WORK/repo" && exec env -u CARGO_TARGET_DIR -u CARGO_HOME "${SCRUB[@]}" \
        -u CLAUDE_CODE_SESSION_ID -u CODEX_SESSION_ID -u CODEX_THREAD_ID -u CODESCOUT_CONTEXT_FILE \
        HOME="$WORK/home" PATH="$WORK/bin:$PATH" "${session_env[@]}" CODESCOUT_GATE_POOL="$POOL" \
        GATE_TEST_TAG="$tag" GATE_TEST_WORK="$WORK" "$@" bash "$GATE" "${mode[@]}" ) \
        > "$WORK/out-$tag" 2>&1
}
# `env` arguments that remove the operator's RAM-tier setup from a driven script: the config
# file's directory (XDG_CONFIG_HOME) and every CODESCOUT_GATE_RAM_* variable of THIS shell, so a
# real ~/.config/codescout/gate-pool.env or an exported knob cannot move cases A-R off the disk
# pool. Computed per call: case S exports one on purpose. Fills the array `SCRUB`.
scrub_ram_env() {
    SCRUB=(-u XDG_CONFIG_HOME)
    local v
    while read -r v; do [ -n "$v" ] && SCRUB+=(-u "$v"); done < <(compgen -v CODESCOUT_GATE_RAM_)
}
first_seen() { head -1 "$WORK/seen-$1" 2>/dev/null; }
wait_for()   { for _ in $(seq 1 100); do [ -e "$1" ] && return 0; sleep 0.1; done; return 1; }
slot_dirs()  { find "$POOL" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | wc -l | tr -d ' '; }
# 2 MiB of real blocks: `du -sm` must read it as over a 1M ceiling and under a 64M one.
fill()       { mkdir -p "$(dirname "$1")"; head -c 2097152 /dev/zero > "$1"; }
state()      { [ -e "$1" ] && echo kept || echo gone; }
# Hold a lock from a background shell until $WORK/hold-<tag> is removed. It exits by
# itself on that, and cleanup kills it by pid in case it does not.
hold_lock() {
    : > "$WORK/hold-$1"
    ( exec {fd}>"$2"; flock -n "$fd" || exit 1; : > "$WORK/locked-$1"
      while [ -e "$WORK/hold-$1" ]; do sleep 0.1; done ) & PIDS+=("$!")
    wait_for "$WORK/locked-$1"
}

echo "A. an empty pool leases slot-0 for the whole run"
run_gate a sid-a
eq "first run leases slot-0" "$(first_seen a)" "$POOL/slot-0"
eq "the stub was reached once per cargo lane" "$(wc -l < "$WORK/seen-a" | tr -d ' ')" "3"
eq "all three cargo lanes share ONE target dir" "$(sort -u "$WORK/seen-a" | wc -l | tr -d ' ')" "1"
# The remedy for bug 097aa5ca2222a91d has to sit where the path appears, or the path
# stays the invitation.
eq "the printed path names the leased way to reuse it" "$(grep -c 'scripts/with-slot.sh' "$WORK/out-a")" "1"

echo "B. a later run from a DIFFERENT session reuses the freed slot"
run_gate b sid-b
eq "the second session gets slot-0 again" "$(first_seen b)" "$POOL/slot-0"
eq "two sessions left ONE tree behind, not two" "$(slot_dirs)" "1"

echo "C. a concurrent run never shares a held slot"
: > "$WORK/hold-c"
run_gate c "" GATE_TEST_HOLD_DEFAULT=1 & PC=$!; PIDS+=("$PC")
if wait_for "$WORK/started-c"; then
    : > "$WORK/hold-d"
    run_gate d codex:sid-d GATE_TEST_HOLD_DEFAULT=1 & PD=$!; PIDS+=("$PD")
    if wait_for "$WORK/started-d"; then
        eq "a Codex run started during a no-identity check leases slot-1" "$(first_seen d)" "$POOL/slot-1"
        flock -n "$POOL/slot-0.lock" true; eq "check-only lease stays held during DEFAULT" "$?" "1"
        flock -n "$POOL/slot-1.lock" true; eq "Codex lease stays held during DEFAULT" "$?" "1"
    else
        no "Codex holder reached DEFAULT" "$(cat "$WORK/out-d")"
    fi
    rm -f "$WORK/hold-d"; wait "$PD" 2>/dev/null
    flock -n "$POOL/slot-1.lock" true; eq "Codex lease releases after DEFAULT" "$?" "0"
else
    no "holder reached its lanes" "run c never started a cargo lane: $(cat "$WORK/out-c")"
fi
rm -f "$WORK/hold-c"; wait "$PC" 2>/dev/null
eq "the holder kept slot-0 throughout" "$(sort -u "$WORK/seen-c")" "$POOL/slot-0"
flock -n "$POOL/slot-0.lock" true; eq "check-only lease releases after DEFAULT" "$?" "0"

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
eq "a preset dir is not called a lease" "$(grep -c 'leased for THIS run' "$WORK/out-h")" "0"

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

echo "G. a slot grown past the ceiling is emptied under its lock before the lanes run"
POOL="$WORK/pool-g"; fill "$POOL/slot-0/stale"
run_gate j sid-j CODESCOUT_SLOT_CEILING_MB=64
eq "a slot under the ceiling keeps its contents" "$(state "$POOL/slot-0/stale")" "kept"
run_gate k sid-k CODESCOUT_SLOT_CEILING_MB=1
eq "a slot over the ceiling is emptied" "$(state "$POOL/slot-0/stale")" "gone"
eq "and the run still builds in that slot" "$(first_seen k)" "$POOL/slot-0"

echo "H. free slots from KEEP up are reclaimed; a held one and the ones below KEEP are not"
POOL="$WORK/pool-h"
for n in 1 2 3 4; do mkdir -p "$POOL/slot-$n"; : > "$POOL/slot-$n/marker"; : > "$POOL/slot-$n.lock"; done
if hold_lock h4 "$POOL/slot-4.lock"; then
    run_gate l sid-l CODESCOUT_GATE_POOL_KEEP=3
    eq "the run leases the lowest free slot" "$(first_seen l)" "$POOL/slot-0"
    eq "slot-1, below KEEP, is kept" "$(state "$POOL/slot-1/marker")" "kept"
    eq "slot-2, below KEEP, is kept" "$(state "$POOL/slot-2/marker")" "kept"
    eq "slot-3, free and at KEEP, is reclaimed" "$(state "$POOL/slot-3")" "gone"
    eq "slot-4, past KEEP but held, is kept" "$(state "$POOL/slot-4/marker")" "kept"
    run_gate m sid-m CODESCOUT_GATE_POOL_KEEP=3
    # Lock files are never unlinked (see scripts/slot-pool.sh), so a pass that reported
    # every lock file would repeat the same line on every run, forever.
    eq "a slot already reclaimed is not reported again" "$(grep -c reclaim "$WORK/out-m")" "0"
else
    no "the slot-4 holder took its lock" "it never did"
fi
rm -f "$WORK/hold-h4"

echo "I. with KEEP=0 every free slot goes, but never the run's own, nor a tree no lock covers"
POOL="$WORK/pool-i"
for d in slot-0 slot-1 sid-unleased; do mkdir -p "$POOL/$d"; : > "$POOL/$d/marker"; done
: > "$POOL/slot-1.lock"
run_gate n sid-n CODESCOUT_GATE_POOL_KEEP=0
eq "the run leases slot-0" "$(first_seen n)" "$POOL/slot-0"
eq "its own slot survives its own reclaim pass" "$(state "$POOL/slot-0/marker")" "kept"
eq "a free slot-1 is reclaimed" "$(state "$POOL/slot-1")" "gone"
# Nothing can prove such a tree idle, so nothing may delete it.
eq "a tree with no lock file is never touched" "$(state "$POOL/sid-unleased/marker")" "kept"

echo "J. a ceiling or KEEP that is not a whole number stops the gate before any lane"
POOL="$WORK/pool-j"
run_gate o sid-o CODESCOUT_SLOT_CEILING_MB=48G; eq "a bad ceiling exits 2" "$?" "2"
eq "and runs no lane" "$([ -e "$WORK/seen-o" ] && echo ran || echo none)" "none"
run_gate p sid-p CODESCOUT_GATE_POOL_KEEP=three; eq "a bad KEEP exits 2" "$?" "2"
eq "and runs no lane" "$([ -e "$WORK/seen-p" ] && echo ran || echo none)" "none"

echo "K. with-slot.sh runs one command inside a leased slot"
WITH="$(dirname "$GATE")/with-slot.sh"
run_with() { # run_with <tag> [VAR=value ...] bash "$WITH" <command...>
    local tag="$1"; shift
    scrub_ram_env
    ( cd "${WITH_CWD:-$WORK/repo}" && exec env -u CARGO_TARGET_DIR -u CARGO_HOME "${SCRUB[@]}" HOME="$WORK/home" PATH="$WORK/bin:$PATH" \
        CODESCOUT_GATE_POOL="$POOL" "$@" ) > "$WORK/out-$tag" 2>&1
}
POOL="$WORK/pool-k"
run_with k1 bash "$WITH" sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-k1"
eq "the command sees the leased slot as its target dir" "$(cat "$WORK/tdir-k1" 2>/dev/null)" "$POOL/slot-0"
run_with k2 bash "$WITH" sh -c 'exit 7'; eq "the command's exit status passes through" "$?" "7"
run_with k3 bash "$WITH"; eq "no command is a usage error" "$?" "2"
run_with k4 CARGO_TARGET_DIR="$WORK/mine-k" bash "$WITH" sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-k4"
eq "a preset CARGO_TARGET_DIR is honoured" "$(cat "$WORK/tdir-k4" 2>/dev/null)" "$WORK/mine-k"
: > "$WORK/hold-k5"
run_with k5 bash "$WITH" sh -c ': > "$1"; while [ -e "$2" ]; do sleep 0.1; done' _ \
    "$WORK/started-k5" "$WORK/hold-k5" & PK=$!; PIDS+=("$PK")
if wait_for "$WORK/started-k5"; then
    run_gate q sid-q
    eq "a gate started while the command runs gets another slot" "$(first_seen q)" "$POOL/slot-1"
else
    no "the held command started" "it never did: $(cat "$WORK/out-k5")"
fi
rm -f "$WORK/hold-k5"; wait "$PK" 2>/dev/null
POOL="$WORK/pool-k6"; fill "$POOL/slot-0/stale"
run_with k6 CODESCOUT_SLOT_CEILING_MB=1 bash "$WITH" true
eq "the ceiling applies to a with-slot.sh lease too" "$(state "$POOL/slot-0/stale")" "gone"
# Falling through here would run the command in the shared target/, the exact place the
# lease exists to keep it out of.
run_with k7 CODESCOUT_GATE_POOL_KEEP=x bash "$WITH" sh -c ': > "$1"' _ "$WORK/ran-k7"
eq "a failed lease exits 2" "$?" "2"
eq "and never runs the command" "$(state "$WORK/ran-k7")" "gone"

echo "O. with-slot.sh --slot N leases exactly that slot, or refuses"
# bug f1162428494d0ad1: a repair meant for one tree was run in whichever slot happened to be
# free, and 17.6 GiB of the wrong one went.
POOL="$WORK/pool-o"; mkdir -p "$POOL/slot-0" "$POOL/slot-1"
run_with o1 bash "$WITH" --slot 1 sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-o1"
eq "a named slot is leased even when a lower one is free" "$(cat "$WORK/tdir-o1" 2>/dev/null)" "$POOL/slot-1"
if hold_lock o2 "$POOL/slot-0.lock"; then
    run_with o2 bash "$WITH" --slot 0 sh -c ': > "$1"' _ "$WORK/ran-o2"; eq "a held slot is refused with exit 2" "$?" "2"
    eq "and the command does not fall back to another slot" "$(state "$WORK/ran-o2")" "gone"
    eq "and the refusal names the slot" "$(grep -c 'slot-0 is held by another run' "$WORK/out-o2")" "1"
else
    no "the slot-0 holder took its lock" "it never did"
fi
rm -f "$WORK/hold-o2"
run_with o3 bash "$WITH" --slot 7 sh -c ': > "$1"' _ "$WORK/ran-o3"; eq "an absent slot is refused with exit 2" "$?" "2"
eq "and the command never runs" "$(state "$WORK/ran-o3")" "gone"
eq "and the pool grows no slot-7" "$(state "$POOL/slot-7.lock")" "gone"
run_with o4 bash "$WITH" --slot x sh -c ': > "$1"' _ "$WORK/ran-o4"; eq "a malformed slot is refused with exit 2" "$?" "2"
eq "and the command never runs" "$(state "$WORK/ran-o4")" "gone"
# `slot-0/../slot-1` is a directory that exists and a lock path that resolves, so only the
# whole-number check stands between a named slot and a path that names another one.
run_with o4b bash "$WITH" --slot '0/../slot-1' sh -c ': > "$1"' _ "$WORK/ran-o4b"; eq "a slot spelled as a path is refused with exit 2" "$?" "2"
eq "and the command never runs" "$(state "$WORK/ran-o4b")" "gone"
# slot-1 and not slot-0: the o2 holder above may not have released slot-0 yet, and a held
# slot is refused with the same exit 2, so a usage error here would pass for the wrong reason.
run_with o5 bash "$WITH" --slot 1; eq "--slot with no command is a usage error" "$?" "2"
eq "and it is the usage error, not a refusal of the slot" "$(grep -c 'wants a slot number and a command' "$WORK/out-o5")" "1"
run_with o6 CARGO_TARGET_DIR="$WORK/mine-o" bash "$WITH" --slot 1 sh -c ': > "$1"' _ "$WORK/ran-o6"
eq "--slot against a preset CARGO_TARGET_DIR is refused" "$?" "2"
eq "and says why" "$(grep -c 'contradicts the preset' "$WORK/out-o6")" "1"
eq "and the command never runs" "$(state "$WORK/ran-o6")" "gone"
: > "$WORK/hold-o7"
run_with o7 bash "$WITH" --slot 1 sh -c ': > "$1"; while [ -e "$2" ]; do sleep 0.1; done' _ \
    "$WORK/started-o7" "$WORK/hold-o7" & PO=$!; PIDS+=("$PO")
if wait_for "$WORK/started-o7"; then
    run_with o8 bash "$WITH" --slot 1 true; eq "a second --slot 1 while it runs is refused" "$?" "2"
else
    no "the named-slot command started" "it never did: $(cat "$WORK/out-o7")"
fi
rm -f "$WORK/hold-o7"; wait "$PO" 2>/dev/null

echo "L. a lease evicts a build script compiled for another checkout, and nothing else"
# bug f1162428494d0ad1. Cargo shares one compiled build script across checkouts and never
# reads the manifest dir baked into it, so a lessee can run a script built for someone else.
# The only owner field a slot carries is the `# env-dep:CARGO_MANIFEST_DIR=` line in the
# script's dep-info; the lease compares it with the lessee's git toplevel.
git init -q "$WORK/repo"
REPO="$WORK/repo"
# A compiled build script as cargo leaves it: its dep-info, the binary, and (when the third
# argument is `-`) no env-dep line at all, which is what a script that reads the variable at
# run time records.
script_for() { # script_for <slot dir> <name> <manifest dir | ->
    local d="$1/debug/build/$2"; mkdir -p "$d"
    { echo "$d/build_script_build-${2##*-}: build.rs"
      [ "$3" = - ] || echo "# env-dep:CARGO_MANIFEST_DIR=$3"; } > "$d/build_script_build-${2##*-}.d"
    : > "$d/build_script_build-${2##*-}"
}
fixture_l() { # fixture_l <slot dir>
    script_for "$1" own-1111     "$REPO"
    script_for "$1" member-2222  "$REPO/crates/codescout-embed"
    script_for "$1" sibling-3333 "$REPO.worktrees/other"
    script_for "$1" foreign-4444 "/elsewhere/checkout"
    script_for "$1" registry-5555 "$WORK/home/.cargo/registry/src/idx/libsqlite3-sys-0.37.0"
    script_for "$1" plain-6666   -
    # Cross-compiled layout: one level deeper, same rule.
    script_for "$1/x86_64-unknown-linux-gnu" triple-8888 "/elsewhere/checkout"
    # The same file name outside build/<pkg>-<hash> must never take its directory with it.
    mkdir -p "$1/debug/deps"
    echo "# env-dep:CARGO_MANIFEST_DIR=/elsewhere/checkout" > "$1/debug/deps/build_script_build-9999.d"
    : > "$1/debug/deps/neighbour.rlib"
    # The run unit's directory beside a foreign script's: only the compiled script goes.
    : > "$1/debug/build/foreign-4444/output"
}
slot_script() { state "$POOL/slot-0/debug/build/$1/build_script_build-${1##*-}.d"; }
POOL="$WORK/pool-l"; fixture_l "$POOL/slot-0"
run_gate l1 sid-l1
eq "a script compiled for another checkout is evicted" "$(slot_script foreign-4444)" "gone"
eq "and its whole build/<pkg>-<hash> directory goes with it" "$(state "$POOL/slot-0/debug/build/foreign-4444")" "gone"
eq "a script compiled for a sibling that merely shares the prefix is evicted" "$(slot_script sibling-3333)" "gone"
eq "a script compiled for the lessee's own tree is kept" "$(slot_script own-1111)" "kept"
eq "a script compiled for a workspace member under the lessee is kept" "$(slot_script member-2222)" "kept"
eq "a script in a --target <triple> layout is evicted too" \
    "$(state "$POOL/slot-0/x86_64-unknown-linux-gnu/debug/build/triple-8888")" "gone"
eq "the same file name outside build/<pkg>-<hash> is left alone" "$(state "$POOL/slot-0/debug/deps/build_script_build-9999.d")" "kept"
eq "and so is everything else in that directory" "$(state "$POOL/slot-0/debug/deps/neighbour.rlib")" "kept"
eq "a registry dependency's script is kept" "$(slot_script registry-5555)" "kept"
eq "a script that records no manifest dir is kept" "$(slot_script plain-6666)" "kept"
eq "the eviction is named on stderr, with the checkout it was compiled for" \
    "$(grep -c 'evicting .*foreign-4444.*compiled for /elsewhere/checkout' "$WORK/out-l1")" "1"
eq "and nothing else was reported" "$(grep -c 'evicting' "$WORK/out-l1")" "3"
run_gate l2 sid-l2
eq "a lease with nothing foreign to evict says nothing" "$(grep -c 'evicting' "$WORK/out-l2")" "0"

POOL="$WORK/pool-l3"; fixture_l "$POOL/slot-0"
script_for "$POOL/slot-0" cargohome-7777 "$WORK/cargohome/registry/src/idx/dep-1.0.0"
run_gate l3 sid-l3 CARGO_HOME="$WORK/cargohome"
eq "a script under \$CARGO_HOME is kept" "$(slot_script cargohome-7777)" "kept"
eq "and a path under the default ~/.cargo is no longer exempt once CARGO_HOME names another" \
    "$(slot_script registry-5555)" "gone"

POOL="$WORK/pool-l4"; fixture_l "$POOL/slot-0"
run_with l4 bash "$WITH" true
eq "with-slot.sh evicts on its lease too" "$(slot_script foreign-4444)" "gone"

POOL="$WORK/pool-l5"; fixture_l "$POOL/slot-0"; mkdir -p "$WORK/nogit"
WITH_CWD="$WORK/nogit" run_with l5 bash "$WITH" true
eq "outside a git tree there is no lessee, so nothing is evicted" "$(slot_script foreign-4444)" "kept"

POOL="$WORK/pool-l6"; fixture_l "$WORK/preset-l"
run_gate l6 sid-l6 CARGO_TARGET_DIR="$WORK/preset-l"
eq "a preset CARGO_TARGET_DIR is not ours to tend" \
    "$(state "$WORK/preset-l/debug/build/foreign-4444/build_script_build-4444.d")" "kept"

echo "N. the function's own edge cases, called directly"
POOLDIR="$(dirname "$GATE")"
POOL="$WORK/pool-n1"; fixture_l "$POOL/slot-0"
( . "$POOLDIR/slot-pool.sh"; slot_evict_foreign_build_scripts direct "$POOL/slot-0" "$REPO" "" ) 2>/dev/null
# An empty cargo-home would otherwise make the exemption `/*`, which matches every path and
# switches eviction off without a word, the way a mistyped ceiling would.
eq "an empty cargo-home does not exempt everything" "$(slot_script foreign-4444)" "gone"
eq "and the lessee's own script is still kept" "$(slot_script own-1111)" "kept"
POOL="$WORK/pool-n2"; fixture_l "$POOL/slot-0"
( . "$POOLDIR/slot-pool.sh"; slot_evict_foreign_build_scripts direct "$POOL/slot-0" "" "$WORK/home/.cargo" ) 2>/dev/null
eq "with no checkout nothing is evicted" "$(slot_script foreign-4444)$(slot_script sibling-3333)" "keptkept"
# The command's stdout is the command's: with-slot.sh sits in front of things like
# `cargo metadata | jq`, so a lease may speak only on stderr.
POOL="$WORK/pool-n3"; fixture_l "$POOL/slot-0"
scrub_ram_env
got="$( cd "$WORK/repo" && env -u CARGO_TARGET_DIR -u CARGO_HOME "${SCRUB[@]}" HOME="$WORK/home" PATH="$WORK/bin:$PATH" \
    CODESCOUT_GATE_POOL="$POOL" bash "$WITH" echo hi 2>/dev/null )"
eq "an eviction is reported on stderr only" "$got" "hi"
eq "and the eviction did happen in that run" "$(slot_script foreign-4444)" "gone"

echo "M. with real cargo, the second checkout builds from its own tree"
# Everything above reads a fixture. This is the half only cargo can answer: that removing
# the directory makes cargo compile the script again, and that cargo would otherwise have
# run the first checkout's. Tree B's build.rs is older than anything in the slot, which is a
# checkout whose build.rs has not changed in a while.
REAL_CARGO="$(PATH="$PATH" command -v cargo)"
if [ -z "$REAL_CARGO" ]; then
    echo "  SKIP  no cargo on PATH"
else
    toy() { # toy <dir> <marker>
        mkdir -p "$1/src"
        printf '[package]\nname = "probe"\nversion = "0.0.0"\nedition = "2021"\nbuild = "build.rs"\n\n[workspace]\n' > "$1/Cargo.toml"
        printf 'fn main() {\n    let p = concat!(env!("CARGO_MANIFEST_DIR"), "/marker.txt");\n    let s = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("read {p}: {e}"));\n    println!("cargo:rustc-env=MARK={}", s.trim());\n}\n' > "$1/build.rs"
        printf 'fn main() { println!("{}", env!("MARK")); }\n' > "$1/src/main.rs"
        echo "$2" > "$1/marker.txt"
        git init -q "$1"
    }
    toy "$WORK/toy-a" marker-A; toy "$WORK/toy-b" marker-B
    touch -d 2020-01-01 "$WORK/toy-b/build.rs"
    POOL="$WORK/pool-m"
    # HOME points at the temp dir, so hand rustup and cargo the real ones.
    real() { WITH_CWD="$1" run_with "$2" CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" \
        RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" PATH="$PATH" \
        bash "$WITH" cargo run --offline -q; }
    real "$WORK/toy-a" m1
    eq "control: tree A prints its own marker" "$(grep -c '^marker-A$' "$WORK/out-m1")" "1"
    real "$WORK/toy-b" m2
    eq "tree B, leasing A's slot, prints its own marker" "$(grep -c '^marker-B$' "$WORK/out-m2")" "1"
    eq "and not A's" "$(grep -c 'marker-A' "$WORK/out-m2")" "0"
fi

echo "P. a lease evicts the workspace crates another checkout compiled, and nothing else"
# Cargo hashes a workspace member by its path relative to the workspace root, so every
# checkout of a repository shares one fingerprint per unit, and it trusts that fingerprint
# when the lessee's files are no newer than the other checkout's dep-info. The owner field
# a slot lacks is the `.codescout-lessee` stamp the lease writes (2026-10-16, E0432 on a
# module the source declares). Cases P-R pin which units go, when, and what the stamp does;
# Q is the half only cargo can answer.
unit_p() { # unit_p <slot dir> <crate> <hash> <first source>
    local d="$1/debug"; mkdir -p "$d/deps" "$d/.fingerprint/$2-$3"
    echo "$d/deps/${2//-/_}-$3.d: $4" > "$d/deps/${2//-/_}-$3.d"
    : > "$d/.fingerprint/$2-$3/lib-$2"
}
# A package's build-script units, as cargo leaves them: the compile unit and the run unit have
# DIFFERENT hashes, neither has a deps/*.d, and the run unit's OUT_DIR is build/<pkg>-<hash>/out.
bs_p() { # bs_p <slot dir> <package> <compile hash> <run hash>
    local d="$1/debug"
    mkdir -p "$d/.fingerprint/$2-$3" "$d/.fingerprint/$2-$4" "$d/build/$2-$3" "$d/build/$2-$4/out"
    : > "$d/.fingerprint/$2-$3/build-script-build-script-build"
    : > "$d/.fingerprint/$2-$3/build-script-build-script-build.json"
    : > "$d/.fingerprint/$2-$4/run-build-script-build-script-build"
    : > "$d/.fingerprint/$2-$4/run-build-script-build-script-build.json"
    : > "$d/build/$2-$4/out/server_instructions.md"
}
fixture_p() { # fixture_p <slot dir>
    unit_p "$1" codescout-embed aaaa111122223333 crates/codescout-embed/src/lib.rs
    unit_p "$1" codescout bbbb111122223333 src/lib.rs
    unit_p "$1" serde cccc111122223333 "$WORK/home/.cargo/registry/src/idx/serde-1.0.0/src/lib.rs"
    # A foreign member under a --target <triple> layout: same rule, one level deeper.
    unit_p "$1/x86_64-unknown-linux-gnu" codescout-embed dddd111122223333 crates/codescout-embed/src/lib.rs
    # A path dependency outside the workspace lists an absolute source that is not the registry's.
    unit_p "$1" outside eeee999922223333 /elsewhere/outside/src/lib.rs
    bs_p "$1" codescout eeee111122223333 ffff111122223333
    bs_p "$1" serde 9999aaaabbbbcccc 8888aaaabbbbcccc
}
fp_p() { state "$POOL/slot-0/debug/.fingerprint/$1"; }
bd_p() { state "$POOL/slot-0/debug/build/$1"; }
POOL="$WORK/pool-p1"; fixture_p "$POOL/slot-0"
run_gate p1 sid-p1
eq "a tree with no recorded lessee has its workspace crates evicted" "$(fp_p codescout-embed-aaaa111122223333)" "gone"
eq "the root crate too" "$(fp_p codescout-bbbb111122223333)" "gone"
eq "a path dependency outside the workspace and the registry too" "$(fp_p outside-eeee999922223333)" "gone"
eq "a registry dependency is kept" "$(fp_p serde-cccc111122223333)" "kept"
eq "its dep-info is kept" "$(state "$POOL/slot-0/debug/deps/codescout_embed-aaaa111122223333.d")" "kept"
eq "a member under a --target <triple> layout is evicted" \
    "$(state "$POOL/slot-0/x86_64-unknown-linux-gnu/debug/.fingerprint/codescout-embed-dddd111122223333")" "gone"
# The build-script units carry no deps/*.d, so only the package name leads to them. Without
# them cargo reruns no build script, and OUT_DIR keeps the other checkout's files.
eq "an evicted package's build-script COMPILE unit is evicted" "$(fp_p codescout-eeee111122223333)" "gone"
eq "an evicted package's build-script RUN unit is evicted" "$(fp_p codescout-ffff111122223333)" "gone"
eq "and the run unit's OUT_DIR goes with it" "$(bd_p codescout-ffff111122223333)" "gone"
eq "and the compiled script's directory" "$(bd_p codescout-eeee111122223333)" "gone"
eq "a registry package's build-script units are kept" "$(fp_p serde-9999aaaabbbbcccc)$(fp_p serde-8888aaaabbbbcccc)" "keptkept"
eq "and its OUT_DIR" "$(bd_p serde-8888aaaabbbbcccc/out)" "kept"
eq "the lessee is recorded" "$(cat "$POOL/slot-0/.codescout-lessee" 2>/dev/null)" "$REPO"
eq "the eviction is ONE line, naming the count and the checkout it was for" \
    "$(grep -c "evicted 6 compiled units of workspace crates .*last leased for no recorded checkout, not for $REPO" "$WORK/out-p1")" "1"
eq "and nothing else was reported" "$(grep -c 'evict' "$WORK/out-p1")" "1"
# Same checkout again: its own edits bump its own mtimes, so cargo's rule is sound there and
# a rebuild on every lease would throw the warm slot away.
fixture_p "$POOL/slot-0"
run_gate p2 sid-p2
eq "the same checkout leasing again evicts nothing" "$(fp_p codescout-embed-aaaa111122223333)$(fp_p codescout-ffff111122223333)" "keptkept"
eq "and says nothing" "$(grep -c 'evict' "$WORK/out-p2")" "0"
echo "/elsewhere/checkout" > "$POOL/slot-0/.codescout-lessee"
run_gate p3 sid-p3
eq "a tree stamped for another checkout is evicted" "$(fp_p codescout-embed-aaaa111122223333)" "gone"
eq "and re-stamped for this one" "$(cat "$POOL/slot-0/.codescout-lessee" 2>/dev/null)" "$REPO"

POOL="$WORK/pool-p4"; fixture_p "$POOL/slot-0"
run_with p4 bash "$WITH" true
eq "with-slot.sh evicts on its lease too" "$(fp_p codescout-embed-aaaa111122223333)" "gone"
POOL="$WORK/pool-p5"; fixture_p "$POOL/slot-0"
WITH_CWD="$WORK/nogit" run_with p5 bash "$WITH" true
eq "outside a git tree there is no lessee, so nothing is evicted" "$(fp_p codescout-embed-aaaa111122223333)" "kept"
eq "and none is recorded" "$([ -e "$POOL/slot-0/.codescout-lessee" ] && echo stamped || echo none)" "none"
# A stale stamp would let a build under THIS lease write units the stamped owner then trusts.
POOL="$WORK/pool-p5b"; fixture_p "$POOL/slot-0"; echo "$REPO" > "$POOL/slot-0/.codescout-lessee"
WITH_CWD="$WORK/nogit" run_with p5b bash "$WITH" true
eq "a lease with no lessee removes the previous owner's stamp" \
    "$([ -e "$POOL/slot-0/.codescout-lessee" ] && echo stamped || echo none)" "none"
eq "and still evicts nothing" "$(fp_p codescout-embed-aaaa111122223333)" "kept"
POOL="$WORK/pool-p6"; fixture_p "$POOL/slot-0"
run_gate p6 sid-p6 CARGO_HOME="$WORK/cargohome"
eq "a path under the default ~/.cargo is no longer exempt once CARGO_HOME names another" "$(fp_p serde-cccc111122223333)" "gone"
POOL="$WORK/pool-p7"; fixture_p "$WORK/preset-p"
run_gate p7 sid-p7 CARGO_TARGET_DIR="$WORK/preset-p"
eq "a preset CARGO_TARGET_DIR is not ours to tend" \
    "$(state "$WORK/preset-p/debug/.fingerprint/codescout-embed-aaaa111122223333")" "kept"
POOL="$WORK/pool-p8"; fixture_p "$POOL/slot-0"
CARGO_HOME="$WORK/not-the-default" run_gate p8 sid-p8
eq "an exported CARGO_HOME does not leak into the suite's leases" "$(fp_p serde-cccc111122223333)" "kept"

echo "Q. with real cargo, the second checkout builds from its own tree"
# The half only cargo can answer. Tree B is a checkout whose sources have not changed in a
# while: everything in it is OLDER than A's build, except where a merge touched it. Without the
# eviction cargo trusts A's compiled units for B's crates.
if [ -z "$REAL_CARGO" ]; then
    echo "  SKIP  no cargo on PATH"
else
    # HOME points at the temp dir, so hand rustup and cargo the real ones.
    realc() { # realc <cwd> <tag> <cargo args...>
        WITH_CWD="$1" run_with "$2" CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}" \
            RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}" PATH="$PATH" bash "$WITH" cargo "${@:3}"; }
    # q1-q3: a member crate whose output differs between the trees.
    ws() { # ws <dir> <marker>
        mkdir -p "$1/emb/src" "$1/src"
        printf '[package]\nname = "probe"\nversion = "0.0.0"\nedition = "2021"\n\n[dependencies]\nemb = { path = "emb" }\n\n[workspace]\nmembers = ["emb"]\n' > "$1/Cargo.toml"
        printf '[package]\nname = "emb"\nversion = "0.0.0"\nedition = "2021"\n' > "$1/emb/Cargo.toml"
        printf 'pub fn mark() -> &'"'"'static str { "%s" }\n' "$2" > "$1/emb/src/lib.rs"
        printf 'fn main() { println!("{}", emb::mark()); }\n' > "$1/src/main.rs"
        git init -q "$1"
    }
    ws "$WORK/ws-a" member-A; ws "$WORK/ws-b" member-B
    touch -d 2020-01-01 "$WORK/ws-b/emb/src/lib.rs" "$WORK/ws-b/src/main.rs" "$WORK/ws-b/emb/Cargo.toml" "$WORK/ws-b/Cargo.toml"
    POOL="$WORK/pool-q"
    realc "$WORK/ws-a" q1 run --offline -q
    eq "control: tree A prints its own member" "$(grep -c '^member-A$' "$WORK/out-q1")" "1"
    realc "$WORK/ws-b" q2 run --offline -q
    eq "tree B, leasing A's slot, prints its own member" "$(grep -c '^member-B$' "$WORK/out-q2")" "1"
    eq "and not A's" "$(grep -c 'member-A' "$WORK/out-q2")" "0"

    # q4-q5: a build script writing into OUT_DIR from its own tree's file, the shape of
    # build.rs's include_str! of server_instructions.md. The run unit has no deps/*.d.
    bs() { # bs <dir> <marker>
        mkdir -p "$1/src"
        printf '[package]\nname = "bsprobe"\nversion = "0.0.0"\nedition = "2021"\n\n[workspace]\n' > "$1/Cargo.toml"
        printf 'fn main() {\n    let d = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());\n    let s = std::fs::read_to_string(d.join("marker.txt")).unwrap();\n    std::fs::write(std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("m.txt"), s.trim()).unwrap();\n    println!("cargo:rerun-if-changed=marker.txt");\n    println!("cargo:rerun-if-changed=build.rs");\n}\n' > "$1/build.rs"
        printf 'fn main() { println!("{}", include_str!(concat!(env!("OUT_DIR"), "/m.txt"))); }\n' > "$1/src/main.rs"
        echo "$2" > "$1/marker.txt"
        git init -q "$1"
    }
    bs "$WORK/bs-a" marker-A; bs "$WORK/bs-b" marker-B
    find "$WORK/bs-b" -path '*/.git' -prune -o -type f -exec touch -d 2020-01-01 {} +
    POOL="$WORK/pool-q2"
    realc "$WORK/bs-a" q4 run --offline -q
    eq "control: tree A's build script writes A's marker into OUT_DIR" "$(grep -c '^marker-A$' "$WORK/out-q4")" "1"
    realc "$WORK/bs-b" q5 run --offline -q
    eq "tree B, leasing A's slot, gets its own OUT_DIR contents" "$(grep -c '^marker-B$' "$WORK/out-q5")" "1"
    eq "and not A's" "$(grep -c 'marker-A' "$WORK/out-q5")" "0"

    # q6-q7: the 2026-10-16 failure. B's member gains a module and a lib-test unit. Only the
    # member's NEW unit is dirty, so cargo prints `Compiling emb` and then links the binary
    # against A's stale lib unit: E0432 on a module B's source declares.
    ceil() { # ceil <dir> <with-module 0|1>
        mkdir -p "$1/emb/src" "$1/src"
        printf '[package]\nname = "probe"\nversion = "0.0.0"\nedition = "2021"\n\n[dependencies]\nemb = { path = "emb" }\n\n[workspace]\nmembers = ["emb"]\n' > "$1/Cargo.toml"
        printf '[package]\nname = "emb"\nversion = "0.0.0"\nedition = "2021"\n' > "$1/emb/Cargo.toml"
        if [ "$2" = 1 ]; then
            printf 'pub mod ceiling;\npub fn mark() -> u32 { 1 }\n#[cfg(test)] mod t { #[test] fn x() {} }\n' > "$1/emb/src/lib.rs"
            printf 'pub fn judge() -> u32 { 7 }\n' > "$1/emb/src/ceiling.rs"
            printf 'use emb::ceiling::judge;\nfn main() { println!("{}", judge() + emb::mark()); }\n' > "$1/src/main.rs"
        else
            printf 'pub fn mark() -> u32 { 1 }\n#[cfg(test)] mod t { #[test] fn x() {} }\n' > "$1/emb/src/lib.rs"
            printf 'fn main() { println!("{}", emb::mark()); }\n' > "$1/src/main.rs"
        fi
        git init -q "$1"
    }
    ceil "$WORK/ce-a" 0; ceil "$WORK/ce-b" 1
    find "$WORK/ce-b" -path '*/.git' -prune -o -type f -exec touch -d 2020-01-01 {} +
    POOL="$WORK/pool-q3"
    realc "$WORK/ce-a" q6 build --offline -q
    sleep 1; touch "$WORK/ce-b/src/main.rs"
    realc "$WORK/ce-b" q7 test --offline --workspace --no-run; rc=$?
    eq "tree B's test build, leasing A's slot, does not hit E0432 on its own module" "$(grep -c 'E0432' "$WORK/out-q7")" "0"
    eq "and succeeds" "$rc" "0"
fi

echo "R. the eviction's own edge cases, called directly"
POOLDIR="$(dirname "$GATE")"
direct() { # direct <tree> <checkout> <cargo-home> [shell prelude]
    ( . "$POOLDIR/slot-pool.sh"; eval "${4:-:}"; slot_evict_foreign_members direct "$1" "$2" "$3" ) 2>/dev/null; }
# The stamp is the last thing written: a lease killed part-way must leave the OLD owner on
# record, or the next lease would trust units this one never finished removing.
POOL="$WORK/pool-r1"; fixture_p "$POOL/slot-0"; echo "/elsewhere/checkout" > "$POOL/slot-0/.codescout-lessee"
direct "$POOL/slot-0" "$REPO" "$WORK/home/.cargo" 'rm() { exit 1; }'
eq "an eviction interrupted at its first removal leaves the previous stamp" "$(cat "$POOL/slot-0/.codescout-lessee")" "/elsewhere/checkout"
direct "$POOL/slot-0" "$REPO" "$WORK/home/.cargo"
eq "and the next lease then completes it" "$(fp_p codescout-embed-aaaa111122223333)" "gone"
eq "and stamps the lessee" "$(cat "$POOL/slot-0/.codescout-lessee")" "$REPO"
# The stamp is compared whole. A worktree nested under the stamped checkout (the shape of
# .claude/worktrees/agent-x) is another lessee, not the same one.
POOL="$WORK/pool-r2"; fixture_p "$POOL/slot-0"; echo "$REPO" > "$POOL/slot-0/.codescout-lessee"
direct "$POOL/slot-0" "$REPO/.claude/worktrees/agent-x" "$WORK/home/.cargo"
eq "a checkout nested under the stamped one is a different lessee" "$(fp_p codescout-embed-aaaa111122223333)" "gone"
eq "and a checkout merely sharing the stamped path as a prefix is too" "$(
    POOL="$WORK/pool-r2b"; fixture_p "$POOL/slot-0"; echo "$REPO" > "$POOL/slot-0/.codescout-lessee"
    direct "$POOL/slot-0" "$REPO.worktrees/x" "$WORK/home/.cargo"; fp_p codescout-embed-aaaa111122223333)" "gone"
# An empty cargo-home would make the exemption `/*`, which matches every absolute source and
# switches the eviction of a path dependency off without a word.
POOL="$WORK/pool-r3"; fixture_p "$POOL/slot-0"
direct "$POOL/slot-0" "$REPO" ""
eq "an empty cargo-home does not exempt every absolute source" "$(fp_p outside-eeee999922223333)" "gone"
eq "and the registry unit is no longer exempt" "$(fp_p serde-cccc111122223333)" "gone"
POOL="$WORK/pool-r4"; fixture_p "$POOL/slot-0"; echo "$REPO" > "$POOL/slot-0/.codescout-lessee"
direct "$POOL/slot-0" "" "$WORK/home/.cargo"
eq "with no checkout nothing is evicted" "$(fp_p codescout-embed-aaaa111122223333)" "kept"
eq "and the previous owner's stamp is removed" "$([ -e "$POOL/slot-0/.codescout-lessee" ] && echo stamped || echo none)" "none"

echo "S. the opt-in RAM tier: a config is parsed, and a mount is admitted only if it is a capped zram"
# Admission is observed through what `lease_gate_target` says on stderr (case T pins the lease
# itself, and its order against the disk pool). Every fixture
# that expects admission sets CODESCOUT_GATE_RAM_MEMINFO far above the floor, because the real
# /proc/meminfo of a CI runner or a busy machine may sit below 12G and turn the case into a flake.
RAMCFG="$WORK/home/.config/codescout/gate-pool.env"
mkdir -p "$WORK/ramstub" "$(dirname "$RAMCFG")"
printf '#!/usr/bin/env bash\ncat "%s/findmnt-source"\n' "$WORK" > "$WORK/ramstub/findmnt"
chmod +x "$WORK/ramstub/findmnt"
# A `df` that models a real filesystem, so the pool rule sees free space fall as a slot grows and
# rise as the evictor empties one: it prints, in `df -Pm` format, Available = $WORK/df-cap (MiB,
# default 1000000) minus `du -sm` of the path it is asked about (its last argument, the RAM pool).
# `du -sm` rounds UP and counts directory blocks, which is how the rule itself measures a slot.
cat > "$WORK/ramstub/df" <<EOF
#!/usr/bin/env bash
cap=\$(cat "$WORK/df-cap" 2>/dev/null || echo 1000000)
used=\$(du -sm "\${@: -1}" 2>/dev/null | cut -f1)
avail=\$((cap - \${used:-0})); [ "\$avail" -ge 0 ] || avail=0
printf 'Filesystem 1048576-blocks Used Available Capacity Mounted on\n'
printf '/dev/zram1 %s %s %s 1%% /mnt/zram1\n' "\$cap" "\${used:-0}" "\$avail"
EOF
chmod +x "$WORK/ramstub/df"
printf '%s\n' "MemAvailable:   33554432 kB" > "$WORK/meminfo-high"      # 32 GiB
printf '%s\n' "MemAvailable:    4194304 kB" > "$WORK/meminfo-low"       #  4 GiB
printf '%s\n' "MemAvailable:   12582912 kB" > "$WORK/meminfo-edge"      # exactly 12288 MiB
printf '%s\n' "1048576 524288 600000 8589934592 700000 0 0 0 0" > "$WORK/mmstat-capped"
printf '%s\n' "0 0 0 0 0 0 0 0 0" > "$WORK/mmstat-uncapped"
printf '%s\n' /dev/zram1 > "$WORK/findmnt-zram"
ram_cfg()  { rm -f "$RAMCFG"; [ "$#" -eq 0 ] || printf '%s\n' "$@" > "$RAMCFG"; }
ram_src()  { printf '%s\n' "$1" > "$WORK/findmnt-source"; }
# s_run <tag> [VAR=value ...] — one lease through with-slot.sh, with the findmnt stub first on PATH.
s_run() {
    local tag="$1"; shift
    rm -f "$WORK/tdir-$tag"
    run_with "$tag" PATH="$WORK/ramstub:$WORK/bin:$PATH" "$@" bash "$WITH" \
        sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-$tag"
}
s_target() { cat "$WORK/tdir-$1" 2>/dev/null; }
POOL="$WORK/pool-s"
S_OK=(CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-high" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-capped")

ram_cfg; ram_src /dev/nvme0n1p2; rm -rf "$POOL"
s_run s1
eq "S1 no config: the disk slot is leased" "$(s_target s1)" "$POOL/slot-0"
eq "S1 and the lease says nothing about a RAM tier" "$(grep -c 'RAM tier' "$WORK/out-s1")" "0"

ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram"; ram_src /dev/nvme0n1p2; rm -rf "$POOL"
s_run s2 "${S_OK[@]}"
eq "S2 a pool that is not a zram mount: the disk slot is leased" "$(s_target s2)" "$POOL/slot-0"
eq "S2 and the skip is reported" "$(grep -c 'RAM tier skipped' "$WORK/out-s2")" "1"
eq "S2 with its cause" "$(grep -c 'not a zram mount' "$WORK/out-s2")" "1"
eq "S2 and the knob that names the pool" "$(grep -c 'CODESCOUT_GATE_RAM_POOL' "$WORK/out-s2")" "1"

ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram" "CODESCOUT_GATE_RAM_MEM_FLOOR_MB=12G"; ram_src /dev/zram1; rm -rf "$POOL"
s_run s3 "${S_OK[@]}"; rc=$?
eq "S3 a floor that is not a whole number: the disk slot is leased" "$(s_target s3)" "$POOL/slot-0"
eq "S3 and the lease still exits 0" "$rc" "0"
eq "S3 the message names the knob" "$(grep -c 'CODESCOUT_GATE_RAM_MEM_FLOOR_MB' "$WORK/out-s3")" "1"
eq "S3 and the value" "$(grep -c '12G' "$WORK/out-s3")" "1"

ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram"; ram_src /dev/zram1; rm -rf "$POOL"
s_run s4 CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-low" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-capped"
eq "S4 MemAvailable under the floor: the disk slot is leased" "$(s_target s4)" "$POOL/slot-0"
eq "S4 the message names MemAvailable" "$(grep -c 'MemAvailable 4096M is below the 12288M floor' "$WORK/out-s4")" "1"
eq "S4 and the knob" "$(grep -c 'CODESCOUT_GATE_RAM_MEM_FLOOR_MB' "$WORK/out-s4")" "1"

ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram"; ram_src /dev/zram1; rm -rf "$POOL"
s_run s5 CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-high" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-uncapped"
eq "S5 a zram with no RAM cap: the disk slot is leased" "$(s_target s5)" "$POOL/slot-0"
eq "S5 the message says there is no cap" "$(grep -c 'no RAM cap' "$WORK/out-s5")" "1"

# The file is parsed, never sourced: a value that is a command must not run.
ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram" "CODESCOUT_GATE_RAM_SLOTS=\$(touch $WORK/pwned)"; ram_src /dev/zram1
rm -rf "$POOL" "$WORK/pwned"
s_run s6 "${S_OK[@]}"
eq "S6 a command in the file does not run" "$([ -e "$WORK/pwned" ] && echo ran || echo not-run)" "not-run"
eq "S6 the disk slot is leased" "$(s_target s6)" "$POOL/slot-0"
eq "S6 the message names the key" "$(grep -c 'CODESCOUT_GATE_RAM_SLOTS' "$WORK/out-s6")" "1"
ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram" "CODESCOUT_GATE_RAM_SLOTS=\$(touch"$'\t'"$WORK/pwned2)"; rm -rf "$POOL" "$WORK/pwned2"
s_run s6b "${S_OK[@]}"
eq "S6 nor does one split across whitespace" "$([ -e "$WORK/pwned2" ] && echo ran || echo not-run)" "not-run"
ram_cfg "CODESCOUT_GATE_RAM_POOL=\$(touch $WORK/pwned3)"; rm -rf "$POOL" "$WORK/pwned3"
s_run s6c "${S_OK[@]}"
eq "S6 nor one in the pool path" "$([ -e "$WORK/pwned3" ] && echo ran || echo not-run)" "not-run"

# Only CODESCOUT_GATE_RAM_* keys are read from the file.
ram_cfg "CODESCOUT_GATE_POOL=$WORK/elsewhere" "CODESCOUT_GATE_RAM_POOL_NOTE_IS_IGNORED_BUT_WELL_FORMED=1"; rm -rf "$POOL"
s_run s7 "${S_OK[@]}"
eq "S7 a non-RAM key in the file does not move the pool" "$(s_target s7)" "$POOL/slot-0"
eq "S7 and nothing is said about a RAM tier" "$(grep -c 'RAM tier' "$WORK/out-s7")" "0"
eq "S7 nor is anything leased under the key's path" "$([ -e "$WORK/elsewhere" ] && echo created || echo absent)" "absent"

# The operator's own exported knob must not reach a driven script (the hermetic harness).
ram_cfg; rm -rf "$POOL"
export CODESCOUT_GATE_RAM_POOL="$WORK/ram-leak"
run_gate s8 sid-s8
unset CODESCOUT_GATE_RAM_POOL
eq "S8 an exported CODESCOUT_GATE_RAM_POOL does not reach run_gate: the disk slot is leased" "$(first_seen s8)" "$POOL/slot-0"
eq "S8 and the run says nothing about a RAM tier" "$(grep -c 'RAM tier' "$WORK/out-s8")" "0"

# Admitted: no message, and the RAM slot is the one leased.
ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram"; ram_src /dev/zram1; rm -rf "$POOL"
s_run s9 "${S_OK[@]}"
eq "S9 a capped zram with memory to spare is admitted without a word" "$(grep -c 'RAM tier' "$WORK/out-s9")" "0"
eq "S9 and the RAM slot is the one leased" "$(s_target s9)" "$WORK/ram/slot-0"
rm -rf "$POOL"
s_run s9b CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-edge" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-capped"
eq "S9 MemAvailable exactly at the floor is admitted" "$(grep -c 'RAM tier' "$WORK/out-s9b")" "0"
printf '%s\n' "MemAvailable:   12582911 kB" > "$WORK/meminfo-edge-1"
rm -rf "$POOL"
s_run s9c CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-edge-1" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-capped"
eq "S9 and one kB under it is not (the floor is in whole MiB, rounded down)" "$(grep -c 'is below the 12288M floor' "$WORK/out-s9c")" "1"

# An environment value overrides the file; the file alone may carry the whole config.
ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram" "CODESCOUT_GATE_RAM_MEM_FLOOR_MB=99999999"; rm -rf "$POOL"
s_run s10 "${S_OK[@]}" CODESCOUT_GATE_RAM_MEM_FLOOR_MB=1
eq "S10 an environment floor overrides the file's" "$(grep -c 'RAM tier' "$WORK/out-s10")" "0"
rm -rf "$POOL"
s_run s10b "${S_OK[@]}"
eq "S10 while the file's floor holds when the environment is silent" "$(grep -c 'is below the 99999999M floor' "$WORK/out-s10b")" "1"
ram_cfg; ram_src /dev/nvme0n1p2; rm -rf "$POOL"
s_run s10c "${S_OK[@]}" CODESCOUT_GATE_RAM_POOL="$WORK/ram"
eq "S10 an environment pool needs no file" "$(grep -c 'not a zram mount' "$WORK/out-s10c")" "1"
ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram"; ram_src /dev/zram1; rm -rf "$POOL"
s_run s10d "${S_OK[@]}" CODESCOUT_GATE_RAM_COLD_MB=lots
eq "S10 a bad value in the environment is named too" "$(grep -c 'CODESCOUT_GATE_RAM_COLD_MB' "$WORK/out-s10d")" "1"
ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram" "# a comment" "" "CODESCOUT_GATE_RAM_SLOTS=1"; rm -rf "$POOL"
s_run s10e "${S_OK[@]}"
eq "S10 comments and blank lines are skipped" "$(grep -c 'RAM tier' "$WORK/out-s10e")" "0"
ram_cfg "CODESCOUT_GATE_RAM_POOL=$WORK/ram" "CODESCOUT_GATE_RAM_SLOTS"; rm -rf "$POOL"
s_run s10f "${S_OK[@]}"
eq "S10 a RAM key with no value is invalid and named" "$(grep -c 'CODESCOUT_GATE_RAM_SLOTS' "$WORK/out-s10f")" "1"
ram_cfg

echo "T. a lease takes a RAM slot before the disk pool, bounded to RAM_SLOTS; --slot ram-N never falls back"
# Case S's stubs (the `df` among them), plus the 20 GiB mm_stat fixture below.
RAM="$WORK/ram-t"
printf '%s\n' "1048576 524288 600000 21474836480 700000 0 0 0 0" > "$WORK/mmstat-20g"       # a 20 GiB cap
T_OK=(CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-high" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-20g")
T_PATH="$WORK/ramstub:$WORK/bin:$PATH"
# t_run <tag> [VAR=value ...] bash "$WITH" ... — one with-slot.sh run on the RAM-tier fixtures.
t_run() { local tag="$1"; shift; rm -f "$WORK/tdir-$tag"; run_with "$tag" PATH="$T_PATH" "${T_OK[@]}" "$@"; }
seen_all() { sort -u "$WORK/seen-$1" 2>/dev/null | tr '\n' ' ' | sed 's/ $//'; }
# Wait until nothing holds <lock>: the holder polls for its hold-file every 0.1 s.
wait_free() { local f; for _ in $(seq 1 50); do ( exec {f}>"$1"; flock -n "$f" ) 2>/dev/null && return 0; sleep 0.1; done; return 1; }
t_release() { rm -f "$WORK/hold-$1"; wait_free "$2"; }
CHECKOUT="$(git -C "$WORK/repo" rev-parse --show-toplevel)"

POOL="$WORK/pool-t"; rm -rf "$POOL" "$RAM"; mkdir -p "$RAM"; ram_src /dev/zram1
ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM"
run_gate t1 sid-t1 PATH="$T_PATH" "${T_OK[@]}"
eq "T1 a gate run's lanes all see the first RAM slot" "$(seen_all t1)" "$RAM/slot-0"
eq "T1 and the disk pool is untouched" "$([ -e "$POOL" ] && echo created || echo absent)" "absent"
eq "T6 the lessee stamp is written in the RAM slot (the eviction runs there)" "$(cat "$RAM/slot-0/.codescout-lessee" 2>/dev/null)" "$CHECKOUT"

if hold_lock t2 "$RAM/slot-0.lock"; then
    run_gate t2 sid-t2 PATH="$T_PATH" "${T_OK[@]}"
    eq "T2 with slot-0 held, the run sees the second RAM slot" "$(seen_all t2)" "$RAM/slot-1"
    eq "T2 and slot-1 was created for it" "$([ -d "$RAM/slot-1" ] && echo dir || echo none)" "dir"
    t_release t2 "$RAM/slot-0.lock"
else
    no "T2 the lock was held" "hold_lock failed"
fi

if hold_lock t3a "$RAM/slot-0.lock" && hold_lock t3b "$RAM/slot-1.lock"; then
    run_gate t3 sid-t3 PATH="$T_PATH" "${T_OK[@]}"
    eq "T3 with both RAM slots held, the run sees the disk slot" "$(seen_all t3)" "$POOL/slot-0"
    eq "T3 and no RAM slot-2 directory is created" "$([ -e "$RAM/slot-2" ] && echo created || echo absent)" "absent"
    eq "T3 nor a RAM slot-2 lock file" "$([ -e "$RAM/slot-2.lock" ] && echo created || echo absent)" "absent"
    eq "T3 and nothing was said about the tier" "$(grep -c 'RAM tier' "$WORK/out-t3")" "0"
    t_release t3a "$RAM/slot-0.lock"; t_release t3b "$RAM/slot-1.lock"
else
    no "T3 both locks were held" "hold_lock failed"
fi

# A named RAM slot: exactly that one, or a refusal that runs nothing.
mkdir -p "$RAM/slot-1"
t_run t4 bash "$WITH" --slot ram-1 sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-t4"; rc=$?
eq "T4 --slot ram-1 leases exactly RAM slot-1 although slot-0 is free" "$(s_target t4)" "$RAM/slot-1"
eq "T4 and exits 0" "$rc" "0"
if hold_lock t4h "$RAM/slot-1.lock"; then
    t_run t4b bash "$WITH" --slot ram-1 sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-t4b"; rc=$?
    eq "T4 a held RAM slot-1 exits 2" "$rc" "2"
    eq "T4 and runs nothing, not even on the free RAM slot-0 or on disk" "$([ -e "$WORK/tdir-t4b" ] && echo ran || echo none)" "none"
    t_release t4h "$RAM/slot-1.lock"
else
    no "T4 slot-1 lock was held" "hold_lock failed"
fi
t_run t4c bash "$WITH" --slot ram-1 sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-t4c"
eq "T4 freed, it is leased again" "$(s_target t4c)" "$RAM/slot-1"
rm -rf "$RAM/slot-1"
t_run t4d bash "$WITH" --slot ram-1 sh -c ': > "$1"' _ "$WORK/ran-t4d"; rc=$?
eq "T4 an absent RAM slot exits 2" "$rc" "2"
eq "T4 and runs nothing" "$([ -e "$WORK/ran-t4d" ] && echo ran || echo none)" "none"
t_run t4e bash "$WITH" --slot ram-2 sh -c ': > "$1"' _ "$WORK/ran-t4e"; rc=$?
eq "T4 a RAM slot at RAM_SLOTS exits 2" "$rc" "2"
eq "T4 and creates no lock file for it" "$([ -e "$RAM/slot-2.lock" ] && echo created || echo absent)" "absent"
t_run t4f bash "$WITH" --slot ram- sh -c ': > "$1"' _ "$WORK/ran-t4f"; rc=$?
eq "T4 'ram-' with no number exits 2" "$rc" "2"
eq "T4 and runs nothing (it must not turn into an unnamed lease)" "$([ -e "$WORK/ran-t4f" ] && echo ran || echo none)" "none"
t_run t4g bash "$WITH" --slot ram-x sh -c ': > "$1"' _ "$WORK/ran-t4g"; rc=$?
eq "T4 a RAM slot that is not a number exits 2" "$rc" "2"
t_run t4h2 CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-low" bash "$WITH" --slot ram-0 sh -c ': > "$1"' _ "$WORK/ran-t4h"; rc=$?
eq "T4 an unadmitted tier exits 2 rather than fall back to disk" "$rc" "2"
eq "T4 saying why" "$(grep -c 'is below the 12288M floor' "$WORK/out-t4h2")" "1"
eq "T4 and runs nothing" "$([ -e "$WORK/ran-t4h" ] && echo ran || echo none)" "none"

# Numeric --slot is the disk pool, as before, even with a RAM tier configured and free.
mkdir -p "$POOL/slot-0"
t_run t4i bash "$WITH" --slot 0 sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-t4i"
eq "T4 a numeric --slot leases the disk slot" "$(s_target t4i)" "$POOL/slot-0"
eq "T4 and says nothing about the tier" "$(grep -c 'RAM tier' "$WORK/out-t4i")" "0"

ram_cfg
t_run t5 bash "$WITH" --slot ram-1 sh -c ': > "$1"' _ "$WORK/ran-t5"; rc=$?
eq "T5 --slot ram-1 with no RAM config exits 2" "$rc" "2"
eq "T5 naming the RAM tier" "$(grep -c 'RAM tier' "$WORK/out-t5")" "1"
eq "T5 and runs nothing" "$([ -e "$WORK/ran-t5" ] && echo ran || echo none)" "none"

# RAM_SLOTS=0 is a valid config with no RAM slots: the disk slot, silently, exit 0.
RAM7="$WORK/ram-t7"; rm -rf "$RAM7" "$POOL"
ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM7" "CODESCOUT_GATE_RAM_SLOTS=0"
t_run t7 bash "$WITH" sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-t7"; rc=$?
eq "T7 RAM_SLOTS=0: the disk slot is leased" "$(s_target t7)" "$POOL/slot-0"
eq "T7 and says nothing about a RAM tier" "$(grep -c 'RAM tier' "$WORK/out-t7")" "0"
eq "T7 and exits 0" "$rc" "0"
eq "T7 and touches nothing under the RAM pool" "$([ -e "$RAM7" ] && echo created || echo absent)" "absent"
t_run t7b bash "$WITH" --slot ram-0 sh -c ': > "$1"' _ "$WORK/ran-t7b"; rc=$?
eq "T7 --slot ram-0 with no RAM slots exits 2" "$rc" "2"
ram_cfg

echo "U. the RAM slots share one budget: a lease counts every held slot, and idle slots are emptied least-recently-leased first"
# The pool rule (spec 2, 4.d) is: need_i = max(GROWTH, COLD - size_i) summed over the own slot and
# every OTHER slot whose lock is held; admit when df free >= sum AND (mm_stat cap - used) * 22 >= sum * 10.
# `du -sm` rounds up and counts directory blocks, so the sizes the rule sees are read the same way
# here: an empty slot (a directory and a stamp) reads 1 MiB, a slot of N `fill`s (2 MiB each) reads
# 2N+1. Each sub-case sets df-cap to the measured `du -sm` of the pool plus the free space it wants,
# and keeps the free space 2 MiB or more away from the threshold.
RAM_U="$WORK/ram-u"; POOL="$WORK/pool-u"
U_KNOBS=(CODESCOUT_GATE_RAM_COLD_MB=8 CODESCOUT_GATE_RAM_GROWTH_MB=2)
ram_src /dev/zram1
ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM_U"
# u_slot <n> <fills> [<touch -d age of the stamp>] — an idle RAM slot with its lock file; no age, no stamp.
u_slot() {
    local d="$RAM_U/slot-$1" i
    rm -rf "$d"; mkdir -p "$d"; : > "$RAM_U/slot-$1.lock"
    for i in $(seq 1 "$2"); do fill "$d/f$i"; done
    if [ -n "${3:-}" ]; then printf '%s\n' /elsewhere > "$d/.codescout-lessee"; touch -d "$3" "$d/.codescout-lessee"; fi
}
u_fresh() { rm -rf "$RAM_U" "$POOL" "$WORK/df-cap"; mkdir -p "$RAM_U"; }
u_cap()   { echo $(( $(du -sm "$RAM_U" | cut -f1) + $1 )) > "$WORK/df-cap"; }
# u_run <tag> [VAR=value ...] — one with-slot.sh lease. The command records its target dir and
# whether the lock of RAM slot-0 can be taken from inside it: a lock fd that leaked from a given-up
# RAM lease would still be held there.
u_run() {
    local tag="$1"; shift
    rm -f "$WORK/state-$tag"
    t_run "$tag" "${U_KNOBS[@]}" "$@" bash "$WITH" bash -c \
        'echo "$CARGO_TARGET_DIR" > "$1"; exec {f}>"$2"; flock -n "$f" && echo free > "$3" || echo held > "$3"' \
        _ "$WORK/tdir-$tag" "$RAM_U/slot-0.lock" "$WORK/state-$tag"
}
u_state() { cat "$WORK/state-$1" 2>/dev/null; }
mkdir -p "$WORK/probe-u" "$WORK/probe-u0"; echo /x > "$WORK/probe-u0/.codescout-lessee"; fill "$WORK/probe-u/a"; fill "$WORK/probe-u/b"
eq "U0 precondition: a directory and a stamp read 1 MiB" "$(du -sm "$WORK/probe-u0" | cut -f1)" "1"
eq "U0 precondition: two fills read at least 4 MiB" "$([ "$(du -sm "$WORK/probe-u/a" "$WORK/probe-u/b" | awk '{s+=$1} END {print s}')" -ge 4 ] && echo yes || echo no)" "yes"

# An empty own slot needs max(2, 8-1) = 7 MiB.
u_fresh; u_cap 5
u_run u1; rc=$?
eq "U1 5 MiB free against a need of 7: the run goes to the disk pool" "$(s_target u1)" "$POOL/slot-0"
eq "U1 and exits 0" "$rc" "0"
eq "U1 the message names the logical budget" "$(grep -c 'logical (df) [0-9]*M free, 7M needed' "$WORK/out-u1")" "1"
eq "U1 and the RAM budget" "$(grep -c 'RAM (mm_stat) [0-9]*M left, 4M needed' "$WORK/out-u1")" "1"
eq "U1 the given-up RAM lease holds no lock fd in the disk run" "$(u_state u1)" "free"
eq "U1 the disk lease was whole: its slot carries the lessee stamp" "$(cat "$POOL/slot-0/.codescout-lessee" 2>/dev/null)" "$CHECKOUT"
u_fresh; u_cap 9
u_run u2
eq "U2 9 MiB free against a need of 7: the run stays in RAM slot-0" "$(s_target u2)" "$RAM_U/slot-0"
eq "U2 without a word about the tier" "$(grep -c 'RAM tier' "$WORK/out-u2")" "0"
eq "U2 and the lease still holds that lock inside the run" "$(u_state u2)" "held"
eq "U2 and the disk pool is untouched" "$([ -e "$POOL" ] && echo created || echo absent)" "absent"

# Idle slots are emptied oldest stamp first, and only as many as the rule needs. The own slot's
# need is 7, free is 5; each idle slot of 3 fills holds at least 6 MiB, so one is enough.
u_fresh; u_slot 1 3 "2 hours ago"; u_slot 2 3 "1 hour ago"; u_cap 5
u_run u3 CODESCOUT_GATE_RAM_SLOTS=3
eq "U3 the run is in RAM slot-0" "$(s_target u3)" "$RAM_U/slot-0"
eq "U3 the slot leased 2 h ago is emptied" "$(state "$RAM_U/slot-1")" "gone"
eq "U3 the slot leased 1 h ago is kept" "$(state "$RAM_U/slot-2/f1")" "kept"
eq "U3 the emptied slot's lock file is not unlinked" "$(state "$RAM_U/slot-1.lock")" "kept"
eq "U3 and the message names it" "$(grep -c 'emptying idle RAM slot-1' "$WORK/out-u3")" "1"
u_fresh; u_slot 1 3 "1 hour ago"; u_slot 2 3 "2 hours ago"; u_cap 5
u_run u3b CODESCOUT_GATE_RAM_SLOTS=3
eq "U3 with the ages swapped, slot-2 is the one emptied" "$(state "$RAM_U/slot-2")" "gone"
eq "U3 and slot-1 is kept" "$(state "$RAM_U/slot-1/f1")" "kept"
# A slot with no stamp counts as the oldest.
u_fresh; u_slot 1 3 "2 hours ago"; u_slot 2 3; u_cap 5
u_run u4 CODESCOUT_GATE_RAM_SLOTS=3
eq "U4 the run is in RAM slot-0" "$(s_target u4)" "$RAM_U/slot-0"
eq "U4 the slot with no stamp is emptied first" "$(state "$RAM_U/slot-2")" "gone"
eq "U4 and the one stamped 2 h ago is kept" "$(state "$RAM_U/slot-1/f1")" "kept"
# The rule is computed again after each removal: with 1 MiB free, one idle slot (at least 4 MiB) is not enough.
u_fresh; u_slot 1 2 "2 hours ago"; u_slot 2 2 "1 hour ago"; u_cap 1
u_run u4b CODESCOUT_GATE_RAM_SLOTS=3
eq "U4 two removals needed: the run is in RAM slot-0" "$(s_target u4b)" "$RAM_U/slot-0"
eq "U4 the older is emptied" "$(state "$RAM_U/slot-1")" "gone"
eq "U4 and the newer too" "$(state "$RAM_U/slot-2")" "gone"

# A held slot is counted and never emptied; with GROWTH 4 a slot of 4 fills (reads 9) needs 4 MiB,
# so own 4 + held 4 = 8, against an own need alone of 4.
u_fresh; u_slot 0 4; u_slot 1 4 "1 hour ago"; u_cap 6
if hold_lock u5 "$RAM_U/slot-1.lock"; then
    u_run u5 CODESCOUT_GATE_RAM_GROWTH_MB=4
    eq "U5 6 MiB free covers the own slot (4) but not the held one too (8): the run goes to disk" "$(s_target u5)" "$POOL/slot-0"
    eq "U5 the held slot keeps its contents" "$(state "$RAM_U/slot-1/f4")" "kept"
    eq "U5 and so does the own slot, which is not emptied to make room" "$(state "$RAM_U/slot-0/f4")" "kept"
    eq "U5 the message counts both slots (a sum of 8)" "$(grep -c 'logical (df) [0-9]*M free, 8M needed' "$WORK/out-u5")" "1"
    t_release u5 "$RAM_U/slot-1.lock"
    u_cap 10
    hold_lock u5b "$RAM_U/slot-1.lock" >/dev/null
    u_run u5b CODESCOUT_GATE_RAM_GROWTH_MB=4
    eq "U5 10 MiB free covers both: the run stays in RAM slot-0" "$(s_target u5b)" "$RAM_U/slot-0"
    t_release u5b "$RAM_U/slot-1.lock"
else
    no "U5 the lock was held" "hold_lock failed"
fi
# Only the own slot is there and it cannot fit: nothing else to empty, and its contents stay.
u_fresh; u_slot 0 2; u_cap 0
u_run u7
eq "U7 a lone own slot that does not fit: the run goes to disk" "$(s_target u7)" "$POOL/slot-0"
eq "U7 and the own slot keeps its contents" "$(state "$RAM_U/slot-0/f2")" "kept"
eq "U7 the verdict is a short budget, not an unreadable one" "$(grep -c 'logical (df) [0-9]*M free, 3M needed' "$WORK/out-u7")" "1"

# Only the RAM budget is short: free space is large (df-cap defaults to 1000000), mm_stat leaves
# cap - used = 1 MiB. The own need is 7, so the RAM side wants 7 * 10 / 22 = 4 MiB (rounded up).
printf '%s\n' "1048576 524288 8588886016 8589934592 8589934592 0 0 0 0" > "$WORK/mmstat-left1"
printf '%s\n' "1048576 524288 8585740288 8589934592 8589934592 0 0 0 0" > "$WORK/mmstat-left4"
printf '%s\n' "1048576 524288 8586788864 8589934592 8589934592 0 0 0 0" > "$WORK/mmstat-left3"
u_fresh
u_run u6 CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-left1"
eq "U6 RAM short, space plentiful: the run goes to the disk pool" "$(s_target u6)" "$POOL/slot-0"
eq "U6 the message names mm_stat and what is left" "$(grep -c 'RAM (mm_stat) 1M left, 4M needed' "$WORK/out-u6")" "1"
u_fresh
u_run u6b CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-left4"
eq "U6 4 MiB left against 4 needed: the run stays in RAM" "$(s_target u6b)" "$RAM_U/slot-0"
u_fresh
u_run u6c CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-left3"
eq "U6 3 MiB left is under it: the run goes to disk" "$(s_target u6c)" "$POOL/slot-0"

# A NAMED RAM slot is the operator's explicit choice, often to repair that very tree, so the pool
# rule never applies to it: with a budget far too short (free 0 against a need of 7 or more), the
# command still runs in exactly that slot, no idle neighbour is emptied, and the status passes through.
u_fresh; u_slot 0 3 "2 hours ago"; u_slot 1 1; u_cap 0
t_run u8 "${U_KNOBS[@]}" bash "$WITH" --slot ram-1 sh -c 'echo "$CARGO_TARGET_DIR" > "$1"; exit 7' _ "$WORK/tdir-u8"; rc=$?
eq "U8 --slot ram-1 with the budget short runs in RAM slot-1" "$(s_target u8)" "$RAM_U/slot-1"
eq "U8 and its exit status passes through" "$rc" "7"
eq "U8 the idle neighbour slot-0 keeps its contents" "$(state "$RAM_U/slot-0/f3")" "kept"
eq "U8 and its stamp" "$(state "$RAM_U/slot-0/.codescout-lessee")" "kept"
eq "U8 nothing is said about the pool budget" "$(grep -c 'pool budget' "$WORK/out-u8")" "0"
eq "U8 the lock of slot-1 is released after the command" "$(wait_free "$RAM_U/slot-1.lock" && echo free || echo held)" "free"
eq "U8 the disk pool is untouched" "$([ -e "$POOL" ] && echo created || echo absent)" "absent"
t_run u8c "${U_KNOBS[@]}" bash "$WITH" --slot ram-1 sh -c 'rm -rf "$CARGO_TARGET_DIR"/*'; rc=$?
eq "U8 the repair, rm -rf of the named tree's contents, succeeds with the budget short" "$rc" "0"
eq "U8 and removes the tree's files" "$(state "$RAM_U/slot-1/f1")" "gone"
eq "U8 while the neighbour still keeps its contents" "$(state "$RAM_U/slot-0/f3")" "kept"
# No neighbour to empty at all: the budget alone must not refuse a named slot.
u_fresh; u_slot 1 1; u_cap 0
t_run u8e "${U_KNOBS[@]}" bash "$WITH" --slot ram-1 sh -c 'echo "$CARGO_TARGET_DIR" > "$1"' _ "$WORK/tdir-u8e"; rc=$?
eq "U8 a short budget with no neighbour does not refuse a named slot" "$rc" "0"
eq "U8 and the command ran in that slot" "$(s_target u8e)" "$RAM_U/slot-1"
rm -f "$WORK/df-cap"; ram_cfg

echo "V. a RAM slot is built with line-tables-only debug info and no incremental; a disk run is not"
# The settings belong to a KEPT RAM lease only (spec 5): they are exported after the pool rule, so a
# RAM lease the rule gives up cannot carry them into the disk run. The caller's own value wins, per
# variable, even an empty one. This shell's own CARGO_* values must not decide the cases.
unset CARGO_PROFILE_DEV_DEBUG CARGO_INCREMENTAL
RAM_V="$WORK/ram-v"; POOL="$WORK/pool-v"; rm -rf "$RAM_V" "$POOL" "$WORK/df-cap"; mkdir -p "$RAM_V"
ram_src /dev/zram1
ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM_V"
# v_seen <tag> — what the command recorded: "<debug> <incremental>", `<unset>` for an unset or empty one.
v_seen() { cat "$WORK/vseen-$1" 2>/dev/null; }
# v_run <tag> [VAR=value ...] — one with-slot.sh run whose command records its target dir (tdir-<tag>)
# and the two settings (vseen-<tag>).
V_PROBE='echo "$CARGO_TARGET_DIR" > "$2"; echo "${CARGO_PROFILE_DEV_DEBUG:-<unset>} ${CARGO_INCREMENTAL:-<unset>}" > "$1"'
v_run() {
    local tag="$1"; shift
    rm -f "$WORK/vseen-$tag"
    t_run "$tag" "$@" bash "$WITH" sh -c "$V_PROBE" _ "$WORK/vseen-$tag" "$WORK/tdir-$tag"
}
v_run_slot() { # v_run_slot <tag> <slot> [VAR=value ...]
    local tag="$1" slot="$2"; shift 2
    rm -f "$WORK/vseen-$tag"
    t_run "$tag" "$@" bash "$WITH" --slot "$slot" sh -c "$V_PROBE" _ "$WORK/vseen-$tag" "$WORK/tdir-$tag"
}
v_run v1
eq "V1 a RAM lease is in a RAM slot" "$(s_target v1 2>/dev/null)" "$RAM_V/slot-0"
eq "V1 and sees line-tables-only and no incremental" "$(v_seen v1)" "line-tables-only 0"
v_run v2 CARGO_INCREMENTAL=1
eq "V2 a preset CARGO_INCREMENTAL=1 stays, the other is still exported" "$(v_seen v2)" "line-tables-only 1"
v_run v2b CARGO_PROFILE_DEV_DEBUG=full
eq "V2 a preset CARGO_PROFILE_DEV_DEBUG=full stays, the other is still exported" "$(v_seen v2b)" "full 0"
rm -f "$WORK/vempty"
t_run v2c CARGO_INCREMENTAL= CARGO_PROFILE_DEV_DEBUG= bash "$WITH" sh -c 'echo "${CARGO_PROFILE_DEV_DEBUG+set}:${CARGO_PROFILE_DEV_DEBUG} ${CARGO_INCREMENTAL+set}:${CARGO_INCREMENTAL}" > "$1"' _ "$WORK/vempty"
eq "V2 a variable present but empty counts as set and stays empty" "$(cat "$WORK/vempty" 2>/dev/null)" "set: set:"
v_run v3 CODESCOUT_GATE_RAM_CARGO_ENV=0
eq "V3 CODESCOUT_GATE_RAM_CARGO_ENV=0 exports neither" "$(v_seen v3)" "<unset> <unset>"
eq "V3 and the run was still in the RAM tier" "$(s_target v3 2>/dev/null)" "$RAM_V/slot-0"
if hold_lock v4a "$RAM_V/slot-0.lock" && hold_lock v4b "$RAM_V/slot-1.lock"; then
    v_run v4
    eq "V4 with both RAM slots held, the run is on disk" "$(s_target v4 2>/dev/null)" "$POOL/slot-0"
    eq "V4 and exports neither" "$(v_seen v4)" "<unset> <unset>"
    t_release v4a "$RAM_V/slot-0.lock"; t_release v4b "$RAM_V/slot-1.lock"
else
    no "V4 both locks were held" "hold_lock failed"
fi
# V5: the pool rule gives the RAM lease up, and the disk run must not inherit the settings.
RAM_U="$RAM_V"; U_KNOBS=(CODESCOUT_GATE_RAM_COLD_MB=8 CODESCOUT_GATE_RAM_GROWTH_MB=2)
u_fresh; u_cap 5
v_run v5 "${U_KNOBS[@]}"
eq "V5 the rule gives the RAM lease up: the run is on disk" "$(s_target v5 2>/dev/null)" "$POOL/slot-0"
eq "V5 and the disk run sees neither setting" "$(v_seen v5)" "<unset> <unset>"
u_fresh; u_cap 9
v_run v5b "${U_KNOBS[@]}"
eq "V5 the same budget with room keeps the RAM lease" "$(s_target v5b 2>/dev/null)" "$RAM_V/slot-0"
eq "V5 and that kept lease carries both settings" "$(v_seen v5b)" "line-tables-only 0"
# V6: a named RAM slot is a RAM-slot lease too.
u_fresh; mkdir -p "$RAM_V/slot-1"
v_run_slot v6 ram-1
eq "V6 --slot ram-1 leases RAM slot-1" "$(s_target v6 2>/dev/null)" "$RAM_V/slot-1"
eq "V6 and sees line-tables-only and no incremental" "$(v_seen v6)" "line-tables-only 0"
v_run_slot v6b ram-1 CODESCOUT_GATE_RAM_CARGO_ENV=0
eq "V6 with CODESCOUT_GATE_RAM_CARGO_ENV=0 a named slot gets neither" "$(v_seen v6b)" "<unset> <unset>"
# A numeric --slot is the disk pool: neither setting.
mkdir -p "$POOL/slot-0"
v_run_slot v6c 0
eq "V6 a numeric --slot is the disk pool, with neither setting" "$(v_seen v6c)" "<unset> <unset>"
rm -f "$WORK/df-cap"; ram_cfg

echo "W. gate.sh names the tier it used, prints the zram figures, and says so when a lane is red in a RAM slot"
# Spec 7. Three lines, each printed only for a RAM lease: the tier and why near the top, the figures
# after the tree size, and on any red lane one line after GATE EXITS that names the tier and says that
# a `No space left on device` or I/O error in the lane output is the tier, not the code. gate.sh does not
# read lane output to decide, so the line is printed on every red RAM run. A disk run, configured or not,
# prints none of them. The stub cargo exits with GATE_TEST_EXIT, the fixture fmt and python lanes with
# GATE_TEST_FMT_EXIT and GATE_TEST_PY_EXIT. mmstat-w is chosen to print as 10.1G / 3.5G / 20.0G.
RAM_W="$WORK/ram-w"; POOL="$WORK/pool-w"; rm -rf "$RAM_W" "$POOL"; mkdir -p "$RAM_W"
printf '%s\n' "10842275840 524288 3758096384 21474836480 700000 0 0 0 0" > "$WORK/mmstat-w"
echo 20480 > "$WORK/df-cap"
ram_src /dev/zram1; ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM_W"
W_OK=(PATH="$T_PATH" CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-high" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-w")
W_REPORT='gate.sh: tier RAM (zram1): 10.1G logical, 3.5G RAM of 20.0G cap, 20.0G free'
w_line()  { grep -n -m1 -F -- "$2" "$WORK/out-$1" | cut -d: -f1; }                  # w_line <tag> <text>: first line number
w_count() { grep -c -F -- "$2" "$WORK/out-$1"; }                                   # w_count <tag> <text>
w_red()   { grep -F 'RAM tier' "$WORK/out-$1" | grep -c -F 'No space left on device'; }
w_after() { [ "$(w_line "$1" "$2")" -gt "$(w_line "$1" "$3")" ] 2>/dev/null && echo after || echo not-after; }

run_gate w1 sid-w1 "${W_OK[@]}"; rc=$?
eq "W1 a green RAM run exits 0" "$rc" "0"
eq "W1 it ran in the RAM slot" "$(seen_all w1)" "$RAM_W/slot-0"
eq "W1 two 'gate.sh: tier RAM (' lines: one near the top, one with the figures" "$(w_count w1 'gate.sh: tier RAM (')" "2"
eq "W1 the first follows the CARGO_TARGET_DIR line directly" \
    "$(w_line w1 'gate.sh: tier RAM (')" "$(( $(w_line w1 'gate.sh: CARGO_TARGET_DIR=') + 1 ))"
eq "W1 and names the RAM pool" "$(grep -m1 -F 'gate.sh: tier RAM (' "$WORK/out-w1" | grep -c -F -- "$RAM_W")" "1"
eq "W1 the figures line is exact" "$(grep -c -Fx -- "$W_REPORT" "$WORK/out-w1")" "1"
eq "W1 and comes after the tree size" "$(w_after w1 "$W_REPORT" 'gate.sh: tree size')" "after"
eq "W1 a green run prints no red-run line" "$(w_count w1 'RAM tier')" "0"

run_gate w2 sid-w2 "${W_OK[@]}" GATE_TEST_EXIT=1; rc=$?
eq "W2 a red RAM run still exits non-zero" "$([ "$rc" -ne 0 ] && echo nonzero || echo "$rc")" "nonzero"
eq "W2 it prints one line naming the RAM tier and No space left on device" "$(w_red w2)" "1"
eq "W2 after GATE EXITS" "$(w_after w2 'No space left on device' 'GATE EXITS ->')" "after"
eq "W2 with the I/O error half" "$(grep -F 'RAM tier' "$WORK/out-w2" | grep -c -F 'I/O error')" "1"
eq "W2 and the mm_stat figures and free space" "$(grep -F 'RAM tier' "$WORK/out-w2" | grep -c -F -- '10.1G logical, 3.5G RAM of 20.0G cap, 20.0G free')" "1"
eq "W2 and the figures line is still printed" "$(grep -c -Fx -- "$W_REPORT" "$WORK/out-w2")" "1"
# Every lane in the red set counts, not only the cargo ones.
run_gate w2b sid-w2b "${W_OK[@]}" GATE_TEST_FMT_EXIT=2; rc=$?
eq "W2 a red formatter lane alone prints the line" "$(w_red w2b)" "1"
eq "W2 and exits non-zero" "$([ "$rc" -ne 0 ] && echo nonzero || echo "$rc")" "nonzero"
run_gate w2c sid-w2c "${W_OK[@]}" GATE_TEST_PY_EXIT=1; rc=$?
eq "W2 a red Python lane alone prints the line" "$(w_red w2c)" "1"
eq "W2 and exits non-zero" "$([ "$rc" -ne 0 ] && echo nonzero || echo "$rc")" "nonzero"

# A disk run prints none of it: unconfigured, and configured but not admitted.
ram_cfg; rm -rf "$POOL"
run_gate w3 sid-w3 PATH="$T_PATH" GATE_TEST_EXIT=1; rc=$?
eq "W3 a red disk run (no RAM config) names no RAM tier" "$(w_count w3 'RAM tier')" "0"
eq "W3 prints no tier line" "$(w_count w3 'tier RAM')" "0"
eq "W3 nor the No space left on device hint" "$(w_count w3 'No space left on device')" "0"
eq "W3 and exits non-zero" "$([ "$rc" -ne 0 ] && echo nonzero || echo "$rc")" "nonzero"
ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM_W"; rm -rf "$POOL"
run_gate w3b sid-w3b PATH="$T_PATH" CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-low" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-w" GATE_TEST_EXIT=1; rc=$?
eq "W3 a red run that fell back to disk says the tier was skipped" "$(w_count w3b 'RAM tier skipped')" "1"
eq "W3 and prints no tier line" "$(w_count w3b 'tier RAM')" "0"
eq "W3 nor the red-run hint" "$(w_count w3b 'No space left on device')" "0"
eq "W3 and ran on disk" "$(seen_all w3b)" "$POOL/slot-0"

# W4: the Python light lane's fixture (tests/test_python_runner.py gate_fixture) replaces slot-pool.sh with
# a one-line comment and presets CARGO_TARGET_DIR, so no ram_* function exists and GATE_TIER is unset. An
# unguarded call there prints `command not found` yet leaves the exit status alone, which is why the pytest
# lane does not see it. Run gate.sh in that shape here, green and red, and read its output.
FX="$WORK/fixture-w4"; mkdir -p "$FX/scripts"
cp "$POOLDIR/gate.sh" "$POOLDIR/session_context.py" "$POOLDIR/session-context-v1.schema.json" "$FX/scripts/"
cp "$WORK/repo/scripts/python-tests.py" "$WORK/repo/scripts/fmt-mine.sh" "$FX/scripts/"
printf '# Fixture supplies CARGO_TARGET_DIR.\n' > "$FX/scripts/slot-pool.sh"
git -C "$FX" init -q
w4_run() { # w4_run <tag> [VAR=value ...]
    local tag="$1"; shift
    scrub_ram_env
    ( cd "$FX" && env "${SCRUB[@]}" -u GATE_TIER -u RAM_DEV -u RAM_MMSTAT -u CODESCOUT_CONTEXT_FILE \
        HOME="$WORK/home" PATH="$WORK/bin:$PATH" CARGO_TARGET_DIR="$WORK/fixture-w4-target" \
        GATE_TEST_TAG="$tag" GATE_TEST_WORK="$WORK" "$@" bash scripts/gate.sh --check ) > "$WORK/out-$tag" 2>&1
}
w4_run w4; rc=$?
eq "W4 the empty slot-pool fixture, green: exits 0" "$rc" "0"
eq "W4 with no 'command not found'" "$(w_count w4 'command not found')" "0"
eq "W4 and no unbound variable" "$(w_count w4 'unbound variable')" "0"
eq "W4 and no tier line" "$(w_count w4 'tier RAM')" "0"
eq "W4 the lanes ran in the preset tree" "$(seen_all w4)" "$WORK/fixture-w4-target"
w4_run w4b GATE_TEST_EXIT=1; rc=$?
eq "W4 the same fixture, red: exits non-zero" "$([ "$rc" -ne 0 ] && echo nonzero || echo "$rc")" "nonzero"
eq "W4 with no 'command not found'" "$(w_count w4b 'command not found')" "0"
eq "W4 no unbound variable" "$(w_count w4b 'unbound variable')" "0"
eq "W4 and no RAM tier line" "$(w_count w4b 'RAM tier')" "0"

# Unreadable mm_stat or df must not break the gate: the report prints what it can and returns 0.
W_FIG="$( . "$POOLDIR/slot-pool.sh"; GATE_TIER=ram; RAM_DEV=zram9; RAM_MMSTAT="$WORK/no-such-mmstat"; GATE_POOL="$WORK/no-such-pool"
          PATH="$WORK/ramstub-none:$PATH"; ram_tier_report; echo "rc=$?" )"
eq "W5 an unreadable mm_stat and pool still report, saying what is unknown" \
    "$W_FIG" "$(printf 'gate.sh: tier RAM (zram9): ? logical, ? RAM of ? cap, ? free\nrc=0')"
rm -f "$WORK/df-cap"; ram_cfg

echo "X. scripts/gate-ram-evict.sh empties idle or pressured RAM slots, and only under the slot's lock"
# Spec 6. A machine-local timer runs this when no gate does. A slot is emptied when `flock -n` takes
# its lock AND (its .codescout-lessee stamp is at least RAM_IDLE_MIN minutes old, or it has no stamp, or
# MemAvailable is under the floor). The directory goes, the lock file stays, and nothing numbered
# RAM_SLOTS or higher is touched. Case S-W's stubs and fixtures are reused; x_run scrubs every
# CODESCOUT_GATE_RAM_* variable and XDG_CONFIG_HOME, as run_gate does, so the operator's config cannot leak.
EVICT="$POOLDIR/gate-ram-evict.sh"
RAM_X="$WORK/ram-x"; POOL="$WORK/pool-x"
X_OK=(CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-high" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-20g")
X_ARGV=()
# x_run <tag> [VAR=value ...] — one evictor run: stdout to out-<tag>, stderr to err-<tag>.
x_run() {
    local tag="$1"; shift
    scrub_ram_env
    ( cd "$WORK/repo" && exec env "${SCRUB[@]}" HOME="$WORK/home" PATH="$T_PATH" "$@" bash "$EVICT" "${X_ARGV[@]}" ) \
        > "$WORK/out-$tag" 2> "$WORK/err-$tag"
}
x_fresh() { rm -rf "$RAM_X" "$POOL"; mkdir -p "$RAM_X"; }
# x_slot <n> [<touch -d age of the stamp>] — a RAM slot with a file in it and its lock file; no age, no stamp.
x_slot() {
    local d="$RAM_X/slot-$1"
    rm -rf "$d"; mkdir -p "$d"; echo data > "$d/f"; : > "$RAM_X/slot-$1.lock"
    if [ -n "${2:-}" ]; then printf '%s\n' /elsewhere > "$d/.codescout-lessee"; touch -d "$2" "$d/.codescout-lessee"; fi
}
x_lines() { grep -c . "$WORK/out-$1"; }                       # non-empty stdout lines
x_has()   { grep -c -F -- "$2" "$WORK/out-$1"; }
x_err()   { grep -c -F -- "$2" "$WORK/err-$1"; }
ram_src /dev/zram1

# X1: nothing configured, nothing said, nothing touched.
x_fresh; ram_cfg; x_slot 0 '-3 hours'
x_run x1 "${X_OK[@]}"; rc=$?
eq "X1 no config: exit 0" "$rc" "0"
eq "X1 prints nothing on stdout" "$(x_lines x1)" "0"
eq "X1 nor on stderr" "$(wc -c < "$WORK/err-x1" | tr -d ' ')" "0"
eq "X1 and the slot is untouched" "$(state "$RAM_X/slot-0")" "kept"
# The operator's own exported knob must not reach the evictor either (the hermetic harness).
export CODESCOUT_GATE_RAM_POOL="$RAM_X"
x_run x1b "${X_OK[@]}"; rc=$?
unset CODESCOUT_GATE_RAM_POOL
eq "X1 an exported CODESCOUT_GATE_RAM_POOL does not reach x_run: exit 0" "$rc" "0"
eq "X1 and the slot is still untouched" "$(state "$RAM_X/slot-0")" "kept"

# X2: a stamp 61 min old is idle; the directory goes, the lock file stays.
x_fresh; ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM_X"; x_slot 0 '-61 minutes'
x_run x2 "${X_OK[@]}"; rc=$?
eq "X2 exit 0" "$rc" "0"
eq "X2 the idle slot's directory is gone" "$(state "$RAM_X/slot-0")" "gone"
eq "X2 its lock file is still there" "$(state "$RAM_X/slot-0.lock")" "kept"
eq "X2 one stdout line" "$(x_lines x2)" "1"
eq "X2 naming the slot" "$(x_has x2 "$RAM_X/slot-0")" "1"
eq "X2 and the reason, with the age in whole minutes" "$(x_has x2 'idle for 61 min')" "1"
eq "X2 nothing on stderr" "$(wc -c < "$WORK/err-x2" | tr -d ' ')" "0"
x_run x2b "${X_OK[@]}"; rc=$?
eq "X2 a second run finds the emptied slot (lock file only) and says nothing" "$(x_lines x2b)" "0"
eq "X2 exit 0 again" "$rc" "0"

# X6 (Review Focus 5): the next lease recreates the emptied slot and builds in it.
run_gate x6 sid-x6 PATH="$T_PATH" "${X_OK[@]}"; rc=$?
eq "X6 a gate run after the eviction exits 0" "$rc" "0"
eq "X6 its lanes all see the emptied RAM slot" "$(seen_all x6)" "$RAM_X/slot-0"
eq "X6 whose directory exists again" "$([ -d "$RAM_X/slot-0" ] && echo dir || echo none)" "dir"
eq "X6 and carries the lessee stamp" "$(cat "$RAM_X/slot-0/.codescout-lessee" 2>/dev/null)" "$CHECKOUT"
x_run x6b "${X_OK[@]}"
eq "X6 the lease's stamp makes it fresh: the evictor keeps it" "$(state "$RAM_X/slot-0")" "kept"

# X3: a 10 min stamp is not idle. Boundaries use seconds: the age is (now - mtime) / 60, rounded down.
x_fresh; x_slot 0 '-10 minutes'
x_run x3 "${X_OK[@]}"; rc=$?
eq "X3 a 10 min old stamp: kept" "$(state "$RAM_X/slot-0")" "kept"
eq "X3 with no output" "$(x_lines x3)" "0"
eq "X3 and exit 0" "$rc" "0"
x_slot 0 '-299 seconds'
x_run x3b "${X_OK[@]}" CODESCOUT_GATE_RAM_IDLE_MIN=5
eq "X3 one second under RAM_IDLE_MIN minutes: kept" "$(state "$RAM_X/slot-0")" "kept"
x_slot 0 '-300 seconds'
x_run x3c "${X_OK[@]}" CODESCOUT_GATE_RAM_IDLE_MIN=5
eq "X3 exactly RAM_IDLE_MIN minutes: emptied (due is >=, not >)" "$(state "$RAM_X/slot-0")" "gone"
eq "X3 saying so" "$(x_has x3c 'idle for 5 min')" "1"
# The age comes from the stamp, not from the lock file: a fresh lock file must not hide an old stamp.
x_slot 0 '-3 hours'; touch "$RAM_X/slot-0.lock"
x_run x3d "${X_OK[@]}"
eq "X3 a fresh lock file does not make an old stamp young" "$(state "$RAM_X/slot-0")" "gone"
x_slot 0 '-1 minute'; touch -d '-3 hours' "$RAM_X/slot-0.lock"
x_run x3e "${X_OK[@]}"
eq "X3 nor an old lock file make a fresh stamp old" "$(state "$RAM_X/slot-0")" "kept"

# X4: a held slot is never emptied, whatever its stamp or the memory says.
x_fresh; x_slot 0 '-2 hours'
if hold_lock x4 "$RAM_X/slot-0.lock"; then
    x_run x4 "${X_OK[@]}"; rc=$?
    eq "X4 a held slot with a 2 h old stamp: kept" "$(state "$RAM_X/slot-0")" "kept"
    eq "X4 its file is intact" "$(cat "$RAM_X/slot-0/f" 2>/dev/null)" "data"
    eq "X4 with no output" "$(x_lines x4)" "0"
    eq "X4 and exit 0" "$rc" "0"
    x_run x4b CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-low" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-20g"
    eq "X4 held under memory pressure: kept" "$(state "$RAM_X/slot-0")" "kept"
    rm -f "$RAM_X/slot-0/.codescout-lessee"
    x_run x4c "${X_OK[@]}"
    eq "X4 held with no stamp: kept" "$(state "$RAM_X/slot-0")" "kept"
    t_release x4 "$RAM_X/slot-0.lock"
    x_run x4d "${X_OK[@]}"
    eq "X4 released, the same slot (no stamp) is emptied" "$(state "$RAM_X/slot-0")" "gone"
else
    no "X4 the lock was held" "hold_lock failed"
fi

# X5: memory pressure empties an idle slot that is not old.
x_fresh; x_slot 0 '-10 minutes'
x_run x5 CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-low" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-20g"; rc=$?
eq "X5 a 10 min stamp with MemAvailable under the floor: emptied" "$(state "$RAM_X/slot-0")" "gone"
eq "X5 saying memory pressure" "$(x_has x5 'memory pressure')" "1"
eq "X5 naming the slot" "$(x_has x5 "$RAM_X/slot-0")" "1"
eq "X5 and exit 0" "$rc" "0"
x_slot 0 '-10 minutes'
x_run x5b CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-edge" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-20g"
eq "X5 MemAvailable exactly at the floor is not pressure: kept" "$(state "$RAM_X/slot-0")" "kept"
x_run x5c CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-edge-1" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-20g"
eq "X5 one kB under it is pressure (the floor is in whole MiB, rounded down)" "$(state "$RAM_X/slot-0")" "gone"
x_slot 0 '-10 minutes'
x_run x5d CODESCOUT_GATE_RAM_MEMINFO="$WORK/no-such-meminfo" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-20g"; rc=$?
eq "X5 an unreadable MemAvailable is not pressure: kept" "$(state "$RAM_X/slot-0")" "kept"
eq "X5 but it is said on stderr" "$(x_err x5d 'MemAvailable')" "1"
eq "X5 and the exit status is 0" "$rc" "0"
x_slot 0 '-10 minutes'
x_run x5e "${X_OK[@]}" CODESCOUT_GATE_RAM_MEM_FLOOR_MB=99999999
eq "X5 the floor is the configured one, not the default" "$(state "$RAM_X/slot-0")" "gone"

# A slot with no stamp counts as due.
x_fresh; x_slot 0; x_slot 1 '-1 minute'
x_run x7 "${X_OK[@]}"; rc=$?
eq "X7 a slot with no stamp is emptied" "$(state "$RAM_X/slot-0")" "gone"
eq "X7 saying so" "$(x_has x7 'no stamp')" "1"
eq "X7 while its fresh neighbour is kept" "$(state "$RAM_X/slot-1")" "kept"
eq "X7 one line only" "$(x_lines x7)" "1"

# Only slots 0 ... RAM_SLOTS-1 are the evictor's.
x_fresh; x_slot 0 '-3 hours'; x_slot 1 '-3 hours'; x_slot 2 '-3 hours'; x_slot 5
x_run x8 "${X_OK[@]}" CODESCOUT_GATE_RAM_SLOTS=1
eq "X8 RAM_SLOTS=1: slot-0 is emptied" "$(state "$RAM_X/slot-0")" "gone"
eq "X8 slot-1 (= RAM_SLOTS) survives although old" "$(state "$RAM_X/slot-1")" "kept"
eq "X8 so does slot-2" "$(state "$RAM_X/slot-2")" "kept"
eq "X8 and a stampless slot-5" "$(state "$RAM_X/slot-5")" "kept"
x_run x8b "${X_OK[@]}"
eq "X8 the default is two slots: slot-1 goes" "$(state "$RAM_X/slot-1")" "gone"
eq "X8 and slot-2 still stays" "$(state "$RAM_X/slot-2")" "kept"
x_slot 0 '-3 hours'
x_run x8c "${X_OK[@]}" CODESCOUT_GATE_RAM_SLOTS=0; rc=$?
eq "X8 RAM_SLOTS=0: nothing to do, exit 0" "$rc" "0"
eq "X8 nothing is emptied" "$(state "$RAM_X/slot-0")" "kept"
eq "X8 nothing is said" "$(x_lines x8c)" "0"

# A lock file is never created, and a slot without one cannot be proven idle.
x_fresh; x_slot 0 '-3 hours'; rm -f "$RAM_X/slot-0.lock"
x_run x9 "${X_OK[@]}"; rc=$?
eq "X9 a slot with no lock file is kept" "$(state "$RAM_X/slot-0")" "kept"
eq "X9 and no lock file is created for it" "$(state "$RAM_X/slot-0.lock")" "gone"
eq "X9 nor for the slot that has no directory" "$(state "$RAM_X/slot-1.lock")" "gone"
eq "X9 exit 0 and silence" "$rc$(x_lines x9)" "00"

# A slot directory that is a symlink is not followed.
x_fresh; x_slot 0 '-3 hours'; x_slot 1 '-3 hours'
rm -rf "$WORK/x-target"; mkdir -p "$WORK/x-target"; echo data > "$WORK/x-target/f"
printf '%s\n' /elsewhere > "$WORK/x-target/.codescout-lessee"; touch -d '-3 hours' "$WORK/x-target/.codescout-lessee"
rm -rf "$RAM_X/slot-0"; ln -s "$WORK/x-target" "$RAM_X/slot-0"
x_run x10 "${X_OK[@]}"; rc=$?
eq "X10 a symlinked slot directory is left in place" "$([ -L "$RAM_X/slot-0" ] && echo link || echo gone)" "link"
eq "X10 and its target keeps its contents" "$(cat "$WORK/x-target/f" 2>/dev/null)" "data"
eq "X10 while the real neighbour is emptied" "$(state "$RAM_X/slot-1")" "gone"
eq "X10 and only that one is reported" "$(x_lines x10)" "1"
eq "X10 exit 0" "$rc" "0"

# An invalid config, or a pool that is not a capped zram, is exit 2 on stderr and removes nothing.
x_fresh; x_slot 0 '-3 hours'
ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM_X" "CODESCOUT_GATE_RAM_SLOTS=two"
x_run x11 "${X_OK[@]}"; rc=$?
eq "X11 an invalid config: exit 2" "$rc" "2"
eq "X11 the message is on stderr and names the knob" "$(x_err x11 'CODESCOUT_GATE_RAM_SLOTS')" "1"
eq "X11 the message already carries the script name once" "$(x_err x11 'gate-ram-evict.sh: RAM tier skipped')" "1"
eq "X11 nothing on stdout" "$(x_lines x11)" "0"
eq "X11 and nothing is removed" "$(state "$RAM_X/slot-0")" "kept"
ram_cfg "CODESCOUT_GATE_RAM_POOL=relative/path"
x_run x11b "${X_OK[@]}"; rc=$?
eq "X11 a relative pool path: exit 2" "$rc" "2"
ram_cfg "CODESCOUT_GATE_RAM_POOL=$RAM_X"
ram_src /dev/nvme0n1p2
x_run x12 "${X_OK[@]}"; rc=$?
eq "X12 a pool that is not a zram mount: exit 2" "$rc" "2"
eq "X12 saying so on stderr" "$(x_err x12 'not a zram mount')" "1"
eq "X12 and nothing on a disk-backed path is removed" "$(state "$RAM_X/slot-0")" "kept"
ram_src /dev/zram1
x_run x12b CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-high" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-uncapped"; rc=$?
eq "X12 a zram with no RAM cap: exit 2" "$rc" "2"
eq "X12 saying so on stderr" "$(x_err x12b 'no RAM cap')" "1"
eq "X12 and nothing is removed" "$(state "$RAM_X/slot-0")" "kept"
# Low memory is a reason to evict, not to stop: the evictor never calls the admission floor check.
x_run x12c CODESCOUT_GATE_RAM_MEMINFO="$WORK/meminfo-low" CODESCOUT_GATE_RAM_MMSTAT="$WORK/mmstat-20g"; rc=$?
eq "X12 MemAvailable under the floor is not a refusal: exit 0" "$rc" "0"
eq "X12 and the slot is emptied" "$(state "$RAM_X/slot-0")" "gone"

# The script takes no arguments; one that looks like a switch must not be read as consent to anything.
x_fresh; x_slot 0 '-3 hours'
X_ARGV=(--dry-run)
x_run x13 "${X_OK[@]}"; rc=$?
X_ARGV=()
eq "X13 an argument is a usage error: exit 2" "$rc" "2"
eq "X13 with a usage line on stderr" "$(x_err x13 'usage')" "1"
eq "X13 and nothing is removed" "$(state "$RAM_X/slot-0")" "kept"

# A removal that fails is reported, not announced as done (a directory that cannot be emptied).
if [ "$(id -u)" != 0 ]; then
    x_fresh; x_slot 0 '-3 hours'; mkdir -p "$RAM_X/slot-0/ro"; echo x > "$RAM_X/slot-0/ro/f"; chmod 555 "$RAM_X/slot-0/ro"
    x_run x14 "${X_OK[@]}"; rc=$?
    chmod 755 "$RAM_X/slot-0/ro"
    eq "X14 a removal that fails exits 1" "$rc" "1"
    eq "X14 does not claim the slot was emptied" "$(x_has x14 'emptied')" "0"
    eq "X14 and says on stderr which slot" "$(x_err x14 "could not empty $RAM_X/slot-0")" "1"
fi
ram_cfg; rm -rf "$RAM_X" "$POOL" "$WORK/x-target"

echo
echo "  $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
[ "$FAIL" -eq 0 ]
