#!/usr/bin/env bash
# Runs as root on the installed target. Never invokes Disko or nixos-anywhere.
set -euo pipefail
umask 077
STATE=${THEPEER_RELAY_STATE:-/var/lib/thepeer-relay}
REPO=${THEPEER_RELAY_REPO:-https://github.com/benjaminpreiss/thepeer.stream.git}
mkdir -p "$STATE"
exec 9>"$STATE/update.lock"
flock -n 9 || { echo 'Another relay update is in progress' >&2; exit 1; }
[[ -f "$STATE/machine.nix" && -f "$STATE/facter.json" ]] || {
  echo 'Missing private machine configuration or hardware report' >&2; exit 1;
}
if [[ ! -d "$STATE/source/.git" ]]; then
  [[ ! -e "$STATE/source" ]] || { echo 'Existing source is not a Git checkout' >&2; exit 1; }
  git clone --branch main "$REPO" "$STATE/source"
fi
cd "$STATE/source"
[[ $(git remote get-url origin) == "$REPO" ]] || { echo 'Unexpected origin' >&2; exit 1; }
[[ -z $(git status --porcelain) ]] || { echo 'Dirty source checkout; refusing update' >&2; exit 1; }
git fetch origin main
git merge-base --is-ancestor HEAD FETCH_HEAD || { echo 'Non-fast-forward source; refusing update' >&2; exit 1; }
# Build in a disposable tree. The only local files copied into it are machine
# facts and public SSH key; never let a fetch overwrite machine-specific state.
stage=$(mktemp -d "$STATE/stage.XXXXXXXX")
trap 'rm -rf "$stage"' EXIT
# Archive the exact fetched tree, not arbitrary files in the checkout.
git archive FETCH_HEAD | tar -x -C "$stage"
mkdir -p "$stage/deployment/generated"
install -m 0600 "$STATE/machine.nix" "$stage/deployment/generated/machine.nix"
install -m 0600 "$STATE/facter.json" "$stage/deployment/generated/facter.json"
revision=$(git rev-parse FETCH_HEAD)
echo "Building relay revision $revision"
nix build --no-link --no-write-lock-file "path:$stage#nixosConfigurations.relay.config.system.build.toplevel"
echo "Switching relay revision $revision"
nixos-rebuild switch --no-write-lock-file --flake "path:$stage#relay"
git merge --ff-only FETCH_HEAD
echo "Active relay source revision $revision"
