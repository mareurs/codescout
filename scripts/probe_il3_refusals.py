#!/usr/bin/env python3
"""Classify every IL-3 refusal in the usage.db corpus by what would unblock it.

MIRRORS the guard's algorithm (src/util/path_security.rs):
  1. split the command into `;` / `&&` / `||` / newline segments
  2. for each segment, split on unquoted `|` into stages
  3. block if: some downstream stage trims, AND the pre-pipe segment is not a
     buffer-ref op, AND the pre-pipe head is an unbounded LHS

The three `FIXES` below SHIPPED in 18f8f9d1 (patch-id 16a9abc0). With all three
flags on, `blocks()` models the guard as of that commit; with none, the guard
before it. The buckets are therefore a DELTA against the historical corpus, not
a to-do list -- re-point them at whatever the next proposed loosening is.

BLIND SPOTS
  * This is a RE-IMPLEMENTATION, not the guard. Its verdicts are only as good
    as `--self-test`, which exercises one case per reportable bucket. Run it
    before believing any number here.
  * usage.db is retention-swept (~30d) -- every count is a FLOOR.
  * The corpus spans guard versions, so a refusal recorded before a fix may
    already be allowed today. `stale-already-allowed` catches the ones this
    script can detect by re-running the current logic; it cannot catch a
    refusal whose fix lives outside the modelled surface.
  * Attribution asks whether a fix ALONE flips a command. One needing two
    lands in `needs-two-or-more-fixes`, not in either contributor's bucket.
  * The first version of this script did not split on `;` / `&&` -- it
    reproduced the very defect it was measuring (Finding 5 of the bug below),
    read `find ... | wc -l && cat ...` as one pipeline, and mis-credited a
    bucket by 34 rows. Segment splitting is load-bearing, not tidiness.

BASELINE 2026-08-27: 37 dbs, 703 refusals -- 82.6% correctly blocked,
14.7% already retired by earlier fixes, 2.7% (19) fixable by the three below.
BUG docs/issues/archive/2026-08-27-il3-blocks-already-collapsed-pipelines-and-its-remedy-yields-a-wrong-hash.md
"""
import glob, json, os, re, sqlite3, sys, collections, argparse

GLOBS = [
    os.path.expanduser("~/work/*/.codescout/usage.db"),
    os.path.expanduser("~/work/*/*/.codescout/usage.db"),
    os.path.expanduser("~/agents/*/.codescout/usage.db"),
]

TRIMMERS = {"tail", "head", "less", "sed", "awk", "cut", "sort", "uniq", "tr", "fmt"}
AGGREGATORS = {"wc"}
UNBOUNDED = {"cargo", "npm", "pnpm", "yarn", "python", "python3", "pytest",
             "go", "mvn", "gradle", "rg", "fd"}
GIT_LIMITERS = {"-n", "--max-count", "--show-current", "--porcelain", "--short",
                "-s", "--stat", "--name-only", "--name-status"}
BUFREF = re.compile(r"@(cmd|bg|file|tool|ack)_[A-Za-z0-9_]+")

# ---- proposed fixes -------------------------------------------------------
# F2: field selectors are 1:1 on records -- they cannot hide a record.
FIELD_SELECTORS = {"cut", "tr"}
# F3: git subcommands whose output is O(1) lines by construction.
ONELINE_GIT = {"rev-parse", "patch-id", "merge-base", "symbolic-ref",
               "describe", "hash-object"}
# F1: stages that collapse an arbitrary stream to bounded output. Nothing
# downstream of one of these can re-expand it.
COLLAPSERS = {"wc", "sha256sum", "md5sum", "sha1sum", "cksum", "b2sum"}


def split_outside_quotes(s, seps):
    out, buf, q = [], "", None
    i = 0
    while i < len(s):
        ch = s[i]
        if q:
            buf += ch
            if ch == q:
                q = None
            i += 1
            continue
        if ch in "'\"":
            q = ch
            buf += ch
            i += 1
            continue
        hit = next((sep for sep in seps if s.startswith(sep, i)), None)
        if hit:
            out.append(buf)
            buf = ""
            i += len(hit)
            continue
        buf += ch
        i += 1
    out.append(buf)
    return out


def segments(cmd):
    return split_outside_quotes(cmd, ["&&", "||", ";", "\n"])


def toks(s):
    try:
        import shlex
        return shlex.split(s)
    except ValueError:
        return s.split()


def head_of(s):
    t = toks(s)
    return os.path.basename(t[0]) if t else ""


def grep_counts(stage):
    return any(re.fullmatch(r"-\w*c", t) or t == "--count" for t in toks(stage))


def stage_trims(stage, f1=False, f2=False):
    h = head_of(stage)
    if h in AGGREGATORS:
        return False
    if h == "grep":
        return not grep_counts(stage)
    if f2 and h in FIELD_SELECTORS:
        return False
    return h in TRIMMERS


def stage_collapses(stage):
    h = head_of(stage)
    if h in COLLAPSERS:
        return True
    if h == "grep" and grep_counts(stage):
        return True
    if h == "git" and "patch-id" in toks(stage):
        return True
    return False


def git_bounded(t, f3=False):
    if f3 and len(t) > 1 and t[1] in ONELINE_GIT:
        return True
    for tok in t[1:]:
        flag = tok.split("=", 1)[0]
        if flag in GIT_LIMITERS:
            return True
        rest = tok[2:] if tok.startswith("-n") else (tok[1:] if tok.startswith("-") else None)
        if rest and rest.isdigit():
            return True
    return False


