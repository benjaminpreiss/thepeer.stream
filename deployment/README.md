# Run a MASQUE relay on NixOS

> ⚠️ **Pilot, not a production installer.** No real-server boot, rollback or cellular CLI/Android Relay test has been completed. Start with a disposable x86_64 machine with **one empty whole disk** and **IPv4 DHCP**. Only the toolkit's `install` command erases a disk; never reinstall to repair a host.

The relay listens on **UDP 10000**. Open it at your provider and restrict **SSH TCP 22** to operator IPs. Clients explicitly choose `--relay PUBLIC_IP:10000`; Direct-first routing is unchanged. Arrange console recovery and an off-host backup before installing.

The [pinned themoss toolkit](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md) owns discovery, staged build, disk confirmation, install and bootstrap. Its [update guide](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md) covers polling, transitions and recovery. **Read both before operating a host.** This repo supplies only relay-specific machine generation, the evaluated pre-erase check, the OS/relay modules and private host inputs.

## Relay-specific install inputs

| Item | Value / operator decision |
| --- | --- |
| Target | x86_64; one whole disk (such as `/dev/vda`); IPv4 DHCP; provider-confirmed NIC/MAC. |
| NixOS output | `relay` from this repository's pinned `flake.nix` and `flake.lock`. |
| SSH access | An operator-owned Ed25519 **public** key; verify rescue and installed host fingerprints independently via console. |
| Private files | Ignored `generated/record.json`, `machine.nix`, optional `facter.json` and install receipt. Back them up off-host. **Never** put credentials in Nix inputs; the store can be world-readable. |
| Relay check | `deployment/scripts/check-machine.sh` compares the *staged* OS disk, boot mode and MAC to the discovered record. |

### Discover, generate and review (non-destructive)

From this reviewed checkout, with Nix (flakes), Bash, Git, SSH and `jq` installed:

```bash
source deployment/scripts/toolkit.sh
mkdir -m 700 generated  # stop if private state already exists
bash "$toolkit_dir/toolkit/deploy.sh" discover root@HOST /dev/vda generated/record.json
bash deployment/scripts/generate-machine.sh "$PWD/generated/record.json" ~/.ssh/id_ed25519.pub relay-host
bash "$toolkit_dir/toolkit/deploy.sh" preflight generated/record.json
bash deployment/scripts/check-machine.sh "$PWD" relay generated/record.json
```

The generator prompts for **IPv4 DHCP confirmation** and rejects unsupported records. Compare target, whole disk, boot mode, NIC/MAC, DHCP and SSH key with the provider console; do not guess. A reviewed `generated/facter.json` is optional and must contain no secrets. Check boot drivers and back up private inputs. On macOS, the toolkit's operator shell needs a Linux Nix builder (or use a Linux VM); run its [operator smoke test](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md#operator-environment) before installation.

### Install the exact pushed commit (destructive)

Commit/push the reviewed branch. Make an executable *ignored* copy of the relay check so Git stays clean; then use the toolkit's [pinned operator shell](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md#operator-environment):

```bash
install -m 0700 deployment/scripts/check-machine.sh generated/check-machine.sh
cmp deployment/scripts/check-machine.sh generated/check-machine.sh
nix develop --no-write-lock-file "path:$toolkit_dir#operator" --command \
  bash "$toolkit_dir/toolkit/deploy.sh" install generated/record.json "$PWD" \
  "$(git rev-parse HEAD)" generated relay "$(git branch --show-current)" \
  generated/install-receipt.json - - "$PWD/generated/check-machine.sh"
```

**Stop before typing `ERASE /dev/vda`** unless the target, whole disk, backup and console access are independently confirmed. The toolkit builds and checks the *staged* flake, re-probes the disk, and writes `generated/install-receipt.json` only after a successful install. Back up the receipt. If it is lost after installation, **do not reinstall** to recreate it; see the upstream [recovery guidance](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md#2-install-the-pushed-commit).

### Verify the new SSH key and bootstrap

After boot, verify the **new** host fingerprint via provider console before updating `known_hosts`:

```bash
bash "$toolkit_dir/toolkit/deploy.sh" bootstrap generated/install-receipt.json "$PWD" \
  generated /var/lib/thepeer-relay/source /var/lib/thepeer-relay/private \
  /var/lib/thepeer-relay/update
```

Bootstrap pins `commit:INSTALLED_SHA` in the root-owned state. The daily timer exists but **cannot fetch/build/switch** until an operator explicitly selects a branch. Check SSH, provider firewall, `thepeer-relay`, CLI + Android over real cellular Relay, and normal Direct *before* enabling updates. No live validation has been performed for this PR.

## Opt in to reviewed updates

The [shared selector policy](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md) keeps a commit pinned or polls **only** the protected branch chosen on the target. On the host **as root**, after validating its installed commit and branch ancestry:

```bash
toolkit=$(nix eval --raw --impure --expr \
  '(builtins.getFlake "path:/var/lib/thepeer-relay/source").inputs.themoss.outPath')
bash "$toolkit/toolkit/deploy.sh" deploy /var/lib/thepeer-relay/source branch:main \
  /var/lib/thepeer-relay/private relay /var/lib/thepeer-relay/update
```

If history diverges (for example after a squash/rebase) or `machine.nix` needs a reviewed change, ordinary polling **fails closed**. Use the toolkit's explicit [non-destructive transition](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md#change-branches-after-a-squash-or-rebase) with your receipt, relay paths and a reviewed credential-free `machine.nix`; do not edit `selector` or `revision` by hand. Only a successful switch advances them. A failed switch can partly activate services and leaves `update/needs-inspection` blocking further automation; follow the upstream [recovery checklist](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md#if-an-update-fails). NixOS generations are not off-host backups.
