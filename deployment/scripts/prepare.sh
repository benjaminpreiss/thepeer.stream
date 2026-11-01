#!/usr/bin/env bash
# Run from the deployment laptop. Only supports one IPv4 DHCP interface.
set -euo pipefail
[[ $# == 2 ]] || { echo 'usage: prepare.sh root@HOST SSH_PUBLIC_KEY_FILE' >&2; exit 2; }
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
target=$1
keyfile=$2
[[ $target == root@* && $target != *[[:space:]]* ]] || { echo 'Target must be root@HOST' >&2; exit 1; }
[[ ! -e "$root/deployment/generated" ]] || { echo 'Private deployment already exists; refusing to replace it' >&2; exit 1; }
ssh -o BatchMode=yes "$target" 'bash -s' <<'PROBE'
set -euo pipefail
printf 'Architecture: '; uname -m
printf 'Boot: '; if [[ -d /sys/firmware/efi ]]; then echo uefi; else echo bios; fi
printf 'IPv4 default routes:\n'; ip -4 route show default
printf 'Whole disks:\n'; lsblk -dn -o NAME,TYPE,SIZE
PROBE
arch=$(ssh -o BatchMode=yes "$target" uname -m)
[[ $arch == x86_64 ]] || { echo 'Only x86_64 supported' >&2; exit 1; }
route=$(ssh -o BatchMode=yes "$target" 'ip -4 route show default')
[[ $(printf '%s\n' "$route" | grep -c '^default ') == 1 ]] || { echo 'Exactly one IPv4 default route required' >&2; exit 1; }
iface=$(awk '{for (i=1;i<=NF;i++) if ($i=="dev") print $(i+1)}' <<<"$route")
[[ $iface =~ ^[a-zA-Z0-9_.-]+$ ]] || exit 1
mac=$(ssh -o BatchMode=yes "$target" "cat /sys/class/net/$iface/address")
[[ $mac =~ ^([0-9a-f]{2}:){5}[0-9a-f]{2}$ ]] || exit 1
boot=$(ssh -o BatchMode=yes "$target" 'if test -d /sys/firmware/efi; then echo uefi; else echo bios; fi')
read -r -p 'Whole disk to ERASE (e.g. /dev/vda): ' disk
[[ $disk =~ ^/dev/[a-zA-Z0-9_-]+$ ]] || { echo 'Invalid disk' >&2; exit 1; }
[[ $(ssh -o BatchMode=yes "$target" "lsblk -dn -o TYPE $disk") == disk ]] || { echo 'Not a whole disk' >&2; exit 1; }
read -r -p 'Hostname (letters, digits, dashes): ' hostname
[[ $hostname =~ ^[a-zA-Z0-9-]+$ ]] || exit 1
read -r -p 'Confirm target uses IPv4 DHCP (yes/no): ' dhcp
[[ $dhcp == yes ]] || { echo 'Static/IPv6-only networking is unsupported; stop' >&2; exit 1; }
read -r kind key _ <"$keyfile"
[[ $kind == ssh-ed25519 && $key =~ ^[a-zA-Z0-9+/=]+$ ]] || { echo 'Expected Ed25519 public key' >&2; exit 1; }
umask 077
mkdir -p "$root/deployment/generated"
printf '%s\n' "$target" >"$root/deployment/generated/target"
printf '%s\n' "$iface" >"$root/deployment/generated/interface"
ssh -o BatchMode=yes "$target" "lsblk -bnd -o SIZE '$disk'" >"$root/deployment/generated/disk-size"
cat >"$root/deployment/generated/machine.nix" <<EOF
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
printf 'Review %s and confirm disk %s, MAC %s, boot %s before install.\n' "$root/deployment/generated/machine.nix" "$disk" "$mac" "$boot"
