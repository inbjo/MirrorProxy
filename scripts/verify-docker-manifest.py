#!/usr/bin/env python3
"""Require both Linux architectures and their linked build attestations."""
import json
from pathlib import Path
import re
import sys


def verify_manifest(manifest):
    images = {}
    attestations = set()
    for entry in manifest.get('manifests', []):
        digest = entry.get('digest', '')
        if not re.fullmatch(r'sha256:[a-f0-9]{64}', digest):
            raise ValueError('invalid manifest digest')
        annotations = entry.get('annotations', {})
        if annotations.get('vnd.docker.reference.type') == 'attestation-manifest':
            attestations.add(annotations.get('vnd.docker.reference.digest'))
            continue
        platform = entry.get('platform', {})
        arch = platform.get('architecture')
        if platform.get('os') != 'linux' or arch not in ('amd64', 'arm64') or arch in images:
            raise ValueError('expected exactly one Linux amd64 and one Linux arm64 image')
        images[arch] = digest
    if set(images) != {'amd64', 'arm64'}:
        raise ValueError('missing image architecture')
    if not set(images.values()) <= attestations:
        raise ValueError('missing linked build attestation')
    return images


if __name__ == '__main__':
    images = verify_manifest(json.loads(Path(sys.argv[1]).read_text()))
    print('Verified multi-platform manifest and attestations:', json.dumps(images, sort_keys=True))
