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
