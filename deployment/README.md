# Run a MASQUE relay on NixOS

> ⚠️ **Pilot only. Do not install on a machine with data to keep.** No real-server boot, rollback or cellular CLI/Android Relay test has been completed. Start with a disposable x86_64 VM, one empty whole disk and IPv4 DHCP. Only `install` erases disks—never reinstall to repair a host.

**Before you start:** arrange provider-console access, verify the rescue SSH host key *out of band*, and back up anything you need. Open **UDP 10000** for the relay; restrict **SSH TCP 22** to operator IPs. Clients explicitly use `--relay PUBLIC_IP:10000`; Direct-first routing stays unchanged.

## Which guide owns what?

| Read | For |
| --- | --- |
| [themoss: deploy a machine](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md) | Operator shell, discovery, install receipt, SSH-key verification and bootstrap. **Read before installing.** |
| [themoss: updates and recovery](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md) | Commit pins, branch tracking, non-destructive transitions and rollback. |
| **This page** | Relay-only choices and exact paths to pass to that pinned toolkit. |

The toolkit owns the lifecycle; this repo supplies the `relay` NixOS output, [machine generator](scripts/generate-machine.sh), [evaluated pre-erase check](scripts/check-machine.sh) and daily update policy. All machine-specific files live in **Git-ignored `generated/`**. Keep credentials out of `machine.nix` and optional `facter.json`: Nix store inputs can be world-readable. Back up private records and the install receipt *off-host*.

> **Why so many command arguments?** A private, Git-ignored operator manifest is proposed in [themoss.net #14](https://github.com/benjaminpreiss/themoss.net/issues/14). Until that tested interface exists, these are the pinned toolkit's actual commands—not a second local installer or a shell-sourced configuration file.

## 1. Discover and review (no disk changes)

From the repository root on a Linux operator machine with Nix (flakes), Bash, Git, SSH and `jq`:

```bash
# Choose these yourself; verify the whole disk against the provider console.
target=root@HOST
disk=/dev/vda
public_key=~/.ssh/id_ed25519.pub
hostname=relay-host

source deployment/scripts/toolkit.sh  # resolve the committed themoss pin
mkdir -m 700 generated              # stop if private state already exists
bash "$toolkit_dir/toolkit/deploy.sh" discover "$target" "$disk" generated/record.json
bash deployment/scripts/generate-machine.sh "$PWD/generated/record.json" "$public_key" "$hostname"
```

The relay generator confirms **IPv4 DHCP** and rejects unsupported records. Review `generated/record.json` and `generated/machine.nix` against the console: target, whole disk, boot mode, NIC/MAC, DHCP and SSH *public* key. Static/IPv6-only networks and ambiguous layouts are unsupported. If your hardware needs `generated/facter.json`, supply a reviewed, credential-free report; this repo does not generate one automatically.

Before any install, re-probe and compare the evaluated relay OS settings:

```bash
bash "$toolkit_dir/toolkit/deploy.sh" preflight generated/record.json
bash deployment/scripts/check-machine.sh "$PWD" relay generated/record.json
```

Stop on any mismatch. Check boot drivers and back up `generated/` off-host. On macOS, use the toolkit's [operator shell](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md#operator-environment) with a Linux Nix builder (or a Linux VM).

## 2. Install the reviewed commit (⚠️ erases the selected disk)

Commit and push the *exact* branch revision you reviewed. Run the toolkit's [operator smoke test](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md#operator-environment), then prepare an executable, **ignored** copy of the relay check without dirtying Git:

```bash
nix develop --no-write-lock-file "path:$toolkit_dir#operator" --command bash "$toolkit_dir/tests/operator-smoke.sh"
install -m 0700 deployment/scripts/check-machine.sh generated/check-machine.sh
cmp deployment/scripts/check-machine.sh generated/check-machine.sh
```

The variables below name the toolkit's positional inputs. Review their values before running the *only* destructive command:

```bash
record="$PWD/generated/record.json"
repo="$PWD"
revision=$(git rev-parse HEAD)
private="$PWD/generated"
output=relay
branch=$(git branch --show-current)
receipt="$private/install-receipt.json"
check="$private/check-machine.sh"

nix develop --no-write-lock-file "path:$toolkit_dir#operator" --command \
  bash "$toolkit_dir/toolkit/deploy.sh" install \
  "$record" "$repo" "$revision" "$private" "$output" "$branch" "$receipt" \
  - - "$check"  # no hardware generator or recovery-file tree; run the relay check
```

**Stop before typing `ERASE /dev/vda`** unless target, disk, off-host backup and console access are independently confirmed. The toolkit checks the pushed tip, builds and evaluates the *staged* flake, re-probes the disk and writes the receipt *only after* a successful install. If the receipt is lost, **do not reinstall** to recreate it; see [themoss recovery guidance](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/deployment.md#2-install-the-pushed-commit).

## 3. Verify the new SSH key; then bootstrap

After first boot, confirm the **new** host fingerprint via provider console *before* updating `known_hosts`. From the same operator checkout and private files:

```bash
bash "$toolkit_dir/toolkit/deploy.sh" bootstrap "$receipt" "$repo" "$private" \
  /var/lib/thepeer-relay/source /var/lib/thepeer-relay/private /var/lib/thepeer-relay/update
```

Bootstrap pins `commit:INSTALLED_SHA`. The daily timer cannot fetch/build/switch until an operator explicitly selects a branch. Validate SSH, provider firewall, `thepeer-relay`, real CLI/Android cellular Relay and ordinary Direct **before** enabling updates. None of that real-host validation has happened for this PR.

## 4. Choose a reviewed branch when ready

Read the shared [selector and recovery guide](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md). A commit pin stays put; a protected `branch:main` polls **only** `main`. On the target **as root**, after validating ancestry:

```bash
toolkit=$(nix eval --raw --impure --expr \
  '(builtins.getFlake "path:/var/lib/thepeer-relay/source").inputs.themoss.outPath')
bash "$toolkit/toolkit/deploy.sh" deploy /var/lib/thepeer-relay/source branch:main \
  /var/lib/thepeer-relay/private relay /var/lib/thepeer-relay/update
```

Diverged history or a changed private `machine.nix` needs the toolkit's **explicit, non-destructive** [transition](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md#change-branches-after-a-squash-or-rebase); never edit the root-only selector/revision to bypass it. If a switch partly fails, stop the timer and follow the upstream [inspection/rollback checklist](https://github.com/benjaminpreiss/themoss.net/blob/05f25bf2e700deb55c59882d00e781a1e1583d07/docs/updates.md#if-an-update-fails). NixOS generations are not off-host backups.
