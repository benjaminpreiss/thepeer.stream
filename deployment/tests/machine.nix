# Only for CI evaluation; not an actual operator key or machine.
{ ... }: {
  peerRelay = {
    hostName = "relay-ci";
    disk = "/dev/vda";
    bootMode = "uefi";
    mac = "52:54:00:12:34:56";
    sshKey = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIG1vY2t0ZXN0LWtleS1ub3QtcmVhbC1rZXBsYWNl";
  };
}
