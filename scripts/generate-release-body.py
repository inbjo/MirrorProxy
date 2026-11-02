#!/usr/bin/env python3
"""Build download tables from the artifacts actually produced by the release matrix."""
import argparse
import os
from pathlib import Path
import re
from urllib.parse import quote


def release_body(artifacts: Path, repository: str, tag: str, notes: str) -> str:
    base = f"https://github.com/{repository}/releases/download/{quote(tag, safe='')}"
    assets = sorted({p.name for p in artifacts.rglob('*') if p.is_file()})

    def cell(component: str, arch: str, system: str) -> str:
        links = []
        for name in assets:
            prefix = 'mirrorproxy-client-' if component == 'client' else 'mirrorproxy-server-'
            target = {'Linux': 'unknown-linux', 'macOS': 'apple-darwin', 'Windows': 'pc-windows'}[system]
            label = None
            if name.startswith(f'{prefix}{arch}-{target}'):
                if name.endswith('.tar.gz'):
                    label = 'TAR.GZ'
                elif name.endswith('.zip'):
                    label = 'ZIP'
            if system == 'Linux':
                deb_arch = {'x86_64': 'amd64', 'aarch64': 'arm64'}[arch]
                package = 'mirrorproxy' if component == 'client' else 'mirrorproxy-server'
                if re.fullmatch(rf'{package}_[^_]+_{deb_arch}\.deb', name):
                    label = 'DEB'
                if component == 'server' and re.fullmatch(rf'mirrorproxy-server-.+\.{arch}\.rpm', name):
                    label = 'RPM'
            if label:
                links.append(f'[{label}]({base}/{quote(name)})')
        return ' · '.join(links) or '—'

    sections = ['## Downloads / 下载',
                'Choose your operating system and architecture. / 按操作系统和架构选择下载。',
                '### Client / 客户端',
                'Configure package managers to use MirrorProxy. / 用于配置包管理器的镜像源。']
    for component in ['client', 'server']:
        if component == 'server':
            sections.extend(['### Server / 服务端',
                             'Self-host the proxy and administration console (Linux). / 自建代理服务与管理后台（Linux）。'])
        table = ['| Architecture / 架构 | Windows | macOS | Linux |', '| --- | --- | --- | --- |']
        for arch, label in [('x86_64', 'x86-64 (64-bit)'), ('aarch64', 'AArch64 (ARM64)')]:
            table.append(f'| {label} | ' + ' | '.join(cell(component, arch, system) for system in ['Windows', 'macOS', 'Linux']) + ' |')
        sections.append('\n'.join(table))
    sections.append(f'Verify downloads with [SHA256SUMS]({base}/SHA256SUMS). / 下载后请校验 SHA256SUMS。\n\n— = No package published for this platform. / 此平台暂无安装包。')
    sections.append(notes.strip())
    return '\n\n'.join(sections) + '\n'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('artifacts', type=Path)
    parser.add_argument('repository')
    parser.add_argument('tag')
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    if not args.artifacts.is_dir():
        parser.error(f'artifact directory does not exist: {args.artifacts}')
    notes_path = Path('docs/releases') / f'{args.tag}.md'
    if args.tag != 'nightly' and notes_path.is_file():
        notes = notes_path.read_text()
    else:
        notes = f"Automated rolling build from `main` at `{os.environ.get('GITHUB_SHA', 'unknown')}`. Assets are replaced after every push to `main` and by the daily scheduled build." if args.tag == 'nightly' else ''
    args.output.write_text(release_body(args.artifacts, args.repository, args.tag, notes))
