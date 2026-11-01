#!/usr/bin/env bash
set -euo pipefail
# shellcheck source=deployment/scripts/toolkit.sh
source "$(dirname "${BASH_SOURCE[0]}")/toolkit.sh"
private="$relay_root/generated"
[[ -f $private/record.json && -f $private/machine.nix ]] || { echo 'Run prepare.sh first' >&2; exit 1; }
bash "$toolkit_dir/toolkit/deploy.sh" preflight "$private/record.json"
# Compare the effective NixOS config, not just generated text, with the
# re-probed target record. Do not permit a private module to change the disk.
for pair in 'disk disk.path' 'bootMode boot' 'mac mac'; do
  read -r option field <<<"$pair"
  actual=$(nix eval --raw "path:$relay_root#nixosConfigurations.relay.config.peerRelay.$option")
  expected=$(jq -er ".$field" "$private/record.json")
  [[ $actual == "$expected" ]] || { echo "NixOS $option disagrees with target record" >&2; exit 1; }
done
[[ $(jq -er .arch "$private/record.json") == x86_64 ]] || { echo 'Unsupported architecture' >&2; exit 1; }
echo 'Target and evaluated relay configuration agree; no disk changes made.'
