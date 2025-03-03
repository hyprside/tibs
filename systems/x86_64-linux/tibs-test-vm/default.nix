{ inputs, pkgs, system, ... }: {
  tibs.enable = true;
  imports = [
    "${inputs.nixpkgs}/nixos/modules/virtualisation/qemu-vm.nix"
  ];

  services.qemuGuest.enable = true;

  users.users.tibs = {
    isNormalUser = true;
    password = "tibs";
    extraGroups = [ "wheel" ];
  };

  networking.useDHCP = true;

  environment.systemPackages = with pkgs; [
    vim
    git
    htop
    curl
  ];

  boot.loader.systemd-boot.enable = true;
  boot.loader.efi.canTouchEfiVariables = true;
  hardware.graphics.enable = true;
  hardware.graphics.extraPackages = [ pkgs.mesa.drivers ];
  system.stateVersion = "25.05";
  virtualisation.useEFIBoot = true;
  virtualisation.useBootLoader = true;
  virtualisation.qemu.options = ["-device" "virtio-gpu-gl" "-display" "gtk,gl=on" "-vga" "none" "-full-screen"];
}
