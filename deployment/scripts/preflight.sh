#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
private="$root/deployment/generated"
[[ -f "$private/machine.nix" && -f "$private/target" && -f "$private/interface" && -f "$private/disk-size" ]] || { echo 'Run prepare.sh first' >&2; exit 1; }
target=$(<"$private/target")
iface=$(<"$private/interface")
value() { awk -F '"' -v k="$1" '$1 ~ "^[[:space:]]*" k " = " { print $2 }' "$private/machine.nix"; }
disk=$(value disk); mac=$(value mac); boot=$(value bootMode)
size=$(<"$private/disk-size")
[[ $target == root@* && $disk == /dev/* && $mac != '' ]] || exit 1
ssh -o BatchMode=yes "$target" "test \$(uname -m) = x86_64 && test \$(lsblk -dn -o TYPE '$disk') = disk && test \$(lsblk -bnd -o SIZE '$disk') = '$size' && test \$(cat '/sys/class/net/$iface/address') = '$mac'"
# Refuse a mounted target disk or any mounted descendant (including swap).
if ssh -o BatchMode=yes "$target" "lsblk -nr -o MOUNTPOINT '$disk' | grep -q '[^[:space:]]'"; then
  echo 'Target disk or partition is mounted; refusing install' >&2; exit 1
fi
actual=$(ssh -o BatchMode=yes "$target" 'if test -d /sys/firmware/efi; then echo uefi; else echo bios; fi')
[[ $actual == "$boot" ]] || { echo 'Boot mode changed' >&2; exit 1; }
route=$(ssh -o BatchMode=yes "$target" 'ip -4 route show default')
[[ $(printf '%s\n' "$route" | grep -c '^default ') == 1 && $route == *" dev $iface"* ]] || { echo 'Network route changed' >&2; exit 1; }
# Evaluating the OS happens without any disk formatting.
nix eval --raw "path:$root#nixosConfigurations.relay.config.networking.hostName" >/dev/null
printf 'Preflight passed: %s, erase %s, %s DHCP, %s\n' "$target" "$disk" "$iface" "$boot"
