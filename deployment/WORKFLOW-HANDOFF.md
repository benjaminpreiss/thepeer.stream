# Workflow edit needed before CI can pass

The GitHub App used by this coding agent cannot push changes to
`.github/workflows/`. The relay PR now uses the pinned themoss toolkit and
**deleted** `deployment/tests/update.sh`; a workflow-authorized maintainer must
edit both existing workflow files on the PR branch:

- `.github/workflows/relay-nixos.yaml`: in the `Test updater and build the
  installed OS` step, replace the old `bash deployment/tests/update.sh`,
  `deployment/generated/machine.nix` fixture copy and two `nix build` commands
  with `bash deployment/tests/integration.sh`. Keep its preceding shell syntax
  check.
- `.github/workflows/relay-nixos-updates.yaml`: in `Build candidate before
  proposing it`, likewise replace the old updater test, fixture copy and two
  Nix builds with `bash deployment/tests/integration.sh`.

`integration.sh` runs the pinned shared toolkit's safety fixtures, evaluates
both candidate (timer off) and production (timer on) policies, and builds the
relay and NixOS output with a synthetic machine. It refuses to replace a real
private machine module. Run `actionlint` after editing.

The scheduled lockfile workflow still carries its consumer-local implementation
because the shared reusable workflow at the pinned toolkit revision does not
install Nix on `ubuntu-latest` ([themoss.net#5](https://github.com/benjaminpreiss/themoss.net/issues/5)). Once that issue is merged, pin the new toolkit revision and replace the scheduled workflow with a thin `workflow_call` caller and `.github/app-permissions.json`; do **not** enable the broken reusable workflow in the meantime.
