#!/usr/bin/env python3
"""Produce a small installable module and a separate corresponding-source archive."""
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED
import hashlib

root = Path(__file__).resolve().parents[1]
dist = root / 'dist'
dist.mkdir(exist_ok=True)
binary = root / 'build/libbutterd_compat.so'
engine = root / 'rust/butter/target/aarch64-linux-android/release/butter'
for path in (binary, engine):
    if not path.is_file():
        raise SystemExit('Missing Actions-built binary: ' + path.name)
vendor = root / 'build/rust-vendor'
if not vendor.is_dir():
    raise SystemExit('Missing vendored Rust sources and licenses')

module_files = [(p, p.relative_to(root / 'module').as_posix())
                for p in sorted((root / 'module').rglob('*')) if p.is_file()]
module_files += [(binary, 'zygisk/arm64-v8a.so'), (engine, 'bin/butter-engine')]
module_files += [(root / name, name) for name in ('LICENSE', 'THIRD_PARTY_NOTICES.txt', 'README.md')]
# Retain dependency notices in the runtime package without installing crate sources or archives.
for crate in sorted(vendor.iterdir()):
    if not crate.is_dir():
        continue
    for path in sorted(crate.rglob('*')):
        if path.is_file() and path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'UNLICENSE')):
            module_files.append((path, 'licenses/rust/' + path.relative_to(vendor).as_posix()))
source_files = [(root / name, 'sources/' + name)
                for name in ('CMakeLists.txt', 'LICENSE', 'THIRD_PARTY_NOTICES.txt', 'README.md')]
for folder in ('module', 'native', 'java', 'tools', 'tests', '.github', 'rust'):
    for path in sorted((root / folder).rglob('*')):
        if path.is_file() and '__pycache__' not in path.parts and 'target' not in path.parts and path.suffix != '.dex':
            source_files.append((path, 'sources/' + path.relative_to(root).as_posix()))
source_files += [(p, 'sources/rust/butter/vendor/' + p.relative_to(vendor).as_posix())
                 for p in sorted(vendor.rglob('*')) if p.is_file()]
config = root / 'build/source-cargo-config.toml'
config.write_text('[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "vendor"\n')
source_files.append((config, 'sources/rust/butter/.cargo/config.toml'))

def write_archive(output, files):
    names = set()
    with ZipFile(output, 'w', ZIP_DEFLATED) as archive:
        for source, name in files:
            if name in names:
                raise ValueError('Duplicate archive entry: ' + name)
            names.add(name)
            entry = ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            executable = name.endswith('.sh') or name in ('bin/butterctl', 'bin/butter', 'bin/butter-engine', 'META-INF/com/google/android/update-binary')
            entry.create_system = 3
            entry.external_attr = (0o100755 if executable else 0o100644) << 16
            entry.compress_type = ZIP_DEFLATED
            archive.writestr(entry, source.read_bytes())
    with ZipFile(output) as archive:
        installed = sum(item.file_size for item in archive.infolist())
    print(output.name, 'compressed=', output.stat().st_size, 'expanded=', installed,
          'sha256=', hashlib.sha256(output.read_bytes()).hexdigest())

# A source/vendor leak must fail the build before publication.
assert not any(name.startswith('sources/') or '/vendor/' in name for _, name in module_files)
assert sum(path.stat().st_size for path, _ in module_files) <= 8 * 1024 * 1024, 'Runtime module exceeds 8 MiB size budget'
write_archive(dist / 'butterd-module.zip', module_files)
write_archive(dist / 'butterd-sources.zip', source_files)
