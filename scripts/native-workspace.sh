#!/usr/bin/env bash
# Host tooling only. Reuse an explicitly supplied, current prepared workspace.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
native=${THEPEER_PREPARED_WORKSPACE:-}
if [[ -z "$native" ]]; then
  exec bash "$root/patches/saorsa-transport/prepare-native.sh"
fi
diff -qr "$root/crates" "$native/crates" >/dev/null || {
  echo 'Prepared workspace sources differ; refresh before testing.' >&2; exit 1;
}
python3 - "$root" "$native" <<'PY'
from pathlib import Path
import json, sys
root, native = (Path(p).resolve() for p in sys.argv[1:])
revision = 'e2f08388088313c82cac393afa6ca27b9ce00516'
source = f'source = "git+https://github.com/WithAutonomi/saorsa-transport?rev={revision}#{revision}"\n'
lock = (root / 'Cargo.lock').read_text()
if lock.count(source) != 1 or (native / 'Cargo.lock').read_text() != lock.replace(source, ''):
    raise SystemExit('Prepared workspace lock differs; prepare again')
manifest = (root / 'Cargo.toml').read_text()
manifest += '\n[patch."https://github.com/WithAutonomi/saorsa-transport"]\nsaorsa-transport = { path = ' + json.dumps(str(native.parent / 'saorsa')) + ' }\n'
if (native / 'Cargo.toml').read_text() != manifest:
    raise SystemExit('Prepared workspace manifest differs; prepare again')
PY
printf '%s\n' "$native"
