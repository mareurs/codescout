---
id: a4d7380dc4643d88
kind: bug
status: investigating
title: Architecture probe omits longer population cycles
owners:
- marius
tags:
- architecture
- measurement
- cluster/selector-narrower-than-its-population
opened: 2026-09-13
owner: marius
related: []
severity: medium
---

# BUG: Architecture probe omits longer population cycles

## Summary

The architecture probe publishes `population_cycles` but detects only reciprocal pairs. A directed cycle spanning intelligence, execution, and knowledge returns an empty cycle list even though its raw edges contain the complete cycle. This can make the proposed boundary measurement report an apparently acyclic graph.

## Symptom (Effect)

Observed 2026-09-13 by calling the production `static_measurement` function on synthetic Rust source files:

```text
long edges: intelligence -> execution; execution -> knowledge; knowledge -> intelligence; orchestration -> intelligence
population_cycles: []
server_constructs_registered_tools: true

pair edges: intelligence -> knowledge; knowledge -> intelligence; orchestration -> intelligence
population_cycles: [["intelligence", "knowledge"]]
server_constructs_registered_tools: true
```

The reciprocal-pair fixture is the positive control. This measures cycle detection on these fixtures; it does not establish how many cycles exist in the codescout source tree.

## Reproduction

Run the following from the repository root. It invokes the real production function with in-memory fixture file contents; it does not modify repository files.

```python
import runpy
from pathlib import Path
from unittest.mock import patch

ns = runpy.run_path('scripts/architecture-boundary-probe.py')
measure = ns['static_measurement']
root = Path('/synthetic-boundary-fixture')
common = {'src/server.rs': 'use crate::tools::symbol::Symbols;\nfn register() { Arc::new(Symbols); }\n'}
fixtures = {
    'long': {**common,
        'src/lsp/mod.rs': 'use crate::tools::read_file::ReadFile;\n',
        'src/tools/read_file.rs': 'use crate::memory::MemoryStore;\n',
        'src/memory/mod.rs': 'use crate::lsp::LspProvider;\n'},
    'pair': {**common,
        'src/lsp/mod.rs': 'use crate::memory::MemoryStore;\n',
        'src/memory/mod.rs': 'use crate::lsp::LspProvider;\n'},
}
for label, files in fixtures.items():
    def read_fixture(path, **kwargs):
        return files[path.relative_to(root).as_posix()]
    with patch.dict(measure.__globals__, {'rust_paths': lambda _: sorted(files)}):
        with patch.object(Path, 'read_text', read_fixture):
            result = measure(root)
    print(label, result['raw_edges'], result['population_cycles'], result['positive_controls'])
```

The initial on-disk reproduction was run with `python3 /tmp/codescout-boundary-cycle-VANLia/repro.py`; both fixtures and stdout were inspected. The in-memory recipe above is included to make the reproduction independent of that temporary directory.

## Environment

Linux, Python 3, codescout shared experiments checkout. HEAD observed during reproduction: `127468ac9fb04acf3caf3c38c5f6347720ed5346`. Probe read from the working tree, which may contain concurrent changes; no claim that this SHA alone identifies its exact bytes.

## Root cause

`scripts/architecture-boundary-probe.py:static_measurement` builds adjacency, then iterates `combinations(POPULATIONS, 2)` and requires each member to directly reach the other. It never traverses paths through additional populations. Measured 2026-09-13 with the production-function fixtures above: longer cycle omitted, reciprocal pair detected.

## Evidence

The exact observable cycle outputs and positive controls appear under Symptom. The tracker `docs/trackers/architecture-boundary-measurement.md` requests population-level cycles without restricting the property to reciprocal pairs.

## Hypotheses tried

1. Cycle analysis covers general directed cycles / invoke production static analysis on the long fixture / rejected: empty list.
2. The fixture did not reach static edge collection / inspect raw edges and registration control / rejected: every edge of the long cycle appears, control true.
3. The implementation detects reciprocal pairs / run the pair fixture / confirmed: pair returned.

## Fix

Implemented in the uncommitted worktree probe `scripts/architecture-boundary-probe.py`: directed_cycles now enumerates canonical simple directed cycles, including non-reciprocal longer cycles.

Verified 2026-09-13: 14 Python regression tests and self-test pass. The second repository gate passed formatting, full clippy, lean tests, then default tests; see `gate2-*.log` under `.codescout/measurements/architecture-boundary/2026-09-13/`. Earlier gate failures are retained separately.

No fix commit has been made. SHA and patch-id are therefore not available; this record is not archived. Full measurement bounds and provenance: docs/trackers/architecture-boundary-measurement.md.
## Tests added

In `tests/test_architecture_boundary_probe.py`: StaticControls.test_detects_long_cycle_without_reciprocal_edges; test_detects_reciprocal_pair_once; test_dag_and_test_only_reverse_edge_do_not_make_a_cycle. An isolated mutation restricting cycles to length two failed the longer-cycle regression.
## Workarounds

Do not interpret an empty `population_cycles` list as absence of cycles. Inspect the complete raw directed-edge graph, or label this output as reciprocal pairs until general cycle analysis is implemented.

## Resume

Implementation and bounded validation are complete in the working tree. Review the exact diff and coordinate peers before any authorized commit; then record the experiments fix SHA and stable patch-id and archive through the librarian. Do not reinterpret the lexical probe as compiler-resolved architecture.
## References

- `docs/trackers/architecture-boundary-measurement.md`
- `scripts/architecture-boundary-probe.py`
- `issue-clusters:IC-18` — the selector covers only a subset of the named population.
