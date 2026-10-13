import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { validateArtifact, validateRoutingArtifact, sourceNames, sourceSnapshot, verifySnapshot } from '../native-stage.mjs';

function header() {
  const bytes = Buffer.alloc(128);
  bytes.set([0x7f, 0x45, 0x4c, 0x46, 2, 1]);
  bytes.writeUInt16LE(3, 16);
  bytes.writeUInt16LE(183, 18);
  return bytes;
}
test('native staging rejects wrong ABI/type, non-ELF and embedded private paths', () => {
  validateArtifact(header(), '/private-test-home');
  assert.throws(() => validateArtifact(Buffer.alloc(128)));
  const wrong = header();
  wrong.writeUInt16LE(62, 18);
  assert.throws(() => validateArtifact(wrong));
  const executable = header();
  executable.writeUInt16LE(2, 16);
  assert.throws(() => validateArtifact(executable));
  const privateBytes = Buffer.concat([header(), Buffer.from('/private-test-home/source')]);
  assert.throws(() => validateArtifact(privateBytes, '/private-test-home'));
});
test('routing staging rejects absent and legacy feature markers', () => {
  assert.throws(() => validateRoutingArtifact(header(), '/private-test-home'));
  assert.throws(() => validateRoutingArtifact(Buffer.concat([header(), Buffer.from('web_p2p_relay_client_v1')]), '/private-test-home'));
  validateRoutingArtifact(Buffer.concat([header(), Buffer.from('web_p2p_routing_client_v2')]), '/private-test-home');
});
test('staging requires matching pre-build inputs, including patches and source membership', () => {
  const expected = sourceSnapshot();
  assert.deepEqual(verifySnapshot(expected), expected);
  for (const name of ['../../crates/tunnel-android/src/lib.rs',
    '../../crates/tunnel-core/src/application.rs',
    '../../patches/saorsa-transport/0001-stream-forwarding-policy.patch',
    '../../patches/saorsa-transport/0003-client-relay-response-bound.patch',
    '../../patches/saorsa-transport/0004-cancellable-peer-attempt.patch',
    '../../patches/saorsa-transport/0005-peer-handshake-hook.patch']) {
    const changed = structuredClone(expected); changed.sources[name] = '0'.repeat(64);
    assert.throws(() => verifySnapshot(changed), /changed during/);
  }
  const missing = structuredClone(expected); delete missing.sources['../../crates/tunnel-android/src/lib.rs'];
  assert.throws(() => verifySnapshot(missing));
  const extra = structuredClone(expected); extra.sources['../../crates/tunnel-core/src/removed.rs'] = '0'.repeat(64);
  assert.throws(() => verifySnapshot(extra));
  assert.throws(() => verifySnapshot({ ...expected, transport: 'unpatched' }));
  assert.throws(() => verifySnapshot({ ...expected, transport: 'saorsa-relay-v1' }));
  assert.throws(() => verifySnapshot(undefined));
});
test('Gradle and Node enumerate the same workspace receipt inputs', () => {
  const gradle = readFileSync(new URL('../app/build.gradle.kts', import.meta.url), 'utf8');
  const quoted = text => [...text.matchAll(/"([^"]+)"/g)].map(match => match[1]);
  const fixed = quoted(gradle.match(/val sources = setOf\(([\s\S]*?)\) \+/)[1]);
  const roots = quoted(gradle.match(/listOf\(([^\n]+)\)\.flatMap \{ directory/)[1]);
  assert.deepEqual(roots.sort(), ['../../crates/tunnel-android/src', '../../crates/tunnel-core/src']);
  const names = sourceNames();
  assert.deepEqual(fixed.sort(), names.filter(name => !name.endsWith('.rs')).sort());
  assert.ok(names.filter(name => name.endsWith('.rs')).every(name => roots.some(root => name.startsWith(root + '/'))));
});
test('native source receipt covers pinned dependencies, runtime, JNI bridge and build tooling', () => {
  const sources = sourceNames();
  for (const file of ['../../Cargo.toml', '../../Cargo.lock',
    '../../crates/tunnel-core/Cargo.toml', '../../crates/tunnel-android/Cargo.toml',
    '../../crates/cli/Cargo.toml', '../../crates/tunnel-core/src/lib.rs',
    '../../crates/tunnel-core/src/pin.rs', '../../crates/tunnel-android/src/lib.rs',
    '../../crates/tunnel-android/src/routing_owner.rs', 'native-build.sh', 'native-stage.mjs',
    '../../patches/saorsa-transport/prepare-native.sh',
    '../../patches/saorsa-transport/0001-stream-forwarding-policy.patch',
    '../../patches/saorsa-transport/0002-bounded-stream-socket.patch',
    '../../patches/saorsa-transport/0003-client-relay-response-bound.patch',
    '../../patches/saorsa-transport/0004-cancellable-peer-attempt.patch',
    '../../patches/saorsa-transport/0005-peer-handshake-hook.patch',
    'app/src/main/java/dev/webp2p/controllerprobe/MainActivity.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ConnectionState.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ViewerQrScanner.java',
    'app/src/main/java/dev/webp2p/controllerprobe/EmbeddedQrDecoderFactory.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ViewerService.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ConnectionHandoff.java',
    '../../crates/tunnel-core/src/http.rs', '../../crates/tunnel-core/src/http_host.rs',
    '../../crates/tunnel-core/src/http_inspector.rs', '../../crates/tunnel-core/src/phone_ingress.rs',
    'app/src/main/java/dev/webp2p/controllerprobe/ViewerLaunch.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ViewerScan.java',
    'app/src/main/java/dev/webp2p/controllerprobe/NativeSaorsaProbe.java']) assert.ok(sources.includes(file));
  assert.ok(!sources.includes('../../crates/cli/src/main.rs'));
  assert.ok(sources.every(name => !name.includes('/tests/') && !name.includes('experiments/')));
  assert.equal(new Set(sources).size, sources.length);
  assert.ok(sources.every(name => !name.includes('target/') && !name.includes('generated/')));
});
