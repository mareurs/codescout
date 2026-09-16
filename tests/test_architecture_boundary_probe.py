"""Behavioral controls for the architecture measurement instrument."""

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/architecture-boundary-probe.py"
SPEC = importlib.util.spec_from_file_location("architecture_boundary_probe", SCRIPT)
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)


class StaticControls(unittest.TestCase):
    def test_external_imports_do_not_invent_local_edges(self):
        report = self.measure({
            "src/ast/mod.rs": "use anyhow::Result; use serde::Serialize; use tokio::task;",
        })
        self.assertEqual([], [edge for edge in report["raw_edges"]
                              if edge["source_file"] == "src/ast/mod.rs"])
        self.assertEqual("anyhow::Result", probe.resolve_relative("anyhow::Result", "src/ast/mod.rs"))
        self.assertEqual("crate::lsp::Thing", probe.resolve_relative("super::Thing", "src/lsp/client.rs"))
        self.assertEqual("crate::lsp::client::Thing", probe.resolve_relative("self::Thing", "src/lsp/client.rs"))

    def test_alias_removal_preserves_identifier_bytes(self):
        self.assertEqual(
            ["crate::lsp::base", "crate::lsp::Task", "crate::lsp::Alias"],
            probe.expand_use_tree("crate::lsp::{base, Task as Local, Alias as Renamed}"),
        )

    def measure(self, files):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            # A real registered-tool import and construction exercises the probe's control.
            files = {"src/server.rs": "use crate::tools::symbol::Symbols;\nfn register() { Arc::new(Symbols); }", **files}
            for relative, body in files.items():
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(body)
            return probe.static_measurement(root)

    def test_detects_long_cycle_without_reciprocal_edges(self):
        result = self.measure({
            "src/lsp/mod.rs": "use crate::tools::read_file::ReadFile;",
            "src/tools/read_file.rs": "use crate::memory::MemoryStore;",
            "src/memory/mod.rs": "use crate::lsp::LspProvider;",
        })
        self.assertEqual(result["population_cycles"], [["execution", "knowledge", "intelligence"]])
        self.assertEqual(result["directed_edge_counts"]["execution->knowledge"], 1)

    def test_detects_reciprocal_pair_once(self):
        result = self.measure({
            "src/lsp/mod.rs": "use crate::memory::MemoryStore;",
            "src/memory/mod.rs": "use crate::lsp::LspProvider;",
        })
        self.assertEqual(result["population_cycles"], [["intelligence", "knowledge"]])

    def test_dag_and_test_only_reverse_edge_do_not_make_a_cycle(self):
        result = self.measure({
            "src/lsp/mod.rs": "use crate::memory::MemoryStore;",
            "src/memory/mod.rs": '#[cfg(test)] mod tests { use crate::lsp::LspProvider; }',
        })
        self.assertEqual(result["population_cycles"], [])
        self.assertEqual(result["directed_edge_counts"]["knowledge->intelligence"], 0)
        self.assertEqual(result["directed_edge_counts"]["intelligence->knowledge"], 1)

    def test_edge_dedup_and_module_granularity(self):
        result = self.measure({
            "src/lsp/mod.rs": "use crate::tools::{read_file::ReadFile, symbol::Symbols};\nfn f() { crate::tools::read_file::read(); }",
        })
        self.assertEqual(result["directed_edge_counts"]["intelligence->execution"], 1)
        self.assertEqual(result["directed_edge_counts"]["intelligence->intelligence"], 1)

    def test_comment_and_literal_paths_never_become_edges(self):
        result = self.measure({
            "src/lsp/mod.rs": '/* nested /* use crate::memory::Store; */ comment */\nconst S: &str = r#"crate::memory::store()"#;\nuse crate::tools::read_file::ReadFile;',
        })
        self.assertEqual(result["directed_edge_counts"]["intelligence->knowledge"], 0)
        self.assertEqual(result["directed_edge_counts"]["intelligence->execution"], 1)

    def test_unrelated_arc_is_not_a_registration_control(self):
        with self.assertRaises(probe.ProbeError):
            self.measure({"src/server.rs": "fn f() { Arc::new(UnrelatedState); }"})


