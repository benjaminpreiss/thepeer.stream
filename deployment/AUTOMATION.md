# Unattended, Git-first relay OS updates

The target uses the flake-pinned themoss.net updater. A scheduled GitHub
workflow opens lockfile PRs; after CI and required checks, the target fetches
reviewed `main`, builds and switches. It does **not** run `nix flake update` on
the server. The toolkit pin has changed; a workflow-authorized maintainer must
apply the staged caller in [`workflow-handoff/`](workflow-handoff/README.md) and
remove that folder before merge. **CI must pass and a scheduled update must be
exercised before claiming this works live.**

The reusable workflow is pinned to [themoss.net `05f25bf`](https://github.com/benjaminpreiss/themoss.net/commit/05f25bf2e700deb55c59882d00e781a1e1583d07),
which fixes Nix installation on GitHub runners. It runs the consumer-supplied
`nix flake update` and compatible `cargo update`, tests the resulting relay and
OS closure, audits the App permissions, opens a PR, and refuses auto-merge if
it cannot verify **classic branch-protection required checks**. Ruleset-only
protection is not yet supported by its audit. A new Saorsa Git revision or
relaxation of deployment policy is outside automated lockfile updates.

## Configure the repository

1. [Open this prefilled GitHub App creation form](https://github.com/settings/apps/new?name=thepeer-relay-lock-updates&description=Automated%20reviewed%20relay%20dependency%20updates&url=https%3A%2F%2Fgithub.com%2Fbenjaminpreiss%2Fthepeer.stream&public=false&webhook_active=false&request_oauth_on_install=false&contents=write&pull_requests=write)
   (or [edit an existing App](https://github.com/settings/apps)). Verify the
   form: private App, webhooks and OAuth-on-install disabled; repository
   **Contents** and **Pull requests** read/write, no Workflows,
   Administration, Secrets or Environments permission. Install on **Only select
   repositories**, selecting `benjaminpreiss/thepeer.stream`. Do not paste its
   key into chat, commit it or put it on the target. The committed
   [App permission policy](../.github/app-permissions.json) expects exactly
   Contents/Pull requests write and Metadata read for this installation.
2. Add the App ID as Actions **secret** `RELAY_UPDATE_APP_ID` and the private
   PEM as Actions **secret** `RELAY_UPDATE_APP_PRIVATE_KEY` (Settings → Secrets
   and variables → Actions). The former workflow used an Actions *variable*
   for the ID; the pinned shared workflow requires both values as secrets.
   The App credential is GitHub-only; no server details or credentials enter
   GitHub Actions. CI uses a fake machine configuration.
3. Enable repository **Allow auto-merge** (Settings → General → Pull requests).
   Configure **classic branch protection** for `main` with required checks
   `Direct/Relay core, desktop helper and host JNI` and `Build relay and
   NixOS` (verify exact names in GitHub's status-check selector). The shared
   workflow's comma-delimited `required-checks` input can only name `Build
   relay and NixOS`: the existing native-check name contains commas. GitHub
   branch protection **must still require both**; auto-merge waits for both.
   Do not grant the App bypass, force-push or direct-main rights. Ruleset-only checks are
   insufficient for this pinned workflow's current branch-protection audit.
   Mandatory human approval would block unattended dependency updates.
4. After both workflow files are merged to `main`, manually dispatch
   **Propose and auto-merge relay NixOS updates** once. Confirm its App audit,
   candidate build, PR checks and queued auto-merge actually work. Then verify
   the target's timer picks up the merged commit. Enable monitoring/alerts for
   failed or stalled workflow runs, PRs and host update failures; no-update
   success is not proof that future dependency updates can pass.

**Trust boundary:** unattended dependency updates can eventually run root-level
NixOS code. Source revisions and both lockfiles remain reviewed Git commits;
CI, branch protection, a staged deployment and provider-console rollback are
necessary but cannot guarantee boot or network health. The pinned Saorsa Git
revision requires a separate compatibility-tested change. The App key stays
only in the consumer repo's Actions secret; machine-specific records stay on
the deployment computer and/or server, never in themoss.net.
