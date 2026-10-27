"""Tests for stack: the study's own retrieval services, never the operator's live stack.

Every test drives stack through a fake `docker` that records argv lists and returns canned
results; no real container is started.
"""
import subprocess
import unittest
from types import SimpleNamespace

import stack

ALL_ON = {"retrieval": {"qdrant": True, "dense": True, "sparse": True, "reranker": False}}
NETWORK = "sab-net-test"
# The URL-valued keys of the measured kat/sdd profile env (2026-10-27).
LIVE_URL_ENV = {
    "CODESCOUT_QDRANT_URL": "http://127.0.0.1:6334",
    "CODESCOUT_EMBEDDER_URL": "http://127.0.0.1:48081",
    "CODESCOUT_EMBED_URL": "http://127.0.0.1:48081/v1",
    "LIBRARIAN_EMBED_URL": "http://127.0.0.1:48081/v1",
    "CODESCOUT_SPARSE_EMBEDDER_URL": "http://127.0.0.1:48084",
    "CODESCOUT_RERANKER_URL": "http://127.0.0.1:48083",
}


class FakeDocker:
    """Records every docker argv; answers the read-only queries stack makes."""

    def __init__(self, containers=(), volumes=(), internal=True, unready=(), exited=()):
        self.calls = []
        self.containers = list(containers)
        self.volumes = set(volumes)
        self.internal = internal
        self.unready = set(unready)
        self.exited = set(exited)

    def __call__(self, *args, **kwargs):
        argv = [str(a) for a in args]
        self.calls.append(argv)
        out, rc = "", 0
        if argv[:2] == ["network", "inspect"]:
            out = "true\n" if self.internal else "false\n"
        elif argv[:2] == ["volume", "inspect"]:
            rc = 0 if argv[2] in self.volumes else 1
        elif argv[:2] == ["volume", "create"]:
            self.volumes.add(argv[2])
        elif argv[:2] == ["volume", "ls"]:
            out = "".join(v + "\n" for v in sorted(self.volumes))
        elif argv[:2] == ["ps", "-a"]:
            out = "".join(c + "\n" for c in self.containers)
        elif argv[0] == "inspect":
            out = "false\n" if argv[-1] in self.exited else "true\n"
        elif argv[0] == "exec":
            rc = 1 if argv[1] in self.unready else 0
        if kwargs.get("check", True) and rc:
            raise RuntimeError(f"docker failed (exit {rc})")
        return SimpleNamespace(returncode=rc, stdout=out)

    def runs(self):
        return [c for c in self.calls if c[0] == "run"]

    def run_named(self, name):
        found = [c for c in self.runs() if "--name" in c and c[c.index("--name") + 1] == name]
        return found[0] if found else None


class FakeClock:
    def __init__(self):
        self.now = 0.0

    def __call__(self):
        return self.now

    def sleep(self, s):
        self.now += s


def mounts(argv):
    return [argv[i + 1] for i, a in enumerate(argv) if a == "-v"]


