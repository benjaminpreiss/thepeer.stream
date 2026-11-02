# Relay-specific paths and cadence; selector is root-owned target state.
{ ... }: {
  services.themossUpdate = {
    enable = true;
    repository = "/var/lib/thepeer-relay/source";
    machine = "/var/lib/thepeer-relay/private";
    host = "relay";
    state = "/var/lib/thepeer-relay/update";
    interval = "daily";
  };
}