def unbounded_lhs(lhs, f3=False):
    t = toks(lhs)
    if not t:
        return False
    h = os.path.basename(t[0])
    if h in UNBOUNDED:
        return True
    if h == "grep":
        return any(x in ("-r", "-R", "--recursive") or
                   (x.startswith("-") and not x.startswith("--") and "r" in x[1:])
                   for x in t)
    if h == "find":
        return not any(x == "-maxdepth" or x.startswith("-maxdepth=") for x in t)
    if h == "git":
        return not git_bounded(t, f3=f3)
    return False


def blocks(cmd, f1=False, f2=False, f3=False):
    """Return the offending LHS, or None. Mirrors detect_il3_violation."""
    for seg in segments(cmd):
        stages = split_outside_quotes(seg, ["|"])
        if len(stages) < 2:
            continue
        pre, rest = stages[0], stages[1:]
        if f1 and any(stage_collapses(s) for s in rest):
            # A collapsing stage means the agent receives bounded output no
            # matter what follows: `git log | grep foo | wc -l` delivers exactly
            # the one number `git log | grep -c foo` does, and that is already
            # allowed. NOTE this is STRONGER than the bug file's "stop scanning
            # at the collapser", which leaves trimmers upstream of it counting --
            # and which therefore does not move the bug's own row-3 example.
            continue
        if not any(stage_trims(s, f1=f1, f2=f2) for s in rest):
            continue
        if BUFREF.search(pre):
            continue
        if not unbounded_lhs(pre, f3=f3):
            continue
        return pre.strip()
    return None


FIXES = {"F1-collapser-stops-scan": dict(f1=True),
         "F2-field-selectors": dict(f2=True),
         "F3-oneline-git": dict(f3=True)}


def classify(cmd):
    if blocks(cmd) is None:
        return "stale-already-allowed"
    flips = [name for name, kw in FIXES.items() if blocks(cmd, **kw) is None]
    if not flips:
        if blocks(cmd, f1=True, f2=True, f3=True) is None:
            return "needs-two-or-more-fixes"
        return "correctly-blocked"
    return "+".join(flips)


# ---- positive control -----------------------------------------------------
CONTROL = [
    # (command, expected bucket) -- one per state the instrument can report
    ("cargo test 2>&1 | tail -50", "correctly-blocked"),
    ("git log --oneline | head -20", "correctly-blocked"),
    ("git show abc123 | git patch-id --stable | cut -d' ' -f1",
     "F1-collapser-stops-scan+F2-field-selectors"),
    ("git rev-parse HEAD | head -1", "F3-oneline-git"),
    ("git log | grep foo | wc -l", "F1-collapser-stops-scan"),
    ("git log --grep='fix|head foo' --oneline -3", "stale-already-allowed"),
    ("ls /tmp | head -5", "stale-already-allowed"),
    ("git status --porcelain | wc -l", "stale-already-allowed"),
    ("git show abc | cut -d' ' -f1 | wc -l", "F1-collapser-stops-scan+F2-field-selectors"),
]


def self_test():
    bad = 0
    seen = collections.Counter()
    for cmd, want in CONTROL:
        got = classify(cmd)
        seen[want] += 1
        ok = got == want
        bad += not ok
        print(f"{'ok ' if ok else 'FAIL'}  {cmd}\n        want={want}\n        got ={got}")
    print(f"\ncontrol states exercised: {len(seen)}   failures: {bad}")
    return bad


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--show", default="", help="dump examples for one bucket")
    a = ap.parse_args()

    if a.self_test:
        sys.exit(1 if self_test() else 0)

    dbs = sorted({p for g in GLOBS for p in glob.glob(g)})
    rows = []
    for db in dbs:
        try:
            c = sqlite3.connect(f"file:{db}?mode=ro", uri=True)
            for inp, err in c.execute(
                "SELECT input_json, error_msg FROM tool_calls WHERE tool_name='run_command' "
                "AND (error_msg LIKE '%IL3 violation%' OR error_msg LIKE '%log-trimmer%')"
            ):
                cmd = None
                if inp:
                    try:
                        cmd = json.loads(inp).get("command")
                    except Exception:
                        pass
                if not cmd and err:
                    m = re.search(r"piped `(.+?)` to a log-trimmer", err, re.S)
                    cmd = m.group(1) if m else None
                if cmd:
                    rows.append((os.path.basename(os.path.dirname(os.path.dirname(db))), cmd))
            c.close()
        except Exception as e:
            print(f"!! {db}: {e}", file=sys.stderr)

    print(f"dbs scanned: {len(dbs)}   IL-3 refusals: {len(rows)}\n")
    if not rows:
        print("POSITIVE CONTROL FAILED: zero refusals. Do NOT read this as "
              "'the guard never fires'.")
        return

    buckets = collections.Counter()
    ex = collections.defaultdict(list)
    for repo, cmd in rows:
        one = " ".join(cmd.split())
        k = classify(one)
        buckets[k] += 1
        if len(ex[k]) < 8:
            ex[k].append((repo, one[:120]))

    total = len(rows)
    print(f"{'bucket':42} {'n':>5} {'%':>6}")
    print("-" * 56)
    for k, n in buckets.most_common():
        print(f"{k:42} {n:>5} {100*n/total:>5.1f}%")
    fp = sum(n for k, n in buckets.items()
             if k not in ("correctly-blocked", "stale-already-allowed"))
    print(f"\nfixable false positives: {fp}  ({100*fp/total:.1f}% of all refusals)")

    for k, _ in buckets.most_common():
        if a.show and a.show not in k:
            continue
        print(f"\n### {k}")
        for repo, c in ex[k]:
            print(f"    [{repo}] {c}")


if __name__ == "__main__":
    main()
