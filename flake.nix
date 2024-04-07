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
      inputs.nixpkgs.follows = "nixpkgs";
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
        config,
        self',
        inputs',
        pkgs,
        system,
        ...
      }: {
        formatter = pkgs.alejandra;

        packages.default = let
          craneLib =
            inputs.crane.lib.${system}.overrideToolchain
            inputs.fenix.packages.${system}.minimal.toolchain;
        in
          craneLib.buildPackage {
            src = ./.;
          };

        devShells.default = pkgs.mkShell {
          RUST_LOG = "info";
          buildInputs = with pkgs; [
            inputs.fenix.packages.${system}.complete.toolchain
            clippy
            rustc
          ];
        };
      };
      flake = {
        homeManagerModule = self.homeManagerModules.waydap; # an alias to the default module
        homeManagerModules = rec {
          waydap = import ./hmModule.nix inputs.self;
          default = waydap;
        };
      };
    };
}
