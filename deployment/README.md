# Run your own MASQUE relay

> ⚠️ **Pilot, not production-ready.** No real-server install, boot, rollback or cellular CLI/Android relay test has been completed. Start with a disposable x86_64 VM with **one empty whole disk** and **IPv4 DHCP**. `install` erases the selected disk; never use it to repair an installed machine.

The relay listens on **UDP 10000**. Open that port at your provider and restrict **SSH TCP 22** to operator IPs. Clients choose it explicitly with `--relay PUBLIC_IP:10000`; Direct-first routing is unchanged. Arrange console recovery and an off-host backup *before* installing.

## What you need

- An operator machine with Nix (flakes enabled), Bash, Git, SSH and `jq`. On macOS, configure a Linux Nix builder for the NixOS build, or use a Linux VM.
- A rescue-booted target with root SSH, `ip` and `lsblk`. **Verify its SSH fingerprint out of band** via the provider console before connecting.
- A reviewed, pushed checkout of this branch and an Ed25519 **public** SSH key. Never put private keys, password hashes or credentials in `machine.nix` or `facter.json`: Nix store inputs may be world-readable.

This repo owns the relay, OS/disk/network policy and the small [machine generator](scripts/generate-machine.sh). The [pinned themoss toolkit](../flake.nix) owns discovery, installation, bootstrap, polling and transitions. Private records in `generated/` are ignored by Git; back them up separately. CI uses only synthetic records and mocks.

## 1. Discover and review — no disk changes

From the relay repo root, choose the **whole disk yourself** after checking the provider console. The shared toolkit discovers the architecture, boot mode, disk identity and default-route NIC/MAC; it refuses ambiguous routes and non-disks.

```bash
source deployment/scripts/toolkit.sh
mkdir -m 700 generated  # do not replace an existing private directory
bash "$toolkit_dir/toolkit/deploy.sh" discover root@HOST /dev/vda generated/record.json
bash deployment/scripts/generate-machine.sh "$PWD/generated/record.json" ~/.ssh/id_ed25519.pub relay-host
bash "$toolkit_dir/toolkit/deploy.sh" preflight generated/record.json
bash deployment/scripts/check-machine.sh "$PWD" relay generated/record.json
```

The relay generator requires explicit confirmation of IPv4 DHCP; static/IPv6-only networks are unsupported. **Stop and inspect** both `generated/record.json` and `generated/machine.nix` against the console: host, whole disk, boot mode, NIC/MAC, DHCP and SSH key. The generator rejects unsupported architecture/layout; the relay check compares the *evaluated* OS disk/boot/MAC against the record. Optional `generated/facter.json` must also be reviewed and credential-free; no hardware report is generated automatically. Check your storage/network drivers and back up `generated/` off-host.

## 2. Install — destructive

Commit and push the **exact** revision to install. Run the toolkit's [pinned operator shell](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md) smoke test first. Copy the relay check to an executable **ignored** path; this keeps the reviewed Git checkout clean:

```bash
nix develop --no-write-lock-file "path:$toolkit_dir#operator" --command bash "$toolkit_dir/tests/operator-smoke.sh"
install -m 0700 deployment/scripts/check-machine.sh generated/check-machine.sh
cmp deployment/scripts/check-machine.sh generated/check-machine.sh
nix develop --no-write-lock-file "path:$toolkit_dir#operator" --command \
  bash "$toolkit_dir/toolkit/deploy.sh" install generated/record.json "$PWD" \
  "$(git rev-parse HEAD)" generated relay "$(git branch --show-current)" \
  generated/install-receipt.json - - "$PWD/generated/check-machine.sh"
```

The shared installer checks the clean checkout against the **pushed branch tip**, stages/builds that commit, runs our check on the *staged* flake, re-probes the target and asks you to type `ERASE /dev/vda`. **Only this command runs Disko/nixos-anywhere.** It writes `generated/install-receipt.json` *after* installation succeeds; back it up off-host. If install succeeds but the receipt is lost, stop and inspect via console—**do not reinstall** just to recreate it.

## 3. Verify the new key, then bootstrap

The installed host has a **new** SSH key. Verify its fingerprint through the console *before* updating `known_hosts`. From the same operator checkout and private files:

```bash
bash "$toolkit_dir/toolkit/deploy.sh" bootstrap generated/install-receipt.json "$PWD" \
  generated /var/lib/thepeer-relay/source /var/lib/thepeer-relay/private \
  /var/lib/thepeer-relay/update
```

Bootstrap checks receipt hashes, uploads private files, clones the exact installed commit and writes the root-only revision **last**. Retry interrupted uploads; conflicting state needs investigation, not a reinstall. The timer exists from first boot but fails closed before bootstrap. Bootstrap pins `commit:INSTALLED_SHA`, so later polls do **no fetch, build or switch**, even if `main` advances.

**Validate before tracking a branch:** SSH, provider firewall, `systemctl status thepeer-relay themoss-update.timer`, `journalctl -u thepeer-relay -b`, CLI and Android over real cellular Relay, and normal Direct. No live validation has been performed for this PR.

## 4. Choose what the timer follows

The root-owned `/var/lib/thepeer-relay/update/selector` is independent of fetched source:

| Selector | Poll result |
| --- | --- |
| `commit:FULL_SHA` | Stay pinned; do not fetch, build or switch. |
| `branch:NAME` | Fetch **only that branch**; build and verify the pushed tip before switching. |

Protect a tracked branch with review and CI: a merge to it can change root-level OS code. On the target **as root**, after validating the installed candidate and its ancestry, explicitly opt in to `main`:

```bash
toolkit=$(nix eval --raw --impure --expr \
  '(builtins.getFlake "path:/var/lib/thepeer-relay/source").inputs.themoss.outPath')
bash "$toolkit/toolkit/deploy.sh" deploy /var/lib/thepeer-relay/source branch:main \
  /var/lib/thepeer-relay/private relay /var/lib/thepeer-relay/update
```

Ordinary polling/deploy refuses divergent history. To adopt a squash/rebase or a reviewed private `machine.nix` change, prepare a separate **credential-free** `REVIEWED_MACHINE_DIR/machine.nix` (mode 0600), verify the target and new branch, then explicitly authorize a *non-destructive, receipt-bound* transition from the operator machine:

```bash
bash "$toolkit_dir/toolkit/deploy.sh" transition generated/install-receipt.json \
  root@HOST /var/lib/thepeer-relay/source main REVIEWED_MACHINE_DIR \
  /var/lib/thepeer-relay/private relay /var/lib/thepeer-relay/update
```

It builds before switching and saves the proposed machine file, selector and revision only after a successful switch; it **never** runs Disko. The installed `facter.json` remains unchanged. A later rebaseline, after the receipt's baseline has changed, requires a separate reviewed procedure. Do not edit `selector` or `revision` to bypass safeguards.

## Recovery and trust boundary

- **Fetch/build fails:** the running generation and known-good state remain unchanged.
- **Switch starts and fails:** services may be partly activated. The toolkit leaves `update/needs-inspection` to stop further automation. Stop the timer; inspect `journalctl -u themoss-update -n 100`, `nixos-rebuild list-generations`, running system, selector and revision. Recover via SSH/console and reconcile state *before* clearing the marker.
- **Backup:** keep `generated/`, including the install receipt, off-host. NixOS generations are not off-host backups. Dependency updates come through reviewed lockfile PRs, never `nix flake update` on the server.

See the pinned [toolkit update and recovery guide](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md) for the full selector and rollback contract.
