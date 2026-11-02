#!/usr/bin/env bash
# Run from a deployment computer. Only x86_64 / IPv4 DHCP is supported.
set -euo pipefail
[[ $# == 2 ]] || {
  echo 'usage: prepare.sh root@HOST SSH_PUBLIC_KEY_FILE' >&2; exit 2;
}
# shellcheck source=deployment/scripts/toolkit.sh
source "$(dirname "${BASH_SOURCE[0]}")/toolkit.sh"
target=$1; keyfile=$2
[[ $target == root@* && $target != *[[:space:]]* ]] || { echo 'Target must be root@HOST' >&2; exit 1; }
private="$relay_root/generated"
[[ ! -e $private ]] || { echo 'Private machine configuration exists; refusing to replace it' >&2; exit 1; }
ssh -o StrictHostKeyChecking=yes "$target" 'lsblk -dn -o PATH,TYPE,SIZE; ip -4 route show default'
read -r -p 'Whole disk to ERASE (e.g. /dev/vda): ' disk
[[ $disk =~ ^/dev/[a-zA-Z0-9_-]+$ ]] || { echo 'Invalid disk' >&2; exit 1; }
read -r -p 'Hostname (letters, digits, dashes): ' hostname
[[ $hostname =~ ^[a-zA-Z0-9-]+$ ]] || exit 1
read -r -p 'Confirm target uses IPv4 DHCP (yes/no): ' dhcp
[[ $dhcp == yes ]] || { echo 'Static/IPv6-only networking is unsupported' >&2; exit 1; }
read -r kind key _ <"$keyfile"
[[ $kind == ssh-ed25519 && $key =~ ^[a-zA-Z0-9+/=]+$ ]] || { echo 'Expected Ed25519 public key' >&2; exit 1; }
umask 077
mkdir -m 700 "$private"
bash "$toolkit_dir/toolkit/deploy.sh" discover "$target" "$disk" "$private/record.json"
arch=$(jq -er .arch "$private/record.json")
boot=$(jq -er .boot "$private/record.json")
mac=$(jq -er .mac "$private/record.json")
[[ $arch == x86_64 && $mac =~ ^([0-9a-f]{2}:){5}[0-9a-f]{2}$ ]] || {
  echo 'Only x86_64 with one valid network MAC is supported' >&2; exit 1;
}
cat >"$private/machine.nix" <<EOF
{ ... }: {
  peerRelay = {
    hostName = "$hostname";
    disk = "$disk";
    bootMode = "$boot";
    mac = "$mac";
    sshKey = "$kind $key";
  };
}
EOF
printf 'Review %s and %s; confirm host, disk, boot and network before install.\n' "$private/record.json" "$private/machine.nix"
