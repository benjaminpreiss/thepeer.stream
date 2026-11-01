# Relay-specific policy for the shared, flake-pinned themoss updater.
{ config, ... }: {
  services.themossUpdate = {
    enable = true;
    timer = config.peerRelay.autoUpdate;
    repository = "/var/lib/thepeer-relay/source";
    branch = "main";
    machine = "/var/lib/thepeer-relay/private";
    host = "relay";
    state = "/var/lib/thepeer-relay/update";
    interval = "daily";
  };
}
