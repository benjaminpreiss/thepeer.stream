#!/usr/bin/env bash
# Only after verifying the NEW SSH host key via the provider console.
set -euo pipefail
# shellcheck source=deployment/scripts/toolkit.sh
source "$(dirname "${BASH_SOURCE[0]}")/toolkit.sh"
private="$relay_root/generated"
bash "$toolkit_dir/toolkit/deploy.sh" bootstrap \
  "$private/install-receipt.json" "$relay_root" "$private" \
  /var/lib/thepeer-relay/source /var/lib/thepeer-relay/private /var/lib/thepeer-relay/update
# Bootstrap does not start the updater: the candidate timer stays off. The
# reviewed transition is the only path to enabling it after validation.
