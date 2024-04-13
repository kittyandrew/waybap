self: {
  config,
  lib,
  pkgs,
  inputs,
  ...
}: let
  inherit (lib) mkIf mkEnableOption mkOption maintainers;

  cfg = config.services.waybap;
in {
  meta.maintainers = with maintainers; [kittyandrew];

  options.services.waybap = with lib.types; {
    enable = mkEnableOption "waybap";
    package = mkOption {
      type = package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      description = "Custom data provider for Waybar/Hyprland";
    };
  };

  config = mkIf cfg.enable {
    home.packages = [cfg.package];
    systemd.user.services.waybap = {
      Unit = {
        Description = "Custom data provider for Waybar/Hyprland";
        Documentation = "https://github.com/kittyandrew/waybap";
        After = ["network-online.target"];
      };

      Service = {
        ExecStart = "${cfg.package}/bin/waybap serve 127.0.0.1:6963";
        ExecReload = "${pkgs.coreutils}/bin/kill -SIGUSR2 $MAINPID";
        Restart = "on-failure";
        KillMode = "mixed";
      };

      Install = {WantedBy = ["hyprland-session.target"];};
    };
  };
}
