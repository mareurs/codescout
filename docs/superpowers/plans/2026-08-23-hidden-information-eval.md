---
id: '89c2984ca7c074a0'
kind: plan
status: draft
title: Hidden-information eval — implementation plan
tags:
- eval
- prompt-tdd
- measurement
topic: hidden information eval implementation plan fixture checker arms
---

# Hidden-Information Eval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Measure whether an agent with codescout finds hidden information better than the same agent with only native tools, on identical synthetic code.

**Architecture:** A seeded generator emits a ~100-file fixture with 12 ground-truth sites in three difficulty bands plus 8 decoys. Two arms run the same change-impact task — one codescout-only, one native-only — and a checker scores recall, precision and F1 by exact set arithmetic against a `## FINDINGS` block. A pilot at N=2 gates a 2×2 baseline at N=8 across Sonnet and Opus.

**Tech Stack:** Python 3.13, prompt-tdd harness (`scripts/run_arms.py`, `scripts/score_arm.py`), pytest, Claude Code CLI 2.1.241.

**Spec:** `docs/superpowers/specs/2026-08-23-hidden-information-eval-design.md` (codescout repo, artifact `556cc34167321863`)

**Repos:** Tasks 1–6 all land in `/home/marius/work/claude/prompt-engineering`. Committing directly to `master` there is correct — no protected-branch rule, unlike codescout.

## Global Constraints

- **12 ground-truth sites**, exactly 4 per band (A literal / B one-hop / C vocabulary-drift). **8 decoys.**
- **Recall denominator is always 12.** A run that names 6 correct sites scores recall 0.5 regardless of how many lines it emitted.
- **Answer contract:** a `## FINDINGS` section, one `path:symbol` per line, nothing else in that section.
- **Path canonical form:** repo-relative, forward slashes, no leading `./`. The checker normalises exactly three deviations — backslash separators, a leading `./`, an absolute path under the fixture root — and classifies anything else as unparseable.
- **Symbol match is exact and case-sensitive.** File-right/symbol-wrong is NOT a hit, but is reported as `recall_file`.
- **Turn cap 60**, identical in both arms. A run exceeding it is reported with its turn count, never dropped.
- **Malformed output is its own class** (`no-findings-block`), never zero recall.
- **`hidden-cs` runs must not use native file tools.** Enforced via `--disallowedTools` (Task 1) *and* detected via a checker veto (`native-tool-used`, Task 3). Both, always — a passthrough that silently stops working looks exactly like compliance.
- **Never `--tools ""`.** Whether it leaves MCP tools intact is unmeasured; the explicit deny-list does not depend on the built-in/MCP distinction.
- **`ground_truth.json` is emitted OUTSIDE the fixture tarball.** The agent must never be able to read the answer key.
- **Pilot gates (all four must hold to proceed to Task 6):** neither arm mean F1 < 0.15 or > 0.90; both arms ≥ 0.9 F1 on positive control; |F1(base) − F1(null)| ≤ 0.10; and |F1(cs) − F1(native)| ≥ 0.10 **or** band-C recall differs by ≥ 0.25.
- **Cost:** pilot ~$2; phase 2 $40–100. Report actual per-run cost from the pilot before starting Task 6.

## File Structure

| File | Responsibility |
|---|---|
| `src/prompt_tdd/adapters/claude_code.py` | *(modify)* `SessionConfig.disallowed_tools` + `_tool_flags()` helper, called from both arg builders |
| `src/prompt_tdd/cli.py` | *(modify)* read `disallowed_tools` from YAML into `SessionConfig` |
| `tests/prompt_tdd/test_tool_restriction.py` | *(create)* unit tests for the flag mapping |
| `scenarios/hidden-info/gen_fixture.py` | *(create)* seeded fixture emitter + `ground_truth.json` |
| `scenarios/hidden-info/test_fixture.py` | *(create)* determinism + ground-truth-consistency tests |
| `scenarios/hidden-info/check_hidden.py` | *(create)* the scorer — parse, normalise, set arithmetic, vetoes |
| `scenarios/hidden-info/test_check_hidden.py` | *(create)* adversarial checker tests |
| `scenarios/hidden-info/gen.py` | *(create)* emits the arm YAMLs, following `scenarios/surface-budget/gen.py` |
| `scenarios/hidden-info/{main,poscontrol,noise}/…` | *(generated)* arm dirs — never hand-edit |

`surface_lib.py` is **reused as-is** from `scenarios/surface-budget/` via a relative import; it already supplies `collect_facts`, `split_facts`, `log_run` and `run`, including `tool_names` and cache-inclusive `prompt` tokens.

---

### Task 1: Tool-restriction passthrough (F-7 prerequisite)

**Files:**
- Modify: `src/prompt_tdd/adapters/claude_code.py:49-58` (SessionConfig), `:172-176` and `:359-363` (arg builders)
- Modify: `src/prompt_tdd/cli.py:68-73` (loader)
- Test: `tests/prompt_tdd/test_tool_restriction.py`

**Interfaces:**
- Produces: `SessionConfig.disallowed_tools: str` (space-separated tool names, `""` = unrestricted) and module-level `_tool_flags(session: SessionConfig) -> list[str]`. Task 4's YAMLs set `claude_code.session.disallowed_tools`.

