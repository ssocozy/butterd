#!/usr/bin/env python3
"""Describe changes since the latest production tag reachable from this build."""
import argparse
import os
import re
import subprocess
from pathlib import Path


def git(*args):
    return subprocess.check_output(['git', *args], text=True).strip()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--version', required=True)
    parser.add_argument('--channel', choices=['production', 'development'], required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', args.version):
        parser.error('invalid module version')
    repository = os.environ.get('GITHUB_REPOSITORY', 'ssocozy/butterd')
    if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', repository):
        parser.error('invalid repository')
    url = f'https://github.com/{repository}'
    head = git('rev-parse', 'HEAD')
    tags = git('tag', '--merged', head, '--sort=-version:refname').splitlines()
    previous = next((tag for tag in tags if re.fullmatch(r'v[0-9]+\.[0-9]+\.[0-9]+', tag) and tag != f'v{args.version}'), None)
    revisions = f'{previous}..{head}' if previous else head
    commits = git('log', '--reverse', '--format=%H', revisions).splitlines()
    lines = ['## Changes', '']
    if not commits:
        lines.append('No module changes since the previous production release.')
    for commit in commits:
        message = git('show', '-s', '--format=%B', commit).splitlines()
        subject = message[0]
        description = '\n'.join(line for line in message[1:] if not line.lower().startswith('signed-off-by:')).strip()
        lines.append(f'- {subject} ([{commit[:7]}]({url}/commit/{commit}))')
        if description:
            lines.extend('  ' + line if line else '' for line in description.splitlines())
        lines.append('')
    if previous:
        lines.extend([f'[Full comparison]({url}/compare/{previous}...{head})', ''])
    filename = f'butterd-{args.version}.zip' if args.channel == 'production' else 'butterd-module.zip'
    lines.extend(['## Install', '', f'Flash `{filename}` in KernelSU or Magisk, then reboot. The sources ZIP is for source code only.', '', 'CI checks passed. On-device behaviour still needs phone validation.', ''])
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text('\n'.join(lines), encoding='utf-8')


if __name__ == '__main__':
    main()
