import hashlib
import json
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent


class ProvenanceTest(unittest.TestCase):
    def test_copied_files_match_recorded_source_hashes(self):
        prov = json.loads((HERE / "PROVENANCE.json").read_text())
        self.assertTrue(prov["files"])
        edits = prov.get("local_edits", {})
        for name, want in prov["files"].items():
            got = hashlib.sha256((HERE / name).read_bytes()).hexdigest()
            if name in edits:
                self.assertEqual(got, edits[name]["sha256"], name)
            else:
                self.assertEqual(got, want, name)


if __name__ == "__main__":
    unittest.main()
