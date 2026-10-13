"""Release packaging and installer tests; no network or real executable launches."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('release_bundle', ROOT / 'scripts/release-bundle.py')
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


class ReleaseToolsTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)

    def metadata(self, license_text=True):
        source = self.base / 'dependency'
        source.mkdir()
        (source / 'Cargo.toml').write_text('[package]\nname="example"\nversion="1.0.0"\n')
        if license_text:
            (source / 'LICENSE-MIT').write_text('Test-only license text')
        return {'workspace_members': [], 'packages': [{
            'id': 'example', 'name': 'example', 'version': '1.0.0',
            'license': 'MIT', 'manifest_path': str(source / 'Cargo.toml'),
        }]}

    def test_notices_and_metadata_exclude_absolute_source_paths(self):
        destination = self.base / 'notices'
        bundle.rust_notices(self.metadata(), destination)
        self.assertEqual((destination / 'example-1.0.0/LICENSE-MIT').read_text(), 'Test-only license text')
        report = (destination / 'DEPENDENCIES.json').read_text()
        self.assertNotIn(str(self.base), report)
        self.assertEqual(json.loads(report)[0]['notice_files'], 1)

    def test_missing_notices_fail_closed(self):
        with self.assertRaisesRegex(ValueError, 'Missing Rust license texts'):
            bundle.rust_notices(self.metadata(False), self.base / 'notices')

    def test_notice_symlinks_are_not_followed(self):
        metadata = self.metadata(False)
        secret = self.base / 'private'
        secret.write_text('not a license')
        (self.base / 'dependency/LICENSE').symlink_to(secret)
        with self.assertRaises(ValueError):
            bundle.rust_notices(metadata, self.base / 'notices')

    def test_unsafe_package_names_are_rejected(self):
        for name in ('../outside', '..', '/absolute', 'x\ny'):
            with self.assertRaises(ValueError):
                bundle.safe_name(name)

    def test_android_nested_jar_notices_and_paths(self):
        source = self.base / 'android'
        (source / 'poms').mkdir(parents=True)
        (source / 'poms/example.pom').write_text('<project/>')
        (source / 'DEPENDENCIES.json').write_text('[]')
        (source / 'artifacts').mkdir()
        nested = io.BytesIO()
        with zipfile.ZipFile(nested, 'w') as jar:
            jar.writestr('../../NOTICE', 'nested notice')
        with zipfile.ZipFile(source / 'artifacts/example.aar', 'w') as aar:
            aar.writestr('classes.jar', nested.getvalue())
        destination = self.base / 'notices'
        bundle.android_notices(source, destination)
        notices = list((destination / 'example.aar').glob('*-NOTICE'))
        self.assertEqual(len(notices), 1)
        self.assertEqual(notices[0].read_text(), 'nested notice')
        self.assertFalse((self.base / 'NOTICE').exists())

    def package_helper(self, platform='linux-x86_64'):
        meta = self.base / 'metadata.json'
        meta.write_text(json.dumps(self.metadata()))
        binary = self.base / 'helper'
        binary.write_bytes(b'fixture binary; never execute')
        args = type('Args', (), dict(kind='helper', platform=platform, tag='v1.2.3',
            commit='a' * 40, binary=str(binary), metadata=str(meta),
            output=str(self.base / 'release'), android_dependencies=None))()
        bundle.package(args)
        return args

    def test_package_contains_only_selected_inputs_and_refuses_overwrite(self):
        args = self.package_helper()
        archive = self.base / 'release/thepeer.stream-v1.2.3-linux-x86_64.tar.gz'
        with tarfile.open(archive) as tar:
            paths = tar.getnames()
            self.assertTrue(all(member.uid == 0 and member.gid == 0 and not member.uname and not member.gname and member.mtime == 0 for member in tar.getmembers()))
            self.assertIn('thepeer.stream-v1.2.3-linux-x86_64/thepeer', paths)
            self.assertIn('thepeer.stream-v1.2.3-linux-x86_64/LICENSE', paths)
            self.assertFalse(any('/.git/' in name or '/Cargo.toml' in name for name in paths))
        with self.assertRaisesRegex(ValueError, 'overwrite'):
            bundle.package(args)

    def installer(self, corrupt=False, version=None, platform='Linux', architecture='x86_64', expected_platform='linux-x86_64'):
        self.package_helper(expected_platform)
        release = self.base / 'release'
        name = f'thepeer.stream-v1.2.3-{expected_platform}.tar.gz'
        digest = hashlib.sha256((release / name).read_bytes()).hexdigest()
        if corrupt:
            digest = '0' * 64
        (release / 'SHA256SUMS').write_text(f'{digest}  {name}\n')
        commands = self.base / 'bin'
        commands.mkdir()
        uname = commands / 'uname'
        uname.write_text(f'#!/bin/sh\ncase "$1" in -s) echo {platform};; -m) echo {architecture};; esac\n')
        uname.chmod(0o755)
        curl = commands / 'curl'
        curl.write_text(f'#!{sys.executable}\n' + '''import os, pathlib, shutil, sys
args=sys.argv[1:]
urls=[a for a in args if a.startswith('https://')]
assert len(urls)==1 and urls[0].startswith('https://github.com/benjaminpreiss/thepeer.stream/')
if urls[0].endswith('/latest'):
    print('https://github.com/benjaminpreiss/thepeer.stream/releases/tag/v1.2.3', end='')
else:
    name=urls[0].split('/')[-1]
    assert name in ('SHA256SUMS', os.environ['FIXTURE_ARCHIVE'])
    shutil.copyfile(pathlib.Path(os.environ['FIXTURE_RELEASE'])/name, args[args.index('-o')+1])
''')
        curl.chmod(0o755)
        env = dict(os.environ, PATH=str(commands) + os.pathsep + os.environ['PATH'],
            THEPEER_PREFIX=str(self.base / 'installed'), FIXTURE_RELEASE=str(release), FIXTURE_ARCHIVE=name)
        env.pop('THEPEER_VERSION', None)
        if version is not None:
            env['THEPEER_VERSION'] = version
        return subprocess.run(['sh', str(ROOT / 'scripts/install.sh')], env=env, capture_output=True, text=True)

    def test_installer_verifies_and_keeps_licenses_without_executing_helper(self):
        result = self.installer()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((self.base / 'installed/bin/thepeer').read_bytes(), b'fixture binary; never execute')
        self.assertTrue((self.base / 'installed/share/thepeer.stream/v1.2.3/thepeer.stream-v1.2.3-linux-x86_64.tar.gz').is_file())

    def test_installer_selects_apple_silicon_archive(self):
        result = self.installer(platform='Darwin', architecture='arm64', expected_platform='macos-arm64')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.base / 'installed/bin/thepeer').is_file())

    def test_installer_selects_intel_mac_archive_and_explicit_version(self):
        result = self.installer(platform='Darwin', architecture='x86_64', expected_platform='macos-x86_64', version='v1.2.3')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((self.base / 'installed/bin/thepeer').is_file())

    def test_checksum_failure_preserves_existing_installation(self):
        installed = self.base / 'installed/bin/thepeer'
        installed.parent.mkdir(parents=True)
        installed.write_text('old helper')
        result = self.installer(corrupt=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Checksum mismatch', result.stderr)
        self.assertEqual(installed.read_text(), 'old helper')

    def test_installer_rejects_multiline_version(self):
        result = self.installer(version='v1.2.3\n../../outside')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.base / 'installed').exists())

    def test_installer_rejects_unsupported_architecture(self):
        result = self.installer(architecture='aarch64')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Supported platforms', result.stderr)


if __name__ == '__main__':
    unittest.main()
