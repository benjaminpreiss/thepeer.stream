# Workflow-authorized handoff for PR #4

The deployment toolkit pin changed to `3074f7c386b4a7d8c0b3a2cd2f7f72ef3d1f6768` (merged themoss.net #7). The GitHub App cannot update `.github/workflows/`, so this branch stages the **complete replacement** workflow here without changing the live workflow.

With workflow-authorized credentials, on branch `feat/relay-nixos-deployment`:

```bash
cp deployment/workflow-handoff/relay-nixos-updates.yaml .github/workflows/relay-nixos-updates.yaml
rm -rf deployment/workflow-handoff
# Review the diff, then commit and push to PR #4.
```

Only the `uses:` and `toolkit-revision:` pins change in that workflow. Confirm the reusable workflow at that revision still passes CI, and run the scheduled workflow manually before enabling unattended operation. Keep PR #4 draft until real-server and CLI/Android Relay validation is complete.
