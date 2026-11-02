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
                'mirrorproxy-client-x86_64-pc-windows-msvc.exe',
                'mirrorproxy-client-aarch64-pc-windows-msvc.zip',
                'mirrorproxy-client-aarch64-pc-windows-msvc.exe',
                'mirrorproxy-client-aarch64-apple-darwin.tar.gz',
                'mirrorproxy-client-x86_64-unknown-linux-musl.tar.gz',
                'mirrorproxy_1.7.1_amd64.deb',
                'mirrorproxy-1.7.1-1.x86_64.rpm',
                'mirrorproxy-1.7.1-1.aarch64.rpm',
                'mirrorproxy-server_1.7.1_arm64.deb',
                'mirrorproxy-server-1.7.1-1.aarch64.rpm',
                'mirrorproxy-server-aarch64-unknown-linux-gnu.tar.gz',
                'mirrorproxy-server-x86_64-pc-windows-msvc.zip',
                'mirrorproxy-server-aarch64-pc-windows-msvc.zip',
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
            client, server = body.split('### Server / 服务端')
            self.assertIn('| Architecture / 架构 | Windows | macOS | Linux |', client)
            self.assertIn('| AArch64 (ARM64) | [EXE]', client)
            self.assertIn(' · [ZIP]', client)
            self.assertIn('/mirrorproxy-1.7.1-1.aarch64.rpm)', client)
            self.assertIn('| Architecture / 架构 | Windows | Linux |', server)
            self.assertNotIn('macOS', server)
            self.assertNotIn('/mirrorproxy-1.7.1-', server)
            self.assertNotIn('/mirrorproxy-server-', client)
            self.assertIn('| AArch64 (ARM64) | [ZIP]', server)
            self.assertTrue(body.endswith('# Curated notes\n\nKeep compatibility.\n'))
            nightly = module.release_body(root, 'inbjo/MirrorProxy', 'nightly', 'Rolling build')
            self.assertIn('/download/nightly/', nightly)

    def test_empty_platforms_do_not_offer_nonexistent_downloads(self):
        with tempfile.TemporaryDirectory() as directory:
            body = module.release_body(Path(directory), 'inbjo/MirrorProxy', 'nightly', '')
            client, server = body.split('### Server / 服务端')
            self.assertEqual(client.count('| — | — | — |'), 2)
            self.assertEqual(server.count('| — | — |'), 2)

    def test_old_releases_do_not_gain_unpublished_windows_links(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'mirrorproxy-client-x86_64-pc-windows-msvc.zip').touch()
            body = module.release_body(root, 'inbjo/MirrorProxy', 'v1.7.2', '')
            self.assertNotIn('[EXE]', body)
            self.assertNotIn('/mirrorproxy-client-aarch64-pc-windows-msvc', body)
            self.assertNotIn('/mirrorproxy-server-', body)


if __name__ == '__main__':
    unittest.main()
