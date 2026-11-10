"""Regression fixtures for Google's bulletin markup; run only in Actions."""
import importlib.util
from pathlib import Path
spec = importlib.util.spec_from_file_location('catalog', Path(__file__).resolve().parents[1] / 'tools/fetch_catalog.py')
catalog = importlib.util.module_from_spec(spec)
spec.loader.exec_module(catalog)
row = '''<tr><td><a href="/docs/security/bulletin/pixel/2026/2026-10-01">October 2026</a></td>
<td>Translations coming soon</td><td>October 6, 2026</td><td>2026-10-05</td></tr>'''
assert catalog.security_patch(row, '2026-10') == '2026-10-05'
assert catalog.security_patch(row, '2026-09') is None
assert catalog.security_patch(row.replace('2026-10-05', '2026-09-05'), '2026-10') is None
assert catalog.security_patch(row.replace('2026-10-05', '2026-10-99'), '2026-10') is None
assert catalog.security_patch(row.replace('<td>2026-10-05</td>', '<td><span>2026-10-05</span></td>'), '2026-10') == '2026-10-05'
assert catalog.INITIAL_API['bluejay'] == 32 and catalog.INITIAL_API['tegu'] == 35
print('Pixel metadata regression fixtures passed')

# A newer Canary can precede its bulletin. Never attach an older patch to it.
builds = [
    {'product': 'bluejay_beta', 'buildId': '200', 'releaseCandidateName': 'NOV.001', 'previewMetadata': {'canary': True, 'id': 'canary-202611'}},
    {'product': 'bluejay_beta', 'buildId': '100', 'releaseCandidateName': 'OCT.001', 'previewMetadata': {'canary': True, 'id': 'canary-202610'}},
]
build, patch = catalog.verified_build(builds, 'bluejay', row)
assert build['buildId'] == '100' and patch == '2026-10-05'
november = row.replace('2026-10', '2026-11')
build, patch = catalog.verified_build(builds, 'bluejay', row + november)
assert build['buildId'] == '200' and patch == '2026-11-05'
for candidates, device, bulletin in [(builds[:1], 'bluejay', row), (builds, 'other', row), (builds, 'bluejay', '')]:
    try:
        catalog.verified_build(candidates, device, bulletin)
    except ValueError:
        pass
    else:
        raise AssertionError('Unverified Canary build accepted')
print('Newest verified Canary selection fixtures passed')

platform_row = '<tr><td><a href="/docs/security/bulletin/2026/2026-11-01">November</a></td><td>Translations coming soon</td><td>November 2, 2026</td><td>2026-11-01<br>2026-11-05</td></tr>'
assert catalog.platform_security_patch(platform_row, '2026-11') == '2026-11-05'
assert catalog.platform_security_patch(platform_row, '2026-10') is None
assert catalog.platform_security_patch(platform_row.replace('2026-11-05', '2026-10-05'), '2026-11') is None
assert catalog.platform_security_patch(platform_row.replace('<td>2026-11-01<br>2026-11-05</td>', '<td>November 2, 2026</td>'), '2026-11') is None
build, patch = catalog.verified_build(builds, 'bluejay', row, platform_row)
assert build['buildId'] == '200' and patch == '2026-11-05'
print('Android bulletin patch-level fixtures passed')
