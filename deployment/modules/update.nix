{ config, lib, pkgs, ... }:
let updater = pkgs.writeShellApplication {
  name = "thepeer-relay-update";
  runtimeInputs = with pkgs; [ bash git nix nixos-rebuild coreutils util-linux gnutar ];
  text = builtins.readFile ../scripts/update.sh;
}; in {
  systemd.services.thepeer-relay-update = {
    description = "Build and switch reviewed relay NixOS main";
    serviceConfig = {
      Type = "oneshot";
      ExecStart = "${updater}/bin/thepeer-relay-update";
      TimeoutStartSec = "2h";
    };
  };
  systemd.timers.thepeer-relay-update = {
    wantedBy = lib.optional config.peerRelay.autoUpdate "timers.target";
    timerConfig = {
      OnBootSec = "20min";
      OnUnitInactiveSec = "24h";
      RandomizedDelaySec = "1h";
      Persistent = true;
    };
  };
}
