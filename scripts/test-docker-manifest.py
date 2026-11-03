"""Negative delivery-contract cases for the merged multi-platform image."""
import copy
import importlib.util
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('docker_manifest', Path(__file__).with_name('verify-docker-manifest.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class DockerManifestTests(unittest.TestCase):
    def fixture(self):
        entries = []
        for index, arch in enumerate(['amd64', 'arm64'], 1):
            digest = 'sha256:' + str(index) * 64
            entries.append({'digest': digest, 'platform': {'os': 'linux', 'architecture': arch}})
            entries.append({'digest': 'sha256:' + str(index + 2) * 64,
                            'platform': {'os': 'unknown', 'architecture': 'unknown'},
                            'annotations': {'vnd.docker.reference.type': 'attestation-manifest', 'vnd.docker.reference.digest': digest}})
        return {'manifests': entries}

    def test_both_architectures_with_attestations(self):
        self.assertEqual(set(module.verify_manifest(self.fixture())), {'amd64', 'arm64'})

    def test_rejects_missing_wrong_duplicate_architecture_and_attestation(self):
        fixture = self.fixture()
        invalid = [
            {'manifests': fixture['manifests'][:2]},
            {'manifests': fixture['manifests'][:3]},
            {'manifests': fixture['manifests'] + [fixture['manifests'][0]]},
        ]
        wrong = copy.deepcopy(fixture)
        wrong['manifests'][2]['platform']['architecture'] = '386'
        invalid.append(wrong)
        for value in invalid:
            with self.subTest(manifest=value), self.assertRaises(ValueError):
                module.verify_manifest(value)


if __name__ == '__main__':
    unittest.main()
