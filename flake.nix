# Take from: https://github.com/sioodmy/barbie/blob/main/flake.nix
{
  description = "Custom data provider for Waybar/Hyprland";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    flake-parts,
    self,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux" "aarch64-darwin" "x86_64-darwin"];
      perSystem = {
        pkgs,
        system,
        ...
      }: let
        inherit (pkgs) lib;

        rustToolchain = inputs.fenix.packages.${system}.stable.withComponents [
          "cargo"
          "clippy"
          "rustc"
          "rustfmt"
        ];

        craneLib =
          (inputs.crane.mkLib pkgs).overrideToolchain
          rustToolchain;

        waybap = craneLib.buildPackage {
          src = craneLib.cleanCargoSource ./.;
        };
      in {
        formatter = pkgs.alejandra;

        packages = {
          default = waybap;
          inherit waybap;
        };

        checks = lib.optionalAttrs pkgs.stdenv.isLinux {
          home-manager-module =
            (inputs.home-manager.lib.homeManagerConfiguration {
              inherit pkgs;
              modules = [
                self.homeManagerModules.waybap
                {
                  home = {
                    username = "waybap";
                    homeDirectory = "/tmp/waybap";
                    stateVersion = "24.11";
                  };
                  services.waybap.enable = true;
                }
              ];
            })
            .activationPackage;
        };

        devShells.default = pkgs.mkShell {
          RUST_LOG = "info";
          packages = with pkgs; [
            actionlint
            alejandra
            curl
            deadnix
            git
            rustToolchain
            zizmor
          ];
        };
      };
      flake = {
        homeManagerModule = self.homeManagerModules.waybap; # an alias to the default module
        homeManagerModules = rec {
          waybap = import ./hmModule.nix inputs.self;
          default = waybap;
        };
      };
    };
}
