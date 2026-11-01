# Enable unattended, Git-first relay OS updates

An operator with workflow write access must merge these two workflow files
to `main` (the deployment App does not need workflow write permission):

- `.github/workflows/relay-nixos.yaml`: required PR check; runs updater tests
  and builds the relay and NixOS system with a fake machine module.
- `.github/workflows/relay-nixos-updates.yaml`: daily lockfile refresh, build,
  PR creation and auto-merge after required checks. It modifies only
  `flake.lock` and `Cargo.lock`, not the pinned Saorsa Git revision, OS source,
  deployment policy or machine configuration.

## One-time repository configuration

1. [Open the prefilled GitHub App creation form](https://github.com/settings/apps/new?name=thepeer-relay-lock-updates&description=Automated%20reviewed%20relay%20dependency%20updates&url=https%3A%2F%2Fgithub.com%2Fbenjaminpreiss%2Fthepeer.stream&public=false&webhook_active=false&request_oauth_on_install=false&contents=write&pull_requests=write)
   (or [edit an existing App](https://github.com/settings/apps)). If the name
   is taken, choose another. **Verify the prefilled settings rather than
   assuming the URL enforces them:** private App, webhooks disabled,
   OAuth-on-install disabled, repository **Contents: read/write** and
   **Pull requests: read/write**, with no Workflows, Administration, Secrets
   or Environments permissions. Install it on **Only select repositories**
   and select `benjaminpreiss/thepeer.stream`. On the App's General page, copy
   its **App ID** and generate/download a private key (PEM). Never paste the
   key into chat or commit it. The App needs no Workflows permission because
   its automated PRs touch only lockfiles.
2. In `benjaminpreiss/thepeer.stream` → Settings → Secrets and variables →
   Actions, add the App ID as repository **variable** `RELAY_UPDATE_APP_ID`
   and the complete PEM as repository **secret**
   `RELAY_UPDATE_APP_PRIVATE_KEY`. This App token, unlike the default
   `GITHUB_TOKEN`, triggers PR CI. It is a GitHub-only automation credential;
   the server never needs it.
3. Enable repository **Allow auto-merge** under Settings → General → Pull
   Requests. Configure a `main` branch ruleset requiring the existing `Build and
   test / Direct/Relay core, desktop helper and host JNI` job and the new
   `Relay NixOS / Build relay and NixOS` job to pass before merge. Use the exact
   status-check names shown in the GitHub UI. Require the branch to be current
   with `main` if available. Ensure the automation App cannot bypass checks.
   A mandatory human-approval rule on these PRs would prevent unattended
   updates; choose the branch policy consciously.
4. After both workflow files land on `main`, run **Propose and auto-merge relay
   NixOS updates** once with workflow_dispatch. Confirm it creates a lockfile-
   only PR, required checks run, and auto-merge waits for them. Confirm the
   installed server's update timer subsequently switches to the merged commit.
5. Enable notifications/monitoring for failed or stalled scheduled workflows,
   failed PR checks, and `thepeer-relay-update.service` on the server. A failed
   build must not silently leave security updates pending indefinitely.

**Trust boundary:** this fully unattended path allows new Nixpkgs, Disko,
Facter and compatible Cargo dependencies to reach a root-level NixOS rebuild
after CI. The pinned Saorsa Git revision requires a separate, compatibility-
tested change. CI cannot prove live relay availability; stage/canary and a
provider-console rollback path are strongly recommended before production use.
The server's timer does not choose dependencies itself: it only builds and
switches committed `main` revisions with their pinned lockfiles.