Why a helper rather than inlining at both sites: the two arg builders would otherwise duplicate the logic, and a pure config→args function is unit-testable without mocking subprocess.

- [ ] **Step 1: Write the failing test**

```python
# tests/prompt_tdd/test_tool_restriction.py
from prompt_tdd.adapters.claude_code import SessionConfig, _tool_flags


def test_no_flags_when_unrestricted():
    assert _tool_flags(SessionConfig()) == []


def test_splits_whitespace_separated_names():
    session = SessionConfig(disallowed_tools="Read Grep Glob")
    assert _tool_flags(session) == ["--disallowedTools", "Read", "Grep", "Glob"]


def test_single_tool():
    session = SessionConfig(disallowed_tools="Bash")
    assert _tool_flags(session) == ["--disallowedTools", "Bash"]


def test_extra_whitespace_is_ignored():
    session = SessionConfig(disallowed_tools="  Read   Grep  ")
    assert _tool_flags(session) == ["--disallowedTools", "Read", "Grep"]
```

- [ ] **Step 2: Run test to verify it fails**

Run: `.venv/bin/python -m pytest tests/prompt_tdd/test_tool_restriction.py -v`
Expected: FAIL — `ImportError: cannot import name '_tool_flags'`

- [ ] **Step 3: Add the field to SessionConfig**

Append to the `SessionConfig` dataclass in `src/prompt_tdd/adapters/claude_code.py` (after `config_dir`):

```python
    # Space-separated tool names passed to the CLI as --disallowedTools, e.g.
    # "Read Grep Glob Bash Edit Write". Empty = unrestricted (prior behaviour).
    # Needed so an MCP-only arm is actually MCP-only: every arm in
    # scenarios/surface-budget/ restricts tools by prompt instruction alone, which
    # a run can ignore without recall, precision, F1 or tokens noticing (F-7).
    # A str, not a list, so the dataclass needs no mutable default.
    disallowed_tools: str = ""
```

- [ ] **Step 4: Add the helper**

Add immediately after the `SessionConfig` class:

```python
def _tool_flags(session: SessionConfig) -> list[str]:
    """CLI args for per-session tool restriction; empty when unrestricted.

    Split out rather than inlined because both `claude -p` invocations need it,
    and a pure config-to-args mapping is testable without mocking subprocess.
    """
    if not session.disallowed_tools:
        return []
    return ["--disallowedTools", *session.disallowed_tools.split()]
```

- [ ] **Step 5: Run test to verify it passes**

Run: `.venv/bin/python -m pytest tests/prompt_tdd/test_tool_restriction.py -v`
Expected: 4 passed

- [ ] **Step 6: Wire it into both arg builders**

In `_evaluate_handler` (~line 172), immediately after the `if self._session.model:` block:

```python
            cmd.extend(_tool_flags(self._session))
```

Do the same in `_run_history_turns` (~line 359), after its own `--model` handling. **Both sites** — the grep for `--permission-mode` returns exactly two builder call sites, and missing one leaves multi-turn scenarios unrestricted while single-turn ones look correct.

- [ ] **Step 7: Wire it into the YAML loader**

In `src/prompt_tdd/cli.py`, inside the `SessionConfig(...)` construction (~line 68):

```python
        disallowed_tools=session_raw.get("disallowed_tools", ""),
```

- [ ] **Step 8: Verify nothing regressed**

Run: `.venv/bin/python -m pytest tests/ -q`
Expected: 398 passed, 7 deselected (394 prior + 4 new)

- [ ] **Step 9: Prove it reaches the CLI**

This is the step that distinguishes "the flag is constructed" from "the flag works". Run a one-off against the real CLI:

```bash
cd /home/marius/work/claude/prompt-engineering
.venv/bin/python -c "
from prompt_tdd.adapters.claude_code import SessionConfig, _tool_flags
print(_tool_flags(SessionConfig(disallowed_tools='Read Grep Glob Bash Edit Write')))
"
claude --disallowedTools Read Grep Glob Bash Edit Write -p "Read the file ./README.md and tell me its first line." 2>&1
```

Expected: the CLI either refuses the tool or reports it cannot read the file. If it reads the file anyway, **stop** — the flag does not do what the help says, and Task 4's arm definition is unsound. Record that as an F-N entry before proceeding.

- [ ] **Step 10: Commit**

```bash
git add src/prompt_tdd/adapters/claude_code.py src/prompt_tdd/cli.py tests/prompt_tdd/test_tool_restriction.py
git commit -F- <<'EOF'
feat(adapters): per-session tool restriction via --disallowedTools

Closes the F-7 gap. SessionConfig had only permission_mode, which governs
prompting rather than tool availability, so an arm could not actually deny
native tools — every existing arm restricts them by prompt instruction, which
a run can ignore without any metric noticing.

Threads through the same four sites permission_mode already uses. Split into
_tool_flags() so both arg builders share one implementation and the mapping is
testable without mocking subprocess.
EOF
```

---

### Task 2: Fixture generator

**Files:**
- Create: `scenarios/hidden-info/gen_fixture.py`
- Create: `scenarios/hidden-info/test_fixture.py`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `build(out_dir: Path, seed: int = 20260823) -> dict` — writes the fixture tree under `out_dir` and returns the ground-truth dict. Also runnable as `python gen_fixture.py <out_dir> [--seed N]`, writing `ground_truth.json` **as a sibling of** `out_dir`, never inside it. Task 3 reads that file; Task 4 tars `out_dir`.

