"""Reject stale, corrupt or incomplete frontend cache/artifact payloads."""
import importlib.util
import json
from pathlib import Path
import tempfile
import sys
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('frontend_manifest', Path(__file__).with_name('frontend-build-manifest.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class FrontendManifestTests(unittest.TestCase):
    def test_binds_inputs_commit_and_complete_output_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            dist = root / 'dist'
            dist.mkdir()
            (dist / 'index.html').write_text('<script src="app.js"></script>')
            (dist / 'app.js').write_text('original-build')
            manifest = root / 'manifest.json'
            manifest.write_text(json.dumps({'version': 1, 'input_key': 'inputs', 'commit': 'commit-a', 'files': module.inventory(dist)}))
            module.verify(dist, manifest, 'inputs', 'commit-a')
            for key, commit in [('changed-inputs', 'commit-a'), ('inputs', 'commit-b')]:
                with self.assertRaises(ValueError):
                    module.verify(dist, manifest, key, commit)
            (dist / 'app.js').write_text('corrupt-build')
            with self.assertRaises(ValueError):
                module.verify(dist, manifest, 'inputs', 'commit-a')
            (dist / 'app.js').write_text('original-build')
            (dist / 'extra.js').write_text('unexpected')
            with self.assertRaises(ValueError):
                module.verify(dist, manifest)
            (dist / 'extra.js').unlink()
            (dist / 'index.html').unlink()
            with self.assertRaises(ValueError):
                module.verify(dist, manifest)


if __name__ == '__main__':
    unittest.main()
