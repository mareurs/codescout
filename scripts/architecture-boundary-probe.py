#!/usr/bin/env python3
"""Measure candidate architecture boundaries without deciding the refactor.

The probe emits raw JSON for four independently interpretable measurements:

* ``static`` counts one unique (production source file, classified target module)
  edge for imports and qualified references separately. It cannot see references
  reached only through re-exports, macros, dynamic dispatch, or unqualified names.
* ``context`` counts core ``ToolContext`` fields read by each live registered
  tool's textual production call path. The path follows same-file free-function
  calls and explicit ``Delegate.call(..., ctx)`` dispatch. It reports unresolved
  tools and does not pretend that textual reachability is a runtime trace.
* ``history`` counts one commit in the frozen ``git rev-list --max-count=N HEAD``
  set when that commit changes at least one tracked Rust path in a population.
* ``weight`` counts unique exact package lines printed by each specified
  ``cargo tree`` command. Package lines are not crate counts or binary bytes.

The frozen populations are copied from
``docs/trackers/architecture-boundary-measurement.md``. Unclassified Rust files,
feature-gated source, parser blind spots, and failed controls remain visible in
the output instead of being assigned by guesswork.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import dataclasses
import datetime as dt
import hashlib
import io
import json
import os
from pathlib import Path
import re
import select
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
from collections import Counter, defaultdict
from itertools import combinations
from typing import Any, Iterable, Iterator, Sequence


POPULATIONS = ("intelligence", "execution", "knowledge", "retrieval", "orchestration")

PATH_RULES: tuple[tuple[str, tuple[str, ...], tuple[str, ...]], ...] = (
    (
        "intelligence",
        ("src/lsp/", "src/ast/", "src/tools/symbol/", "src/library/"),
        ("src/tools/library.rs",),
    ),
    (
        "execution",
        ("src/tools/edit_file/", "src/tools/run_command/"),
        (
            "src/tools/read_file.rs",
            "src/tools/tree.rs",
            "src/tools/grep.rs",
            "src/tools/create_file.rs",
            "src/tools/approve_write.rs",
            "src/util/path_security.rs",
            "src/agent/write_guard.rs",
        ),
    ),
    (
        "knowledge",
        ("src/librarian/", "src/memory/", "src/tools/memory/"),
        (),
    ),
    (
        "retrieval",
        ("src/retrieval/", "src/embed/", "src/tools/semantic/", "crates/codescout-embed/src/"),
        (),
    ),
    (
        "orchestration",
        ("src/tools/core/", "src/tools/config/", "src/prompts/"),
        ("src/server.rs",),
    ),
)

MODULE_RULES: tuple[tuple[str, str], ...] = (
    ("crate::agent::write_guard", "execution"),
    ("crate::tools::edit_file", "execution"),
    ("crate::tools::run_command", "execution"),
    ("crate::tools::read_file", "execution"),
    ("crate::tools::tree", "execution"),
    ("crate::tools::grep", "execution"),
    ("crate::tools::create_file", "execution"),
    ("crate::tools::approve_write", "execution"),
    ("crate::util::path_security", "execution"),
    ("crate::tools::symbol", "intelligence"),
    ("crate::tools::library", "intelligence"),
    ("crate::library", "intelligence"),
    ("crate::lsp", "intelligence"),
    ("crate::ast", "intelligence"),
    ("crate::tools::memory", "knowledge"),
    ("crate::librarian", "knowledge"),
    ("crate::memory", "knowledge"),
    ("crate::tools::semantic", "retrieval"),
    ("crate::retrieval", "retrieval"),
    ("crate::embed", "retrieval"),
    ("codescout_embed", "retrieval"),
    ("crate::tools::core", "orchestration"),
    ("crate::tools::config", "orchestration"),
    ("crate::prompts", "orchestration"),
    ("crate::server", "orchestration"),
    ("crate::agent", "orchestration"),
    ("crate::tools::ToolContext", "orchestration"),
    ("crate::tools::Tool", "orchestration"),
    ("crate::tools::RecoverableError", "orchestration"),
    ("crate::tools::OutputGuard", "orchestration"),
)

CORE_CONTEXT_FIELDS = (
    "agent",
    "lsp",
    "output_buffer",
    "progress",
    "peer",
    "section_coverage",
    "guide_hints_emitted",
    "workspace_override",
)

LIBRARIAN_CONTEXT_FIELDS = (
    "catalog",
    "workspace",
    "rules",
    "embedding",
    "artifact_store",
    "current_project",
    "lsp",
    "temp_guard",
    "progress",
)


class ProbeError(RuntimeError):
    """A failed measurement prerequisite or positive control."""


@dataclasses.dataclass(frozen=True)
class Snapshot:
    timestamp: str
    head: str
    branch: str
    tracked_rust_changes: tuple[str, ...]
    source_digest: str


def run(
    args: Sequence[str],
    repo: Path,
    *,
    timeout: int = 120,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        args,
        cwd=repo,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=timeout,
        check=False,
    )
    if check and completed.returncode != 0:
        rendered = " ".join(args)
        raise ProbeError(
            f"command failed ({completed.returncode}): {rendered}\n{completed.stderr.strip()}"
        )
    return completed


def git(repo: Path, *args: str, timeout: int = 120) -> str:
    return run(("git", *args), repo, timeout=timeout).stdout


def classify_path(path: str) -> str | None:
    normalized = path.replace(os.sep, "/")
    if normalized.startswith("src/agent/") and normalized != "src/agent/write_guard.rs":
        return "orchestration"
    for population, prefixes, exact in PATH_RULES:
        if normalized in exact or any(normalized.startswith(prefix) for prefix in prefixes):
            return population
    return None


def classify_module(reference: str) -> tuple[str, str] | None:
    for module, population in MODULE_RULES:
        if reference == module or reference.startswith(module + "::"):
            return module, population
    return None


def rust_paths(repo: Path) -> list[str]:
    paths: list[str] = []
    for path in repo.rglob("*.rs"):
        relative = path.relative_to(repo).as_posix()
        if any(part in {".git", "target"} for part in path.relative_to(repo).parts):
            continue
        paths.append(relative)
    return sorted(paths)


def is_test_only_path(path: str) -> bool:
    parts = Path(path).parts
    return (
        path.startswith("tests/")
        or path.startswith("examples/")
        or "tests" in parts[:-1]
        or Path(path).name == "tests.rs"
    )


def source_digest(repo: Path) -> str:
    digest = hashlib.sha256()
    paths = rust_paths(repo)
    for manifest in ("Cargo.toml", "Cargo.lock"):
        if (repo / manifest).is_file():
            paths.append(manifest)
    for relative in sorted(paths):
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update((repo / relative).read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def snapshot(repo: Path) -> Snapshot:
    changes = git(repo, "status", "--porcelain=v1", "--untracked-files=all", "--", "*.rs")
    return Snapshot(
        timestamp=dt.datetime.now().astimezone().isoformat(timespec="seconds"),
        head=git(repo, "rev-parse", "HEAD").strip(),
        branch=git(repo, "branch", "--show-current").strip() or "DETACHED",
        tracked_rust_changes=tuple(line for line in changes.splitlines() if line),
        source_digest=source_digest(repo),
    )


@contextmanager
def frozen_source_tree(repo: Path, head: str) -> Iterator[Path]:
    """Materialize only source and Cargo metadata tracked at ``head``.

    Static and dependency measurements consume this tree, so concurrent edits in
    the shared checkout cannot move their population. The live MCP registry still
    uses the real project root and carries its own binary/runtime identity.
    """

    tracked = git(repo, "ls-tree", "-r", "--name-only", head).splitlines()
    selected = [
        path
        for path in tracked
        if path.endswith(".rs")
        or Path(path).name in {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "build.rs"}
        or path.startswith(".cargo/")
    ]
    if not selected:
        raise ProbeError(f"positive control failed: no source paths found at {head}")
    completed = subprocess.run(
        ("git", "archive", "--format=tar", head, "--", *selected),
        cwd=repo,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise ProbeError(
            f"git archive failed ({completed.returncode}): {completed.stderr.decode(errors='replace').strip()}"
        )
    with tempfile.TemporaryDirectory(prefix="codescout-architecture-boundary-") as directory:
        root = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(completed.stdout), mode="r:") as archive:
            archive.extractall(root, filter="data")
        yield root


def replace_span(text: str, start: int, end: int) -> str:
    return text[:start] + "".join("\n" if char == "\n" else " " for char in text[start:end]) + text[end:]


def lexical_mask(text: str) -> str:
    """Blank comments and literals while preserving byte positions and newlines.

    This scanner handles nested block comments and Rust raw strings. It does not
    expand macros, so dependency paths manufactured by macro expansion remain a
    declared blind spot rather than being inferred.
    """

    chars = list(text)
    index = 0
    length = len(chars)
    while index < length:
        if text.startswith("//", index):
            end = text.find("\n", index)
            end = length if end < 0 else end
            for pos in range(index, end):
                chars[pos] = " "
            index = end
            continue
        if text.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if text.startswith("/*", end):
                    depth += 1
                    end += 2
                elif text.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            for pos in range(index, end):
                if chars[pos] != "\n":
                    chars[pos] = " "
            index = end
            continue
        raw_match = re.match(r"(?:b)?r(#{0,255})\"", text[index:])
        if raw_match:
            hashes = raw_match.group(1)
            marker = '"' + hashes
            end = text.find(marker, index + raw_match.end())
            end = length if end < 0 else end + len(marker)
            for pos in range(index, end):
                if chars[pos] != "\n":
                    chars[pos] = " "
            index = end
            continue
        char_match = re.match(
            r"(?:b)?'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]+\}|.)|[^\\'\n])'",
            text[index:],
        )
        if char_match:
            end = index + char_match.end()
            for pos in range(index, end):
                chars[pos] = " "
            index = end
            continue
        prefix_length = 2 if text.startswith('b"', index) else 1
        quote_index = index + prefix_length - 1
        if quote_index < length and chars[quote_index] == '"':
            quote = '"'
            end = quote_index + 1
            escaped = False
            while end < length:
                char = text[end]
                if char == quote and not escaped:
                    end += 1
                    break
                escaped = char == "\\" and not escaped
                if char != "\\":
                    escaped = False
                end += 1
            for pos in range(index, end):
                if chars[pos] != "\n":
                    chars[pos] = " "
            index = end
            continue
        index += 1
    return "".join(chars)


def matching_brace(text: str, opening: int, left: str = "{", right: str = "}") -> int:
    depth = 0
    for index in range(opening, len(text)):
        if text[index] == left:
            depth += 1
        elif text[index] == right:
            depth -= 1
            if depth == 0:
                return index
    raise ProbeError(f"unmatched {left!r} at byte {opening}")


def production_mask(text: str) -> tuple[str, list[dict[str, Any]]]:
    masked = lexical_mask(text)
    feature_gates: list[dict[str, Any]] = []
    cfg_pattern = re.compile(r"#\s*\[\s*cfg\s*\((.*?)\)\s*\]", re.DOTALL)
    for match in cfg_pattern.finditer(masked):
        predicate = " ".join(match.group(1).split())
        if "feature" in predicate:
            feature_gates.append(
                {"line": text.count("\n", 0, match.start()) + 1, "predicate": predicate}
            )

    test_attributes = list(
        re.finditer(
            r"#\s*\[\s*(?:cfg\s*\(\s*test\s*\)|(?:[A-Za-z_]\w*::)?test)\s*\]",
            masked,
        )
    )
    for attribute in reversed(test_attributes):
        cursor = attribute.end()
        # Fields (including struct initializers) terminate at an outer comma,
        # not at the next item's semicolon/brace. Nested generics/calls/structs
        # may contain commas themselves, so keep a delimiter stack.
        field = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?[A-Za-z_]\w*\s*:(?!:)", masked[cursor:])
        if field:
            stack: list[str] = []
            pairs = {"(": ")", "[": "]", "{": "}", "<": ">"}
            end = cursor + field.end()
            while end < len(masked):
                char = masked[end]
                if char == "," and not stack:
                    end += 1
                    break
                if char == "}" and not stack:
                    break
                if char in pairs:
                    stack.append(pairs[char])
                elif stack and char == stack[-1]:
                    stack.pop()
                end += 1
            masked = replace_span(masked, attribute.start(), end)
            continue
        next_brace = masked.find("{", cursor)
        next_semicolon = masked.find(";", cursor)
        if next_semicolon >= 0 and (next_brace < 0 or next_semicolon < next_brace):
            masked = replace_span(masked, attribute.start(), next_semicolon + 1)
        elif next_brace >= 0:
            end = matching_brace(masked, next_brace) + 1
            masked = replace_span(masked, attribute.start(), end)
    return masked, feature_gates


def split_top_level(text: str, separator: str = ",") -> list[str]:
    parts: list[str] = []
    depth = 0
    start = 0
    for index, char in enumerate(text):
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
        elif char == separator and depth == 0:
            parts.append(text[start:index])
            start = index + 1
    parts.append(text[start:])
    return [part.strip() for part in parts if part.strip()]


def join_module(prefix: str, suffix: str) -> str:
    if not prefix:
        return suffix.strip(":")
    if suffix == "self":
        return prefix.strip(":")
    return prefix.rstrip(":") + "::" + suffix.strip(":")


def expand_use_tree(expression: str, prefix: str = "") -> list[str]:
    # Preserve token boundaries until aliases are removed: `Task` and `base`
    # contain `as`, but neither is an alias declaration.
    expression = re.sub(r"\s+as\s+[A-Za-z_]\w*\s*(?=[,}]|$)", "", expression)
    expression = re.sub(r"\s+", "", expression.strip())
    parts = split_top_level(expression)
    if len(parts) > 1:
        return [leaf for part in parts for leaf in expand_use_tree(part, prefix)]
    expression = parts[0] if parts else ""
    opening = expression.find("{")
    if opening >= 0:
        closing = matching_brace(expression, opening)
        base = expression[:opening].rstrip(":")
        nested_prefix = join_module(prefix, base) if base else prefix
        inner = expression[opening + 1 : closing]
        return [leaf for part in split_top_level(inner) for leaf in expand_use_tree(part, nested_prefix)]
    return [join_module(prefix, expression)] if expression else []


def source_module(path: str) -> list[str]:
    relative = Path(path)
    if relative.parts[0] != "src":
        if path.startswith("crates/codescout-embed/src/"):
            relative = Path(path).relative_to("crates/codescout-embed/src")
            prefix = ["codescout_embed"]
        else:
            return []
    else:
        relative = relative.relative_to("src")
        prefix = ["crate"]
    parts = list(relative.with_suffix("").parts)
    if parts and parts[-1] in {"mod", "lib", "main"}:
        parts.pop()
    return prefix + parts


def resolve_relative(reference: str, path: str) -> str:
    if reference.startswith("crate::") or reference.startswith("codescout_embed"):
        return reference
    # Bare roots may be external crates or local names. Without compiler name
    # resolution neither can be promoted to a proved internal dependency.
    if reference != "self" and not reference.startswith(("self::", "super::")):
        return reference
    module = source_module(path)
    while reference.startswith("super::"):
        if len(module) > 1:
            module.pop()
        reference = reference[len("super::") :]
    if reference.startswith("self::"):
        reference = reference[len("self::") :]
    if reference == "self":
        return "::".join(module)
    if module and not reference.startswith(("std::", "tokio::")):
        return "::".join(module + [reference])
    return reference


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def directed_cycles(adjacency: dict[str, set[str]]) -> list[list[str]]:
    """All simple directed cycles, each rotated to its smallest node.

    The candidate graph has only the declared ownership populations. Enumerating
    its cycles is cheap and preserves the actual direction, unlike listing SCCs
    as though every permutation of their members were a path.
    """
    cycles: list[list[str]] = []
    for start in sorted(adjacency):
        def visit(path: list[str]) -> None:
            for target in sorted(adjacency.get(path[-1], set())):
                if target == start and len(path) > 1:
                    cycles.append(path.copy())
                elif target > start and target not in path:
                    visit([*path, target])
        visit([start])
    return sorted(cycles)


def static_measurement(repo: Path) -> dict[str, Any]:
    population_files: dict[str, list[str]] = {population: [] for population in POPULATIONS}
    unclassified: list[str] = []
    for path in rust_paths(repo):
        if is_test_only_path(path):
            continue
        population = classify_path(path)
        if population:
            population_files[population].append(path)
        else:
            unclassified.append(path)

    raw_references: list[dict[str, Any]] = []
    feature_gated: list[dict[str, Any]] = []
    unresolved_targets: list[dict[str, Any]] = []
    registration_types: list[dict[str, Any]] = []

    for source_population, files in population_files.items():
        for path in files:
            original = (repo / path).read_text(encoding="utf-8")
            try:
                production, gates = production_mask(original)
            except ProbeError as error:
                raise ProbeError(f"{path}: {error}") from error
            feature_gated.extend({"file": path, **gate} for gate in gates)
            import_spans: list[tuple[int, int]] = []
            for match in re.finditer(r"\buse\s+(.+?);", production, re.DOTALL):
                import_spans.append(match.span())
                for leaf in expand_use_tree(match.group(1)):
                    reference = resolve_relative(leaf, path)
                    target = classify_module(reference)
                    record = {
                        "source_file": path,
                        "source_population": source_population,
                        "line": line_number(original, match.start()),
                        "kind": "use",
                        "reference": reference,
                    }
                    if target:
                        record.update(target_module=target[0], target_population=target[1])
                        raw_references.append(record)
                    elif reference.startswith(("crate::", "codescout_embed")):
                        unresolved_targets.append(record)

            without_imports = production
            for start, end in reversed(import_spans):
                without_imports = replace_span(without_imports, start, end)
            for match in re.finditer(r"\b(?:crate|codescout_embed)(?:::[A-Za-z_]\w*)+", without_imports):
                reference = match.group(0)
                target = classify_module(reference)
                following = without_imports[match.end() :]
                kind = "qualified_call" if re.match(r"\s*(?:::<[^>]+>)?\s*\(", following) else "qualified_reference"
                record = {
                    "source_file": path,
                    "source_population": source_population,
                    "line": line_number(original, match.start()),
                    "kind": kind,
                    "reference": reference,
                }
                if target:
                    record.update(target_module=target[0], target_population=target[1])
                    raw_references.append(record)
                else:
                    unresolved_targets.append(record)

            if path == "src/server.rs":
                for match in re.finditer(r"Arc::new\((?:crate::[\w:]+::)?([A-Z][A-Za-z0-9_]*)", production):
                    registration_types.append(
                        {"type": match.group(1), "line": line_number(original, match.start())}
                    )

    evidence_by_edge: dict[tuple[str, str, str, str], set[str]] = defaultdict(set)
    for item in raw_references:
        key = (
            item["source_file"],
            item["source_population"],
            item["target_module"],
            item["target_population"],
        )
        evidence_by_edge[key].add(item["kind"])
    raw_edges = [
        {
            "source_file": source_file,
            "source_population": source_population,
            "target_module": target_module,
            "target_population": target_population,
            "evidence_kinds": sorted(evidence_by_edge[(source_file, source_population, target_module, target_population)]),
        }
        for source_file, source_population, target_module, target_population in sorted(evidence_by_edge)
    ]
    directed_counts: Counter[str] = Counter(
        {f"{source}->{target}": 0 for source in POPULATIONS for target in POPULATIONS}
    )
    for edge in raw_edges:
        directed_counts[f"{edge['source_population']}->{edge['target_population']}"] += 1

    adjacency: dict[str, set[str]] = {population: set() for population in POPULATIONS}
    for edge in raw_edges:
        if edge["source_population"] != edge["target_population"]:
            adjacency[edge["source_population"]].add(edge["target_population"])
    cycles = directed_cycles(adjacency)

    concrete_cross_domain = [
        reference
        for reference in raw_references
        if reference["source_population"] != reference["target_population"]
        and reference["kind"] != "use"
    ]
    imported_tool_types = {
        item["reference"].split("::")[-1]
        for item in raw_references
        if item["source_file"] == "src/server.rs"
        and item["kind"] == "use"
        and item["reference"].startswith("crate::tools::")
    }
    registration_types = [item for item in registration_types if item["type"] in imported_tool_types]
    controls = {
        "server_constructs_registered_tools": bool(registration_types),
        "registration_count_in_source": len(registration_types),
    }
    if not controls["server_constructs_registered_tools"]:
        raise ProbeError("positive control failed: src/server.rs tool registrations were not found")

    return {
        "unit": "unique (production source file, classified target module) edge",
        "population_files": population_files,
        "population_file_counts": {
            population: len(files) for population, files in population_files.items()
        },
        "unclassified_rust_files": unclassified,
        "unclassified_rust_file_count": len(unclassified),
        "feature_gated_sites": feature_gated,
        "feature_gated_site_count": len(feature_gated),
        "unresolved_internal_targets": unresolved_targets,
        "unresolved_internal_target_count": len(unresolved_targets),
        "raw_references": raw_references,
        "raw_reference_count": len(raw_references),
        "raw_edges": raw_edges,
        "raw_edge_count": len(raw_edges),
        "directed_edge_counts": dict(sorted(directed_counts.items())),
        "population_cycles": cycles,
        "cycle_definition": "simple directed cross-population cycles; canonical rotation, closing edge implicit",
        "concrete_cross_domain_references": concrete_cross_domain,
        "server_registration_types": registration_types,
        "positive_controls": controls,
    }


def extract_blocks(text: str, pattern: re.Pattern[str]) -> Iterator[tuple[re.Match[str], str]]:
    for match in pattern.finditer(text):
        opening = text.find("{", match.end())
        if opening < 0:
            continue
        closing = matching_brace(text, opening)
        yield match, text[opening + 1 : closing]


def impl_tool_blocks(text: str, original: str | None = None) -> list[dict[str, str]]:
    pattern = re.compile(
        r"\bimpl(?:\s*<[^>{}]+>)?\s+(?:crate::tools::)?Tool\s+for\s+([A-Za-z_][\w:]*)"
    )
    blocks: list[dict[str, str]] = []
    for match in pattern.finditer(text):
        opening = text.find("{", match.end())
        if opening < 0:
            continue
        closing = matching_brace(text, opening)
        blocks.append(
            {
                "type": match.group(1).split("::")[-1],
                "body": text[opening + 1 : closing],
                "original_body": (original or text)[opening + 1 : closing],
            }
        )
    return blocks


def function_body(text: str, name: str) -> str | None:
    pattern = re.compile(rf"\b(?:async\s+)?fn\s+{re.escape(name)}\b[^{{;]*")
    for _match, body in extract_blocks(text, pattern):
        return body
    return None


def named_function_bodies(text: str, *, free_only: bool = False) -> dict[str, list[str]]:
    """Function bodies keyed by name. `free_only` drops `self`-receiver methods.

    Two consumers want opposite things, which is why this is a flag and not a
    behaviour change. `methods_for_type` indexes a type's METHODS to resolve
    `self.f()`, so it needs the receivers and keeps the default. The free-call
    loop in `reachable_context` resolves `f(x)` -- its
    `(?<![.:])\\b([a-z_]\\w*)\\s*\\(` regex already rules out `x.f()` and `T::f()`
    by lookbehind -- and a free call cannot reach `fn f(&mut self)`. Resolving one
    to a method is not a near-miss; it picks a body the caller could not have been
    invoking.

    `drop` is the live case and it has both failure modes. With several
    `impl Drop for T { fn drop(&mut self) }` in one file the name resolves
    ambiguously and lands in `unresolved_same_file_helpers`; with exactly one it
    resolves UNIQUELY, the trait body gets walked, and its `ctx` reads are
    attributed to the tool. The first is noise in a published population; the
    second inflates the measurement and nothing flags it.

    The receiver is the discriminator and it is structural, so no name list is
    needed -- which matters because the guard it replaces was an enumerated
    denylist of four keywords over an open namespace.
    docs/issues/archive/2026-09-16-probe-counts-rust-keywords-as-unresolved-helpers.md
    """
    self_receiver = re.compile(r"\(\s*(?:&\s*(?:'[A-Za-z_][A-Za-z0-9_]*\s+)?)?(?:mut\s+)?self\b")
    functions: dict[str, list[str]] = defaultdict(list)
    pattern = re.compile(r"\b(?:async\s+)?fn\s+([a-z_][A-Za-z0-9_]*)\b[^{{;]*")
    for match in pattern.finditer(text):
        opening = text.find("{", match.end())
        if opening < 0:
            continue
        if free_only and self_receiver.search(text[match.start() : opening]):
            continue
        try:
            closing = matching_brace(text, opening)
        except ProbeError as error:
            line = line_number(text, match.start())
            raise ProbeError(f"function {match.group(1)!r} at line {line}: {error}") from error
        functions[match.group(1)].append(text[opening + 1 : closing])
    return functions


def methods_for_type(file_text: str, tool_type: str) -> dict[str, list[str]]:
    methods: dict[str, list[str]] = defaultdict(list)
    pattern = re.compile(
        rf"\bimpl(?:\s*<[^>{{}}]+>)?(?:\s+[A-Za-z_][\w:<>]*\s+for)?\s+{re.escape(tool_type)}\s*"
    )
    for _match, impl_body in extract_blocks(file_text, pattern):
        for name, bodies in named_function_bodies(impl_body).items():
            methods[name].extend(bodies)
    return methods


def string_literal(text: str) -> str | None:
    match = re.search(r'"([^"\\]*(?:\\.[^"\\]*)*)"', text)
    return bytes(match.group(1), "utf-8").decode("unicode_escape") if match else None


def tool_name(tool_type: str, impl_body: str, file_text: str) -> str | None:
    name_body = function_body(impl_body, "name")
    literal = string_literal(name_body or "")
    if literal:
        return literal
    const_pattern = re.compile(
        rf"impl\s+{re.escape(tool_type)}\s*\{{.*?const\s+NAME\s*:\s*[^=]+\s*=\s*\"([^\"]+)\"",
        re.DOTALL,
    )
    match = const_pattern.search(file_text)
    return match.group(1) if match else None


def reachable_context(
    tool_type: str,
    impl_body: str,
    file_text: str,
    global_functions: dict[str, list[tuple[str, str]]] | None = None,
) -> dict[str, Any]:
    call_body = function_body(impl_body, "call") or ""
    all_impls = {item["type"]: item["body"] for item in impl_tool_blocks(file_text)}
    method_maps = {name: methods_for_type(file_text, name) for name in all_impls}
    method_maps.setdefault(tool_type, methods_for_type(file_text, tool_type))
    local_functions = named_function_bodies(file_text, free_only=True)

    bodies = [call_body]
    queue = [(call_body, tool_type)]
    visited_bodies = {call_body}
    unresolved_helpers: set[str] = set()
    while queue:
        body, owner_type = queue.pop()
        for method in re.findall(r"\bself\.([a-z_][A-Za-z0-9_]*)\s*\(", body):
            candidates = method_maps.get(owner_type, {}).get(method, [])
            if len(candidates) == 1:
                candidate = candidates[0]
                if candidate not in visited_bodies:
                    visited_bodies.add(candidate)
                    bodies.append(candidate)
                    queue.append((candidate, owner_type))
            else:
                unresolved_helpers.add(f"{owner_type}::{method}")
        for delegate in re.findall(r"\b([A-Z][A-Za-z0-9_]*)\s*\.\s*call\s*\(", body):
            delegate_impl = all_impls.get(delegate)
            delegate_body = function_body(delegate_impl or "", "call")
            if delegate_body and delegate_body not in visited_bodies:
                visited_bodies.add(delegate_body)
                bodies.append(delegate_body)
                queue.append((delegate_body, delegate))
        for helper in re.findall(r"(?<![.:])\b([a-z_][A-Za-z0-9_]*)\s*\(", body):
            local_candidates = local_functions.get(helper, [])
            global_candidates = (global_functions or {}).get(helper, [])
            candidates = local_candidates or [candidate for _path, candidate in global_candidates]
            if len(candidates) == 1:
                candidate = candidates[0]
                if candidate not in visited_bodies:
                    visited_bodies.add(candidate)
                    bodies.append(candidate)
                    queue.append((candidate, owner_type))
            elif len(candidates) > 1 and "ctx" in body:
                # Report only names AMBIGUOUS between real free-function definitions
                # -- the case where the probe genuinely cannot pick a body to follow.
                # Zero candidates means syntax (`let (a, b) = ...`), a closure, or an
                # out-of-tree call; none is a same-file helper. The enumerated
                # {"if","match","while","for"} denylist this replaced was four names
                # over an OPEN namespace, so every other keyword in call position was
                # reported -- `let` reached 15 of 15 tools, 20 of 28 rows were syntax,
                # and the list could only ever be extended by someone who had already
                # noticed the specific miss.
                unresolved_helpers.add(helper)

    fields: set[str] = set()
    agent_methods: list[dict[str, Any]] = []
    for body in bodies:
        fields.update(re.findall(r"\bctx\s*\.\s*([a-z_][A-Za-z0-9_]*)", body))
        aliases = set(re.findall(r"let\s+([a-z_][A-Za-z0-9_]*)\s*=\s*ctx\s*\.\s*agent(?:\s*\.\s*clone\(\))?", body))
        for method in re.findall(r"\bctx\s*\.\s*agent\s*\.\s*([a-z_][A-Za-z0-9_]*)", body):
            agent_methods.append({"receiver": "ctx.agent", "method_or_field": method})
        for alias in aliases:
            for method in re.findall(rf"\b{re.escape(alias)}\s*\.\s*([a-z_][A-Za-z0-9_]*)", body):
                agent_methods.append({"receiver": alias, "method_or_field": method})
    return {
        "tool_type": tool_type,
        "direct_context_fields": sorted(set(re.findall(r"\bctx\s*\.\s*([a-z_][A-Za-z0-9_]*)", call_body)).intersection(CORE_CONTEXT_FIELDS)),
        "context_fields": sorted(fields.intersection(CORE_CONTEXT_FIELDS)),
        "unknown_context_fields": sorted(fields.difference(CORE_CONTEXT_FIELDS)),
        "agent_reach_through": sorted(
            {json.dumps(item, sort_keys=True) for item in agent_methods}
        ),
        "reachable_body_count": len(bodies),
        "unresolved_same_file_helpers": sorted(unresolved_helpers),
    }


def read_json_rpc_response(process: subprocess.Popen[str], request_id: int, timeout: float) -> dict[str, Any]:
    assert process.stdout is not None
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        ready, _, _ = select.select([process.stdout], [], [], max(0.0, deadline - time.monotonic()))
        if not ready:
            break
        line = process.stdout.readline()
        if not line:
            break
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("id") == request_id:
            return message
    raise ProbeError(f"timed out waiting for MCP response id={request_id}")


def live_tool_registry(repo: Path, binary: Path, timeout: int = 30) -> dict[str, Any]:
    """Query a newly started MCP process; this counts tools it actually advertises.

    The result describes the supplied executable and current runtime configuration,
    not necessarily the repository's current HEAD. Both identities are reported so
    callers cannot silently substitute one for the other.
    """

    process = subprocess.Popen(
        (str(binary), "start", "--project", str(repo)),
        cwd=repo,
        text=True,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        bufsize=1,
    )
    try:
        assert process.stdin is not None
        initialize = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "architecture-boundary-probe", "version": "1"},
            },
        }
        process.stdin.write(json.dumps(initialize) + "\n")
        process.stdin.flush()
        initialized = read_json_rpc_response(process, 1, timeout)
        if "error" in initialized:
            raise ProbeError(f"MCP initialize failed: {initialized['error']}")
        process.stdin.write(
            json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}})
            + "\n"
        )
        process.stdin.write(json.dumps({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}) + "\n")
        process.stdin.flush()
        listed = read_json_rpc_response(process, 2, timeout)
        if "error" in listed:
            raise ProbeError(f"MCP tools/list failed: {listed['error']}")
        tools = listed.get("result", {}).get("tools", [])
        if not tools:
            raise ProbeError("positive control failed: live MCP tools/list returned no tools")
        return {
            "binary": str(binary),
            "binary_size": binary.stat().st_size,
            "binary_mtime": dt.datetime.fromtimestamp(binary.stat().st_mtime).astimezone().isoformat(),
            "initialize_result": initialized.get("result", {}),
            "tools": tools,
        }
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)


def direct_fields(body: str) -> list[str]:
    return sorted(set(re.findall(r"\bctx\s*\.\s*([a-z_]\w*)", body)))


def action_branches(source: str, tool_type: str, actions: list[str]) -> dict[str, Any]:
    """Textual action branches, not an execution trace or an exhaustive call graph.

    Unknown routing remains unknown. Shared pre-dispatch reads are kept separate
    from branch reads, and a literal delegate is evidence rather than a guessed
    cross-file expansion.
    """
    result = {action: {"resolution": "unresolved"} for action in actions}
    production, _ = production_mask(source)
    impl = next((row for row in impl_tool_blocks(production, source) if row["type"] == tool_type), None)
    if impl is None:
        return result
    masked_impl, original_impl = impl["body"], impl["original_body"]
    call = re.search(r"\b(?:async\s+)?fn\s+call\b[^{;]*", masked_impl)
    if call is None:
        return result
    opening = masked_impl.find("{", call.end())
    closing = matching_brace(masked_impl, opening)
    masked = masked_impl[opening + 1:closing]
    original = original_impl[opening + 1:closing]
    dispatch = re.search(r"\bmatch\s+action\s*\{", masked)
    if dispatch is None:
        return result
    match_open = masked.find("{", dispatch.start())
    match_close = matching_brace(masked, match_open)
    prefix = direct_fields(masked[:dispatch.start()])
    for arm in re.finditer(r'"([a-z_]+)"\s*=>\s*', original[match_open + 1:match_close]):
        action = arm.group(1)
        start = match_open + 1 + arm.start()
        before = masked[match_open + 1:start]
        if action not in result or before.count("{") != before.count("}"):
            continue
        body_start = match_open + 1 + arm.end()
        if masked[body_start:body_start + 1] == "{":
            end = matching_brace(masked, body_start) + 1
        else:
            end, stack = body_start, []
            pairs = {"(": ")", "[": "]", "{": "}"}
            while end < match_close:
                char = masked[end]
                if char == "," and not stack:
                    break
                if char in pairs:
                    stack.append(pairs[char])
                elif stack and char == stack[-1]:
                    stack.pop()
                end += 1
        body = masked[body_start:end]
        delegate = re.search(r"\b(super::[a-z_]\w*::call|[A-Z]\w*\s*\.\s*call)\s*\(", body)
        result[action] = {
            "resolution": "textual_branch",
            "branch_direct_fields": direct_fields(body),
            "shared_prefix_direct_fields": prefix,
            "delegate": delegate.group(1) if delegate else None,
            "branch_source": original[body_start:end].strip(),
        }
    return result


def action_measurement(registry: dict[str, Any], source_tools: dict[str, Any],
                       inner_tools: dict[str, Any], documents: dict[str, tuple[str, str]]) -> list[dict[str, Any]]:
    rows = []
    for tool in registry["tools"]:
        name = tool["name"]
        measurement = inner_tools.get(name, source_tools.get(name))
        actions = tool.get("inputSchema", {}).get("properties", {}).get("action", {}).get("enum", [])
        if not actions:
            rows.append({"tool": name, "action": None, "resolution": "no_advertised_action_enum"})
            continue
        path = measurement["source_file"] if measurement else None
        branches = action_branches(documents[path][0], measurement["tool_type"], actions) if path in documents else {}
        for action in actions:
            row = {"tool": name, "action": action, "source_file": path,
                   "context_kind": "librarian" if name in inner_tools else "core",
                   **branches.get(action, {"resolution": "unresolved"})}
            delegate = row.get("delegate") or ""
            if delegate.startswith("super::") and path:
                module = delegate.split("::")[1]
                base = Path(path).parent
                candidates = [(base / f"{module}.rs").as_posix(), (base / module / "mod.rs").as_posix()]
                target = next((candidate for candidate in candidates if candidate in documents), None)
                if target:
                    body = function_body(documents[target][1], "call")
                    if body is not None:
                        row["handler_source_file"] = target
                        row["handler_direct_fields"] = direct_fields(body)
            elif delegate and path:
                target_type = delegate.split(".")[0].strip()
                impl = next((item for item in impl_tool_blocks(documents[path][1]) if item["type"] == target_type), None)
                if impl:
                    row["handler_source_file"] = path
                    row["handler_direct_fields"] = direct_fields(function_body(impl["body"], "call") or "")
            rows.append(row)
    return rows


def context_measurement(source_root: Path, live_project_root: Path, binary: Path) -> dict[str, Any]:
    registry = live_tool_registry(live_project_root, binary)
    live_names = [tool.get("name", "") for tool in registry["tools"]]
    source_tools: dict[str, dict[str, Any]] = {}
    duplicate_names: dict[str, list[str]] = defaultdict(list)
    source_documents: dict[str, tuple[str, str]] = {}
    global_functions: dict[str, list[tuple[str, str]]] = defaultdict(list)

    for path in rust_paths(source_root):
        if not path.startswith("src/") or is_test_only_path(path):
            continue
        original = (source_root / path).read_text(encoding="utf-8")
        try:
            production, _ = production_mask(original)
        except ProbeError as error:
            raise ProbeError(f"{path}: {error}") from error
        source_documents[path] = (original, production)
        try:
            named_bodies = named_function_bodies(production, free_only=True)
        except ProbeError as error:
            raise ProbeError(f"{path} (function index): {error}") from error
        for name, bodies in named_bodies.items():
            global_functions[name].extend((path, body) for body in bodies)

    adapter_measurement: dict[str, Any] | None = None
    for path, (original, production) in source_documents.items():
        try:
            tool_impls = impl_tool_blocks(production, original)
        except ProbeError as error:
            raise ProbeError(f"{path} (Tool impl index): {error}") from error
        for impl in tool_impls:
            try:
                measurement = reachable_context(
                    impl["type"], impl["body"], production, global_functions
                )
            except ProbeError as error:
                raise ProbeError(f"{path} ({impl['type']}): {error}") from error
            measurement["source_file"] = path
            if impl["type"] == "LibrarianAdapter":
                adapter_measurement = measurement
                continue
            name = tool_name(impl["type"], impl["original_body"], original)
            if not name:
                continue
            if name in source_tools:
                duplicate_names[name].extend([source_tools[name]["source_file"], path])
            else:
                source_tools[name] = measurement

    inner_librarian_tools: dict[str, Any] = {}
    if adapter_measurement:
        for name, measurement in list(source_tools.items()):
            if measurement["source_file"].startswith("src/librarian/tools/"):
                inner_librarian_tools[name] = measurement
                source_tools[name] = {
                    **adapter_measurement,
                    "inner_tool_type": measurement["tool_type"],
                    "inner_source_file": measurement["source_file"],
                }

    by_tool: dict[str, Any] = {}
    unresolved_live_tools: list[str] = []
    for name in live_names:
        if name in source_tools:
            by_tool[name] = source_tools[name]
        else:
            unresolved_live_tools.append(name)
            by_tool[name] = {
                "source_file": None,
                "tool_type": None,
                "context_fields": [],
                "unknown_context_fields": [],
                "agent_reach_through": [],
                "reachable_body_count": 0,
                "unresolved_same_file_helpers": [],
            }

    field_to_tools = {
        field: sorted(name for name, item in by_tool.items() if field in item["context_fields"])
        for field in CORE_CONTEXT_FIELDS
    }
    capability_buckets: dict[str, list[str]] = {"0": [], "1": [], "2": [], "3": [], "4+": []}
    for name, item in by_tool.items():
        count = len(item["context_fields"])
        bucket = "4+" if count >= 4 else str(count)
        capability_buckets[bucket].append(name)
    for names in capability_buckets.values():
        names.sort()

    librarian_field_files: dict[str, list[str]] = {field: [] for field in LIBRARIAN_CONTEXT_FIELDS}
    for path in rust_paths(source_root):
        if not path.startswith("src/librarian/"):
            continue
        original = (source_root / path).read_text(encoding="utf-8")
        try:
            production, _ = production_mask(original)
        except ProbeError as error:
            raise ProbeError(f"{path}: {error}") from error
        for field in LIBRARIAN_CONTEXT_FIELDS:
            if re.search(rf"\bctx\s*\.\s*{re.escape(field)}\b", production):
                librarian_field_files[field].append(path)

    workspace_fields = by_tool.get("workspace", {}).get("context_fields", [])
    controls = {
        "live_registry_nonempty": bool(live_names),
        "workspace_tool_is_live": "workspace" in live_names,
        "workspace_call_path_reads_agent": "agent" in workspace_fields,
    }
    failed_controls = [name for name, passed in controls.items() if not passed]
    if failed_controls:
        raise ProbeError(
            "context positive controls failed: "
            + ", ".join(failed_controls)
            + f"; workspace={by_tool.get('workspace')!r}"
            + f"; unresolved_live_tools={unresolved_live_tools!r}"
        )

    agent_grouped: dict[str, list[str]] = defaultdict(list)
    for name, item in by_tool.items():
        for encoded in item["agent_reach_through"]:
            method = json.loads(encoded)["method_or_field"]
            agent_grouped[method].append(name)

    return {
        "unit": "distinct live registered tool whose textual production call path reads a core ToolContext field",
        "live_registry": registry,
        "live_tool_names": live_names,
        "source_tool_count": len(source_tools),
        "unresolved_live_tools": unresolved_live_tools,
        "duplicate_source_tool_names": {key: sorted(set(value)) for key, value in duplicate_names.items()},
        "tools": by_tool,
        "core_context_field_to_tools": field_to_tools,
        "direct_core_context_field_to_tools": {
            field: sorted(name for name, item in by_tool.items() if field in item.get("direct_context_fields", []))
            for field in CORE_CONTEXT_FIELDS
        },
        "action_unit": "advertised (tool, action) with direct textual branch/handler reads; unresolved routing is explicit",
        "action_rows": action_measurement(registry, source_tools, inner_librarian_tools, source_documents),
        "capability_definition": "one capability equals one distinct core ToolContext field read on the traced call path",
        "capability_buckets": capability_buckets,
        "agent_reach_through_by_method_or_field": {
            method: sorted(set(names)) for method, names in sorted(agent_grouped.items())
        },
        "librarian_adapter_core_context": adapter_measurement,
        "librarian_inner_tools": inner_librarian_tools,
        "librarian_adapter_context": {
            "unit": "production librarian source file containing a direct read of one librarian ToolContext field",
            "field_to_files": librarian_field_files,
        },
        "positive_controls": controls,
    }


def historical_measurement(repo: Path, limit: int, frozen_head: str) -> dict[str, Any]:
    shas = git(repo, "rev-list", f"--max-count={limit}", frozen_head, timeout=300).splitlines()
    if not shas:
        raise ProbeError("positive control failed: frozen git history window is empty")
    memberships: dict[str, set[str]] = {}
    raw_commits: list[dict[str, Any]] = []
    for sha in shas:
        paths = git(
            repo,
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--name-only",
            "-r",
            sha,
            "--",
            "*.rs",
            timeout=120,
        ).splitlines()
        populations = {population for path in paths if (population := classify_path(path))}
        memberships[sha] = populations
        subject = git(repo, "show", "-s", "--format=%s", sha).strip()
        raw_commits.append(
            {
                "sha": sha,
                "subject": subject,
                "rust_paths": paths,
                "populations": sorted(populations),
                "unclassified_rust_paths": sorted(path for path in paths if not classify_path(path)),
            }
        )

    per_population = {
        population: sum(population in touched for touched in memberships.values())
        for population in POPULATIONS
    }
    pairs: list[dict[str, Any]] = []
    for left, right in combinations(POPULATIONS, 2):
        left_set = {sha for sha, touched in memberships.items() if left in touched}
        right_set = {sha for sha, touched in memberships.items() if right in touched}
        intersection = left_set & right_set
        union = left_set | right_set
        pairs.append(
            {
                "left": left,
                "right": right,
                "intersection": len(intersection),
                "union": len(union),
                "jaccard": len(intersection) / len(union) if union else None,
                f"P({right}|{left})": len(intersection) / len(left_set) if left_set else None,
                f"P({left}|{right})": len(intersection) / len(right_set) if right_set else None,
                "intersection_shas": sorted(intersection),
            }
        )
    multi_population = [commit for commit in raw_commits if len(commit["populations"]) >= 3]
    control_commit = multi_population[0] if multi_population else None
    if control_commit is None:
        raise ProbeError("positive control failed: no commit in the frozen window touches 3+ populations")
    return {
        "unit": "commit in the frozen SHA set touching at least one tracked Rust file in a population",
        "command": f"git rev-list --max-count={limit} {frozen_head}",
        "frozen_head": frozen_head,
        "sha_count": len(shas),
        "shas": shas,
        "per_population_commit_counts": per_population,
        "pairs": pairs,
        "commits_touching_three_or_more_populations": multi_population,
        "positive_control_inspected_commit": control_commit,
        "raw_commits": raw_commits,
    }


def package_name(line: str) -> str:
    return line.split(maxsplit=1)[0] if line.split() else ""


def binary_identity(path: Path) -> dict[str, Any]:
    if not path.is_file():
        return {"path": str(path), "present": False}
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    return {
        "path": str(path),
        "present": True,
        "bytes": path.stat().st_size,
        "sha256": digest,
        "mtime": dt.datetime.fromtimestamp(path.stat().st_mtime).astimezone().isoformat(),
        "identity_note": "Size is not evidence about current HEAD; hash and mtime identify only these bytes.",
    }


def dependency_measurement(repo: Path, runtime_binary: Path) -> dict[str, Any]:
    commands = {
        "lean": ("cargo", "tree", "--no-default-features", "-e", "normal", "--prefix", "none", "--format", "{p}"),
        "default": ("cargo", "tree", "-e", "normal", "--prefix", "none", "--format", "{p}"),
        "server_stack": (
            "cargo",
            "tree",
            "--no-default-features",
            "--features",
            "server-stack",
            "-e",
            "normal",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ),
        "deployed": (
            "cargo", "tree", "--features", "server-stack,local-embed",
            "-e", "normal", "--prefix", "none", "--format", "{p}",
        ),
    }
    lanes: dict[str, Any] = {}
    package_sets: dict[str, set[str]] = {}
    for name, command in commands.items():
        completed = run(command, repo, timeout=300)
        lines = sorted({line.strip() for line in completed.stdout.splitlines() if line.strip()})
        package_sets[name] = {package_name(line) for line in lines}
        lanes[name] = {
            "command": " ".join(command),
            "unique_exact_package_line_count": len(lines),
            "unique_exact_package_lines": lines,
            "stderr": completed.stderr,
        }

    librarian_packages = ("serde_yml", "pulldown-cmark", "jsonschema")
    controls = {
        "librarian_packages_absent_from_lean": all(
            package not in package_sets["lean"] for package in librarian_packages
        ),
        "librarian_packages_present_in_default": all(
            package in package_sets["default"] for package in librarian_packages
        ),
        "qdrant_absent_from_default": "qdrant-client" not in package_sets["default"],
        "qdrant_present_in_server_stack": "qdrant-client" in package_sets["server_stack"],
    }
    failed_controls = [name for name, passed in controls.items() if not passed]
    if failed_controls:
        raise ProbeError("dependency positive controls failed: " + ", ".join(failed_controls))

    binaries = [runtime_binary, repo / "target/debug/codescout", repo / "target/release/codescout"]
    return {
        "unit": "unique exact non-empty package line printed by cargo tree",
        "lanes": lanes,
        "positive_controls": controls,
        "binary_files": [binary_identity(path) for path in binaries],
    }


def write_report(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def self_test() -> None:
    assert classify_path("src/lsp/client.rs") == "intelligence"
    assert classify_path("src/agent/write_guard.rs") == "execution"
    assert classify_path("src/agent/mod.rs") == "orchestration"
    assert classify_path("src/tools/output.rs") is None
    assert classify_module("crate::lsp::LspProvider") == ("crate::lsp", "intelligence")
    expanded = expand_use_tree("crate::{agent::Agent, tools::{core::ToolContext, symbol::Symbols}}")
    assert expanded == [
        "crate::agent::Agent",
        "crate::tools::core::ToolContext",
        "crate::tools::symbol::Symbols",
    ]
    fixture = """
