#!/usr/bin/env bash
# Consumer pre-erase hook; called with the *staged* flake, not the live tree.
# Also useful for a non-destructive local preflight before invoking install.
set -euo pipefail
[[ $# == 3 ]] || { echo 'usage: check-machine.sh STAGED_FLAKE HOST_OUTPUT RECORD' >&2; exit 2; }
source_dir=$1 host=$2 record=$3
[[ $host == relay ]] || { echo 'Unsupported NixOS output' >&2; exit 1; }
[[ $(jq -er .arch "$record") == x86_64 ]] || { echo 'Unsupported architecture' >&2; exit 1; }
for pair in 'disk disk.path' 'bootMode boot' 'mac mac'; do
  read -r option field <<<"$pair"
  actual=$(nix eval --raw --no-write-lock-file "path:$source_dir#nixosConfigurations.$host.config.peerRelay.$option")
  expected=$(jq -er ".$field" "$record")
  [[ $actual == "$expected" ]] || { echo "NixOS $option disagrees with target record" >&2; exit 1; }
done
echo 'Evaluated relay disk, boot mode and MAC agree with the target record.'
