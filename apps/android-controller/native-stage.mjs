import { createHash } from 'node:crypto';
import { readFileSync, lstatSync, readdirSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { homedir } from 'node:os';

const root = dirname(fileURLToPath(import.meta.url));
export function sourceNames(base = root) {
  function walk(dir) {
    return readdirSync(join(base, dir), { withFileTypes: true }).flatMap(entry => {
      const name = `${dir}/${entry.name}`;
      if (entry.isSymbolicLink()) throw new Error('Symlink in native sources');
      return entry.isDirectory() ? walk(name) : name.endsWith('.rs') ? [name] : [];
    });
  }
  return ['../../Cargo.toml', '../../Cargo.lock',
    '../../crates/tunnel-core/Cargo.toml', '../../crates/tunnel-android/Cargo.toml',
    '../../crates/cli/Cargo.toml', 'native-build.sh', 'native-stage.mjs',
    'app/src/main/java/dev/webp2p/controllerprobe/NativeSaorsaProbe.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ViewerLaunch.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ViewerScan.java',
    'app/src/main/java/dev/webp2p/controllerprobe/MainActivity.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ConnectionState.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ViewerQrScanner.java',
    'app/src/main/java/dev/webp2p/controllerprobe/RelayQrDecoder.java',
    'app/src/main/java/dev/webp2p/controllerprobe/EmbeddedQrDecoderFactory.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ViewerService.java',
    'app/src/main/java/dev/webp2p/controllerprobe/ConnectionHandoff.java',
    '../../patches/saorsa-transport/prepare-native.sh',
    '../../patches/saorsa-transport/0003-client-relay-response-bound.patch',
    '../../patches/saorsa-transport/0004-cancellable-peer-attempt.patch',
    '../../patches/saorsa-transport/0005-peer-handshake-hook.patch',
    ...walk('../../crates/tunnel-core/src'),
    ...walk('../../crates/tunnel-android/src')].sort();
}
export function validateArtifact(bytes, home = homedir()) {
  if (bytes.length < 64 || bytes.length > 64 * 1024 * 1024
      || !bytes.subarray(0, 4).equals(Buffer.from([0x7f, 0x45, 0x4c, 0x46]))
      || bytes[4] !== 2 || bytes[5] !== 1 || bytes.readUInt16LE(18) !== 183
      || bytes.readUInt16LE(16) !== 3) throw new Error('Expected bounded AArch64 ELF shared library');
  if (bytes.includes(Buffer.from(home))) throw new Error('Native library contains private home path; not staged');
}
export function validateRoutingArtifact(bytes, home = homedir()) {
  validateArtifact(bytes, home);
  if (!bytes.includes(Buffer.from('web_p2p_routing_client_v2'))) throw new Error('Routing native build required; run native-build.sh');
}
function digest(bytes) { return createHash('sha256').update(bytes).digest('hex'); }
export function sourceSnapshot(base = root) {
  const sources = {};
  for (const name of sourceNames(base)) {
    const path = join(base, name);
    if (!lstatSync(path).isFile() || lstatSync(path).isSymbolicLink()) throw new Error('Invalid native source');
    sources[name] = digest(readFileSync(path));
  }
  return { transport: 'saorsa-routing-v2', sources };
}
export function verifySnapshot(expected, base = root) {
  const current = sourceSnapshot(base);
  if (!expected || Object.keys(expected).length !== 2 || expected.transport !== current.transport ||
      !expected.sources || Object.keys(expected.sources).length !== Object.keys(current.sources).length ||
      !Object.entries(current.sources).every(([name, hash]) => expected.sources[name] === hash)) {
    throw new Error('Native inputs changed during preparation/build; rebuild before staging');
  }
  return current;
}
export function stage(file, expected, base = root) {
  const stat = lstatSync(file);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 64 * 1024 * 1024) throw new Error('Invalid native artifact');
  const bytes = readFileSync(file);
  validateRoutingArtifact(bytes);
  const current = verifySnapshot(expected, base);
  const properties = [`transport=${current.transport}`, `artifact=${digest(bytes)}`,
    ...Object.entries(current.sources).map(([name, hash]) => `source.${name}=${hash}`)];
  const dir = join(base, 'generated/jniLibs/arm64-v8a');
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'libcontroller_saorsa_probe.so'), bytes);
  writeFileSync(join(base, 'generated/native.properties'), properties.join('\n') + '\n');
  console.log('Staged checked routing arm64-v8a native transport; source/artifact hashes recorded outside APK assets.');
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  if (process.argv.length !== 4) throw new Error('Use --snapshot <new-file> or <artifact> <pre-build-snapshot>');
  if (process.argv[2] === '--snapshot') {
    writeFileSync(process.argv[3], JSON.stringify(sourceSnapshot()), { flag: 'wx', mode: 0o600 });
  } else {
    const stat = lstatSync(process.argv[3]);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 65536) throw new Error('Invalid pre-build snapshot');
    let expected;
    try { expected = JSON.parse(readFileSync(process.argv[3], 'utf8')); }
    catch { throw new Error('Invalid pre-build snapshot'); }
    stage(process.argv[2], expected);
  }
}
