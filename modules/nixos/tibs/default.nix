{
  config,
  lib,
  pkgs,
  inputs,
  system,
  ...
}:
with lib; let
  tibs = inputs.self.packages.${system}.tibs;
  driversEnv = pkgs.buildEnv {
    name = "graphics-drivers";
    paths = [ config.hardware.graphics.package ] ++ config.hardware.graphics.extraPackages;
  };
in {
  options.tibs = {
    enable = mkOption {
      type = types.bool;
      default = false;
      description = "Activate Tiago's Incredible Boot Screen";
    };
  };

  config = mkIf config.tibs.enable {
    assertions = [
      {
        assertion = !config.boot.plymouth.enable;
        message = "Conflict: Plymouth is enabled. Please disable Plymouth when TIBS is enabled.";
      }
    ];
    boot.kernelParams = mkForce ["quiet" "loglevel=0" "systemd.show_status=0" "udev.log_level=3" "vt.global_cursor_default=0"];
    systemd.services.tibs = {
      description = "Tiago's Incredible Boot Screen";
      before = [ "display-manager.target" "multi-user.target" "basic.target" ];
      wantedBy = [ "default.target" ];
      unitConfig.DefaultDependencies = "no";
      serviceConfig = {
        Type = "simple";
        ExecStart = pkgs.writeShellScript "tibs-service" ''
          export OPENGL_DRIVER_PATH=${driversEnv}
          ln -sfn $OPENGL_DRIVER_PATH /run/opengl-driver
          LD_LIBRARY_PATH="${lib.getLib pkgs.libGL}/lib" ${tibs}/bin/tibs
        '';
      };
    };
    boot.consoleLogLevel = 0;
    boot.initrd.systemd.enable = true;
    boot.initrd.systemd.dbus.enable = true;
    console.enable = false;
  };
}
