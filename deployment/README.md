# Self-hosted MASQUE relay: NixOS pilot

This deployment uses the **pinned [themoss.net toolkit](../flake.nix)** for target discovery, destructive install and non-destructive Git updates. This repo owns the Saorsa relay, disk/network/SSH policy, machine-specific generation and update cadence. It is **not yet validated on a real server**. Only x86_64, a single whole disk and IPv4 DHCP are supported. Do not run on a machine with data you need to keep.

The relay listens on UDP 10000; allow that port in the provider firewall. Restrict TCP 22 (SSH) by source IP there if possible. No public admin endpoint or client-side default relay is configured. Clients use `--relay PUBLIC_IP:10000` explicitly. The host must have provider-console recovery and an independently verified off-server backup before disk erasure.

## Initial installation (Linux deployment computer)

Requires Bash, SSH, `jq`, Git, Nix with flakes and network access for pinned Nix inputs and `nixos-anywhere`. Rescue environment: root SSH with a verified host key, Bash, `ip` and `lsblk`. Verify the **rescue SSH host-key fingerprint out of band** before running any of these commands.

1. On an empty target booted into rescue, from this repository checkout run:

   ```bash
   bash deployment/scripts/prepare.sh root@HOST ~/.ssh/id_ed25519.pub --test-deploy
   ```

   The last flag disables the update timer and the first update for a **pre-merge candidate**. Without it, a production install updates itself from `main` once bootstrapped. The shared discover command records architecture, boot mode, NIC/MAC, disk identity/size and target in private `generated/record.json` (mode 0600). The consumer generates `generated/machine.nix`. Inspect both files; choose and independently verify the **whole disk** to erase. The shared toolkit does not choose a safe disk for you. If discovery fails, stop; it does not support ambiguous default routes. Generated files are Git-ignored. Do not add private keys, hashes or secrets to them; Nix inputs enter the world-readable store.
2. Run `bash deployment/scripts/preflight.sh`. It re-probes the remote record and compares the evaluated OS disk, boot mode and MAC to it. Review provider networking, backups and recovery access yourself. This does **not** format the disk.
3. Commit and push the exact revision being installed, then run `bash deployment/scripts/install.sh --install`. The pinned toolkit stages committed source with the private machine module, builds, probes the target again and requires typing `ERASE /dev/WHOLE_DISK` before calling Disko through pinned `nixos-anywhere`. **This erases the entire disk.** Never run this command as an update or repair.
4. The installed machine has a **new SSH host key**. Verify its fingerprint via the provider console before trusting it or updating `known_hosts`; then run `bash deployment/scripts/bootstrap.sh`. This non-destructively uploads the private module, clones the exact installed revision and records it as the known-good revision for the updater. If bootstrap fails, recover via the console and retry bootstrap; **do not reinstall**. No SSH host key is automatically accepted.
5. Confirm `systemctl status thepeer-relay` and `journalctl -u thepeer-relay -b`, SSH and firewall reachability. With `--test-deploy`, `themoss-update.timer` must be disabled. Test CLI and Android over real cellular Relay, plus ordinary Direct, before merging.

The shared toolkit supports an optional private `generated/facter.json` hardware report, but this wrapper **does not generate one automatically**. Verify the target boots with its actual storage/network drivers; for hardware needing facter, supply a reviewed report before installation. A Nix build alone does not validate boot or relay packet forwarding. Back up `generated/` securely off-server; never restore another host's record onto this one.

## Updates and branch-test transition

The consumer's [`update.nix`](modules/update.nix) wires the shared `themoss-update` service to the target's root-only checkout `/var/lib/thepeer-relay/source`, private `/var/lib/thepeer-relay/private` and state `/var/lib/thepeer-relay/update/revision`. With `peerRelay.autoUpdate = true`, the target runs a daily timer. The shared updater fetches reviewed `main`, requires a fast-forward from the **installed revision**, builds before switching and never runs Disko. Check `journalctl -u themoss-update -n 100` and `nixos-rebuild list-generations`. On failure, stop the timer and use SSH or the provider console to roll back a NixOS generation. A failed `switch` can have partially activated the system.

**Important for pre-merge tests:** Do not start `themoss-update` before merging; it would fetch old `main`. The shared updater also intentionally rejects divergent history. To transition a candidate install into unattended `main`, merge the tested branch with a **merge commit that retains the installed commit as an ancestor** (not squash or rebase), then set `autoUpdate = true;` in the root-only target machine module and in your off-server copy, and start `themoss-update.service` once. It should switch to the merged `main` and enable its timer. If the merge was squash/rebase, or the branch was deleted before bootstrap, **stop**: the normal updater cannot safely cross that history and a separately reviewed non-destructive rebaseline is required. Do not modify `update/revision` alone or reinstall to bypass ancestry checks.

Pin/toolkit upgrades are independent consumer lockfile changes. The upstream toolkit's [update guide](https://github.com/benjaminpreiss/themoss.net/blob/dac598fad0dfd8c6fb3bd415f5617eb53d0a0884/docs/updates.md) covers manual invocation and recovery. CI uses a synthetic machine module and shared fixture tests; it cannot prove real-server success.
