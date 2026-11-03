#!/usr/bin/env python3
"""Bind shared frontend files to their build inputs and the current workflow commit."""
import argparse
import hashlib
import json
from pathlib import Path


def inventory(directory):
    if directory.is_symlink():
        raise ValueError('frontend output directory must not be a symlink')
    files = {}
    for path in sorted(directory.rglob('*')):
        if path.is_symlink():
            raise ValueError('frontend output must not contain symlinks')
        if path.is_file():
            files[path.relative_to(directory).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    if 'index.html' not in files or not any(name.endswith('.js') for name in files):
        raise ValueError('frontend build is incomplete')
    return files


def verify(directory, manifest, input_key=None, commit=None):
    state = json.loads(manifest.read_text())
    if state.get('version') != 1:
        raise ValueError('unsupported frontend manifest version')
    if input_key is not None and state.get('input_key') != input_key:
        raise ValueError('frontend build inputs changed')
    if commit is not None and state.get('commit') != commit:
        raise ValueError('frontend artifact is from a different commit')
    if state.get('files') != inventory(directory):
        raise ValueError('frontend output hashes or inventory changed')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['record', 'verify'])
    parser.add_argument('--directory', type=Path, default=Path('web/dist'))
    parser.add_argument('--manifest', type=Path, default=Path('target/frontend-artifact.json'))
    parser.add_argument('--input-key')
    parser.add_argument('--commit')
    args = parser.parse_args()
    if args.command == 'record':
        if not args.input_key or not args.commit:
            parser.error('record requires --input-key and --commit')
        args.manifest.parent.mkdir(parents=True, exist_ok=True)
        args.manifest.write_text(json.dumps({'version': 1, 'input_key': args.input_key,
                                             'commit': args.commit, 'files': inventory(args.directory)}, sort_keys=True))
    else:
        verify(args.directory, args.manifest, args.input_key, args.commit)
    print('Frontend build manifest:', args.command, 'passed')
