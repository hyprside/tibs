{
  description = "Tiago's Incredible Boot Screen";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    snowfall-lib = {
      url = "github:snowfallorg/lib";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";

    hyprutils = {
      url = "github:hyprwm/hyprutils";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    aquamarine.url = "github:hyprwm/aquamarine";
    aquamarine.inputs.hyprutils.follows = "hyprutils";
    
    hyprwayland-scanner = {
      url = "github:hyprwm/hyprwayland-scanner";
      inputs.nixpkgs.follows = "nixpkgs";
    };

  };

  outputs = inputs:
    inputs.snowfall-lib.mkFlake {
      inherit inputs;
      src = ./.;
      alias.packages.default = "tibs";
      alias.modules.nixos.default = "tibs";
      outputs-builder = channels: {
        apps.run-vm = {
          type = "app";
          program = let runVmScript = channels.nixpkgs.writeShellScript "run-vm" ''
            set -e
            nix build .#nixosConfigurations.tibs-test-vm.config.system.build.vm
            result/bin/run-tibs-test-vm-vm
          ''; in "${runVmScript}";
        };
      };
    };
}
