"""Stage 3: which sequential rounds could have been one round?

Round  = the tool calls of ONE assistant message (same message id).
A later round R2 "depends" on an earlier call a when
  (data)    a novel token of a's RESULT (not seen anywhere earlier in the file) is in R2's INPUT, or
  (control) a is an error / a verification command / a knowledge loader (Skill, ToolSearch) whose
            outcome decides what comes next.
Effect order needs no test: inside one message the harness runs calls in order and only runs
adjacent read-only calls concurrently (measured in usage_times.py), so a write -> build chain is fine.
Output: summary on stdout, samples in samples.txt, per-pair rows in pairs.jsonl.
"""
import json, re, sys, collections, datetime, random, statistics

random.seed(7)
TOK = re.compile(r"[A-Za-z_][A-Za-z0-9_]{5,}|[\w.\-]+/[\w./\-]+|@[a-z]+_[0-9a-f]{6,}|\b[0-9a-f]{7,40}\b|\b\d{3,6}\b")
FAIL = re.compile(r'"ok":\s*false|\bBLOCKED\b|\brefused\b|\bFAILED\b|\bfailed\b|\berror\b|\bviolation\b|'
                  r'\b0 matches|\bNo matches|not found|no such|did not match|Outline only|showing 0 of', re.I)


def ts(s):
    return datetime.datetime.fromisoformat(s.replace("Z", "+00:00")).timestamp()


def short(n):
    return n.replace("mcp__codescout__", "")


def jin(rec):
    try:
        return json.loads(rec["inp"])
    except Exception:
        return {}


RO_NAMES = {"symbols", "grep", "read_file", "tree", "references", "symbol_at", "call_graph", "semantic_search",
            "get_guide", "Read", "Grep", "Glob", "WebFetch", "WebSearch", "research"}
RO_ACTIONS = {"doc": {"find", "read", "list", "get", "search", "links", "query", "stats", "check", "preview"},
              "memory": {"read", "list", "recall", "search"},
              "workspace": {"status", "list", "info"},
              "librarian": {"find", "list", "read", "stats"}}
BARRIER = {"Agent", "SendMessage", "Monitor", "ScheduleWakeup", "AskUserQuestion", "SubagentHandback",
           "StructuredOutput", "TaskStop", "ListAgents", "TaskUpdate", "approve_write"}
LOADER = {"Skill", "ToolSearch"}
VERIFY = re.compile(r"\b(cargo\s+(test|nextest|clippy|build|check|fmt|doc)|gate\.sh|pytest|npm\s+(test|run)|"
                    r"with-slot\.sh|rb\.sh|fmt-mine|mutation-probe|make\b|go\s+test|mvn|gradle)")
RO_CMD = re.compile(r"^\s*(git\s+(status|log|diff|show|branch|rev-parse|rev-list|ls-files|blame|stash\s+list|"
                    r"remote|config\s+--get|merge-base|describe|cat-file|worktree\s+list|grep|reflog)|ls|cat|head|"
                    r"tail|wc|grep|rg|find|stat|pwd|date|echo|sqlite3|file|du|df|which|ps|python3?\s+-I\s+-c|"
                    r"jq|sort|uniq|awk|sed\s+-n|readlink|realpath|test|\[)\b")
IRREV = re.compile(r"\bgit\s+(commit|push|reset|checkout|switch|rebase|merge|cherry-pick|stash\s+(push|pop|drop)|"
                   r"clean|pull|tag|branch\s+-[dD])|\brm\s|\bmv\s|--force|-f\b.*push|\bkill\b|\bdrop\s+table",
                   re.I)


def klass(name, inp):
    """RO | MUT | VERIFY | BARRIER | LOADER"""
    n = short(name)
    if n in BARRIER:
        return "BARRIER"
    if n in LOADER:
        return "LOADER"
    if n in RO_NAMES:
        return "RO"
    a = inp.get("action") if isinstance(inp, dict) else None
    if n in RO_ACTIONS:
        return "RO" if a in RO_ACTIONS[n] else "MUT"
    if n in ("run_command", "Bash"):
        c = inp.get("command", "") if isinstance(inp, dict) else ""
        if VERIFY.search(c):
            return "VERIFY"
        if RO_CMD.match(c) and not re.search(r"[^|&]>|>>|\bsed\s+-i|\btee\b", c) and not IRREV.search(c):
            return "RO"
        return "MUT"
    return "MUT"


def irreversible(name, inp):
    n = short(name)
    if n in ("run_command", "Bash"):
        return bool(IRREV.search(inp.get("command", "") if isinstance(inp, dict) else ""))
    return False


# ---- load -----------------------------------------------------------------
files = collections.defaultdict(list)     # f -> events in order
for l in open("out/all.jsonl"):
    r = json.loads(l)
    files[r["f"]].append(r)

# document frequency of tokens over calls' inputs -> stoplist of generic tokens
df = collections.Counter()
for evs in files.values():
    seen = set()
    for r in evs:
        if r["k"] == "call":
            seen.update(TOK.findall(r["inp"]))
    df.update(seen)
