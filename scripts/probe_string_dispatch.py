#!/usr/bin/env python3
"""LEXICAL_ONLY, measured on names where bare-name matching is actually valid.

v1 (string_dispatch.py) matched symbol references on the bare name, which made
`get` collect every `.get(` in the corpus: 495,054 references across 1,209 names,
~409 per name. The resulting 0.62% string share was an artifact of the
denominator, not a fact about the code.

The fix is to restrict to names where a bare-name match means what it says, and
the restriction is not arbitrary -- it is the shape of the fixture's own symbol.
`duty_multiplier` is long, underscored, and defined exactly once. A name meeting
those three conditions cannot collide with an unrelated `.attr` access.

  DISTINCTIVE  len >= 8, snake_case with at least one underscore, and defined
               EXACTLY ONCE across the whole corpus.

Unit is the FILE, not the site, because that is what the fixture counts: "4 of 12
dependent FILES hold the name as data." A file that reaches the callable only by
string is a LEXICAL_ONLY dependent; one that also names it symbolically is not.

Reported per corpus:
  reach-by-str   distinctive names reached by a string at least once
  str-only       ... of those, how many are reached by string in files that
                 NEVER name them symbolically -- the true LEXICAL_ONLY population
  file-share     over names reached by string, the share of their dependent files
                 that reach them ONLY by string
"""
import ast
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

SKIP_DIRS = {
    ".git", ".venv", "venv", "node_modules", "__pycache__", ".mypy_cache",
    ".pytest_cache", "build", "dist", ".tox", ".eggs", "site-packages",
    "blast-radius", "hidden-info", "surface-budget", "conclude-last",
}
DYN2 = {"getattr", "hasattr", "setattr", "delattr"}
DYN1 = {"attrgetter", "methodcaller"}
NAMESPACE = {"globals", "vars", "locals"}
CONFIG_EXT = {".toml", ".yaml", ".yml", ".json", ".ini", ".cfg", ".conf"}
IDENT = re.compile(r"^[a-z_][a-z0-9_]*$")


def distinctive(n, def_counts):
    return (len(n) >= 8 and "_" in n and IDENT.match(n)
            and def_counts.get(n) == 1)


def iter_files(root, suffixes, cap=40000):
    n = 0
    for p in root.rglob("*"):
        if p.suffix not in suffixes or not p.is_file():
            continue
        if any(part in SKIP_DIRS for part in p.relative_to(root).parts):
            continue
        n += 1
        if n > cap:
            return
        yield p


def const_str(node):
    return node.value if isinstance(node, ast.Constant) and isinstance(node.value, str) else None


def scan(root: Path):
    def_counts = Counter()
    trees = []
    for p in iter_files(root, {".py"}):
        try:
            tree = ast.parse(p.read_bytes())
        except (SyntaxError, ValueError, OSError, RecursionError):
            continue
        trees.append((p, tree))
        for node in ast.walk(tree):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                def_counts[node.name] += 1
    if len(trees) < 20:
        return None

    str_files = defaultdict(set)   # name -> files reaching it by string
    ref_files = defaultdict(set)   # name -> files naming it symbolically
    str_sites = Counter()
    files_with_dispatch = 0

    for p, tree in trees:
        local_str, local_ref = set(), set()
        for node in ast.walk(tree):
            if isinstance(node, ast.Call):
                f = node.func
                fname = (f.id if isinstance(f, ast.Name)
                         else f.attr if isinstance(f, ast.Attribute) else None)
                s = None
                if fname in DYN2 and len(node.args) >= 2:
                    s = const_str(node.args[1])
                elif fname in DYN1 and node.args:
                    s = const_str(node.args[0])
                if s:
                    local_str.add(s)
                    str_sites[s] += 1
                if isinstance(f, ast.Subscript):
                    s2 = const_str(f.slice)
                    if s2:
                        local_str.add(s2)
                        str_sites[s2] += 1
            elif isinstance(node, ast.Subscript):
                v = node.value
                if (isinstance(v, ast.Call) and isinstance(v.func, ast.Name)
                        and v.func.id in NAMESPACE):
                    s = const_str(node.slice)
                    if s:
                        local_str.add(s)
                        str_sites[s] += 1
            elif isinstance(node, ast.Name) and isinstance(node.ctx, ast.Load):
                local_ref.add(node.id)
            elif isinstance(node, ast.Attribute) and isinstance(node.ctx, ast.Load):
                local_ref.add(node.attr)
        if local_str:
            files_with_dispatch += 1
        for n in local_str:
            str_files[n].add(p)
        for n in local_ref:
            ref_files[n].add(p)

    # Config half: a distinctive callable named as a bare string in a config file.
    cfg_names = set()
    cfg_files = cfg_hits = 0
    for p in iter_files(root, CONFIG_EXT, cap=8000):
        try:
            text = p.read_text(errors="ignore")
        except OSError:
            continue
        cfg_files += 1
        # Word-boundary, not quote-delimited: YAML and TOML both carry bare
        # keys, and requiring quotes silently dropped them. `distinctive()` is
        # the filter that keeps this from matching prose -- len >= 8, snake_case,
        # defined exactly once in the corpus.
        for m in re.finditer(r'\b([A-Za-z_][A-Za-z0-9_]*)\b', text):
            tok = m.group(1)
            if distinctive(tok, def_counts):
                cfg_hits += 1
                cfg_names.add(tok)

    hit = [n for n in str_files if distinctive(n, def_counts)]
    dep_total = str_only_files = 0
    str_only_names = 0
    for n in hit:
        sf, rf = str_files[n], ref_files.get(n, set())
        only = sf - rf
        dep_total += len(sf | rf)
        str_only_files += len(only)
        if only:
            str_only_names += 1

    return {
        "parsed": len(trees), "distinct_defined": len(def_counts),
        "distinctive_defined": sum(1 for n in def_counts if distinctive(n, def_counts)),
        "hit": len(hit), "str_only_names": str_only_names,
        "dep_total": dep_total, "str_only_files": str_only_files,
        "str_sites": sum(str_sites[n] for n in hit),
        "files_with_dispatch": files_with_dispatch,
        "cfg_files": cfg_files, "cfg_hits": cfg_hits, "cfg_names": len(cfg_names),
    }