The 20 planted sites are declared as data, not generated randomly — randomness controls only the surrounding filler, so ground truth is exact and reviewable.

- [ ] **Step 1: Write the failing test**

```python
# scenarios/hidden-info/test_fixture.py
import json, subprocess, sys
from pathlib import Path

import gen_fixture


def test_ground_truth_has_twelve_sites_four_per_band(tmp_path):
    gt = gen_fixture.build(tmp_path / "repo")
    assert len(gt["sites"]) == 12
    for band in ("A", "B", "C"):
        assert sum(s["band"] == band for s in gt["sites"]) == 4, band


def test_eight_decoys(tmp_path):
    gt = gen_fixture.build(tmp_path / "repo")
    assert len(gt["decoys"]) == 8


def test_every_site_exists_and_contains_its_symbol(tmp_path):
    root = tmp_path / "repo"
    gt = gen_fixture.build(root)
    for site in gt["sites"]:
        f = root / site["path"]
        assert f.is_file(), site["path"]
        assert site["symbol"].split(".")[-1] in f.read_text(), site


def test_decoys_never_read_the_rate(tmp_path):
    """A decoy that genuinely reads the rate is a ground-truth bug, not a decoy."""
    root = tmp_path / "repo"
    gt = gen_fixture.build(root)
    site_paths = {s["path"] for s in gt["sites"]}
    for decoy in gt["decoys"]:
        assert decoy["path"] not in site_paths, decoy


def test_band_a_greppable_band_c_is_not(tmp_path):
    """Band A must be findable by a literal grep; band C must not be."""
    root = tmp_path / "repo"
    gt = gen_fixture.build(root)
    for site in gt["sites"]:
        text = (root / site["path"]).read_text()
        if site["band"] == "A":
            assert "TAX_RATE" in text, f"band A site not greppable: {site}"
        if site["band"] == "C":
            assert "TAX_RATE" not in text, f"band C site is greppable: {site}"


def test_deterministic(tmp_path):
    a, b = tmp_path / "a", tmp_path / "b"
    gen_fixture.build(a); gen_fixture.build(b)
    fa = sorted(p.relative_to(a) for p in a.rglob("*") if p.is_file())
    fb = sorted(p.relative_to(b) for p in b.rglob("*") if p.is_file())
    assert fa == fb
    for rel in fa:
        assert (a / rel).read_bytes() == (b / rel).read_bytes(), rel


def test_answer_key_is_not_inside_the_fixture(tmp_path):
    root = tmp_path / "repo"
    gen_fixture.build(root)
    assert not list(root.rglob("ground_truth.json"))
    assert not any("apply_levy" in p.read_text() and p.suffix == ".json"
                   for p in root.rglob("*.json"))


def test_scale_is_large_enough_to_price_brute_force(tmp_path):
    root = tmp_path / "repo"
    gen_fixture.build(root)
    py = list(root.rglob("*.py"))
    assert len(py) >= 90, len(py)
    assert sum(len(p.read_text().splitlines()) for p in py) >= 12_000
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd scenarios/hidden-info && ../../.venv/bin/python -m pytest test_fixture.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'gen_fixture'`

- [ ] **Step 3: Write the generator**