class StackUpTest(unittest.TestCase):
    def up(self, manifest=ALL_ON, **fake):
        d = FakeDocker(**fake)
        clock = FakeClock()
        urls = stack.stack_up(NETWORK, manifest, docker=d, clock=clock, sleep=clock.sleep)
        return d, urls

    def test_absent_service_is_not_started(self):
        d, urls = self.up()
        self.assertIsNone(d.run_named("sab-reranker"))
        self.assertFalse(any("rerank" in c for argv in d.runs() for c in argv))
        self.assertNotIn("CODESCOUT_RERANKER_URL", urls)
        self.assertFalse(any("rerank" in u for u in urls.values()))

    def test_service_marked_off_is_not_started_nor_returned(self):
        manifest = {"retrieval": {"qdrant": True, "dense": True, "sparse": False, "reranker": False}}
        d, urls = self.up(manifest)
        self.assertIsNone(d.run_named("sab-sparse"))
        self.assertNotIn("CODESCOUT_SPARSE_EMBEDDER_URL", urls)
        self.assertIsNotNone(d.run_named("sab-dense"))

    def test_reranker_marked_present_is_refused_not_invented(self):
        manifest = {"retrieval": {"qdrant": True, "dense": True, "sparse": True, "reranker": True}}
        with self.assertRaisesRegex(ValueError, "reranker"):
            stack.stack_up(NETWORK, manifest, docker=FakeDocker())

    def test_no_url_points_at_the_host_stack(self):
        _, urls = self.up()
        self.assertTrue(urls)
        for key, url in urls.items():
            for host in ("127.0.0.1", "localhost", "host.docker.internal"):
                self.assertNotIn(host, url, key)
            self.assertRegex(url, r"^http://sab-[a-z-]+:\d+(/v1)?$", key)

    def test_every_live_url_key_but_the_absent_reranker_has_a_study_url_of_the_same_shape(self):
        _, urls = self.up()
        expected = set(LIVE_URL_ENV) - {"CODESCOUT_RERANKER_URL"}
        self.assertEqual(set(urls), expected)
        for key in expected:
            self.assertEqual(urls[key].endswith("/v1"), LIVE_URL_ENV[key].endswith("/v1"), key)
        self.assertEqual(urls["CODESCOUT_QDRANT_URL"], "http://sab-qdrant:6334")
        self.assertEqual(urls["CODESCOUT_EMBEDDER_URL"], "http://sab-dense:8080")
        self.assertEqual(urls["CODESCOUT_EMBED_URL"], "http://sab-dense:8080/v1")
        self.assertEqual(urls["CODESCOUT_SPARSE_EMBEDDER_URL"], "http://sab-sparse:80")

    def test_every_container_joins_the_study_network_and_publishes_no_port(self):
        d, _ = self.up()
        for argv in d.runs():
            self.assertIn(("--network", NETWORK) if "--rm" not in argv else ("--network", "none"),
                          list(zip(argv, argv[1:])), argv)
            self.assertNotIn("-p", argv)
            self.assertNotIn("--publish", argv)

    def test_a_network_that_is_not_internal_is_refused(self):
        d = FakeDocker(internal=False)
        with self.assertRaisesRegex(RuntimeError, "internal"):
            stack.stack_up(NETWORK, ALL_ON, docker=d)
        self.assertEqual(d.runs(), [])

    def test_dense_copies_the_live_dense_container(self):
        d, _ = self.up()
        argv = d.run_named("sab-dense")
        self.assertEqual(argv[-22], "rocm/llama.cpp:llama.cpp-b6652.amd0_rocm7.0.0_ubuntu24.04_server"
                         "@sha256:be175932c3c96e882dfbc7e20e0e834f58c89c2925f48b222837ee929dfc47ee")
        self.assertEqual(argv[-21:], [
            "--model", "/models/CodeRankEmbed-Q4_K_M.gguf", "--host", "0.0.0.0", "--port", "8080",
            "--embedding", "--pooling", "mean", "--ctx-size", "65536", "--batch-size", "4096",
            "--ubatch-size", "4096", "--parallel", "16", "--n-gpu-layers", "999",
            "--flash-attn", "on"])
        pairs = list(zip(argv, argv[1:]))
        for pair in [("--entrypoint", "/app/llama-server"), ("--device", "/dev/kfd"),
                     ("--device", "/dev/dri"), ("--group-add", "video"),
                     ("--group-add", "render"), ("-e", "LLAMA_ARG_HOST=0.0.0.0")]:
            self.assertIn(pair, pairs)
        self.assertEqual(mounts(argv), ["/home/marius/models:/models:ro"])

    def test_sparse_reads_a_readonly_copy_of_the_model_cache_never_the_live_volume(self):
        d, _ = self.up()
        argv = d.run_named("sab-sparse")
        self.assertEqual(argv[-8], "ghcr.io/huggingface/text-embeddings-inference:cpu-1.8@sha256:"
                                   "8de25e75ce39617f17f2f6c77d60a4f75b65e779ed5005420eb1400072a15c1c")
        self.assertEqual(mounts(argv), ["sab-model-cache:/data:ro"])
        pairs = list(zip(argv, argv[1:]))
        for pair in [("--entrypoint", "text-embeddings-router"), ("-e", "HF_HUB_OFFLINE=1"),
                     ("-e", "HUGGINGFACE_HUB_CACHE=/data"), ("-e", "PORT=80")]:
            self.assertIn(pair, pairs)
        self.assertEqual(argv[-7:], ["--model-id", "prithivida/Splade_PP_en_v1", "--pooling",
                                     "splade", "--dtype", "float32", "--auto-truncate"])
        # The live volume is only ever read, read-only, by the short-lived copy helper.
        live = [a for a in d.calls if any("codescout-retrieval_model_cache" in x for x in a)]
        self.assertEqual(len(live), 1)
        self.assertIn("--rm", live[0])
        self.assertIn("codescout-retrieval_model_cache:/s:ro", live[0])
        self.assertIn("sab-model-cache:/d", live[0])

    def test_model_cache_copy_happens_once(self):
        d, _ = self.up(volumes={"sab-model-cache"})
        self.assertFalse(any("codescout-retrieval_model_cache" in x for a in d.calls for x in a))
        self.assertNotIn(["volume", "create", "sab-model-cache"], d.calls)
    def test_a_failed_cache_copy_leaves_no_half_populated_cache(self):
        d = FakeDocker()
        real = d.__call__

        def failing_copy(*args, **kwargs):
            if args[0] == "run" and "--rm" in args:
                d.calls.append([str(a) for a in args])
                raise RuntimeError("docker failed (exit 1): no space left")
            return real(*args, **kwargs)
        with self.assertRaisesRegex(RuntimeError, "no space"):
            stack.stack_up(NETWORK, ALL_ON, docker=failing_copy)
        self.assertIn(["volume", "rm", "sab-model-cache"], d.calls)
        self.assertIsNone(d.run_named("sab-sparse"))

    def test_qdrant_starts_on_a_fresh_study_volume(self):
        d, _ = self.up()
        argv = d.run_named("sab-qdrant")
        self.assertIn("qdrant/qdrant:v1.17.0@sha256:"
                      "f1c7272cdac52b38c1a0e89313922d940ba50afd90d593a1605dbbc214e66ffb", argv)
        self.assertEqual(mounts(argv), ["sab-qv-index:/qdrant/storage"])
        self.assertIn(["volume", "create", "sab-qv-index"], d.calls)
        with self.assertRaisesRegex(RuntimeError, "sab-qv-index"):
            self.up(volumes={"sab-qv-index"})

    def test_an_unready_service_times_out_naming_it(self):
        clock = FakeClock()
        d = FakeDocker(unready={"sab-dense"})
        with self.assertRaisesRegex(RuntimeError, "sab-dense"):
            stack.stack_up(NETWORK, ALL_ON, docker=d, clock=clock, sleep=clock.sleep, timeout=30)
        self.assertGreaterEqual(clock.now, 30)
        self.assertLess(clock.now, 40)

    def test_a_service_that_exits_fails_fast_naming_it(self):
        clock = FakeClock()
        d = FakeDocker(unready={"sab-sparse"}, exited={"sab-sparse"})
        with self.assertRaisesRegex(RuntimeError, "sab-sparse.*exited"):
            stack.stack_up(NETWORK, ALL_ON, docker=d, clock=clock, sleep=clock.sleep, timeout=300)
        self.assertLess(clock.now, 300)

    def test_a_probe_that_hangs_counts_as_not_ready(self):
        clock = FakeClock()
        d = FakeDocker()
        real = d.__call__

        def hanging(*args, **kwargs):
            if args[0] == "exec" and args[1] == "sab-qdrant":
                d.calls.append([str(a) for a in args])
                raise subprocess.TimeoutExpired(["docker", *args], 10)
            return real(*args, **kwargs)
        with self.assertRaisesRegex(RuntimeError, "sab-qdrant"):
            stack.stack_up(NETWORK, ALL_ON, docker=hanging, clock=clock, sleep=clock.sleep,
                           timeout=20)