def pct(a, b):
    return 0.0 if not b else 100.0 * a / b


def main(roots):
    rows = []
    for r in roots:
        root = Path(r)
        if not root.exists():
            continue
        s = scan(root)
        if s:
            rows.append((root.name or str(root), s))

    hdr = (f"{'corpus':<20}{'files':>7}{'distinctive':>12}{'reach-by-str':>13}"
           f"{'str-only':>10}{'dep-files':>11}{'str-only-f':>11}{'file-share':>11}"
           f"{'cfg-names':>10}")
    print(hdr)
    print("-" * len(hdr))
    tot = Counter()
    for name, s in rows:
        print(f"{name[:19]:<20}{s['parsed']:>7}{s['distinctive_defined']:>12}"
              f"{s['hit']:>13}{s['str_only_names']:>10}{s['dep_total']:>11}"
              f"{s['str_only_files']:>11}"
              f"{pct(s['str_only_files'], s['dep_total']):>10.1f}%"
              f"{s['cfg_names']:>10}")
        for k, v in s.items():
            tot[k] += v
    print("-" * len(hdr))
    print(f"{'TOTAL':<20}{tot['parsed']:>7}{tot['distinctive_defined']:>12}"
          f"{tot['hit']:>13}{tot['str_only_names']:>10}{tot['dep_total']:>11}"
          f"{tot['str_only_files']:>11}"
          f"{pct(tot['str_only_files'], tot['dep_total']):>10.1f}%"
          f"{tot['cfg_names']:>10}")
    print()
    print(f"files containing ANY string dispatch: {tot['files_with_dispatch']}"
          f"/{tot['parsed']} ({pct(tot['files_with_dispatch'], tot['parsed']):.1f}%)")
    print(f"config files scanned: {tot['cfg_files']}; distinctive callables named "
          f"as bare strings in them: {tot['cfg_hits']} hits / {tot['cfg_names']} names")
    print()
    print("FIXTURE: 4 of 12 dependents (33.3%) hold the name as data.")


BASELINE_ROOTS = [
    "/usr/lib/python3.14/site-packages", "/usr/lib/python3.14",
    "~/work/claude/serena", "~/work/claude/gpt-researcher",
    "~/work/claude/Skill_Seekers", "~/work/claude/researcher",
    "~/work/claude/mempalace", "~/work/claude/headroom",
    "~/work/claude/prompt-engineering", "~/work/claude/topictracker",
]

if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help"):
        print(__doc__)
        print(f"usage: {Path(sys.argv[0]).name} ROOT [ROOT ...]\n")
        print("baseline corpora (2026-08-26, 8517 files / 62135 distinctive callables):")
        for r in BASELINE_ROOTS:
            print("   ", r)
        sys.exit(0)
    main(sys.argv[1:])