```python
#!/usr/bin/env python3
"""Seeded generator for the hidden-information fixture.

The 20 planted sites (12 truth + 8 decoys) are DATA, not random output —
randomness controls only surrounding filler, so ground truth is exact and a
human can review it. Band A uses the literal TAX_RATE identifier; band B
reaches the value through a caller's parameter; band C spells the concept
differently and never contains the literal.
"""
import argparse, json, random
from pathlib import Path

SEED = 20260823
RATE_CONST = "TAX_RATE"

# (band, path, symbol, why) — the answer key, authored by hand.
SITES = [
    ("A", "src/billing/invoice.py",        "compute_total",        f"reads {RATE_CONST} directly"),
    ("A", "src/billing/quote.py",          "Quote.subtotal",       f"reads {RATE_CONST} directly"),
    ("A", "src/config/rates.py",           "TAX_RATE",             "defines the constant"),
    ("A", "src/checkout/cart.py",          "Cart.grand_total",     f"reads {RATE_CONST} directly"),
    ("B", "src/billing/statement.py",      "render_line",          "receives rate via caller parameter"),
    ("B", "src/orders/fulfilment.py",      "settle",               "receives rate from compute_total"),
    ("B", "src/refunds/processor.py",      "reverse_charge",       "receives rate through two hops"),
    ("B", "src/exports/ledger_writer.py",  "write_row",            "receives rate via **kwargs from settle"),
    ("C", "src/intl/customs.py",           "apply_levy",           "same concept, named levy"),
    ("C", "src/pricing/adjust.py",         "surcharge_pct",        "same concept, named surcharge"),
    ("C", "src/intl/duties.py",            "duty_multiplier",      "same concept, named duty"),
    ("C", "src/pricing/basis.py",          "_rate_bp",             "same concept, basis points"),
]

DECOYS = [
    ("src/reports/tax_report.py",      "TaxReport.render",   "formats an already-computed total"),
    ("src/reports/summary.py",         "monthly_summary",    "the word tax appears only in a comment"),
    ("tests/fixtures/rates.py",        "SAMPLE_RATE",        "test fixture constant, never imported by src"),
    ("src/legacy/old_pricing.py",      "legacy_total",       "defined, never called"),
    ("src/intl/currency.py",           "convert",            "rate means exchange rate here"),
    ("src/billing/discount.py",        "apply_discount",     "percentage maths, unrelated to tax"),
    ("docs/pricing.md",                "pricing-overview",   "prose mentioning tax, no code"),
    ("src/analytics/metrics.py",       "rate_of_change",     "rate as derivative, not a tax rate"),
]


def build(out_dir: Path, seed: int = SEED) -> dict:
    rng = random.Random(seed)
    out_dir = Path(out_dir)
    _emit_planted(out_dir)
    _emit_filler(out_dir, rng)
    return {
        "seed": seed,
        "task_id": "tax-rate-change-impact",
        "sites": [{"id": f"{b}{i}", "band": b, "path": p, "symbol": s, "why": w}
                  for i, (b, p, s, w) in enumerate(SITES, 1)],
        "decoys": [{"path": p, "symbol": s, "why": w} for p, s, w in DECOYS],
    }


def _write(root: Path, rel: str, text: str) -> None:
    f = root / rel
    f.parent.mkdir(parents=True, exist_ok=True)
    f.write_text(text, encoding="utf-8")


def _emit_planted(root: Path) -> None:
    _write(root, "src/config/rates.py",
           f'"""Rate configuration."""\n\n{RATE_CONST} = 0.0825\nSHIPPING_FLAT = 4.99\n')
    _write(root, "src/billing/invoice.py",
           "from src.config.rates import TAX_RATE\n\n\n"
           "def compute_total(subtotal: float) -> float:\n"
           "    return round(subtotal * (1 + TAX_RATE), 2)\n")
    _write(root, "src/billing/quote.py",
           "from src.config.rates import TAX_RATE\n\n\n"
           "class Quote:\n"
           "    def __init__(self, lines): self.lines = lines\n\n"
           "    def subtotal(self) -> float:\n"
           "        base = sum(self.lines)\n"
           "        return base * (1 + TAX_RATE)\n")
    _write(root, "src/checkout/cart.py",
           "from src.config.rates import TAX_RATE\n\n\n"
           "class Cart:\n"
           "    def __init__(self, items): self.items = items\n\n"
           "    def grand_total(self) -> float:\n"
           "        return sum(i.price for i in self.items) * (1 + TAX_RATE)\n")
    # Band B — the rate arrives as a parameter; the literal never appears.
    _write(root, "src/billing/statement.py",
           "def render_line(amount: float, rate: float) -> str:\n"
           "    return f'{amount:.2f} @ {rate:.4f}'\n")
    _write(root, "src/orders/fulfilment.py",
           "from src.billing.invoice import compute_total\n"
           "from src.billing.statement import render_line\n"
           "from src.config.rates import TAX_RATE\n\n\n"
           "def settle(order) -> str:\n"
           "    total = compute_total(order.subtotal)\n"
           "    return render_line(total, TAX_RATE)\n")
    _write(root, "src/refunds/processor.py",
           "from src.orders.fulfilment import settle\n\n\n"
           "def reverse_charge(order, rate: float) -> float:\n"
           "    settle(order)\n"
           "    return order.subtotal * rate * -1\n")
    _write(root, "src/exports/ledger_writer.py",
           "def write_row(**kwargs) -> str:\n"
           "    rate = kwargs.get('rate', 0.0)\n"
           "    return f\"{kwargs.get('ref')},{rate}\"\n")
    # Band C — same concept, different vocabulary, no TAX_RATE literal anywhere.
    _write(root, "src/intl/customs.py",
           "LEVY = 0.0825\n\n\n"
           "def apply_levy(amount: float) -> float:\n"
           "    return amount * (1 + LEVY)\n")
    _write(root, "src/pricing/adjust.py",
           "def surcharge_pct() -> float:\n"
           "    return 8.25 / 100\n")
    _write(root, "src/intl/duties.py",
           "def duty_multiplier() -> float:\n"
           "    return 1.0825\n")
    _write(root, "src/pricing/basis.py",
           "def _rate_bp() -> int:\n"
           "    return 825  # basis points\n")
    # Decoys.
    _write(root, "src/reports/tax_report.py",
           "class TaxReport:\n"
           "    def __init__(self, total): self.total = total\n\n"
           "    def render(self) -> str:\n"
           "        return f'Tax report: {self.total:.2f}'\n")
    _write(root, "src/reports/summary.py",
           "def monthly_summary(rows):\n"
           "    # totals here already include tax\n"
           "    return sum(rows)\n")
    _write(root, "tests/fixtures/rates.py", "SAMPLE_RATE = 0.0825\n")
    _write(root, "src/legacy/old_pricing.py",
           "def legacy_total(subtotal):\n"
           "    return subtotal * 1.0825  # superseded; no callers\n")
    _write(root, "src/intl/currency.py",
           "def convert(amount: float, rate: float) -> float:\n"
           "    return amount * rate  # FX rate\n")
    _write(root, "src/billing/discount.py",
           "def apply_discount(amount: float, pct: float) -> float:\n"
           "    return amount * (1 - pct / 100)\n")
    _write(root, "docs/pricing.md",
           "# Pricing overview\n\nPrices shown include tax where applicable.\n")
    _write(root, "src/analytics/metrics.py",
           "def rate_of_change(a: float, b: float) -> float:\n"
           "    return (b - a) / a if a else 0.0\n")


def _emit_filler(root: Path, rng: random.Random) -> None:
    """Plausible surrounding modules, so the planted sites are not the only code."""
    domains = ["orders", "catalog", "shipping", "accounts", "notifications",
               "inventory", "audit", "search", "webhooks", "scheduling"]
    verbs = ["build", "resolve", "collect", "normalise", "validate",
             "dispatch", "reconcile", "expand", "flatten", "summarise"]
    nouns = ["record", "batch", "entry", "payload", "bundle",
             "manifest", "segment", "window", "cursor", "token"]
    for d in domains:
        for n in range(9):
            fns = []
            for k in range(rng.randint(6, 12)):
                v, nn = rng.choice(verbs), rng.choice(nouns)
                fns.append(
                    f"def {v}_{nn}_{k}(items):\n"
                    f"    \"\"\"{v.capitalize()} the {nn} set for {d}.\"\"\"\n"
                    f"    out = []\n"
                    f"    for it in items:\n"
                    f"        if it is None:\n"
                    f"            continue\n"
                    f"        out.append(it)\n"
                    f"    return out\n")
            _write(root, f"src/{d}/mod_{n}.py", "\n\n".join(fns) + "\n")
    _write(root, "README.md", "# Orders platform\n\nInternal billing and fulfilment.\n")
    _write(root, "pyproject.toml", '[project]\nname = "orders-platform"\nversion = "0.1.0"\n')


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("out_dir", type=Path)
    ap.add_argument("--seed", type=int, default=SEED)
    args = ap.parse_args()
    gt = build(args.out_dir, args.seed)
    key = args.out_dir.parent / "ground_truth.json"
    key.write_text(json.dumps(gt, indent=2), encoding="utf-8")
    print(f"fixture -> {args.out_dir}\nanswer key -> {key}  (OUTSIDE the fixture)")


if __name__ == "__main__":
    main()
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd scenarios/hidden-info && ../../.venv/bin/python -m pytest test_fixture.py -v`
Expected: 8 passed. If `test_scale_is_large_enough_to_price_brute_force` fails, raise the filler loop's range — do not lower the assertion.

