#!/usr/bin/env bash
# Run only after independently verifying the newly installed SSH host key.
# Initializes target-local state without repartitioning or rebuilding.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
private="$root/generated"
for file in record.json machine.nix install-revision install-branch; do
  [[ -f $private/$file && ! -L $private/$file ]] || { echo "Missing $file; never reinstall to repair bootstrap" >&2; exit 1; }
done
target=$(jq -er .target "$private/record.json")
rev=$(<"$private/install-revision"); branch=$(<"$private/install-branch")
[[ $rev =~ ^[0-9a-f]{40}$ && $branch =~ ^[a-zA-Z0-9_/-]+$ && $branch != *..* ]] || exit 1
ssh -o StrictHostKeyChecking=yes "$target" 'install -d -m 0700 /var/lib/thepeer-relay/private /var/lib/thepeer-relay/update'
scp -q "$private/machine.nix" "$target:/var/lib/thepeer-relay/private/machine.nix"
if [[ -f $private/facter.json ]]; then
  scp -q "$private/facter.json" "$target:/var/lib/thepeer-relay/private/facter.json"
fi
ssh -o StrictHostKeyChecking=yes "$target" bash -s -- "$branch" "$rev" <<'REMOTE'
set -euo pipefail
branch=$1 rev=$2
umask 077
chmod 0600 /var/lib/thepeer-relay/private/*
[[ ! -e /var/lib/thepeer-relay/source ]] || { echo 'Existing source checkout; refusing overwrite' >&2; exit 1; }
git clone --branch "$branch" https://github.com/benjaminpreiss/thepeer.stream.git /var/lib/thepeer-relay/source
[[ $(git -C /var/lib/thepeer-relay/source rev-parse HEAD) == "$rev" ]] || { echo 'Installed revision disagrees with remote' >&2; exit 1; }
printf '%s\n' "$rev" >/var/lib/thepeer-relay/update/revision
chmod 0600 /var/lib/thepeer-relay/update/revision
REMOTE
if grep -q '^[[:space:]]*autoUpdate = true;' "$private/machine.nix"; then
  ssh -o StrictHostKeyChecking=yes "$target" 'systemctl start themoss-update.service'
else
  echo 'TEST DEPLOY: timer disabled; do not start update against old main.'
fi
