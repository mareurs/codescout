"""How many run_command calls are read-only, and what would overlapping them be worth?

Classifies by the command text (every segment of a && ; | chain must be read-only), compares with the
`effect` the agent declared, and measures from usage.db what the read-only calls cost.
Needs out/all.jsonl from extract.py in the current directory.
"""
import json, re, sqlite3, datetime, collections, statistics

RO_WORDS = {"ls", "cat", "head", "tail", "wc", "grep", "egrep", "fgrep", "rg", "stat", "pwd", "date", "echo",
            "file", "du", "df", "which", "ps", "jq", "sort", "uniq", "awk", "readlink", "realpath", "test", "[",
            "cd", "diff", "cut", "tr", "true", "false", "printf", "basename", "dirname", "tree", "nl", "od",
            "xxd", "column", "comm", "cmp", "md5sum", "sha256sum", "b3sum", "id", "whoami", "uname", "env",
            "printenv", "type", "command", "hostname", "nproc", "free", "uptime", "pgrep", "lsof", "findmnt",
            "seq", "sleep", "tac", "rev", "fold", "expand", "paste", "join", "strings", "less", "more"}
GIT_RO = {"status", "log", "diff", "show", "branch", "rev-parse", "rev-list", "ls-files", "blame", "remote",
          "merge-base", "describe", "cat-file", "grep", "reflog", "shortlog", "ls-tree", "name-rev",
          "check-ignore", "diff-tree", "for-each-ref", "var", "config", "worktree", "stash", "tag",
          "patch-id", "count-objects", "fsck", "whatchanged", "cherry", "ls-remote"}
GIT_RO_ARGS_BAD = {"worktree": {"add", "remove", "prune", "move", "lock", "unlock", "repair"},
                   "stash": {"push", "pop", "drop", "apply", "clear", "save", "branch", "create", "store"},
                   "branch": {"-d", "-D", "-m", "-M", "-c", "-C", "--delete", "--move", "--copy"},
                   "tag": {"-d", "-a", "-s", "-f", "--delete"}, "config": {"--unset", "--add", "--replace-all", "set"}}
VERIFY = re.compile(r"\b(cargo\s+(test|nextest|clippy|build|check|fmt|doc|run)|gate\.sh|pytest|npm\s+(test|run)|"
                    r"with-slot\.sh|rb\.sh|fmt-mine|mutation-probe|make\b|go\s+test|mvn|gradle|rustfmt)")
BAD_REDIR = re.compile(r"(?<![0-9&])>>?(?!&|\s*/dev/null)|\btee\b|\bsed\s+(-[a-zA-Z]*i|--in-place)")
SPLIT = re.compile(r"\s*(?:&&|\|\||;|\||\n)\s*")


def seg_ro(seg):
    seg = seg.strip()
    if not seg or seg.startswith("#"):
        return True
    seg = re.sub(r"^\(+|\)+$", "", seg).strip()
    seg = re.sub(r"^(?:[A-Za-z_][A-Za-z0-9_]*=\S*\s+)+", "", seg)       # VAR=x prefix
    if re.match(r"^[A-Za-z_][A-Za-z0-9_]*=\S*$", seg):
        return True                                                      # plain assignment
    words = seg.split()
    if not words:
        return True
    w = words[0]
    if w in ("for", "while", "do", "done", "if", "then", "fi", "else", "{", "}"):
        return True
    if w == "git":
        i = 1
        while i < len(words) and words[i].startswith("-"):
            i += 2 if words[i] in ("-C", "-c") else 1
        sub = words[i] if i < len(words) else ""
        if sub not in GIT_RO:
            return False
        return not (set(words[i + 1:]) & GIT_RO_ARGS_BAD.get(sub, set()))
    if w == "find":
        return not re.search(r"-(delete|exec|execdir|ok)\b|-fprint", seg)
    if w == "sed":
        return not re.search(r"\s-[a-zA-Z]*i|--in-place", seg)
    if w == "sqlite3":
        return not re.search(r"\b(insert|update|delete|create|drop|alter|replace|vacuum|pragma\s+\w+\s*=)\b", seg, re.I)
    if w == "python3" or w == "python":
        return False
    return w in RO_WORDS