- [ ] **Step 5: Eyeball the output**

```bash
cd scenarios/hidden-info
../../.venv/bin/python gen_fixture.py /tmp/hidden-fixture/repo
find /tmp/hidden-fixture/repo -name '*.py' | wc -l
```

Read three filler files. If they read as obviously templated, vary the bodies further — the spec requires a reviewer be unable to tell it was generated, or band results will not generalise.

- [ ] **Step 6: Commit**

```bash
git add scenarios/hidden-info/gen_fixture.py scenarios/hidden-info/test_fixture.py
git commit -m "feat(hidden-info): seeded fixture generator with hand-authored ground truth"
```

---

### Task 3: The checker

**Files:**
- Create: `scenarios/hidden-info/check_hidden.py`
- Create: `scenarios/hidden-info/test_check_hidden.py`

**Interfaces:**
- Consumes: `ground_truth.json` from Task 2 (path from `$HIDDEN_GROUND_TRUTH`); `surface_lib` from `scenarios/surface-budget/`.
- Produces: an executable checker for Task 4's `assertions.custom.script`, and `score(text, facts, gt) -> dict` carrying `recall`, `precision`, `f1`, `recall_band_a|b|c`, `recall_file`, `n_found`, `n_unparseable`.

- [ ] **Step 1: Write the failing tests**

