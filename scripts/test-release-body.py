"""Offline contracts for the download links displayed in GitHub Releases."""
import importlib.util
from pathlib import Path
import tempfile
import sys
import unittest

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location('release_body', Path(__file__).with_name('generate-release-body.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ReleaseBodyTests(unittest.TestCase):
    def test_links_only_published_assets_and_preserves_notes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            assets = [
                'mirrorproxy-client-x86_64-pc-windows-msvc.zip',
                'mirrorproxy-client-aarch64-apple-darwin.tar.gz',
                'mirrorproxy-client-x86_64-unknown-linux-musl.tar.gz',
                'mirrorproxy_1.7.1_amd64.deb',
                'mirrorproxy-server_1.7.1_arm64.deb',
                'mirrorproxy-server-1.7.1-1.aarch64.rpm',
                'mirrorproxy-server-aarch64-unknown-linux-gnu.tar.gz',
            ]
            for name in assets:
                target = root / 'nested' / name
                target.parent.mkdir(exist_ok=True)
                target.touch()
                target.with_name(name + '.sha256').touch()
            body = module.release_body(root, 'inbjo/MirrorProxy', 'v1.7.1', '# Curated notes\n\nKeep compatibility.')
            for name in assets:
                self.assertIn(f'https://github.com/inbjo/MirrorProxy/releases/download/v1.7.1/{name})', body)
            self.assertNotIn('.sha256)', body)
            self.assertNotIn('MSI', body)
            self.assertNotIn('EXE', body)
            self.assertIn('| x86-64 (64-bit) | [ZIP]', body)
            self.assertIn('| AArch64 (ARM64) | — | [TAR.GZ]', body)
            self.assertIn('| AArch64 (ARM64) | — | — | [RPM]', body)
            self.assertTrue(body.endswith('# Curated notes\n\nKeep compatibility.\n'))
            nightly = module.release_body(root, 'inbjo/MirrorProxy', 'nightly', 'Rolling build')
            self.assertIn('/download/nightly/', nightly)

    def test_empty_platforms_do_not_offer_nonexistent_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            body = module.release_body(Path(directory), 'inbjo/MirrorProxy', 'nightly', '')
            self.assertEqual(body.count('| — | — | — |'), 4)


if __name__ == '__main__':
    unittest.main()