use crate::lsp::Client;
#[cfg(test)]
mod tests { use crate::librarian::Catalog; }
fn live(ctx: &ToolContext) { crate::agent::Agent::new(); let _ = ctx.agent.project_status(); }
"""
    production, _ = production_mask(fixture)
    assert "crate::librarian" not in production
    assert "crate::lsp::Client" in production
    impl_fixture = """
impl Tool for Workspace {
    fn name(&self) -> &str { "workspace" }
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        ActivateProject.call(input, ctx).await
    }
}
impl Tool for ActivateProject {
    fn name(&self) -> &str { "activate_project" }
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        ctx.agent.activate(input).await
    }
}
"""
    workspace_impl = impl_tool_blocks(impl_fixture)[0]
    reach = reachable_context("Workspace", workspace_impl["body"], impl_fixture)
    assert reach["context_fields"] == ["agent"]
    assert any("activate" in item for item in reach["agent_reach_through"])


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("all", "static", "context", "history", "weight", "self-test"))
    parser.add_argument("--repo", type=Path, default=Path.cwd(), help="repository root (default: cwd)")
    parser.add_argument("--output", type=Path, help="write the raw JSON report to this path")
    parser.add_argument("--history-limit", type=int, default=500)
    parser.add_argument(
        "--runtime-binary",
        type=Path,
        default=Path(
            shutil.which("codescout")
            or Path(os.environ.get("CARGO_HOME") or Path.home() / ".cargo") / "bin" / "codescout"
        ),
        help="codescout executable queried with MCP tools/list",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv or sys.argv[1:])
    if args.command == "self-test":
        self_test()
        print("self-test: ok")
        return 0

    repo = args.repo.resolve()
    if not (repo / ".git").exists() and not git(repo, "rev-parse", "--git-dir").strip():
        raise ProbeError(f"not a git repository: {repo}")
    start = snapshot(repo)
    report: dict[str, Any] = {
        "schema_version": 1,
        "measurement_started": dataclasses.asdict(start),
        "frozen_populations": list(POPULATIONS),
        "blind_spots": [
            "static references created by macros, re-exports, dynamic dispatch, and unqualified calls",
            "context calls leaving a tool's source file unless the core field is read before the call",
            "feature-gated paths are parsed as source but not compiled under every feature combination",
            "runtime registry describes the supplied executable and runtime config, not necessarily HEAD",
        ],
    }
    commands = POPULATIONS if False else (args.command,)
    del commands
    report["source_basis"] = {
        "kind": "git archive",
        "head": start.head,
        "note": "Static/context source and Cargo dependency measurements use tracked bytes at this commit.",
    }
    with frozen_source_tree(repo, start.head) as frozen_root:
        if args.command in {"all", "static"}:
            report["static"] = static_measurement(frozen_root)
        if args.command in {"all", "context"}:
            report["context"] = context_measurement(
                frozen_root, repo, args.runtime_binary.resolve()
            )
        if args.command in {"all", "history"}:
            report["history"] = historical_measurement(repo, args.history_limit, start.head)
        if args.command in {"all", "weight"}:
            report["weight"] = dependency_measurement(frozen_root, args.runtime_binary.resolve())

    end = snapshot(repo)
    stable = start.head == end.head
    report["measurement_finished"] = dataclasses.asdict(end)
    report["measurement_stable"] = stable
    report["worktree_changed_during_measurement"] = start.source_digest != end.source_digest
    if args.output:
        write_report(args.output, report)
    print(json.dumps(report, indent=2, sort_keys=True))
    if not stable:
        raise ProbeError("HEAD changed during the frozen measurement")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (ProbeError, OSError, subprocess.TimeoutExpired) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(2)