def classify(cmd):
    if VERIFY.search(cmd):
        return "VERIFY"
    if BAD_REDIR.search(cmd):
        return "MUT"
    segs = [s for s in SPLIT.split(cmd) if s.strip()]
    return "RO" if segs and all(seg_ro(s) for s in segs) else "MUT"


def ts(s):
    return datetime.datetime.fromisoformat(s.replace("Z", "+00:00")).timestamp()


db = sqlite3.connect("file:/home/marius/work/claude/codescout/.codescout/usage.db?mode=ro", uri=True, timeout=20)
U = {}
for tid, st, lat in db.execute("select tool_use_id, started_at, latency_ms from tool_calls "
                               "where tool_use_id is not null and started_at is not null and tool_name='run_command'"):
    try:
        t0 = datetime.datetime.fromisoformat(st.replace("Z", "")).replace(tzinfo=datetime.timezone.utc).timestamp()
    except Exception:
        continue
    U[tid] = (t0, lat / 1000.0)

calls = []
for l in open("out/all.jsonl"):
    r = json.loads(l)
    if r["k"] == "call" and r["name"] in ("mcp__codescout__run_command", "run_command", "Bash"):
        try:
            inp = json.loads(r["inp"])
        except Exception:
            inp = {}
        r["cmd"] = inp.get("command", "") if isinstance(inp, dict) else ""
        r["effect"] = inp.get("effect") if isinstance(inp, dict) else None
        r["bg"] = bool(inp.get("run_in_background")) if isinstance(inp, dict) else False
        r["cls"] = classify(r["cmd"])
        calls.append(r)

n = len(calls)
print("run_command / Bash calls:", n)
c = collections.Counter(r["cls"] for r in calls)
for k, v in c.most_common():
    print(f"  {k:7} {v:6} {100*v/n:5.1f}%")
for kind in ("main", "sub"):
    cc = collections.Counter(r["cls"] for r in calls if r["kind"] == kind)
    t = sum(cc.values())
    print(f"  {kind}: " + ", ".join(f"{k} {100*v/t:.1f}%" for k, v in cc.most_common()))

print("\ndeclared `effect` parameter vs text classification:")
eff = collections.Counter((r["effect"], r["cls"]) for r in calls)
for (e, k), v in sorted(eff.items(), key=lambda kv: -kv[1])[:14]:
    print(f"  effect={e!s:10} text={k:7} {v:6}")
declared_read = [r for r in calls if r["effect"] == "read"]
bad = [r for r in declared_read if r["cls"] != "RO"]
print(f"  declared read: {len(declared_read)}; text says not read-only: {len(bad)} ({100*len(bad)/max(1,len(declared_read)):.1f}%)")

print("\nlatency (usage.db) by class:")
for k in ("RO", "MUT", "VERIFY"):
    ls = [U[r["id"]][1] for r in calls if r["cls"] == k and r["id"] in U]
    if ls:
        ls.sort()
        print(f"  {k:7} n={len(ls):6} median {statistics.median(ls):6.2f}s  mean {statistics.mean(ls):6.2f}s  "
              f"p90 {ls[int(.9*len(ls))]:6.2f}s  total {sum(ls)/3600:6.1f} h")

# in-message neighbours and adjacent-round neighbours, run_command both
by_msg = collections.defaultdict(list)
for r in calls:
    by_msg[(r["f"], r["mid"])].append(r)
pairs_in = [(a, b) for v in by_msg.values() for a, b in zip(v, v[1:])]
ro_in = [(a, b) for a, b in pairs_in if a["cls"] == "RO" and b["cls"] == "RO"]
print(f"\nrun_command -> run_command inside one message: {len(pairs_in)} pairs; both read-only: {len(ro_in)}")
save = 0.0; k = 0
for a, b in ro_in:
    if a["id"] in U and b["id"] in U:
        save += min(U[a["id"]][1], U[b["id"]][1]); k += 1
print(f"  with timing: {k}; execution time that overlapping would hide (sum of the shorter call): {save/3600:.2f} h")
