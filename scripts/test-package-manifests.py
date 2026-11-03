"""Exercise generated multi-architecture package-manager metadata offline."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class ManifestTests(unittest.TestCase):
    def test_architectures_keep_their_own_urls_and_checksums(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifacts = root / 'artifacts'
            artifacts.mkdir()
            targets = [
                'x86_64-unknown-linux-musl.tar.gz',
                'x86_64-apple-darwin.tar.gz',
                'aarch64-apple-darwin.tar.gz',
                'x86_64-pc-windows-msvc.zip',
                'aarch64-pc-windows-msvc.zip',
            ]
            hashes = {}
            for index, target in enumerate(targets, 1):
                name = 'mirrorproxy-client-' + target
                hashes[name] = f'{index:064x}'
                (artifacts / (name + '.sha256')).write_text(f'{hashes[name]}  {name}\n')
            output = root / 'manifests'
            subprocess.run(['bash', str(Path(__file__).with_name('generate-package-manifests.sh')),
                            '1.7.3', str(artifacts), str(output)], check=True,
                           env={**os.environ, 'MIRRORPROXY_RELEASE_DATE': '2026-11-02'})
            scoop = json.loads((output / 'mirrorproxy.json').read_text())
            winget = (output / 'winget/manifests/i/Inbjo/MirrorProxy/1.7.3/Inbjo.MirrorProxy.installer.yaml').read_text()
            self.assertEqual(set(scoop['architecture']), {'64bit', 'arm64'})
            common = winget.split('\nInstallers:\n')[0]
            self.assertIn('\nNestedInstallerFiles:\n- RelativeFilePath: mirrorproxy.exe\n  PortableCommandAlias: mirrorproxy\n', common)
            self.assertIn('ReleaseDate: 2026-11-02', common)
            self.assertIn('ManifestVersion: 1.10.0', winget)
            for scoop_arch, winget_arch, target in [('64bit', 'x64', targets[3]), ('arm64', 'arm64', targets[4])]:
                name = 'mirrorproxy-client-' + target
                self.assertTrue(scoop['architecture'][scoop_arch]['url'].endswith('/' + name))
                self.assertEqual(scoop['architecture'][scoop_arch]['hash'], hashes[name])
                self.assertIn('/v$version/' + name, scoop['autoupdate']['architecture'][scoop_arch]['url'])
                entry = winget.split('- Architecture: ' + winget_arch + '\n')[1].split('- Architecture:')[0]
                self.assertIn('/' + name, entry)
                self.assertIn('InstallerSha256: ' + hashes[name].upper(), entry)
                self.assertIn(f'PackageIdentifier: Microsoft.VCRedist.2015+.{winget_arch}', entry)
                self.assertNotIn('NestedInstallerFiles:', entry)
            locale = (output / 'winget/manifests/i/Inbjo/MirrorProxy/1.7.3/Inbjo.MirrorProxy.locale.en-US.yaml').read_text()
            self.assertIn('ReleaseNotesUrl: https://github.com/inbjo/MirrorProxy/releases/tag/v1.7.3', locale)
            self.assertIn('Documentations:\n- DocumentLabel: Wiki\n  DocumentUrl: https://github.com/inbjo/MirrorProxy/wiki', locale)
            brew = (output / 'mirrorproxy.rb').read_text()
            for target in targets[:3]:
                self.assertIn('/mirrorproxy-client-' + target, brew)


if __name__ == '__main__':
    unittest.main()
