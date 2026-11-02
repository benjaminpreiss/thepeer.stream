#!/usr/bin/env bash
# After testing a candidate and merging it into main without rewriting history.
# NEW_MACHINE_DIR must contain a reviewed, credential-free machine.nix (0600).
set -euo pipefail
[[ $# == 1 ]] || { echo 'Usage: transition.sh NEW_MACHINE_DIR' >&2; exit 2; }
# shellcheck source=deployment/scripts/toolkit.sh
source "$(dirname "${BASH_SOURCE[0]}")/toolkit.sh"
private="$relay_root/generated"
target=$(jq -er .target "$private/install-receipt.json")
bash "$toolkit_dir/toolkit/deploy.sh" transition \
  "$private/install-receipt.json" "$target" /var/lib/thepeer-relay/source main \
  "$1" /var/lib/thepeer-relay/private relay /var/lib/thepeer-relay/update
