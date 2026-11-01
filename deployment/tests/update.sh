#!/usr/bin/env bash
# Unit test of updater's staging/fast-forward/failure behavior; no NixOS switch.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/repo" "$tmp/state" "$tmp/bin"
cd "$tmp/repo"
git init -q -b main
git config user.email ci@example.invalid
git config user.name CI
printf 'first\n' > version
git add version && git commit -qm first
printf 'machine\n' > "$tmp/state/machine.nix"
printf '{}\n' > "$tmp/state/facter.json"
cat > "$tmp/bin/nix" <<'MOCK'
#!/usr/bin/env bash
[[ ${FAIL_BUILD:-0} == 0 ]] || exit 1
# The flake target has a fragment; extract the stage path.
path=${!#}; path=${path#path:}; path=${path%%#*}
test -f "$path/deployment/generated/machine.nix"
MOCK
cat > "$tmp/bin/nixos-rebuild" <<'MOCK'
#!/usr/bin/env bash
[[ ${FAIL_SWITCH:-0} == 0 ]] || exit 1
touch "$THEPEER_RELAY_STATE/switched"
MOCK
chmod +x "$tmp/bin/"*
export PATH="$tmp/bin:$PATH" THEPEER_RELAY_STATE="$tmp/state" THEPEER_RELAY_REPO="$tmp/repo"
# Initial clone and switch.
bash "$root/deployment/scripts/update.sh"
test -f "$tmp/state/switched"
rm "$tmp/state/switched"
printf 'second\n' > version && git add version && git commit -qm second
old=$(git -C "$tmp/state/source" rev-parse HEAD)
if FAIL_BUILD=1 bash "$root/deployment/scripts/update.sh"; then echo 'Broken build switched' >&2; exit 1; fi
test "$(git -C "$tmp/state/source" rev-parse HEAD)" = "$old"
test ! -e "$tmp/state/switched"
bash "$root/deployment/scripts/update.sh"
test "$(git -C "$tmp/state/source" rev-parse HEAD)" = "$(git rev-parse HEAD)"
test "$(cat "$tmp/state/machine.nix")" = machine
# Reject a non-fast-forward remote rather than resetting local source.
git reset -q --hard HEAD~1
printf 'alternate\n' > version && git add version && git commit -qm alternate
if bash "$root/deployment/scripts/update.sh"; then echo 'Diverged source switched' >&2; exit 1; fi
test "$(cat "$tmp/state/machine.nix")" = machine
echo 'updater tests passed'