```python
# scenarios/hidden-info/test_check_hidden.py
import check_hidden as ch

GT = {
    "sites": [
        {"id": "A1", "band": "A", "path": "src/billing/invoice.py", "symbol": "compute_total"},
        {"id": "A2", "band": "A", "path": "src/billing/quote.py",   "symbol": "Quote.subtotal"},
        {"id": "B1", "band": "B", "path": "src/orders/fulfilment.py", "symbol": "settle"},
        {"id": "C1", "band": "C", "path": "src/intl/customs.py",    "symbol": "apply_levy"},
    ],
    "decoys": [{"path": "src/reports/tax_report.py", "symbol": "TaxReport.render"}],
}
MCP = {"tool_names": ["mcp__codescout__symbols"]}


def _answer(*lines):
    return "Here is what I found.\n\n## FINDINGS\n" + "\n".join(lines) + "\n"


def test_perfect_answer_scores_one():
    r = ch.score(_answer("src/billing/invoice.py:compute_total",
                         "src/billing/quote.py:Quote.subtotal",
                         "src/orders/fulfilment.py:settle",
                         "src/intl/customs.py:apply_levy"), MCP, GT)
    assert r["recall"] == 1.0 and r["precision"] == 1.0 and r["f1"] == 1.0


def test_listing_everything_craters_precision():
    """Recall alone is gameable; precision is what stops it."""
    lines = [f"{s['path']}:{s['symbol']}" for s in GT["sites"]]
    lines += [f"src/filler/mod_{i}.py:fn_{i}" for i in range(40)]
    r = ch.score(_answer(*lines), MCP, GT)
    assert r["recall"] == 1.0
    assert r["precision"] < 0.15
    assert r["f1"] < 0.3


def test_right_file_wrong_symbol_is_not_a_hit():
    r = ch.score(_answer("src/billing/invoice.py:wrong_name"), MCP, GT)
    assert r["recall"] == 0.0
    assert r["recall_file"] == 0.25


def test_decoy_costs_precision():
    r = ch.score(_answer("src/billing/invoice.py:compute_total",
                         "src/reports/tax_report.py:TaxReport.render"), MCP, GT)
    assert r["precision"] == 0.5


def test_missing_findings_block_is_its_own_class():
    assert ch.predicate("I could not determine the answer.", MCP) == "no-findings-block"


def test_empty_findings_block_is_its_own_class():
    assert ch.predicate("## FINDINGS\n", MCP) == "empty-findings-block"


def test_native_tool_use_vetoes_the_run():
    facts = {"tool_names": ["mcp__codescout__symbols", "Read"]}
    verdict = ch.predicate(_answer("src/billing/invoice.py:compute_total"), facts)
    assert verdict == "native-tool-used"


def test_path_normalisation():
    for variant in ("./src/billing/invoice.py:compute_total",
                    "src\\billing\\invoice.py:compute_total"):
        r = ch.score(_answer(variant), MCP, GT)
        assert r["recall"] == 0.25, variant


def test_unparseable_line_is_counted_not_guessed():
    r = ch.score(_answer("I think it might be in the billing module"), MCP, GT)
    assert r["n_unparseable"] == 1
    assert r["n_found"] == 0


def test_band_recall_is_reported_separately():
    r = ch.score(_answer("src/intl/customs.py:apply_levy"), MCP, GT)
    assert r["recall_band_c"] == 1.0
    assert r["recall_band_a"] == 0.0
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd scenarios/hidden-info && ../../.venv/bin/python -m pytest test_check_hidden.py -v`
Expected: FAIL — `ModuleNotFoundError: No module named 'check_hidden'`

- [ ] **Step 3: Write the checker**

```python
#!/usr/bin/env python3
"""hidden-info — score a change-impact sweep by exact set arithmetic.

Scoring is set arithmetic, not substring matching, because every fuzzy checker
written in this repo has failed the same way: a predicate that demanded the wrong
token, and a null control that scored a denial as a pass. A constrained answer
shape makes the checker mechanical.
"""
import json, os, re, sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "surface-budget"))
import surface_lib  # noqa: E402

FINDINGS_RE = re.compile(r"^##\s+FINDINGS\s*$", re.M)
LINE_RE = re.compile(r"^[-*\s]*(?P<path>[\w./\\-]+\.(?:py|md|toml)):(?P<symbol>[\w.]+)\s*$")
NATIVE_TOOLS = {"Read", "Grep", "Glob", "Bash", "Edit", "Write", "NotebookEdit"}


def _ground_truth() -> dict:
    return json.loads(Path(os.environ["HIDDEN_GROUND_TRUTH"]).read_text())


def _norm(path: str) -> str:
    p = path.replace("\\", "/")
    if p.startswith("./"):
        p = p[2:]
    marker = "/repo/"
    if p.startswith("/") and marker in p:
        p = p.split(marker, 1)[1]
    return p


def parse_findings(text: str):
    """Return (entries, unparseable_count), or (None, 0) when no block exists."""
    m = FINDINGS_RE.search(text)
    if not m:
        return None, 0
    entries, bad = set(), 0
    for raw in text[m.end():].splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        hit = LINE_RE.match(line)
        if hit:
            entries.add(f"{_norm(hit.group('path'))}:{hit.group('symbol')}")
        else:
            bad += 1
    return entries, bad


def score(text: str, facts: dict, gt: dict) -> dict:
    found, bad = parse_findings(text)
    found = found or set()
    truth = {f"{s['path']}:{s['symbol']}" for s in gt["sites"]}
    truth_files = {s["path"] for s in gt["sites"]}
    hits = found & truth
    recall = len(hits) / len(truth) if truth else 0.0
    precision = len(hits) / len(found) if found else 0.0
    f1 = (2 * recall * precision / (recall + precision)) if (recall + precision) else 0.0
    out = {
        "recall": round(recall, 4), "precision": round(precision, 4), "f1": round(f1, 4),
        "recall_file": round(len({p.split(":")[0] for p in found} & truth_files)
                             / len(truth_files), 4) if truth_files else 0.0,
        "n_found": len(found), "n_hits": len(hits), "n_unparseable": bad,
    }
    for band in ("a", "b", "c"):
        band_truth = {f"{s['path']}:{s['symbol']}"
                      for s in gt["sites"] if s["band"].lower() == band}
        out[f"recall_band_{band}"] = round(len(found & band_truth) / len(band_truth), 4) \
            if band_truth else 0.0
    return out


def predicate(text: str, facts: dict) -> str:
    # Veto first: an arm that used a native file tool is not the arm under test,
    # and its score would be a measurement of the wrong configuration. Kept even
    # after --disallowedTools lands, because a passthrough that silently stops
    # working looks exactly like compliance.
    used = set(facts.get("tool_names", [])) & NATIVE_TOOLS
    if used and os.environ.get("HIDDEN_ARM") == "cs":
        return "native-tool-used"

    entries, _ = parse_findings(text)
    if entries is None:
        return "no-findings-block"
    if not entries:
        return "empty-findings-block"

    gt = _ground_truth()
    m = score(text, facts, gt)
    facts.update(m)                      # ride along into the run log
    return "" if m["f1"] > 0 else "zero-f1"


if __name__ == "__main__":
    surface_lib.run(predicate)
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd scenarios/hidden-info && ../../.venv/bin/python -m pytest test_check_hidden.py -v`
Expected: 10 passed

