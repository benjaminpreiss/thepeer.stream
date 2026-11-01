{ config, lib, pkgs, ... }:
let cfg = config.peerRelay; in {
  options.peerRelay = {
    hostName = lib.mkOption { type = lib.types.strMatching "[a-zA-Z0-9-]+"; };
    disk = lib.mkOption { type = lib.types.str; };
    bootMode = lib.mkOption { type = lib.types.enum [ "uefi" "bios" ]; };
    mac = lib.mkOption { type = lib.types.str; };
    sshKey = lib.mkOption { type = lib.types.str; };
    sshPort = lib.mkOption { type = lib.types.port; default = 22; };
    relayPort = lib.mkOption { type = lib.types.port; default = 10000; };
    bindIP = lib.mkOption { type = lib.types.str; default = "0.0.0.0"; };
    autoUpdate = lib.mkOption { type = lib.types.bool; default = true; };
  };
  config = {
    system.stateVersion = "26.05";
    networking.hostName = cfg.hostName;
    networking.useNetworkd = true;
    networking.useDHCP = false;
    systemd.network.enable = true;
    systemd.network.wait-online.anyInterface = true;
    systemd.network.networks."10-relay" = {
      matchConfig.MACAddress = cfg.mac;
      networkConfig.DHCP = "ipv4";
    };
    time.timeZone = "UTC";
    nix.settings.experimental-features = [ "nix-command" "flakes" ];
    users.mutableUsers = false;
    users.users.root.openssh.authorizedKeys.keys = [ cfg.sshKey ];
    services.openssh = {
      enable = true;
      ports = [ cfg.sshPort ];
      settings = {
        PasswordAuthentication = false;
        KbdInteractiveAuthentication = false;
        PermitRootLogin = "prohibit-password";
        X11Forwarding = false;
      };
    };
    services.fail2ban.enable = true;
    security.apparmor.enable = true;
    networking.firewall = {
      enable = true;
      allowedTCPPorts = [ cfg.sshPort ];
      allowedUDPPorts = [ cfg.relayPort ];
    };
    boot.kernel.sysctl = {
      "kernel.dmesg_restrict" = 1;
      "kernel.kptr_restrict" = 2;
      "fs.protected_hardlinks" = 1;
      "fs.protected_symlinks" = 1;
      "net.ipv4.conf.all.accept_redirects" = 0;
      "net.ipv4.conf.default.accept_redirects" = 0;
    };
    environment.systemPackages = [ pkgs.git ];
    assertions = [
      { assertion = cfg.mac != ""; message = "Confirmed network MAC required"; }
      { assertion = lib.hasPrefix "/dev/" cfg.disk; message = "Whole disk device required"; }
    ];
  };
}
