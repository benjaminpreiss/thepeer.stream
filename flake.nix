{
  description = "thepeer.stream MASQUE relay NixOS";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
    disko.url = "github:nix-community/disko";
    disko.inputs.nixpkgs.follows = "nixpkgs";
    facter.url = "github:numtide/nixos-facter-modules";
  };
  outputs = { nixpkgs, disko, facter, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      relay = pkgs.rustPlatform.buildRustPackage {
        pname = "thepeer-relay";
        version = "0.1.0";
        src = pkgs.lib.fileset.toSource {
          root = ./.;
          fileset = pkgs.lib.fileset.unions [ ./Cargo.toml ./Cargo.lock ./crates ./patches ];
        };
        cargoLock = {
          lockFile = ./Cargo.lock;
          outputHashes = {
            "saorsa-transport-0.37.0" = "sha256-E1lkrwKlBeVX6QC2I6dnOrO3Oi5A3vhSr2M82FBcnsY=";
          };
        };
        cargoBuildFlags = [ "-p" "thepeer-relay" ];
        cargoTestFlags = [ "-p" "thepeer-relay" ];
      };
    in {
      packages.${system}.relay = relay;
      nixosConfigurations.relay = nixpkgs.lib.nixosSystem {
        inherit system;
        modules = [
          disko.nixosModules.disko
          facter.nixosModules.facter
          ./deployment/modules/base.nix
          ./deployment/modules/disk.nix
          ./deployment/modules/relay.nix
          ./deployment/modules/update.nix
          ./deployment/generated/machine.nix
          ({ lib, ... }: lib.mkIf (builtins.pathExists ./deployment/generated/facter.json) {
            hardware.facter.reportPath = ./deployment/generated/facter.json;
          })
          { _module.args.relayPackage = relay; }
        ];
      };
    };
}
