# Minimal MASQUE relay on NixOS

This is an **x86_64, single-disk, IPv4 DHCP** deployment. It runs the pinned
Saorsa transport relay from `crates/relay`, not an HTTP proxy. Direct-first
routing and the client's opt-in `--relay IP:PORT` behavior do not change. The
relay exposes UDP 10000 and SSH TCP 22; allow both in the provider firewall
(restrict SSH by source IP there if possible). There is no public admin API.

**This installer wipes the entire selected disk.** Use an empty VM first. Have
an off-machine backup and working provider console before proceeding. A public
IPv4 address/NAT mapping and inbound UDP reachability are required. Static
networking, multiple default IPv4 routes, IPv6-only, ARM and multiple disks
with ambiguous identities are not automated; stop and adapt the private
configuration for those cases. Discovery is advisory, not proof of a safe disk.

## Initial install

On a Linux/macOS deployment computer with Bash, SSH and Nix (flakes enabled):

1. Boot the target into the provider's rescue environment, enable root SSH
   using a trusted key, and verify the **SSH host key out of band**.
2. From this repository's root, run
   `bash deployment/scripts/prepare.sh root@HOST ~/.ssh/id_ed25519.pub`.
   For a **pre-merge test of this PR branch**, append `--test-deploy`.
   It disables the update timer and the installer's first update, so the
   target cannot fetch `main` over the candidate before validation.
   This queries architecture, boot mode, default route/NIC MAC and whole disks.
   Enter the full **whole disk** to erase, confirm DHCP and review
   `deployment/generated/machine.nix`. These files are Git-ignored, private
   per-target records; never put credentials in the flake or Nix store. The
   generated module contains a public SSH key, not a private key.
3. `bash deployment/scripts/preflight.sh` checks the target again and evaluates
   the NixOS config. Independently confirm disk identity, NIC MAC, DHCP,
   boot mode, provider console and off-server backups.
4. `bash deployment/scripts/install.sh --install` requires an explicit erase
   confirmation, calls pinned `nixos-anywhere` with Disko and produces a local
   facter report. **Do not use this for updates or repairs.** After reboot,
   it uploads the machine module and hardware report to `/var/lib/thepeer-relay/`
   on the target and (except in `--test-deploy` mode) starts the first non-destructive update. If the post-boot
   upload fails, recover via the console and copy these two files manually;
   do not reinstall.
5. Verify the installed machine's new SSH host fingerprint via the provider
   console; check `systemctl status thepeer-relay`,
   `systemctl status thepeer-relay-update.timer` (inactive in test mode) and
   `journalctl -u thepeer-relay-update -u thepeer-relay -b`.
   Test `thepeer --target localhost:PORT --relay PUBLIC_IP:10000` and Android
   cellular Relay after a real Direct failure; also verify normal Direct.
   Packet forwarding/physical end-to-end operation cannot be established by
   a Nix build alone.

Keep a protected off-server backup of `deployment/generated/` (including the
facter report). It is *not* committed and must never be copied from another
machine. To change disk, SSH key, MAC, bind IP or port, review the corresponding
`peerRelay` options and update **both** the private operator copy and the
installed `/var/lib/thepeer-relay/machine.nix` before starting an update. Disko
is present in the evaluation but is invoked only by the installer.

## Unattended updates and recovery

The target timer (after boot and then daily with jitter) runs as root when
`peerRelay.autoUpdate = true` (the default). Test deployments set it to false. It
fetches `main` from this public GitHub repository, requires a clean checkout
and a fast-forward, archives the fetched revision into a disposable stage,
adds its private machine config/facter report, and builds the locked flake
before `nixos-rebuild switch`. It logs revisions/results to journald. A failed
fetch/build cannot reach `switch`; a switch failure requires operator diagnosis
and may have partially activated services. The current Git checkout is
advanced only after `switch` succeeds. It never runs Disko. Review merges to
`main`: **anyone who can merge there can deploy arbitrary root-level OS code**.
`flake.lock` and the Cargo lock/source hash are committed. No unreviewed PR
branch is executed on the target. Check flake lock changes in review.

- After a successful pre-merge test and merge to `main`, enable updates by
  setting `autoUpdate = true;` in the private machine module on both the target
  and the deployment computer, then run
  `systemctl start thepeer-relay-update.service` on the target. It fetches and
  switches to the now-merged `main`, activating the timer. Confirm that the
  running revision and timer match expectations. Do **not** start the service
  before merging or it will switch to the *old* `main`.
- Trigger: `systemctl start thepeer-relay-update.service`
- Pause: `systemctl stop thepeer-relay-update.timer` and
  `systemctl disable thepeer-relay-update.timer` (re-enable after diagnosis).
- Pin: pause timer before changing tracked branch policy; do not rewrite
  `main` to force a downgrade. To roll back, use the provider console or SSH
  with `nixos-rebuild switch --rollback` (or choose an older generation in
  the boot menu), **then disable the timer** until the bad commit is fixed.
  Keep an off-server backup: on-server generations are not backups.
- Inspect: `journalctl -u thepeer-relay-update -n 100 --no-pager`,
  `git -C /var/lib/thepeer-relay/source log -1 --oneline` and
  `nixos-rebuild list-generations`.

The public repo is fetched without credentials. No private key is ever staged
or uploaded through Nix. Machine files are root-only; they contain hardware
facts and a *public* SSH key, not secrets. This workflow is deliberately more
automatic than [agentic's manual update guide](https://github.com/benjaminpreiss/agentic/blob/main/docs/update.md).