- [ ] **Step 5: Set the exec bit and prove it**

```bash
chmod +x scenarios/hidden-info/check_hidden.py
ls -l scenarios/hidden-info/check_hidden.py
```

Expected: mode shows `-rwxr-xr-x`. **Paste that line into the task report.** A checker without its exec bit reports a clean 0/N that is byte-identical to a genuine floor — this repo has already lost a result to it once.

- [ ] **Step 6: Commit**

```bash
git add scenarios/hidden-info/check_hidden.py scenarios/hidden-info/test_check_hidden.py
git commit -m "feat(hidden-info): set-arithmetic scorer with band recall and native-tool veto"
```

---

### Task 4: Arms

**Files:**
- Create: `scenarios/hidden-info/gen.py`
- Generated: `scenarios/hidden-info/{main,poscontrol,noise}/**/scenario.yaml` + `prompt_tdd.yaml`

**Interfaces:**
- Consumes: Task 1's `disallowed_tools`, Task 2's fixture tarball, Task 3's checker.
- Produces: runnable arms — `main/hidden-cs`, `main/hidden-native`, `poscontrol/pos-cs`, `poscontrol/pos-native`, `noise/noise-base`, `noise/noise-null`.

Follow `scenarios/surface-budget/gen.py` exactly for structure: a Python module that writes the YAMLs, with the generated files carrying `Generated by gen.py — edit that, not this file.` in their description.

- [ ] **Step 1: Build the fixture tarball**

```bash
cd scenarios/hidden-info
../../.venv/bin/python gen_fixture.py /tmp/hidden-build/repo
tar czf /home/marius/work/claude/eval-bins/hidden-fixture.tar.gz -C /tmp/hidden-build/repo .
cp /tmp/hidden-build/ground_truth.json /home/marius/work/claude/eval-bins/hidden-ground-truth.json
tar tzf /home/marius/work/claude/eval-bins/hidden-fixture.tar.gz | grep -c ground_truth || echo "answer key absent — correct"
```

Expected: the final line prints `answer key absent — correct`.

- [ ] **Step 2: Write gen.py**

The task prompt, identical in every main arm:

```
There is a codebase in this directory. We are changing the sales tax rate.

Find every place that would need to change, and every place that reads the
current value. Include indirect readers — code that receives the rate from a
caller counts.

End your reply with a section exactly like this, and put nothing else in it:

## FINDINGS
path/to/file.py:symbol_name
path/to/other.py:Class.method
```

Per-arm session config:

| Arm | `mcp_config` | `disallowed_tools` |
|---|---|---|
| `hidden-cs` | codescout-base | `Read Grep Glob Bash Edit Write NotebookEdit` |
| `hidden-native` | *(omitted)* | `""` |
| `pos-cs` / `pos-native` | as above | as above; prompt additionally names `src/config/rates.py` |
| `noise-base` / `noise-null` | codescout-base / codescout-null | as `hidden-cs` |

Every scenario sets `runs: 2` for the pilot (Task 5 raises it), `mode: trace`, `pass_threshold: 0.0`, and:

```yaml
setup:
  commands:
    - tar xzf /home/marius/work/claude/eval-bins/hidden-fixture.tar.gz -C .
  env:
    HIDDEN_GROUND_TRUTH: /home/marius/work/claude/eval-bins/hidden-ground-truth.json
    HIDDEN_ARM: cs        # or "native"
assertions:
  custom:
    - script: /home/marius/work/claude/prompt-engineering/scenarios/hidden-info/check_hidden.py
```

**If `setup.env` is not a supported key** — check `src/prompt_tdd/types.py` before assuming — export both variables in the `prompt_tdd.yaml` wrapper or from the shell that invokes `run_arms.py`, and note which you used in the task report.

- [ ] **Step 3: Generate and validate**

```bash
cd scenarios/hidden-info && ../../.venv/bin/python gen.py
find . -name scenario.yaml | wc -l          # expect 6
../../.venv/bin/python -c "
import yaml, pathlib
for p in pathlib.Path('.').rglob('scenario.yaml'):
    yaml.safe_load(p.read_text()); print('ok', p)
"
```

- [ ] **Step 4: Prove the denial actually holds**

```bash
cd /home/marius/work/claude/prompt-engineering
PROMPT_TDD_RUN_LOG=/tmp/hidden-denial.log .venv/bin/python scripts/run_arms.py \
  --config scenarios/hidden-info/main/prompt_tdd.yaml --all
grep '^TOOLS' /tmp/prompt-tdd-arms-*/hidden-cs.log
```

Expected: the `hidden-cs` TOOLS line contains **no** native tool name. If it does, the veto should have fired — confirm the verdict reads `FAIL(native-tool-used)` rather than a score. Either outcome is informative; a native tool present *with* a passing score means the veto is broken and must be fixed before Task 5.

