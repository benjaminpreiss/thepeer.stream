#!/usr/bin/env bash
set -euo pipefail
[[ $# == 1 && $1 == --install ]] || { echo 'Usage: install.sh --install' >&2; exit 2; }
# shellcheck source=deployment/scripts/toolkit.sh
source "$(dirname "${BASH_SOURCE[0]}")/toolkit.sh"
bash "$relay_root/deployment/scripts/preflight.sh"
private="$relay_root/generated"
rev=$(git -C "$relay_root" rev-parse HEAD)
branch=$(git -C "$relay_root" branch --show-current)
[[ $branch =~ ^[a-zA-Z0-9_/-]+$ && $branch != *..* ]] || { echo 'Checkout a named branch' >&2; exit 1; }
[[ -z $(git -C "$relay_root" status --porcelain) ]] || { echo 'Commit/review changes before install' >&2; exit 1; }
remote=$(git -C "$relay_root" ls-remote origin "refs/heads/$branch" | cut -f1)
[[ $remote == "$rev" ]] || { echo 'Push the exact install revision before erasing a disk' >&2; exit 1; }
echo 'DANGER: The pinned themoss installer will erase the disk in generated/record.json.'
# The shared installer builds, re-probes, and requires a typed disk confirmation.
nix shell github:nix-community/nixos-anywhere/58132e1d1f813b845382a2aeca4203b18d2ac807 --command \
  bash "$toolkit_dir/toolkit/deploy.sh" install "$private/record.json" "$relay_root" "$rev" "$private" relay
umask 077
printf '%s\n' "$rev" >"$private/install-revision"
printf '%s\n' "$branch" >"$private/install-branch"
# The host key changes on reinstall. Never auto-accept it. Operator checks it
# through the provider console and then runs the non-destructive bootstrap.
echo 'Installation finished. Verify the NEW SSH host key through provider console,'
echo 'then run: bash deployment/scripts/bootstrap.sh'