class QdrantForRunTest(unittest.TestCase):
    def start(self, d, template, run):
        clock = FakeClock()
        return stack.qdrant_for_run(template, run, network=NETWORK, docker=d, clock=clock,
                                    sleep=clock.sleep)

    def test_each_run_gets_its_own_qdrant_volume_copied_from_template(self):
        d = FakeDocker(volumes={"sab-qtemplate"})
        url_a = self.start(d, "sab-qtemplate", "a1")
        url_b = self.start(d, "sab-qtemplate", "b2")
        self.assertEqual(url_a, "http://sab-qdrant-a1:6334")
        self.assertEqual(url_b, "http://sab-qdrant-b2:6334")
        for run in ("a1", "b2"):
            vol = f"sab-qv-{run}"
            self.assertIn(["volume", "create", vol], d.calls)
            copy = [c for c in d.runs() if f"{vol}:/d" in c]
            self.assertEqual(len(copy), 1)
            self.assertIn("sab-qtemplate:/s:ro", copy[0])
            self.assertIn("--rm", copy[0])
            self.assertEqual(copy[0][-3:], ["-a", "/s/.", "/d/"])
            q = d.run_named(f"sab-qdrant-{run}")
            self.assertEqual(mounts(q), [f"{vol}:/qdrant/storage"])
            self.assertIn(("--network", NETWORK), list(zip(q, q[1:])))
            # The copy finishes before Qdrant opens the volume.
            self.assertLess(d.calls.index(copy[0]), d.calls.index(q))
        self.assertIsNone(d.run_named("sab-qdrant"))

    def test_a_run_never_reuses_an_existing_volume(self):
        d = FakeDocker(volumes={"sab-qtemplate", "sab-qv-a1"})
        with self.assertRaisesRegex(RuntimeError, "sab-qv-a1"):
            self.start(d, "sab-qtemplate", "a1")
        self.assertEqual(d.runs(), [])

    def test_a_live_volume_is_never_a_template(self):
        d = FakeDocker(volumes={"codescout-retrieval_qdrant_storage"})
        with self.assertRaisesRegex(ValueError, "sab-"):
            self.start(d, "codescout-retrieval_qdrant_storage", "a1")
        self.assertEqual(d.calls, [])

    def test_a_missing_template_is_refused(self):
        d = FakeDocker()
        with self.assertRaisesRegex(RuntimeError, "sab-qtemplate"):
            self.start(d, "sab-qtemplate", "a1")
        self.assertEqual(d.runs(), [])

    def test_a_run_name_that_is_not_a_plain_token_is_refused(self):
        for bad in ("", "../x", "a b", "A:1", "-x"):
            with self.assertRaises(ValueError, msg=bad):
                self.start(FakeDocker(volumes={"sab-qtemplate"}), "sab-qtemplate", bad)


