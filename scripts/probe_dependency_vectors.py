#!/usr/bin/env python3
"""Candidate mechanisms for the fixture's EMPTY fourth cell, measured.

The blast-radius fixture partitions dependents by which tool reaches them:
BOTH_FIND (both), CHASE_REQUIRED (LSP only), LEXICAL_ONLY (grep only). The
fourth cell -- reachable by NEITHER a grep for the symbol's name nor
references() -- is unfilled. A dependent there can only be found by reading
code and reasoning, which makes it the cell that tests the AGENT rather than
the TOOL.

Six candidate mechanisms, each counted as "% of files containing at least one".
That denominator is chosen because it is robust: no per-site normalisation, no
name matching, nothing to condition on.

  assembled    getattr/setattr whose NAME ARGUMENT IS COMPUTED -- f-string,
               concatenation, or a variable. Neither grep-for-the-name nor
               references() can reach the target; the name does not exist as
               a token anywhere.
  monkeypatch  `mod.attr = <callable>` -- rebinding an attribute on an
               imported module or object. The dependency runs through
               mutation, invisible to both.
  registry     a decorator called with a string literal, `@register("name")`.
               The function is symbol-referenced at its definition, so
               references() reaches the REGISTRY and stops; dispatch is by
               data.
  entrypoint   importlib.metadata entry-point discovery -- dependency declared
               in package metadata, not in code at all.
  callback     a function passed as a bare Name argument to another call
               (`run(handler)`). Blast radius flows the WRONG WAY: the defect
               calls the dependent. `call_graph(direction="callers")` misses
               it by construction, which is the direction an agent asking
               "who depends on this?" naturally searches.
  inherit      a class with a base that is not `object` -- behaviour reached
               by override rather than by call.
"""
import ast
import sys
from collections import Counter
from pathlib import Path

SKIP_DIRS = {
    ".git", ".venv", "venv", "node_modules", "__pycache__", ".mypy_cache",
    ".pytest_cache", "build", "dist", ".tox", ".eggs", "site-packages",
    "blast-radius", "hidden-info", "surface-budget", "conclude-last",
}
DYN = {"getattr", "setattr", "hasattr", "delattr"}
COMPUTED = (ast.JoinedStr, ast.BinOp, ast.Name, ast.Call, ast.Subscript,
            ast.IfExp, ast.Attribute)


def iter_py(root, cap=40000):
    n = 0
    for p in root.rglob("*.py"):
        if any(part in SKIP_DIRS for part in p.relative_to(root).parts):
            continue
        n += 1
        if n > cap:
            return
        yield p


def scan(root: Path):
    files = Counter()
    sites = Counter()
    parsed = 0

    for p in iter_py(root):
        try:
            tree = ast.parse(p.read_bytes())
        except (SyntaxError, ValueError, OSError, RecursionError):
            continue
        parsed += 1
        # Names defined in THIS file -- a callback argument only counts if the
        # thing passed is a function this file actually defines, otherwise every
        # `key=len` looks like a callback.
        local_funcs = {n.name for n in ast.walk(tree)
                       if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))}
        # Names this file IMPORTED. Monkey-patching means rebinding an attribute
        # on something that came from elsewhere; `self.foo = bar` in __init__ is
        # ordinary construction and matched 38.4% of files before this filter,
        # which measured attribute assignment rather than the mechanism.
        imported = set()
        for n in ast.walk(tree):
            if isinstance(n, ast.Import):
                for a in n.names:
                    imported.add(a.asname or a.name.split(".")[0])
            elif isinstance(n, ast.ImportFrom):
                for a in n.names:
                    imported.add(a.asname or a.name)
        seen = set()

        for node in ast.walk(tree):
            if isinstance(node, ast.Call):
                f = node.func
                fname = (f.id if isinstance(f, ast.Name)
                         else f.attr if isinstance(f, ast.Attribute) else None)
                if fname in DYN and len(node.args) >= 2 and isinstance(node.args[1], COMPUTED):
                    seen.add("assembled"); sites["assembled"] += 1
                if fname == "entry_points":
                    seen.add("entrypoint"); sites["entrypoint"] += 1
                for a in node.args:
                    if isinstance(a, ast.Name) and a.id in local_funcs:
                        seen.add("callback"); sites["callback"] += 1
                for kw in node.keywords:
                    if isinstance(kw.value, ast.Name) and kw.value.id in local_funcs:
                        seen.add("callback"); sites["callback"] += 1
            elif isinstance(node, ast.Assign):
                for t in node.targets:
                    if not (isinstance(t, ast.Attribute) and isinstance(
                            node.value, (ast.Name, ast.Lambda, ast.Attribute))):
                        continue
                    base = t.value
                    while isinstance(base, ast.Attribute):
                        base = base.value
                    if (isinstance(base, ast.Name) and base.id in imported
                            and base.id not in ("self", "cls")):
                        seen.add("monkeypatch"); sites["monkeypatch"] += 1
            elif isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                for d in node.decorator_list:
                    if isinstance(d, ast.Call) and any(
                            isinstance(a, ast.Constant) and isinstance(a.value, str)
                            for a in d.args):
                        seen.add("registry"); sites["registry"] += 1
                if isinstance(node, ast.ClassDef):
                    for b in node.bases:
                        nm = (b.id if isinstance(b, ast.Name)
                              else b.attr if isinstance(b, ast.Attribute) else None)
                        if nm and nm != "object":
                            seen.add("inherit"); sites["inherit"] += 1

        for k in seen:
            files[k] += 1

    if parsed < 20:
        return None
    return parsed, files, sites


VECTORS = ["assembled", "monkeypatch", "registry", "entrypoint", "callback", "inherit"]


def main(roots):
    rows = []
    for r in roots:
        root = Path(r)
        if not root.exists():
            continue
        got = scan(root)
        if got:
            rows.append((root.name or str(root), *got))

    hdr = f"{'corpus':<20}{'files':>7}" + "".join(f"{v:>13}" for v in VECTORS)
    print(hdr)
    print("-" * len(hdr))
    tp = 0
    tf, ts = Counter(), Counter()
    for name, parsed, files, sites in rows:
        print(f"{name[:19]:<20}{parsed:>7}" +
              "".join(f"{100.0 * files[v] / parsed:>12.1f}%" for v in VECTORS))
        tp += parsed
        tf.update(files)
        ts.update(sites)
    print("-" * len(hdr))
    print(f"{'TOTAL (% of files)':<20}{tp:>7}" +
          "".join(f"{100.0 * tf[v] / tp:>12.1f}%" for v in VECTORS))
    print(f"{'raw sites':<20}{'':>7}" + "".join(f"{ts[v]:>13}" for v in VECTORS))


BASELINE_ROOTS = [
    "/usr/lib/python3.14/site-packages", "/usr/lib/python3.14",
    "~/work/claude/serena", "~/work/claude/gpt-researcher",
    "~/work/claude/Skill_Seekers", "~/work/claude/researcher",
    "~/work/claude/mempalace", "~/work/claude/headroom",
]

if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1] in ("-h", "--help"):
        print(__doc__)
        print(f"usage: {Path(sys.argv[0]).name} ROOT [ROOT ...]\n")
        print("baseline corpora (2026-08-26, 8379 files):")
        for r in BASELINE_ROOTS:
            print("   ", r)
        sys.exit(0)
    main(sys.argv[1:])