class ContextControls(unittest.TestCase):
    def test_test_only_field_does_not_erase_constructor_delimiters(self):
        source = '''
struct State {
    #[cfg(test)]
    seen: Option<Arc<Thing>>,
    live: bool,
}
impl State {
    fn new() -> Self {
        Self {
            #[cfg(test)]
            seen: Some(Arc::new(Thing {})),
            live: true,
        }
    }
}
fn after(ctx: &ToolContext) { ctx.lsp.shutdown_all(); }
'''
        production, _ = probe.production_mask(source)
        bodies = probe.named_function_bodies(production)
        self.assertIn("live: true", bodies["new"][0])
        self.assertNotIn("seen:", production)
        self.assertIn("ctx.lsp.shutdown_all()", bodies["after"][0])

    def trace(self, source, tool_type="Workspace"):
        production, _ = probe.production_mask(source)
        impl = next(row for row in probe.impl_tool_blocks(production) if row["type"] == tool_type)
        return probe.reachable_context(tool_type, impl["body"], production)

    def test_multiline_access_and_alias_method_are_observed(self):
        result = self.trace('''
impl Tool for Workspace {
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let agent = ctx
            .agent.clone();
        agent
            .project_status().await;
        ctx
            .lsp.notify_file_changed(path).await;
    }
}
''')
        self.assertEqual(result["context_fields"], ["agent", "lsp"])
        methods = {json.loads(row)["method_or_field"] for row in result["agent_reach_through"]}
        self.assertIn("project_status", methods)

    def test_unresolved_helpers_exclude_syntax_and_self_receiver_methods(self):
        # Three names sit in call position without being same-file helpers, and no
        # enumerated denylist reaches all three:
        #   `let (a, b) = ...`  a KEYWORD in call position (15 of 15 tools, live)
        #   `split(...)`        a name with no definition anywhere
        #   `drop(guard)`       std::mem::drop, shadowed by trait-impl `fn drop(&mut self)`
        # Only `ambiguous` is a real unresolved route: two free definitions, so the
        # probe cannot pick one. Asserted by EQUALITY rather than assertNotIn, so this
        # reds on over-reporting AND on losing the genuine member.
        # docs/issues/2026-09-16-probe-counts-rust-keywords-as-unresolved-helpers.md
        result = self.trace('''
impl Tool for Workspace {
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let (head, tail) = split(ctx.agent.name());
        drop(guard);
        ambiguous(ctx);
        unique_helper(ctx);
    }
}
impl Drop for FirstGuard {
    fn drop(&mut self) { self.release(); }
}
impl Drop for SecondGuard {
    fn drop(&mut self) { self.release(); }
}
fn ambiguous(ctx: &ToolContext) -> usize { ctx.lsp.count() }
fn ambiguous(ctx: &ToolContext, n: usize) -> usize { n }
fn unique_helper(ctx: &ToolContext) { ctx.output_buffer.clear(); }
''')
        self.assertEqual(result["unresolved_same_file_helpers"], ["ambiguous"])
        # Control: the uniquely-resolvable helper is still FOLLOWED, so the fix
        # narrows the report without disabling traversal. Without this line, a
        # change that stopped resolving free calls at all would pass the one above.
        self.assertIn("output_buffer", result["context_fields"])

    def test_a_free_call_does_not_resolve_to_a_trait_method_of_the_same_name(self):
        # The quieter half of the same defect, and the more expensive one. With ONE
        # `impl Drop`, `fn drop` resolves UNIQUELY, so the probe follows it and
        # attributes that body's ctx reads to the tool -- inflating the measured
        # blast radius rather than the unresolved list, where nothing flags it.
        # A free `drop(x)` can never reach `fn drop(&mut self)`; the receiver is
        # what rules it out, and no name-based list can.
        result = self.trace('''
impl Tool for Workspace {
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        drop(guard);
        ctx.agent.project_status();
    }
}
impl Drop for Guard {
    fn drop(&mut self) { ctx.lsp.shutdown_all(); }
}
''')
        self.assertEqual(result["context_fields"], ["agent"])

    def test_a_self_method_call_is_followed_through_the_type_index(self):
        # Pins `named_function_bodies`' free_only DEFAULT, which nothing else does.
        # `methods_for_type` uses that index to resolve `self.f()`, and every method
        # it wants has a `self` receiver -- so flipping the default to True empties
        # it and silently kills this whole resolution path. Found by mutation: the
        # flip left 16 of 16 tests green, because no fixture exercised `self.f()`.
        # docs/issues/2026-09-16-probe-counts-rust-keywords-as-unresolved-helpers.md
        result = self.trace('''
impl Tool for Workspace {
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        self.helper(ctx)
    }
}
impl Workspace {
    fn helper(&self, ctx: &ToolContext) { ctx.lsp.shutdown_all(); }
}
''')
        self.assertEqual(result["context_fields"], ["lsp"])

    def test_direct_and_delegated_reads_are_separate(self):
        result = self.trace('''
impl Tool for Workspace {
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        ctx.agent.project_status();
        ActivateProject.call(input, ctx).await
    }
}
impl Tool for ActivateProject {
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        ctx.lsp.shutdown_all().await
    }
}
''')
        self.assertEqual(result["direct_context_fields"], ["agent"])
        self.assertEqual(result["context_fields"], ["agent", "lsp"])


