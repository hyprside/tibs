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
    boot.kernelParams = mkForce ["quiet" "loglevel=0" "systemd.show_status=0"];
        
    boot.postBootCommands = ''
      echo "Iniciando TIBS..."
      export OPENGL_DRIVER_PATH=${driversEnv}
      echo "OPENGL_DRIVER_PATH=$OPENGL_DRIVER_PATH"
      ln -sfn $OPENGL_DRIVER_PATH /run/opengl-driver
      ls /run/opengl-driver/
      LD_LIBRARY_PATH="${lib.getLib pkgs.libGL}/lib" ${tibs}/bin/tibs &
    '';
  };
}
