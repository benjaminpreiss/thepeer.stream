# Relay dependency updates

> ⚠️ **Not live-validated.** Synthetic CI does not prove the GitHub App, scheduled auto-merge or an installed host's next switch. Test them separately before relying on unattended updates.

**Shared mechanics:** the [pinned themoss reusable workflow](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/.github/workflows/lock-updates.yml) proposes lockfile PRs; the [toolkit App guide](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/app-permissions.md) explains its permission audit. The target's [selector/poller](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md) independently fetches a chosen branch and builds before switching. The workflow **never** connects to the server; the poller **never** runs Disko or updates Nix flake inputs.

## What this repository supplies

- [`.github/workflows/relay-nixos-updates.yaml`](../.github/workflows/relay-nixos-updates.yaml) calls the reusable workflow at the same immutable `05f25bf` toolkit revision as [`flake.lock`](../flake.lock). Its schedule and test command are relay-owned: `nix flake update`, Cargo **1.95** `cargo update`, then [`deployment/tests/integration.sh`](tests/integration.sh). Saorsa source upgrades or OS-policy changes require separate review.
- [`.github/app-permissions.json`](../.github/app-permissions.json) expects **Contents** and **Pull requests** write plus Metadata read. Create a [private, repo-scoped App](https://github.com/settings/apps/new?name=thepeer-relay-lock-updates&description=Automated%20reviewed%20relay%20dependency%20updates&url=https%3A%2F%2Fgithub.com%2Fbenjaminpreiss%2Fthepeer.stream&public=false&webhook_active=false&request_oauth_on_install=false&contents=write&pull_requests=write) with webhooks/OAuth-on-install off and **no** Workflows, Administration, Secrets or Environments permission. Install it on **only** `benjaminpreiss/thepeer.stream`; review the form before submitting.
- Store its ID as Actions **secret** `RELAY_UPDATE_APP_ID` and its private key as **secret** `RELAY_UPDATE_APP_PRIVATE_KEY`. Keep the key out of Git, operator machines and the host.
- Protect `main` with classic required checks **Direct/Relay core, desktop helper and host JNI** and **Build relay and NixOS**; allow auto-merge but grant the App no bypass, direct-main or force-push rights. The shared comma-delimited check audit can name only the second job because the first contains commas; branch protection **must require both**. Ruleset-only checks are not audited, and mandatory human approval blocks unattended auto-merge.

## Validate before trusting it

1. Manually dispatch **Propose and auto-merge relay NixOS updates** after the workflow reaches `main`. Verify its App audit, candidate build, both PR checks and queued auto-merge. A no-change success does not prove the next dependency update will work.
2. Only after real-host validation, [select `branch:main`](README.md#opt-in-to-reviewed-updates) on the target and confirm it picks up a reviewed commit. Bootstrap defaults to a safe `commit:SHA` pin, so no push alone changes the machine.
3. Monitor stalled PRs, failed workflow runs and `journalctl -u themoss-update` on the host. Use the upstream [switch-failure recovery steps](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md#if-an-update-fails) before resuming automation.

**Trust boundary:** anyone able to merge to the selected branch can change root-level NixOS code. Keep private machine inputs off GitHub Actions and themoss.net. Branch protection, CI and provider-console recovery reduce risk but do not guarantee boot or network access.
