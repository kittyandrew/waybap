# Take from: https://github.com/sioodmy/barbie/blob/main/flake.nix
{
  description = "Custom data provider for Waybar/Hyprland";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = {
    self,
    nixpkgs,
    fenix,
    crane,
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
    devShells = nixpkgs.lib.mapAttrs (_: o: o.devShells) perSystemAll;
  };
}
