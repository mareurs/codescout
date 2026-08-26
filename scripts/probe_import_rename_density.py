#!/usr/bin/env python3
"""How often does real Python reach a symbol under a DIFFERENT name?

That is the blast-radius fixture's CHASE_REQUIRED mechanism: the dependent file
does not spell the symbol, because it imported it under a rename, so a grep for
the original name never finds the file and references() stops at the shim.

The fixture puts 4 of 12 dependents (33%) in that bucket. This measures the same
mechanism in real corpora.

AST-based, not grep: `import ... as ...` inside a comment, a docstring, or a
string literal is not an import, and grep cannot tell the difference.

Two rates per corpus, because they answer different questions:

  site%     of all `from X import Y` binding sites, how many rename.
            The population-level rate.
  symbol%   restricted to (module, symbol) pairs that are renamed AT LEAST ONCE
            somewhere in the corpus, what fraction of THAT symbol's import sites
            rename it. This is the fixture's question: given a symbol some
            callers alias, how many of its dependents reach it under the other
            name? A corpus-wide average hides exactly this.
"""
import ast
import sys
from collections import defaultdict
from pathlib import Path

SKIP_DIRS = {
    ".git", ".venv", "venv", "node_modules", "__pycache__", ".mypy_cache",
    ".pytest_cache", "build", "dist", ".tox", ".eggs", "site-packages",
    # The synthetic fixtures. Including them would be measuring our own
    # authoring habits and calling it evidence about the world.
    "blast-radius", "hidden-info", "surface-budget", "conclude-last",
}


def iter_py(root: Path, cap: int = 30000):
    n = 0
    for p in root.rglob("*.py"):
        # Skip relative to the root, not absolutely: passing site-packages AS a
        # root must work, while site-packages nested under a stdlib root is
        # still skipped. Checking p.parts made the two indistinguishable and
        # silently dropped the largest corpus on the machine.
        if any(part in SKIP_DIRS for part in p.relative_to(root).parts):
            continue
        n += 1
        if n > cap:
            return
        yield p


def scan(root: Path):
    parsed = 0
    from_sites = renamed_sites = 0
    mod_imports = mod_renamed = 0
    star_imports = 0
    inits = inits_reexport = 0
    per_symbol = defaultdict(lambda: [0, 0])

    for p in iter_py(root):
        try:
            tree = ast.parse(p.read_bytes())
        except (SyntaxError, ValueError, OSError, RecursionError):
            continue
        parsed += 1

        is_init = p.name == "__init__.py"
        if is_init:
            inits += 1
        init_has_reexport = False

        for node in ast.walk(tree):
            if isinstance(node, ast.ImportFrom):
                if node.level and is_init:
                    init_has_reexport = True
                mod = node.module or ("." * (node.level or 0))
                for a in node.names:
                    if a.name == "*":
                        star_imports += 1
                        continue
                    from_sites += 1
                    key = (mod, a.name)
                    per_symbol[key][0] += 1
                    if a.asname and a.asname != a.name:
                        renamed_sites += 1
                        per_symbol[key][1] += 1
            elif isinstance(node, ast.Import):
                for a in node.names:
                    mod_imports += 1
                    if a.asname and a.asname != a.name.split(".")[-1]:
                        mod_renamed += 1

        if is_init and init_has_reexport:
            inits_reexport += 1

    if parsed < 20:
        return None

    ever = {k: v for k, v in per_symbol.items() if v[1] > 0}
    return {
        "parsed": parsed,
        "from_sites": from_sites, "renamed_sites": renamed_sites,
        "mod_imports": mod_imports, "mod_renamed": mod_renamed,
        "star_imports": star_imports,
        "inits": inits, "inits_reexport": inits_reexport,
        "distinct_symbols": len(per_symbol),
        "symbols_ever_renamed": len(ever),
        "ever_sites": sum(v[0] for v in ever.values()),
        "ever_renamed": sum(v[1] for v in ever.values()),
    }


def pct(a, b):
    return 0.0 if not b else 100.0 * a / b


def main(roots):
    rows = []
    for r in roots:
        root = Path(r)
        if not root.exists():
            print(f"  (skip, missing) {r}", file=sys.stderr)
            continue
        s = scan(root)
        if s is None:
            print(f"  (skip, <20 parsed .py) {r}", file=sys.stderr)
            continue
        rows.append((root.name or str(root), s))

    hdr = (f"{'corpus':<20}{'files':>7}{'from-sites':>11}{'renamed':>9}"
           f"{'site%':>8}{'sym-ever':>9}{'their-sites':>12}{'symbol%':>9}"
           f"{'init-reexp':>13}")
    print(hdr)
    print("-" * len(hdr))
    tot = defaultdict(int)
    for name, s in rows:
        print(f"{name[:19]:<20}{s['parsed']:>7}{s['from_sites']:>11}"
              f"{s['renamed_sites']:>9}"
              f"{pct(s['renamed_sites'], s['from_sites']):>7.2f}%"
              f"{s['symbols_ever_renamed']:>9}{s['ever_sites']:>12}"
              f"{pct(s['ever_renamed'], s['ever_sites']):>8.1f}%"
              f"{s['inits_reexport']:>7}/{s['inits']:<5}")
        for k in ("parsed", "from_sites", "renamed_sites", "mod_imports",
                  "mod_renamed", "star_imports", "inits", "inits_reexport",
                  "ever_sites", "ever_renamed", "symbols_ever_renamed",
                  "distinct_symbols"):
            tot[k] += s[k]
    print("-" * len(hdr))
    print(f"{'TOTAL':<20}{tot['parsed']:>7}{tot['from_sites']:>11}"
          f"{tot['renamed_sites']:>9}"
          f"{pct(tot['renamed_sites'], tot['from_sites']):>7.2f}%"
          f"{tot['symbols_ever_renamed']:>9}{tot['ever_sites']:>12}"
          f"{pct(tot['ever_renamed'], tot['ever_sites']):>8.1f}%"
          f"{tot['inits_reexport']:>7}/{tot['inits']:<5}")
    print()
    print(f"module-level `import X as Y`: {tot['mod_renamed']}/{tot['mod_imports']} "
          f"({pct(tot['mod_renamed'], tot['mod_imports']):.2f}%)    "
          f"star-imports: {tot['star_imports']}")
    print(f"distinct (module, symbol) pairs: {tot['distinct_symbols']}; "
          f"ever renamed: {tot['symbols_ever_renamed']} "
          f"({pct(tot['symbols_ever_renamed'], tot['distinct_symbols']):.2f}%)")
    print()
    print("FIXTURE'S CLAIM, for comparison: 4 of 12 dependents (33.3%) reach the")
    print("symbol under a rename they never spell.")


# The corpora the recorded baseline was measured against. Machine-specific, and
# kept here so the figure quoted in docs/PROBES.md is reproducible rather than
# merely cited. Paths differ per host -- adjust, then re-derive before comparing.
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
        print("baseline corpora (2026-08-26, 8517 files / 72968 binding sites):")
        for r in BASELINE_ROOTS:
            print("   ", r)
        sys.exit(0)
    main(sys.argv[1:])
