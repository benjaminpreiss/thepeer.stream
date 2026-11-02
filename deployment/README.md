# MASQUE relay: NixOS pilot

**Draft; not validated on a real server.** Supports x86_64, one whole disk, IPv4 DHCP. The relay listens on UDP 10000 (allow it at the provider firewall); restrict SSH TCP 22 to operator IPs at the provider. Clients explicitly choose `--relay PUBLIC_IP:10000`; Direct-first behavior is unchanged. Arrange provider-console access and an off-host backup before installing.

The [pinned themoss toolkit](../flake.nix) owns discovery, destructive install, bootstrap, selector-based polling and non-destructive transition. This repo owns the Saorsa executable, disk/network/SSH policy, machine generation, evaluated pre-erase check, private host inputs and daily cadence. Do not copy secrets into Nix inputs: `machine.nix` and `facter.json` can enter the world-readable store. Private files and receipts under `generated/` are Git-ignored. No disk operation is exercised in CI.

## Prepare and install on a disposable machine

On a Linux operator computer (or macOS with a Linux Nix builder), install Nix with flakes, Bash, Git, SSH and `jq`. Verify the rescue SSH fingerprint **out of band** via provider console. Boot a disposable target in rescue, then from this checked-out, reviewed branch:

```bash
bash deployment/scripts/prepare.sh root@HOST ~/.ssh/id_ed25519.pub
source deployment/scripts/toolkit.sh
nix develop --no-write-lock-file "path:$toolkit_dir#operator" --command bash "$toolkit_dir/tests/operator-smoke.sh"
bash "$toolkit_dir/toolkit/deploy.sh" preflight generated/record.json
bash deployment/scripts/check-machine.sh "$PWD" relay generated/record.json
```

Review `generated/record.json` and `generated/machine.nix` against the provider console: target, **whole disk to erase**, boot mode, NIC/MAC, IPv4 DHCP and SSH public key. Preparation refuses existing private state. Unsupported/ambiguous target settings must be corrected, not guessed. An optional reviewed `generated/facter.json` must contain no secrets; this wrapper does not generate one automatically. Back up `generated/` off-host and check storage/network boot drivers and SSH recovery.

Commit and push the exact branch revision. Within the toolkit's pinned operator shell, run the shared installer; the last argument is our executable relay-specific pre-erase check:

```bash
nix develop --no-write-lock-file "path:$toolkit_dir#operator" --command \
  bash "$toolkit_dir/toolkit/deploy.sh" install generated/record.json "$PWD" \
  "$(git rev-parse HEAD)" generated relay "$(git branch --show-current)" \
  generated/install-receipt.json - - "$PWD/deployment/scripts/check-machine.sh"
```

**Only this command invokes Disko/nixos-anywhere and erases the entire chosen disk.** The shared installer checks the exact pushed branch tip and clean checkout, stages/builds, runs the pre-erase check against the **staged** NixOS output, re-probes and requires typing `ERASE /dev/WHOLE_DISK`. It writes a private receipt only after successful installation. If install succeeded but receipt creation failed, inspect through the console; **do not reinstall** to repair it. Never use install as an upgrade.

After first boot, verify the **new** SSH fingerprint via console before changing `known_hosts`. Then, from the same operator checkout and private files:

```bash
bash "$toolkit_dir/toolkit/deploy.sh" bootstrap generated/install-receipt.json "$PWD" \
  generated /var/lib/thepeer-relay/source /var/lib/thepeer-relay/private \
  /var/lib/thepeer-relay/update
```

Bootstrap validates the receipt's revision and file hashes, uploads the private machine files and initializes root-owned source/state. Interrupted uploads are retryable; unrelated target state is never overwritten. The timer runs from first boot but **does nothing before bootstrap**; bootstrap selects `commit:INSTALLED_SHA`, so polling still cannot fetch/build/switch. A pre-merge candidate remains pinned even when `main` changes. Inspect `systemctl status thepeer-relay themoss-update.timer` and `journalctl -u thepeer-relay -b`. Test real CLI and Android cellular Relay, ordinary Direct, SSH and provider firewall before merging.

## Select updates after validation

The root-owned `/var/lib/thepeer-relay/update/selector` controls the timer, independent of fetched source. `commit:SHA` polls do nothing; `branch:NAME` polls **only that branch**, builds before switching, checks its pushed tip again and refuses divergence. Protect any tracked branch with review/CI; merging to it grants root-level NixOS authority. The timer is always present; it never runs Disko.

On the target as root, resolve the toolkit pinned by the installed checkout and explicitly select a protected branch (after reviewing the candidate and its ancestry):

```bash
toolkit=$(nix eval --raw --impure --expr \
  '(builtins.getFlake "path:/var/lib/thepeer-relay/source").inputs.themoss.outPath')
bash "$toolkit/toolkit/deploy.sh" deploy /var/lib/thepeer-relay/source branch:main \
  /var/lib/thepeer-relay/private relay /var/lib/thepeer-relay/update
```

For a squash/rebase or a reviewed private `machine.nix` change, ordinary deploy must **fail closed**. From the operator computer, prepare a separate reviewed directory `REVIEWED_MACHINE_DIR` containing `machine.nix` (mode 0600, credential-free). Verify the host and new branch, then explicitly authorize the non-destructive receipt-bound transition:

```bash
bash "$toolkit_dir/toolkit/deploy.sh" transition generated/install-receipt.json \
  root@HOST /var/lib/thepeer-relay/source main REVIEWED_MACHINE_DIR \
  /var/lib/thepeer-relay/private relay /var/lib/thepeer-relay/update
```

It stages/builds the proposed file and advances selector/revision only after switch. See the pinned [transition and recovery guide](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md). The installed `facter.json`, if any, remains unchanged by transition. A later rebaseline after the receipt baseline changes needs a separately reviewed procedure. Do not edit selector/revision to bypass guards.

Watch `journalctl -u themoss-update -n 100`, `systemctl status themoss-update.timer`, `nixos-rebuild list-generations` and the root-only `update/selector` and `update/revision`. A failed fetch/build preserves state; a switch failure may partially activate services and leaves `update/needs-inspection` blocking future updates. Stop the timer, inspect via SSH/provider console, roll back to a known-good generation and reconcile state before clearing the marker. Back up private inputs and the receipt off-host; generations are not backups. Routine lockfile upgrades are reviewed PRs, not target-side `nix flake update`.
