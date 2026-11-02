#!/usr/bin/env python3
"""Build and inspect real client packages without installing them on the host."""
from pathlib import Path
import subprocess
import sys
import tempfile


def main():
    binary = Path(sys.argv[1]).resolve()
    arch = sys.argv[2] if len(sys.argv) > 2 else 'x86_64'
    deb_arch = {'x86_64': 'amd64', 'aarch64': 'arm64'}[arch]
    version = '1.7.3'
    scripts = Path(__file__).resolve().parent
    with tempfile.TemporaryDirectory(prefix='mirrorproxy-packages-') as directory:
        root = Path(directory)
        for kind in ['deb', 'rpm']:
            subprocess.run(['bash', str(scripts / f'package-client-{kind}.sh'),
                            str(binary), version, arch, str(root)], check=True)
        deb = root / f'mirrorproxy_{version}_{deb_arch}.deb'
        rpm = root / f'mirrorproxy-{version}-1.{arch}.rpm'
        rpm_query = ['rpm', '--dbpath', str(root / 'rpmdb'), '-qp']
        fields = subprocess.check_output(['dpkg-deb', '-f', str(deb), 'Package', 'Version', 'Architecture'], text=True)
        assert fields == f'Package: mirrorproxy\nVersion: {version}\nArchitecture: {deb_arch}\n', fields
        fields = subprocess.check_output(rpm_query + ['--qf', '%{NAME}\n%{VERSION}\n%{ARCH}\n', str(rpm)], text=True)
        assert fields == f'mirrorproxy\n{version}\n{arch}\n', fields
        assert subprocess.check_output(rpm_query + ['--scripts', str(rpm)]) == b''
        deb_root = root / 'deb'
        subprocess.run(['dpkg-deb', '-x', str(deb), str(deb_root)], check=True)
        rpm_root = root / 'rpm'
        rpm_root.mkdir()
        payload = subprocess.check_output(['rpm2cpio', str(rpm)])
        subprocess.run(['cpio', '-idm', '--quiet', '--no-absolute-filenames'], input=payload, cwd=rpm_root, check=True)
        for unpacked in [deb_root, rpm_root]:
            installed = unpacked / 'usr/bin/mirrorproxy'
            assert installed.read_bytes() == binary.read_bytes(), installed
            assert installed.stat().st_mode & 0o777 == 0o755, installed
            files = sorted(p.relative_to(unpacked).as_posix() for p in unpacked.rglob('*') if p.is_file() or p.is_symlink())
            assert files == ['usr/bin/mirrorproxy'], files
        print(f'{arch} DEB/RPM metadata, exact executable payload, mode and absence of install scripts verified')


if __name__ == '__main__':
    main()
