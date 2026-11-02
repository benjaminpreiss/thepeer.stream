#!/usr/bin/env bash
# Resolve the reviewed themoss input from this repo's committed flake.lock.
# Source from operator scripts; no mutable Git branch or curl | sh.
set -euo pipefail
relay_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)
toolkit_revision=$(jq -er '.nodes.themoss.locked.rev | select(test("^[0-9a-f]{40}$"))' "$relay_root/flake.lock")
# `nix flake metadata --json` does not consistently expose `.path` across
# Nix versions. Resolve the immutable flake source through builtins.getFlake.
toolkit_dir=$(nix eval --raw --impure --expr \
  "(builtins.getFlake \"github:benjaminpreiss/themoss.net/$toolkit_revision\").outPath")
[[ -f $toolkit_dir/toolkit/deploy.sh ]] || { echo 'Pinned themoss toolkit missing' >&2; exit 1; }
