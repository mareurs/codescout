#!/usr/bin/env bash
# Discriminators for scripts/pre-commit-dead-artifact-ids.sh.
#
# WHY A STUB BINARY. The check's own question needs a codescout build AND a populated
# machine-wide catalog, neither of which a CI runner has — that is the whole reason the
# guard lives at commit time (ADR 2026-09-14). So the cases below drive it through
# `CODESCOUT_BIN` pointing at a shell stub that prints a canned audit report. That tests
# the half this script owns — which findings refuse, which are exempt, and what happens
# when it cannot ask at all — without pretending to test the resolver, which has its own
# tests in src/librarian/tools/audit_doc_refs/.
#
# The end-to-end path (a real binary, a real catalog, a real dead id) was measured by
# hand on 2026-09-14 and is recorded in the commit; it is deliberately NOT here, because
# a case that silently skips itself for a missing toolchain is a case that reports
# success while testing nothing.
set -uo pipefail

SELF_ROOT=$(cd "$(dirname "$0")/.." && pwd)
CHECK="$SELF_ROOT/scripts/pre-commit-dead-artifact-ids.sh"
PASS=0; FAIL=0

has() { # has <label> <haystack> <needle>
    case "$2" in
        *"$3"*) PASS=$((PASS + 1)) ;;
        *) FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  expected to find: $3"; echo "  in: $2" ;;
    esac
}
eq() { # eq <label> <actual> <expected>
    if [ "$2" = "$3" ]; then PASS=$((PASS + 1))
    else FAIL=$((FAIL + 1)); echo "FAIL: $1"; echo "  actual:   $2"; echo "  expected: $3"; fi
}

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT

# A stub standing in for `codescout`. Answers `--help` (the liveness probe) and writes
# whatever report the case asked for to stdout.
mkstub() { # mkstub <report-json> -> echoes the stub path
    local p="$WORK/stub$RANDOM.sh"
    { echo '#!/usr/bin/env bash'
      echo 'case "${1:-}" in --help) exit 0 ;; esac'
      printf 'cat <<%s\n' "'REPORT'"
      cat "$1"
      echo "REPORT"
    } > "$p"
    chmod +x "$p"; echo "$p"
}

newrepo() { # newrepo <md-body> -> echoes repo path, with the file STAGED and clean
    local p="$WORK/repo$RANDOM"
    mkdir -p "$p"
    git -C "$p" init -q
    git -C "$p" config user.email t@t; git -C "$p" config user.name t
    printf '%s' "$1" > "$p/doc.md"
    git -C "$p" add doc.md >/dev/null
    echo "$p"
}

run() { # run <repo> <stub> -> sets OUT and RC
    OUT=$(cd "$1" && CODESCOUT_BIN="$2" bash "$CHECK" 2>&1); RC=$?
}

report() { # report <ref_kind> <verdict> <severity> -> echoes a report file path
    local f="$WORK/rep$RANDOM.json"
    cat > "$f" <<JSON
{"findings":[{"md_file":"doc.md","md_line":3,"raw_ref":"0000000000000000",
  "ref_kind":"$1","verdict":"$2","severity":"$3"}]}
JSON
    echo "$f"
}

# --- 1. The refusal: a high artifact_missing in a staged file ----------------
R=$(newrepo '# D

Filed as `0000000000000000`.
')
run "$R" "$(mkstub "$(report artifact_id artifact_missing high)")"
eq   "1 a dead id refuses the commit" "$RC" "1"
has  "1 names the file and line" "$OUT" "doc.md:3"
has  "1 names the id" "$OUT" "0000000000000000"
# The remedy half. A guard's PREDICATE is what suites test and its REMEDY TEXT is what
# nobody asserts on, so the two branches are pinned by shape: both must survive, because
# picking the wrong one destroys the record rather than merely failing.
has  "1 offers the repoint remedy" "$OUT" "repoint it"
has  "1 offers the fence remedy for a deliberate mention" "$OUT" "FENCE it"
has  "1 names the stale-catalog case, which is neither" "$OUT" "your catalog is stale"

# --- 2-4. MUST NOT refuse: the audit's own exemptions are not re-derived here -
# The script filters on three fields. Each case flips exactly one, so a filter that
# silently widened to "any finding" fails here rather than on the live corpus.
R=$(newrepo '# D

`0000000000000000`
')
run "$R" "$(mkstub "$(report artifact_id artifact_missing med)")"
eq   "2 a MED finding does not refuse (archive/fence/issues drops land here)" "$RC" "0"

run "$R" "$(mkstub "$(report file_path missing high)")"
eq   "3 a high finding of another REF KIND does not refuse" "$RC" "0"

run "$R" "$(mkstub "$(report artifact_id resolved high)")"
eq   "4 a resolved artifact id does not refuse" "$RC" "0"

# --- 5. Staged bytes differ from the worktree: decline, do not guess ---------
# Reading the worktree while claiming to check the index is a filed defect here. This
# check cannot read the index (materializing to a temp path re-keys every file's own id
# and would flag every self-citation), so it declines instead — and PASSES, because
# being unable to ask is not grounds to refuse.
R=$(newrepo '# D

clean
')
printf '# D\n\nnow dirty `0000000000000000`\n' > "$R/doc.md"
run "$R" "$(mkstub "$(report artifact_id artifact_missing high)")"
eq   "5 divergent staged/worktree bytes pass open" "$RC" "0"
has  "5 and say so, naming the file" "$OUT" "doc.md"
has  "5 wording declines rather than claiming" "$OUT" "declines rather than guessing"

# --- 6. No binary: degrade open, loudly -------------------------------------
# A guard whose ABSENCE blocks every commit is worse than the hole it closes.
# PATH is narrowed to the system dirs rather than emptied — `codescout` installs to
# ~/.cargo/bin, so this hides it while leaving `bash` and `git` reachable. Emptying it
# breaks the harness instead of the subject, which this case did on its first run.
R=$(newrepo '# D

`0000000000000000`
')
OUT=$(cd "$R" && PATH=/usr/bin:/bin CODESCOUT_BIN=/nonexistent/codescout bash "$CHECK" 2>&1); RC=$?
eq   "6 a missing binary passes open" "$RC" "0"
has  "6 and names what it looked for" "$OUT" "CODESCOUT_BIN"
has  "6 says why passing open is right" "$OUT" "cannot ask its question"

# --- 7. Unreadable report: degrade open -------------------------------------
BADSTUB="$WORK/bad.sh"
printf '#!/usr/bin/env bash\ncase "${1:-}" in --help) exit 0 ;; esac\necho "not json"\n' > "$BADSTUB"
chmod +x "$BADSTUB"
R=$(newrepo '# D

`0000000000000000`
')
run "$R" "$BADSTUB"
eq   "7 an unparseable report passes open" "$RC" "0"
has  "7 and names it" "$OUT" "unreadable audit report"

# --- 8. MUST-SURVIVE: no staged markdown is silent, not noisy ---------------
# A hook that prints on every commit to say nothing is how `--no-verify` gets learned.
R=$(newrepo '# D

clean
')
git -C "$R" commit -qm init
run "$R" "$(mkstub "$(report artifact_id artifact_missing high)")"
eq   "8 nothing staged exits 0" "$RC" "0"
eq   "8 and prints nothing at all" "$(printf '%s' "$OUT" | wc -c)" "0"

echo
echo "dead-artifact-ids: $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ] || exit 1
