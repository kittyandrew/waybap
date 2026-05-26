# Take from: https://github.com/sioodmy/barbie/blob/main/flake.nix
{
  description = "Custom data provider for Waybar/Hyprland";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    home-manager = {
      url = "github:nix-community/home-manager";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    fenix,
    crane,
    home-manager,
  }: let
    forAllSystems = nixpkgs.lib.genAttrs ["x86_64-linux" "aarch64-linux"];

    perSystem = system: let
      pkgs = nixpkgs.legacyPackages.${system};
      inherit (pkgs) lib;

      rustToolchain = fenix.packages.${system}.stable.withComponents [
        "cargo"
        "clippy"
        "rustc"
        "rustfmt"
      ];

      craneLib =
        (crane.mkLib pkgs).overrideToolchain
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
          (home-manager.lib.homeManagerConfiguration {
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

    perSystemAll = forAllSystems perSystem;
  in {
    formatter = nixpkgs.lib.mapAttrs (_: o: o.formatter) perSystemAll;
    packages = nixpkgs.lib.mapAttrs (_: o: o.packages) perSystemAll;
    checks = nixpkgs.lib.mapAttrs (_: o: o.checks) perSystemAll;
    devShells = nixpkgs.lib.mapAttrs (_: o: o.devShells) perSystemAll;

    homeManagerModule = self.homeManagerModules.waybap; # an alias to the default module
    homeManagerModules = rec {
      waybap = import ./hmModule.nix self;
      default = waybap;
    };
  };
}
