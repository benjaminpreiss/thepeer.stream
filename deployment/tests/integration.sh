#!/usr/bin/env bash
# Consumer integration check; no server, secrets or destructive commands.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
cd "$root"
source deployment/scripts/toolkit.sh
# Shared fixtures need a writable copy; the pinned toolkit source is read-only.
created_generated=false
if [[ ! -d generated ]]; then
  mkdir -m 700 generated
  created_generated=true
fi
[[ ! -e generated/machine.nix && ! -e generated/record.json ]] || { echo 'Refusing to overwrite private machine config or record' >&2; exit 1; }
tmp=$(mktemp -d "$root/generated/integration.XXXXXXXX")
export TMPDIR="$tmp"
cleanup() {
  chmod -R u+rwX "$tmp"
  rm -rf "$tmp"
  rm -f "$root/generated/machine.nix" "$root/generated/record.json"
  if [[ $created_generated == true ]]; then rmdir "$root/generated"; fi
}
trap cleanup EXIT
cp -R "$toolkit_dir/." "$tmp/toolkit"
chmod u+w "$tmp/toolkit"
bash "$tmp/toolkit/tests/test.sh"
bash "$tmp/toolkit/tests/operator-smoke.sh"
printf '%s\n' '{"arch":"x86_64","disk":{"path":"/dev/vda"},"boot":"uefi","mac":"52:54:00:12:34:56"}' >"$tmp/record.json"
cp "$tmp/record.json" generated/record.json
printf '%s\n' 'ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG1vY2t0ZXN0LWtleS1ub3QtcmVhbC1rZXBsYWNl' >"$tmp/operator.pub"
jq '.arch = "aarch64"' "$tmp/record.json" >generated/record.json
if bash deployment/scripts/generate-machine.sh "$root/generated/record.json" "$tmp/operator.pub" relay-ci >/dev/null 2>&1; then
  echo 'Generator accepted unsupported architecture' >&2; exit 1
fi
cp "$tmp/record.json" generated/record.json
printf 'no\n' | bash deployment/scripts/generate-machine.sh "$root/generated/record.json" "$tmp/operator.pub" relay-ci >/dev/null 2>&1 && {
  echo 'Generator accepted unsupported network configuration' >&2; exit 1;
}
printf 'yes\n' | bash deployment/scripts/generate-machine.sh "$root/generated/record.json" "$tmp/operator.pub" relay-ci
[[ $(nix eval --raw 'path:.#nixosConfigurations.relay.config.peerRelay.hostName') == relay-ci ]]
[[ $(nix eval --json 'path:.#nixosConfigurations.relay.config.services.themossUpdate.enable') == true ]]
[[ $(nix eval --raw 'path:.#nixosConfigurations.relay.config.services.themossUpdate.state') == /var/lib/thepeer-relay/update ]]
[[ $(nix eval --json --apply 'timers: builtins.hasAttr "themoss-update" timers' 'path:.#nixosConfigurations.relay.config.systemd.timers') == true ]]
[[ $(nix eval --raw 'path:.#nixosConfigurations.relay.config.systemd.services.themoss-update.serviceConfig.ExecStart') == *' poll '* ]]
[[ $(nix eval --json --apply 'options: builtins.hasAttr "autoUpdate" options' 'path:.#nixosConfigurations.relay.options.peerRelay') == false ]]
bash deployment/scripts/check-machine.sh "$root" relay "$tmp/record.json"
# Mismatched disk and NIC/MAC fail before the destructive boundary.
for field in disk.path mac; do
  jq --arg field "$field" 'if $field == "mac" then .mac = "52:54:00:00:00:00" else .disk.path = "/dev/sdb" end' "$tmp/record.json" >"$tmp/bad.json"
  if bash deployment/scripts/check-machine.sh "$root" relay "$tmp/bad.json" >/dev/null 2>&1; then
    echo "Pre-erase check accepted mismatched $field" >&2; exit 1
  fi
done
nix build 'path:.#relay' --no-link
nix build 'path:.#nixosConfigurations.relay.config.system.build.toplevel' --no-link
echo 'thepeer relay + themoss integration passed'