class StackDownTest(unittest.TestCase):
    def test_stack_down_never_touches_foreign_containers(self):
        d = FakeDocker(containers=["pbr27-x", "rae-y", "codescout-qdrant", "sab-qdrant",
                                   "x-sab-qdrant"])
        stack.stack_down(docker=d)
        removed = [n for c in d.calls if c[:2] == ["rm", "-f"] for n in c[2:]]
        self.assertEqual(removed, ["sab-qdrant"])

    def test_stack_down_removes_per_run_volumes_only(self):
        d = FakeDocker(containers=["sab-qdrant-a1"],
                       volumes={"sab-qv-a1", "sab-qv-index", "sab-qtemplate", "sab-model-cache",
                                "codescout-retrieval_qdrant_storage", "x-sab-qv-1"})
        stack.stack_down(docker=d)
        removed = [n for c in d.calls if c[:2] == ["volume", "rm"] for n in c[2:]]
        self.assertEqual(sorted(removed), ["sab-qv-a1", "sab-qv-index"])
        # Containers go first: a volume in use cannot be removed.
        rm_c = next(i for i, c in enumerate(d.calls) if c[:2] == ["rm", "-f"])
        rm_v = next(i for i, c in enumerate(d.calls) if c[:2] == ["volume", "rm"])
        self.assertLess(rm_c, rm_v)

    def test_nothing_to_remove_issues_no_rm(self):
        d = FakeDocker(containers=["rae-y"], volumes={"codescout-x"})
        stack.stack_down(docker=d)
        self.assertFalse(any(c[0] == "rm" or c[:2] == ["volume", "rm"] for c in d.calls))

    def test_a_prefix_outside_the_study_namespace_is_refused(self):
        for bad in ("", "s", "sa", "codescout-", "rae-"):
            with self.assertRaises(ValueError, msg=bad):
                stack.stack_down(bad, docker=FakeDocker(containers=["codescout-qdrant"]))


if __name__ == "__main__":
    unittest.main()
