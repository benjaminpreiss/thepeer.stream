# Reviewed updates, unattended on the host

> ⚠️ **Not live-validated.** The GitHub App and scheduled workflow must be tested before relying on automatic dependency updates. CI passing does *not* prove a server will boot or a cellular Relay session will work.

Two separate loops keep the deployment current:

| Loop | What it does | What it never does |
| --- | --- | --- |
| **GitHub workflow** | Proposes Nix/Cargo lockfile changes as PRs; tests them and requests auto-merge only after checking classic branch protection. | Connect to the host or supply its machine secrets. |
| **Target timer** | If the root-owned selector tracks `branch:main`, fetches reviewed `main`, builds, then switches. | Run Disko or update flake inputs on the host. |

Bootstrap pins `commit:SHA`, so the timer **does not fetch** until an operator explicitly selects a protected branch. See the [operator guide](README.md#4-choose-what-the-timer-follows).

## Before enabling auto-merge

1. **Install the workflow update.** The workflow-authorized maintainer has installed the pinned caller in `.github/workflows/`. Its [themoss.net `05f25bf`](https://github.com/benjaminpreiss/themoss.net/commit/05f25bf2e700deb55c59882d00e781a1e1583d07) revision installs Nix on the GitHub runner before running the consumer-supplied `nix flake update`, Cargo 1.95 `cargo update` and [relay build](tests/integration.sh). A new Saorsa source revision or a relaxed deployment policy is **not** an automated lockfile update.
2. **Create a narrowly scoped GitHub App.** [Open the prefilled form](https://github.com/settings/apps/new?name=thepeer-relay-lock-updates&description=Automated%20reviewed%20relay%20dependency%20updates&url=https%3A%2F%2Fgithub.com%2Fbenjaminpreiss%2Fthepeer.stream&public=false&webhook_active=false&request_oauth_on_install=false&contents=write&pull_requests=write) or edit an existing App. Verify: *private*, webhook/OAuth-on-install off, **Contents** and **Pull requests** write, no Workflows/Administration/Secrets/Environments permission. Install on **only** `benjaminpreiss/thepeer.stream`. The [committed policy](../.github/app-permissions.json) expects Contents/Pull requests write and Metadata read. Keep the private key out of Git, chats, operator machines and the target.
3. **Set Actions secrets:** `RELAY_UPDATE_APP_ID` and `RELAY_UPDATE_APP_PRIVATE_KEY` (the ID must be a *secret*, not an Actions variable). Enable **Allow auto-merge**. Protect `main` with classic required checks `Direct/Relay core, desktop helper and host JNI` and `Build relay and NixOS`; grant the App no direct-main/force-push/bypass rights. The shared workflow's comma-delimited audit can name only `Build relay and NixOS` because the other check contains commas; branch protection must still require **both**. Ruleset-only protection is not audited; mandatory human approval blocks unattended auto-merge.
4. **Prove the loop before scheduling it.** After the workflows reach `main`, manually dispatch **Propose and auto-merge relay NixOS updates**. Verify App audit, candidate build, both PR checks, queued auto-merge and the host's later timer switch. Monitor failed or stalled runs, PRs and target updates; a successful no-change run does not prove future updates will work.

## Trust boundary and recovery

A reviewer who can merge to the tracked branch can change **root-level NixOS code**. The target independently fetches that branch; no webhook or GitHub App chooses its revision. CI, branch protection, a staged rollout and provider-console rollback reduce risk but cannot guarantee boot/network health. Keep private machine facts on the operator computer and/or the server, *never* in themoss.net or Actions. If a switch becomes uncertain, stop the timer and follow the [recovery checklist](README.md#recovery-and-trust-boundary) before resuming.
