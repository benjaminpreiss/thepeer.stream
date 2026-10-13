#!/usr/bin/env python3
"""Package only explicit release inputs; collect notices from resolved sources."""
import argparse
import hashlib
import gzip
import io
import json
import os
from pathlib import Path
import re
import shutil
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent
NOTICE = re.compile(r'^(licen[cs]e|copying|copyright|notice|unlicense)([._-]|$)|^third[-_]party.*(license|notice)', re.I)
TAG = re.compile(r'v[0-9]+\.[0-9]+\.[0-9]+(-[A-Za-z0-9][A-Za-z0-9.-]*)?')


def safe_name(value):
    if not re.fullmatch(r'[A-Za-z0-9_.+-]+', value) or value in ('.', '..'):
        raise ValueError('Unsafe package or asset name')
    return value


def copy_notices(source, destination):
    count = 0
    for directory, dirs, files in os.walk(source, followlinks=False):
        dirs[:] = [d for d in dirs if d not in ('.git', 'target', 'node_modules') and not (Path(directory) / d).is_symlink()]
        for name in files:
            path = Path(directory) / name
            if NOTICE.search(name) and not path.is_symlink():
                relative = path.relative_to(source)
                target = destination / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(path, target)
                count += 1
    return count


def rust_notices(metadata, destination):
    members = set(metadata['workspace_members'])
    report = []
    missing = []
    for package in metadata['packages']:
        if package['id'] in members:
            continue
        name = safe_name(package['name'] + '-' + package['version'])
        source = Path(package['manifest_path']).parent
        target = destination / name
        count = copy_notices(source, target)
        declared = package.get('license_file')
        if declared:
            license_file = (source / declared).resolve()
            if not license_file.is_relative_to(source.resolve()) or not license_file.is_file():
                raise ValueError(f'Invalid declared license file: {name}')
            target.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(license_file, target / 'DECLARED-LICENSE')
            count += 1
        override = ROOT / 'licenses' / 'overrides' / name
        if override.is_dir():
            count += copy_notices(override, target / 'supplemental')
        if not count:
            missing.append(name)
        report.append({'name': package['name'], 'version': package['version'], 'license': package.get('license'), 'notice_files': count})
    destination.mkdir(parents=True, exist_ok=True)
    (destination / 'DEPENDENCIES.json').write_text(json.dumps(report, indent=2) + '\n')
    if missing:
        raise ValueError('Missing Rust license texts; add reviewed licenses/overrides/<name-version> inputs: ' + ', '.join(missing))


def android_notices(source, destination):
    """Include resolved POM metadata plus notices embedded in AAR/JAR artifacts."""
    destination.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source / 'DEPENDENCIES.json', destination / 'DEPENDENCIES.json')
    poms = source / 'poms'
    if not poms.is_dir() or not list(poms.glob('*.pom')):
        raise ValueError('Android dependency POMs missing')
    shutil.copytree(poms, destination / 'poms')
    def collect(archive, artifact_name, prefix='', depth=0):
        count = 0
        for info in archive.infolist():
            name = Path(info.filename)
            if info.is_dir():
                continue
            if NOTICE.search(name.name):
                if info.file_size > 2 * 1024 * 1024:
                    raise ValueError('Oversized dependency notice')
                # Flatten entry names; never trust archive paths for extraction.
                identity = prefix + info.filename
                target = destination / artifact_name / (hashlib.sha256(identity.encode()).hexdigest()[:12] + '-' + name.name)
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(archive.read(info))
                count += 1
            elif name.suffix == '.jar' and depth < 2:
                if info.file_size > 64 * 1024 * 1024:
                    raise ValueError('Oversized nested dependency JAR')
                with zipfile.ZipFile(io.BytesIO(archive.read(info))) as nested:
                    count += collect(nested, artifact_name, prefix + info.filename + '/', depth + 1)
        return count

    count = 0
    for artifact in (source / 'artifacts').iterdir():
        if artifact.suffix not in ('.jar', '.aar'):
            continue
        with zipfile.ZipFile(artifact) as archive:
            count += collect(archive, artifact.name)
    (destination / 'REVIEW.md').write_text(
        'Review the resolved POM license declarations and embedded notices before publication.\n'
        'POM URLs alone are not license texts. Supply any missing license/NOTICE texts in\n'
        'licenses/android/ and rebuild. The workflow creates only a draft, not a compliance certification.\n'
        f'Collected {count} embedded notice files.\n')
    if (ROOT / 'licenses/android').is_dir():
        copy_notices(ROOT / 'licenses/android', destination / 'supplemental')
    # Full Apache-2.0 text supplied by the Gradle wrapper, not a generated paraphrase.
    with zipfile.ZipFile(ROOT / 'apps/android-controller/gradle/wrapper/gradle-wrapper.jar') as wrapper:
        (destination / 'LICENSE-APACHE-2.0').write_bytes(wrapper.read('META-INF/LICENSE'))


