#!/usr/bin/env bash
# Consumer integration check; no server, secrets or destructive commands.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
cd "$root"
source deployment/scripts/toolkit.sh
# Shared tests create fixtures inside their checkout; copy the pinned,
# read-only store source into a disposable writable directory.
mkdir -p generated
[[ ! -e generated/machine.nix ]] || { echo 'Refusing to overwrite private machine config' >&2; exit 1; }
toolkit_test=$(mktemp -d "$root/generated/toolkit-test.XXXXXXXX")
trap 'chmod -R u+rwX "$toolkit_test"; rm -rf "$toolkit_test"; rm -f "$root/generated/machine.nix"' EXIT
cp -R "$toolkit_dir/." "$toolkit_test/"
chmod u+w "$toolkit_test"
bash "$toolkit_test/tests/test.sh"
cp deployment/tests/machine.nix generated/machine.nix
[[ $(nix eval --json 'path:.#nixosConfigurations.relay.config.services.themossUpdate.timer') == false ]]
# The test candidate must not install an update timer for main.
[[ $(nix eval --json --apply 'timers: builtins.hasAttr "themoss-update" timers' 'path:.#nixosConfigurations.relay.config.systemd.timers') == false ]]
# Test the production policy too; restore the fake fixture on exit.
sed -i 's/autoUpdate = false/autoUpdate = true/' generated/machine.nix
[[ $(nix eval --json 'path:.#nixosConfigurations.relay.config.services.themossUpdate.timer') == true ]]
[[ $(nix eval --json --apply 'timers: builtins.hasAttr "themoss-update" timers' 'path:.#nixosConfigurations.relay.config.systemd.timers') == true ]]
nix build 'path:.#relay' --no-link
nix build 'path:.#nixosConfigurations.relay.config.system.build.toplevel' --no-link
echo 'thepeer relay + themoss integration passed'
