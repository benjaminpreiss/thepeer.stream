#!/usr/bin/env bash
set -euo pipefail
[[ $# == 1 && $1 == --install ]] || { echo 'Usage: install.sh --install' >&2; exit 2; }
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
bash "$root/deployment/scripts/preflight.sh"
private="$root/deployment/generated"
target=$(<"$private/target")
disk=$(awk -F '"' '/^[[:space:]]*disk = / { print $2 }' "$private/machine.nix")
echo "WARNING: This will PERMANENTLY ERASE $disk on $target. Ensure off-server backup and console access."
read -r -p 'Type ERASE-AND-INSTALL-NIXOS: ' confirmation
[[ $confirmation == ERASE-AND-INSTALL-NIXOS ]] || { echo 'Cancelled'; exit 1; }
# Pinned installer; the evaluated OS uses this repository's committed flake.lock.
nix run github:nix-community/nixos-anywhere/58132e1d1f813b845382a2aeca4203b18d2ac807 -- \
  --flake "path:$root#relay" \
  --generate-hardware-config nixos-facter "$private/facter.json" \
  --target-host "$target"
# Keep the only machine-specific files outside the Git checkout on the target.
# If the network is not reachable post-reboot, use the provider console and
# retry these two commands manually rather than rerunning the disk installer.
ssh -o BatchMode=yes "$target" 'install -d -m 0700 /var/lib/thepeer-relay'
scp -q "$private/machine.nix" "$private/facter.json" "$target:/var/lib/thepeer-relay/"
ssh -o BatchMode=yes "$target" 'chmod 0600 /var/lib/thepeer-relay/{machine.nix,facter.json}'
if grep -q '^[[:space:]]*autoUpdate = true;' "$private/machine.nix"; then
  ssh -o BatchMode=yes "$target" 'systemctl start thepeer-relay-update.service'
else
  echo 'TEST DEPLOY: update timer disabled and no update started. Do not enable until ready to switch to main.'
fi
echo 'Verify relay and SSH host fingerprint through provider console.'