class WeightControls(unittest.TestCase):
    def test_deployed_lane_retains_defaults_and_both_release_features(self):
        calls = []

        def cargo(args, repo, **kwargs):
            calls.append(args)
            features = args[args.index("--features") + 1] if "--features" in args else ""
            packages = ["codescout v0.15.0"]
            if "--no-default-features" not in args:
                packages += ["serde_yml v0.0.12", "pulldown-cmark v0.13.0", "jsonschema v0.33.0"]
            if "server-stack" in features:
                packages += ["qdrant-client v1.15.0"]
            return subprocess.CompletedProcess(args, 0, "\n".join(packages), "")

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(probe, "run", side_effect=cargo):
                result = probe.dependency_measurement(root, root / "absent-binary")
        self.assertIn("deployed", result["lanes"])
        deployed = next(args for args in calls if "server-stack,local-embed" in args)
        self.assertNotIn("--no-default-features", deployed)


class ActionControls(unittest.TestCase):
    def test_actions_have_distinct_direct_fields_and_keep_shared_reads_separate(self):
        source = '''
impl Tool for Example {
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        ctx.agent.project_status();
        match action {
            "read" => { ctx.lsp.get_or_start(); },
            "write" => { ctx.output_buffer.store_tool(); },
            _ => Err(error),
        }
    }
}
'''
        result = probe.action_branches(source, "Example", ["read", "write", "missing"])
        self.assertEqual(result["read"]["branch_direct_fields"], ["lsp"])
        self.assertEqual(result["write"]["branch_direct_fields"], ["output_buffer"])
        self.assertEqual(result["read"]["shared_prefix_direct_fields"], ["agent"])
        self.assertEqual(result["missing"]["resolution"], "unresolved")

    def test_librarian_action_handler_is_located_without_following_other_actions(self):
        source = '''
impl Tool for Artifact {
    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        match action {
            "find" => super::find::call(ctx, args).await,
            "update" => super::update::call(ctx, args).await,
            _ => Err(error),
        }
    }
}
'''
        result = probe.action_branches(source, "Artifact", ["find", "update"])
        self.assertEqual(result["find"]["delegate"], "super::find::call")
        self.assertEqual(result["update"]["delegate"], "super::update::call")


if __name__ == "__main__":
    unittest.main()
