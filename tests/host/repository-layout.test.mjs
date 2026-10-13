import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';

const root = new URL('../../', import.meta.url);

test('retired implementations are absent, not merely labelled historical', () => {
  for (const path of [
    'apps/browser', 'apps/relay-status', 'experiments', 'web',
    'deployment/saorsa-relay',
    'cmd', 'internal', 'go.mod', 'go.sum', 'demo.mov', 'diagram.png',
    '.github/workflows/pages.yaml',
    'apps/android-controller/native/Cargo.toml', 'apps/android-controller/native/src',
    'apps/android-controller/app/src/main/assets',
    'apps/android-controller/APPLICATION-MVP.md',
    'apps/android-controller/NATIVE-PROBE.md',
    'apps/android-controller/NATIVE-VIEWER.md',
    'apps/android-controller/RELAY-LAN-TEST.md',
  ]) assert.equal(existsSync(new URL(path, root)), false, `Obsolete implementation remains: ${path}`);
  for (const path of [
    'crates/tunnel-core/Cargo.toml', 'crates/tunnel-android/Cargo.toml',
    'crates/cli/Cargo.toml', 'tests/fixtures/application-app.mjs',
    'tests/fixtures/application-client.js', 'patches/saorsa-transport/UPSTREAM-LICENSE-MIT',
  ]) assert.ok(existsSync(new URL(path, root)), `Missing live input: ${path}`);
});