STOP = {t for t, c in df.items() if c >= 0.04 * len(files)}
print("files", len(files), "stop tokens", len(STOP))

rows = []                    # adjacent-round pairs
seg_stats = []               # per segment ideal vs actual
M_all = []

for f, evs in files.items():
    kind = None
    # build rounds with boundaries
    rounds = []              # list of dict(calls=[], bound_before=bool)
    pending_bound = True
    for r in evs:
        if r["k"] == "bound":
            pending_bound = True
            continue
        kind = r["kind"]
        if rounds and rounds[-1]["mid"] == r["mid"] and not pending_bound:
            rounds[-1]["calls"].append(r)
        else:
            rounds.append({"mid": r["mid"], "calls": [r], "bound": pending_bound})
        pending_bound = False
    seen = set()
    # per call novel tokens (relative to everything before its round)
    for rd in rounds:
        for c in rd["calls"]:
            c["_inp"] = jin(c)
            c["_cls"] = klass(c["name"], c["_inp"])
            c["_in_tok"] = {t for t in TOK.findall(c["inp"]) if t not in STOP}
        rd["novel"] = []
        for c in rd["calls"]:
            res_tok = {t for t in TOK.findall(c["res"] or "") if t not in STOP}
            nov = res_tok - seen - c["_in_tok"]
            rd["novel"].append(nov)
        for c in rd["calls"]:
            seen.update(TOK.findall(c["inp"]))
            seen.update(TOK.findall(c["res"] or ""))
        rd["t_issue"] = max(ts(c["tu"]) for c in rd["calls"])
        rd["t_first"] = min(ts(c["tu"]) for c in rd["calls"])
        rd["t_done"] = max(ts(c["tr"]) for c in rd["calls"] if c["tr"])

    def depends(rd_b, rd_a, ia_list):
        """does round b depend on any call of round a? returns reason or None"""
        for c, nov in zip(rd_a["calls"], rd_a["novel"]):
            if c["_cls"] in ("VERIFY", "LOADER"):
                return c["_cls"].lower()
            if c["err"] or re.search(r'"exit_code":\s*[1-9]', c["res"] or ""):
                return "error"
            if FAIL.search((c["res"] or "")[:300]):
                return "failsig"
        b_tok = set()
        for c in rd_b["calls"]:
            b_tok |= c["_in_tok"]
        for c, nov in zip(rd_a["calls"], rd_a["novel"]):
            hit = nov & b_tok
            if hit:
                return "data:" + sorted(hit, key=len, reverse=True)[0][:40]
        return None

    # adjacent pairs
    for k in range(len(rounds) - 1):
        a, b = rounds[k], rounds[k + 1]
        if b["bound"]:
            continue
        ca = {c["_cls"] for c in a["calls"]}
        cb = {c["_cls"] for c in b["calls"]}
        if "BARRIER" in ca or "BARRIER" in cb:
            rows.append({"f": f, "kind": kind, "cat": "barrier"})
            continue
        why = depends(b, a, None)
        M = b["t_first"] - a["t_done"]
        if M < 0:
            M = 0.0
        a_ro = ca == {"RO"}
        b_ro = cb == {"RO"}
        rows.append({"f": f, "kind": kind, "cat": "dep" if why else "indep", "why": why,
                     "M": M, "a_names": [short(c["name"]) for c in a["calls"]],
                     "b_names": [short(c["name"]) for c in b["calls"]],
                     "a_cls": sorted(ca), "b_cls": sorted(cb), "a_n": len(a["calls"]), "b_n": len(b["calls"]),
                     "ro_ro": a_ro and b_ro, "b_irrev": any(irreversible(c["name"], c["_inp"]) for c in b["calls"]),
                     "Da": a["t_done"] - a["t_issue"], "Db": b["t_done"] - b["t_issue"],
                     "idx": (f, k)})

    # segment leveling: ideal rounds vs actual, over runs of rounds with no boundary
    seg = []
    def flush(seg):
        if len(seg) < 2:
            return
        lvl = []
        for j, rd in enumerate(seg):
            L = 1
            if any(c["_cls"] == "BARRIER" for c in rd["calls"]):
                L = max(lvl[:j], default=0) + 1
            else:
                for i in range(max(0, j - 40), j):
                    if any(c["_cls"] == "BARRIER" for c in seg[i]["calls"]):
                        L = max(L, lvl[i] + 1)
                        continue
                    if depends(rd, seg[i], None):
                        L = max(L, lvl[i] + 1)
            lvl.append(L)
        wall = seg[-1]["t_done"] - seg[0]["t_first"]
        seg_stats.append({"f": f, "kind": kind, "actual": len(seg), "ideal": max(lvl), "wall": wall,
                          "calls": sum(len(r["calls"]) for r in seg)})
    for rd in rounds:
        if rd["bound"] and seg:
            flush(seg)
            seg = []
        seg.append(rd)
    flush(seg)

json.dump(rows, open("pairs.json", "w"))
json.dump(seg_stats, open("segs.json", "w"))
print("pairs", len(rows), "segments", len(seg_stats))