- [ ] **Step 5: Commit**

```bash
git add scenarios/hidden-info/gen.py scenarios/hidden-info/main scenarios/hidden-info/poscontrol scenarios/hidden-info/noise
git commit -m "feat(hidden-info): six arms — main pair, positive control, noise floor"
```

---

### Task 5: Pilot and gate

**Files:** none created. Produces a gate decision.

- [ ] **Step 1: Run all six arms at N=2 on Sonnet**

```bash
cd /home/marius/work/claude/prompt-engineering
for suite in main poscontrol noise; do
  .venv/bin/python scripts/run_arms.py --config scenarios/hidden-info/$suite/prompt_tdd.yaml --all
done
```

- [ ] **Step 2: Re-score with the real denominator**

```bash
for log in /tmp/prompt-tdd-arms-*/*.log; do
  .venv/bin/python scripts/score_arm.py scenarios/hidden-info/check_hidden.py "$log" --expect 2
done
```

Expected: no `⚠ MISSING` warning. If one appears, runs died before logging — fix that before reading any number.

- [ ] **Step 3: Evaluate the four gates**

Record each explicitly in the task report as pass or fail with its value:

1. Neither arm mean F1 < 0.15 or > 0.90.
2. `pos-cs` and `pos-native` both ≥ 0.9 F1.
3. `|F1(noise-base) − F1(noise-null)| ≤ 0.10`.
4. `|F1(hidden-cs) − F1(hidden-native)| ≥ 0.10` **or** band-C recall differs by ≥ 0.25.

- [ ] **Step 4: Report cost, and stop**

```bash
grep -h '^COST_USD' /tmp/prompt-tdd-arms-*/*.log
```

**Do not start Task 6 on your own.** Report the four gate outcomes, the per-run cost, and the extrapolated phase-2 cost, and wait. Gate 1 or 4 failing means tune the fixture (hop count, drift aggressiveness, scale) and re-pilot — not that the finding is null. Gate 2 or 3 failing means something is broken.

---

### Task 6: Baseline (only after Task 5's gates pass and the user approves)

- [ ] **Step 1: Pre-register**

Append the hypothesis, arms, N, metrics and thresholds to `docs/trackers/prompt-hamsa-audit-log.md` **in the codescout repo** before the first run. A count is not believed here without a pre-registration.

- [ ] **Step 2: Raise N and add the Opus cells**

Set `runs: 8` in the four main-arm scenarios; duplicate `main/` as `main-opus/` with `model: opus`. Re-run `gen.py` rather than hand-editing.

- [ ] **Step 3: Run the 2×2 plus controls**

```bash
for suite in main main-opus poscontrol noise; do
  .venv/bin/python scripts/run_arms.py --config scenarios/hidden-info/$suite/prompt_tdd.yaml --all
done
```

- [ ] **Step 4: Score with `--expect 8` and report**

Report **medians and per-run values**, never means alone — token counts are long-tailed and one brute-force run moves a mean without moving the median. Read the `distinct` column before believing any tie. State per-band recall per cell; that is what localises the value to a capability.

- [ ] **Step 5: Write findings to the session log**

Append a W-N or F-N to `docs/trackers/prompt-surface-measurement-session-log.md` (codescout, artifact `db65023089245832`) via `artifact(action="append_entry", id_prefix="W"|"F", anchor_heading="## Template for new entries", …)`. Include the counterfactual and the numbers.

---

## Self-Review

**Spec coverage.** § 1 fixture → Task 2. § 2 arms and both controls → Tasks 1 and 4. § 3 task and output contract → Tasks 3 and 4. § 4 metrics → Task 3 (`score()` emits every field in the spec's table; `surface_lib` supplies tokens, turns, tool calls, guidechars). § 5 phases → Tasks 5 and 6. § 6 cost → Task 5 step 4. § 7 risks — each mitigation has a step: fixture realism → Task 2 step 5; brute force priced → Task 3's precision tests; format non-compliance → `no-findings-block`; contamination → Task 2's seeded generator; N too small → `--expect`; rigged-against-grep → Task 2's band A/C test; spend → Task 5 step 4. § 8 librarian eval is explicitly out of scope. § 9's turn-cap question is unresolved in the spec and stays unresolved here — it is carried as a prompt instruction plus a reported metric.

**Placeholders.** None. Task 4 step 2 flags one genuine unknown (`setup.env` support) with a named file to check and a concrete fallback, rather than leaving it blank.

**Type consistency.** `_tool_flags` / `disallowed_tools` consistent across Tasks 1 and 4. `build(out_dir, seed) -> dict` and the `sites` / `decoys` / `band` / `path` / `symbol` key names are consistent across Tasks 2, 3 and 4. `score()` and `predicate()` names match their test files. `HIDDEN_GROUND_TRUTH` and `HIDDEN_ARM` are spelled identically in Tasks 3 and 4.

**Gap found and fixed during review:** Task 3's `predicate` originally vetoed native-tool use in every arm, which would have failed every `hidden-native` run by construction — the same inverted-predicate defect as F-7's sibling in `check_routing.py`. Now gated on `HIDDEN_ARM == "cs"`, and Task 4 sets that variable per arm.

