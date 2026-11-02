# Workflow-authorized handoff for PR #4

The consumer now pins themoss.net `05f25bf2e700deb55c59882d00e781a1e1583d07`. The GitHub App can push ordinary files but cannot update `.github/workflows/`. With workflow-authorized credentials, on `feat/relay-nixos-deployment`:

```bash
cp deployment/workflow-handoff/relay-nixos-updates.yaml .github/workflows/relay-nixos-updates.yaml
rm -rf deployment/workflow-handoff
git add -A && git diff --cached --check && git commit -m 'Pin relay update caller to shared toolkit' && git push
```

The staged workflow differs only in its `uses` and `toolkit-revision` pins. Run CI and manually dispatch the update workflow before trusting unattended updates. Do not merge until a disposable-server install, rollback and real CLI/Android cellular Relay have been validated.
