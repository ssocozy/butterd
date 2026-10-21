#!/usr/bin/env python3
from pathlib import Path
import subprocess
root = Path(__file__).resolve().parents[1]
for path in (root / 'module').rglob('*'):
    if path.is_file() and (path.suffix == '.sh' or path.name in ('butterctl', 'butter', 'update-binary')):
        subprocess.run(['sh', '-n', str(path)], check=True)
props = dict(line.split('=', 1) for line in (root / 'module/compatibility/pif.prop').read_text().splitlines() if '=' in line and not line.startswith('#'))
assert {key for key in props if key.startswith('spoof')} == {'spoofBuild', 'spoofVendingBuild', 'spoofProps'}
assert all(props[key] == 'true' for key in props if key.startswith('spoof'))
assert 'DEVICE_INITIAL_SDK_INT' not in props
assert not (root / 'module/system.prop').exists()
assert (root / 'module/skip_mount').exists()
assert not (root / 'module/post-fs-data.sh').exists()
profiles = list((root / 'module/compatibility/profiles').glob('*.prop'))
assert len(profiles) == 11, 'Every selectable model needs an offline profile'
for profile in profiles:
    values = dict(line.split('=', 1) for line in profile.read_text().splitlines() if '=' in line)
    assert all(values.get(key) == 'true' for key in ('spoofBuild', 'spoofVendingBuild', 'spoofProps'))
    assert set(values) <= {'FINGERPRINT', 'MANUFACTURER', 'MODEL', 'SECURITY_PATCH', 'spoofBuild', 'spoofVendingBuild', 'spoofProps'}
print('Module structure and shell syntax validated; no device scripts executed.')

assert (root / 'rust/butter/Cargo.toml').is_file()
assert not (root / 'java/dev/butterd/butter/Engine.java').exists()
assert not (root / 'module/lib/cpu.sh').exists()
assert 'app_process' not in (root / 'module/bin/butter').read_text()
assert 'butter-engine' in (root / 'module/bin/butter').read_text()
