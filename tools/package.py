#!/usr/bin/env python3
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED
import hashlib
root = Path(__file__).resolve().parents[1]
binary = root / 'build/libbutterd_compat.so'
if not binary.is_file():
    raise SystemExit('Missing Actions-built compatibility library')
output = root / 'dist/butterd-module.zip'
output.parent.mkdir(exist_ok=True)
files = [(p, p.relative_to(root / 'module').as_posix()) for p in sorted((root / 'module').rglob('*')) if p.is_file()]
files += [(binary, 'zygisk/arm64-v8a.so')]
files += [(root / name, name) for name in ('LICENSE', 'THIRD_PARTY_NOTICES.txt', 'README.md')]
files += [(p, 'sources/' + p.relative_to(root).as_posix()) for p in sorted((root / 'native').rglob('*')) if p.is_file()]
files += [(root / 'CMakeLists.txt', 'sources/CMakeLists.txt')]
files += [(p, 'sources/' + p.relative_to(root).as_posix()) for folder in ('java', 'tools', 'tests', '.github') for p in sorted((root / folder).rglob('*')) if p.is_file() and '__pycache__' not in p.parts]
with ZipFile(output, 'w', ZIP_DEFLATED) as archive:
    for source, name in files:
        entry = ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
        executable = name.endswith('.sh') or name in ('bin/butterctl', 'META-INF/com/google/android/update-binary')
        entry.create_system = 3
        entry.external_attr = (0o100755 if executable else 0o100644) << 16
        entry.compress_type = ZIP_DEFLATED
        archive.writestr(entry, source.read_bytes())
print(output.name, hashlib.sha256(output.read_bytes()).hexdigest())
