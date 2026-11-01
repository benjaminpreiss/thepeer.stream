{ config, lib, relayPackage, ... }:
let cfg = config.peerRelay; in {
  systemd.services.thepeer-relay = {
    description = "Saorsa MASQUE relay";
    wantedBy = [ "multi-user.target" ];
    after = [ "network-online.target" ];
    wants = [ "network-online.target" ];
    serviceConfig = {
      ExecStart = "${relayPackage}/bin/thepeer-relay ${cfg.bindIP}:${toString cfg.relayPort}";
      DynamicUser = true;
      Restart = "on-failure";
      RestartSec = 5;
      NoNewPrivileges = true;
      ProtectSystem = "strict";
      ProtectHome = true;
      PrivateTmp = true;
      PrivateDevices = true;
      RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" ];
      CapabilityBoundingSet = "";
      AmbientCapabilities = "";
      SystemCallArchitectures = "native";
      LimitNOFILE = 8192;
      MemoryMax = "1G";
      TasksMax = 512;
    };
  };
}
