"""Prepared-workspace reuse checks; no Cargo, network or upstream checkout required."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
REVISION = 'e2f08388088313c82cac393afa6ca27b9ce00516'


class NativeWorkspaceTest(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.native = Path(temp.name).resolve() / 'tree'
        self.native.mkdir()
        shutil.copytree(ROOT / 'crates', self.native / 'crates')
        source = f'source = "git+https://github.com/WithAutonomi/saorsa-transport?rev={REVISION}#{REVISION}"\n'
        lock = (ROOT / 'Cargo.lock').read_text()
        self.assertEqual(lock.count(source), 1)
        (self.native / 'Cargo.lock').write_text(lock.replace(source, ''))
        manifest = (ROOT / 'Cargo.toml').read_text()
        manifest += '\n[patch."https://github.com/WithAutonomi/saorsa-transport"]\nsaorsa-transport = { path = ' + json.dumps(str(self.native.parent / 'saorsa')) + ' }\n'
        (self.native / 'Cargo.toml').write_text(manifest)
        self.receipt = self.native / 'preparation.json'
        patch_dir = ROOT / 'patches/saorsa-transport'
        digest = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
        self.inputs = {
            'recipe': digest(patch_dir / 'prepare-native.sh'),
            'patches': {path.name: digest(path) for path in sorted(patch_dir.glob('*.patch'))},
        }
        self.receipt.write_text(json.dumps(self.inputs))

    def run_reuse(self):
        env = dict(os.environ, THEPEER_PREPARED_WORKSPACE=str(self.native))
        return subprocess.run(['bash', str(ROOT / 'scripts/native-workspace.sh')],
                              env=env, capture_output=True, text=True, timeout=10)

    def test_matching_preparation_inputs_allow_reuse(self):
        result = self.run_reuse()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), str(self.native))

    def test_old_workspace_without_preparation_receipt_is_rejected(self):
        self.receipt.unlink()
        result = self.run_reuse()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('prepare again', result.stderr)

    def test_changed_recipe_or_patch_membership_or_bytes_is_rejected(self):
        for field in ['recipe', 'patch-membership', 'patch-bytes']:
            with self.subTest(field=field):
                inputs = json.loads(json.dumps(self.inputs))
                if field == 'recipe':
                    inputs['recipe'] = '0' * 64
                elif field == 'patch-membership':
                    inputs['patches']['obsolete.patch'] = '0' * 64
                else:
                    inputs['patches'][next(iter(inputs['patches']))] = '0' * 64
                self.receipt.write_text(json.dumps(inputs))
                result = self.run_reuse()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('prepare again', result.stderr)

    def test_malformed_receipt_is_rejected(self):
        self.receipt.write_text('{')
        self.assertNotEqual(self.run_reuse().returncode, 0)


if __name__ == '__main__':
    unittest.main()
