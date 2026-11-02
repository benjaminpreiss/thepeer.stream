#!/usr/bin/env bash
# Relay-only policy: convert the shared discovery record into a private NixOS module.
set -euo pipefail
[[ $# == 3 ]] || { echo 'usage: generate-machine.sh RECORD SSH_PUBLIC_KEY_FILE HOSTNAME' >&2; exit 2; }
record=$1 keyfile=$2 hostname=$3
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
private="$root/generated"
[[ $record == "$private/record.json" && -f $record && ! -L $record ]] || {
  echo 'Discover into generated/record.json first' >&2; exit 1;
}
[[ ! -e $private/machine.nix && ! -L $private/machine.nix ]] || {
  echo 'Private machine configuration already exists; refusing to replace it' >&2; exit 1;
}
[[ $hostname =~ ^[a-zA-Z0-9-]+$ ]] || { echo 'Invalid hostname' >&2; exit 1; }
read -r kind key _ <"$keyfile"
[[ $kind == ssh-ed25519 && $key =~ ^[a-zA-Z0-9+/=]+$ ]] || {
  echo 'Expected Ed25519 public key' >&2; exit 1;
}
arch=$(jq -er .arch "$record")
boot=$(jq -er .boot "$record")
disk=$(jq -er .disk.path "$record")
mac=$(jq -er .mac "$record")
[[ $arch == x86_64 && ( $boot == uefi || $boot == bios ) &&
   $disk =~ ^/dev/[a-zA-Z0-9_-]+$ && $mac =~ ^([0-9a-f]{2}:){5}[0-9a-f]{2}$ ]] || {
  echo 'Only x86_64, a whole disk and one valid network MAC are supported' >&2; exit 1;
}
read -r -p 'Confirm this NIC uses IPv4 DHCP (yes/no): ' dhcp
[[ $dhcp == yes ]] || { echo 'Static/IPv6-only networking is unsupported' >&2; exit 1; }
# Values are checked before interpolation. No private keys or hashes enter Nix.
umask 077
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
printf 'Review %s and %s against the provider console before install.\n' "$record" "$private/machine.nix"
