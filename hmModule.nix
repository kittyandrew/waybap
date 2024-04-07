self: {
  config,
  lib,
  pkgs,
  inputs,
  ...
}: let
  inherit (lib) mkIf mkEnableOption mkOption maintainers;

  cfg = config.services.waybar-data-provider;
in {
  meta.maintainers = with maintainers; [kittyandrew];

  options.services.waybar-data-provider = with lib.types; {
    enable = mkEnableOption "barbie";
    package = mkOption {
      type = package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "Custom data provider for Waybar/Hyprland";
    };
  };

  config = mkIf cfg.enable {
    home.packages = [cfg.package];
    systemd.user.services.waybar-data-provider = {
      Unit = {
        Description = "Custom data provider for Waybar/Hyprland";
        Documentation = "https://github.com/kittyandrew/waybar-data-provider";
        After = ["network-online.target"];
      };

      Service = {
        ExecStart = "${cfg.package}/bin/waybar-data-provider";
        ExecReload = "${pkgs.coreutils}/bin/kill -SIGUSR2 $MAINPID";
        Restart = "on-failure";
        KillMode = "mixed";
      };

      Install = {WantedBy = ["hyprland-session.target"];};
    };
  };
}
