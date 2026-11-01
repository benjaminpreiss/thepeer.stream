# Enable unattended, Git-first relay OS updates

The local worktree contains two workflow files that the current GitHub App
cannot push (`workflows` permission is missing). An operator with workflow
write access must add **both** files from this worktree to `main`:

- `.github/workflows/relay-nixos.yaml`: required PR check; runs updater tests
  and builds the relay and NixOS system with a fake machine module.
- `.github/workflows/relay-nixos-updates.yaml`: daily lockfile refresh, build,
  PR creation and auto-merge after required checks. It modifies only
  `flake.lock` and `Cargo.lock`, not the pinned Saorsa Git revision, OS source,
  deployment policy or machine configuration.

## One-time repository configuration

1. Create/install a **dedicated GitHub App** scoped to this repository, granting
   repository **Contents: read/write** and **Pull requests: read/write**.
   It needs no Workflows permission because its automated PRs touch only lockfiles.
   Store its App ID as repository Actions **variable** `RELAY_UPDATE_APP_ID`
   and its private key as repository Actions **secret**
   `RELAY_UPDATE_APP_PRIVATE_KEY`. Never put the key in Git or the Nix store.
   This App token, unlike the default `GITHUB_TOKEN`, triggers PR CI.
2. Enable repository **Allow auto-merge** under Settings → General → Pull
   Requests. Configure a `main` branch ruleset requiring the existing `Build and
   test / Direct/Relay core, desktop helper and host JNI` job and the new
   `Relay NixOS / Build relay and NixOS` job to pass before merge. Use the exact
   status-check names shown in the GitHub UI. Require the branch to be current
   with `main` if available. Ensure the automation App cannot bypass checks.
   A mandatory human-approval rule on these PRs would prevent unattended
   updates; choose the branch policy consciously.
3. After both workflow files land on `main`, run **Propose and auto-merge relay
   NixOS updates** once with workflow_dispatch. Confirm it creates a lockfile-
   only PR, required checks run, and auto-merge waits for them. Confirm the
   installed server's update timer subsequently switches to the merged commit.
4. Enable notifications/monitoring for failed or stalled scheduled workflows,
   failed PR checks, and `thepeer-relay-update.service` on the server. A failed
   build must not silently leave security updates pending indefinitely.

**Trust boundary:** this fully unattended path allows new Nixpkgs, Disko,
Facter and compatible Cargo dependencies to reach a root-level NixOS rebuild
after CI. The pinned Saorsa Git revision requires a separate, compatibility-
tested change. CI cannot prove live relay availability; stage/canary and a
provider-console rollback path are strongly recommended before production use.
The server's timer does not choose dependencies itself: it only builds and
switches committed `main` revisions with their pinned lockfiles.
