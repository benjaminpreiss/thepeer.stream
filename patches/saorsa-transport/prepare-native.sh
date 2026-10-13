#!/usr/bin/env bash
# Shared, offline source preparation for review and operator Android builds.
# stdout: generated Rust workspace path ONLY. Does not touch caches or staging.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
revision=e2f08388088313c82cac393afa6ca27b9ce00516
checkout=${SAORSA_CHECKOUT:-}
if [[ -z "$checkout" ]]; then
  shopt -s nullglob
  candidates=("${CARGO_HOME:-$HOME/.cargo}"/git/checkouts/saorsa-transport-*/e2f0838)
  [[ ${#candidates[@]} -eq 1 ]] || { echo 'Set SAORSA_CHECKOUT to the cached pinned checkout; fetch dependencies first.' >&2; exit 1; }
  checkout=${candidates[0]}
fi
git -C "$checkout" cat-file -e "$revision^{commit}"
base=${1:-"$root/target/patched-source"}
mkdir -p "$base"
base=$(cd "$base" && pwd)
work=$(mktemp -d "$base/run.XXXXXX")
native="$work/tree"
mkdir -p "$work/saorsa" "$native/crates"
patches=(
  "$root/patches/saorsa-transport/0003-client-relay-response-bound.patch"
  "$root/patches/saorsa-transport/0004-cancellable-peer-attempt.patch"
  "$root/patches/saorsa-transport/0005-peer-handshake-hook.patch"
)
# A changed patch requires an explicit review/update of this preparation recipe.
python3 - "${patches[@]}" <<'PY'
from pathlib import Path
import hashlib,sys
expected=[
    '6c55a783245e3045d9cf852bfa5836fb0b15528542da6d903c0b3f14f038d04d',
    'f68612aed60d8df84ceeb3bba85a14b06cc145d1596567cb335d3a4183217dba',
    '5f4f7e02a92fd01e5bcb721958b633574e1d8da8eeeacd6475803886cd19e386',
]
if len(sys.argv[1:]) != len(expected):
    raise SystemExit('Unexpected reviewed patch count; preparation aborted')
for path,digest in zip(sys.argv[1:],expected):
    p=Path(path)
    if p.is_symlink() or hashlib.sha256(p.read_bytes()).hexdigest()!=digest:
        raise SystemExit('Unexpected reviewed patch bytes; preparation aborted')
PY
git -C "$checkout" archive "$revision" | tar -x -C "$work/saorsa"
for patch in "${patches[@]}"; do
  git -C "$work/saorsa" apply --check "$patch"
  git -C "$work/saorsa" apply "$patch"
done
cp "$root/Cargo.toml" "$root/Cargo.lock" "$native/"
cp -R "$root/crates/tunnel-core" "$root/crates/tunnel-android" "$root/crates/cli" "$native/crates/"
python3 - "$work" "$native" "$revision" "$root" "${patches[@]}" <<'PY'
from pathlib import Path
import hashlib,json,sys
work,native=map(Path,sys.argv[1:3]); revision=sys.argv[3]
root=Path(sys.argv[4]); patches=list(map(Path,sys.argv[5:]))
lock=native/'Cargo.lock'
source=f'source = "git+https://github.com/WithAutonomi/saorsa-transport?rev={revision}#{revision}"\n'
text=lock.read_text()
if text.count(source)!=1: raise SystemExit('Expected exactly one pinned Saorsa source')
lock.write_text(text.replace(source,''))
with (native/'Cargo.toml').open('a') as f:
    f.write('\n[patch."https://github.com/WithAutonomi/saorsa-transport"]\nsaorsa-transport = { path = '+json.dumps(str(work/'saorsa'))+' }\n')
digest=lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
(native/'preparation.json').write_text(json.dumps({
    'recipe': digest(root/'patches/saorsa-transport/prepare-native.sh'),
    'patches': {path.name: digest(path) for path in patches},
}, sort_keys=True)+'\n')
PY
printf 'Pinned revision: %s\nPrepared source: %s\n' "$revision" "$work" >&2
shasum -a 256 "${patches[@]}" >&2
printf '%s\n' "$native"
