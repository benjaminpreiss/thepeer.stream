# Workflow-authorized handoff before CI/merge

The GitHub App used by this coding agent cannot push `.github/workflows/`.
A maintainer with workflow write permission must copy the **exact** files
committed under `deployment/workflow-templates/` on this PR branch to:

```text
deployment/workflow-templates/relay-nixos.yaml
  → .github/workflows/relay-nixos.yaml
deployment/workflow-templates/relay-nixos-updates.yaml
  → .github/workflows/relay-nixos-updates.yaml
```

The old CI still calls the deleted `deployment/tests/update.sh`, so it cannot
pass until the first replacement is made. The new CI runs
`deployment/tests/integration.sh` with a synthetic machine, checks both timer
policies, the pinned themoss toolkit fixtures and a full NixOS build.

The new scheduled caller pins the **fixed** shared themoss workflow at
`bdf91bcd0a36d729e9fed9a28b8ca3c508543160` ([issue #5](https://github.com/benjaminpreiss/themoss.net/issues/5))
and removes duplicated lockfile-update logic. It requires
`.github/app-permissions.json` and a dedicated App ID/private key in Actions
**secrets** (`RELAY_UPDATE_APP_ID`, `RELAY_UPDATE_APP_PRIVATE_KEY`), not an ID
in Actions variables as the previous workflow used. See
[`AUTOMATION.md`](AUTOMATION.md) for permissions and required classic branch
protection. Run `actionlint` on both files and verify the scheduled workflow
with `workflow_dispatch` after they are on `main`. No real App, server or
live auto-merge has been tested here.
