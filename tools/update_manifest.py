#!/usr/bin/env python3
"""Publish root-manager metadata alongside a completed production release."""
import argparse
import json
import re
from pathlib import Path


def manifest(props, repository):
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
        raise ValueError('Invalid GitHub repository')
    version = props['version']
    code = props['versionCode']
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', version):
        raise ValueError('Invalid release version')
    if not re.fullmatch(r'[1-9][0-9]*', code):
        raise ValueError('Invalid version code')
    base = f'https://github.com/{repository}/releases/download/v{version}'
    return {'version': version, 'versionCode': int(code),
            'zipUrl': f'{base}/butterd-{version}.zip',
            'changelog': f'{base}/changelog.txt'}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--repository', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    props = dict(line.split('=', 1) for line in Path('module/module.prop').read_text().splitlines()
                 if '=' in line and not line.startswith('#'))
    result = manifest(props, args.repository)
    archive = args.output.parent / f"butterd-{result['version']}.zip"
    if not archive.is_file() or not archive.stat().st_size:
        parser.error('Production module ZIP must exist before generating updater metadata')
    args.output.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