def package(args):
    if not TAG.fullmatch(args.tag) or not re.fullmatch(r'[0-9a-f]{40}', args.commit):
        raise ValueError('Invalid release tag or commit')
    safe_name(args.platform)
    binary = Path(args.binary)
    data = binary.read_bytes()
    if not data:
        raise ValueError('Empty release binary')
    if args.kind == 'helper':
        for private in (str(Path.home()), str(ROOT)):
            if private.encode() in data:
                raise ValueError('Private build path in helper; use prefix remapping')
    output = Path(args.output)
    output.mkdir(parents=True, exist_ok=True)
    name = f'thepeer.stream-{args.tag}-{args.platform}'
    for existing in (name + '.tar.gz', name + '.apk', name + '-licenses.tar.gz'):
        if (output / existing).exists():
            raise ValueError('Refusing to overwrite an existing release asset')
    with tempfile.TemporaryDirectory() as temp:
        tree = Path(temp) / name
        tree.mkdir()
        for path in ('LICENSE', 'LICENSING.md', 'patches/saorsa-transport/UPSTREAM-LICENSE-MIT', 'crates/tunnel-core/src/route-plan-LICENSE-MIT'):
            target = tree / path
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / path, target)
        rust_notices(json.loads(Path(args.metadata).read_text()), tree / 'licenses/rust')
        if args.kind == 'android':
            if not args.android_dependencies:
                raise ValueError('Android dependency report required')
            android_notices(Path(args.android_dependencies), tree / 'licenses/android')
        (tree / 'BUILD.json').write_text(json.dumps({
            'project': 'thepeer.stream', 'tag': args.tag, 'commit': args.commit,
            'platform': args.platform, 'binary_sha256': hashlib.sha256(data).hexdigest(),
            'rust_toolchain': os.environ.get('THEPEER_RUST_TOOLCHAIN', 'unknown'),
            'android_version_code': os.environ.get('THEPEER_ANDROID_VERSION_CODE') if args.kind == 'android' else None,
        }, indent=2) + '\n')
        (tree / 'NOTICE-REVIEW.md').write_text(
            'License collection includes a Rust dependency superset (including build/dev dependencies).\n'
            'Review licenses, notices and actual binary contents before publishing this draft.\n')
        if args.kind == 'helper':
            shutil.copyfile(binary, tree / 'thepeer')
            (tree / 'thepeer').chmod(0o755)
            archive_name = name + '.tar.gz'
        else:
            shutil.copyfile(binary, output / (name + '.apk'))
            archive_name = name + '-licenses.tar.gz'
        def normalized(info):
            info.uid = info.gid = 0
            info.uname = info.gname = ''
            info.mtime = 0
            info.pax_headers = {}
            return info
        with (output / archive_name).open('xb') as raw:
            with gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as compressed:
                with tarfile.open(fileobj=compressed, mode='w') as archive:
                    archive.add(tree, arcname=name, filter=normalized)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--kind', choices=['helper', 'android'], required=True)
    for flag in ('binary', 'platform', 'tag', 'commit', 'metadata'):
        parser.add_argument('--' + flag, required=True)
    parser.add_argument('--android-dependencies')
    parser.add_argument('--output', default='release')
    package(parser.parse_args())
