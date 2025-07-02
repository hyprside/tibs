# This is a devshell I use when i work on my aquamarine fork
{
  pkgs,
  mkShell,
  lib,
  inputs,
  system,
  ...
}:
mkShell rec {
  packages = with pkgs; [
    mesa
    libGL.dev
    cmake
    pkg-config
    python3
    ninja
    fontconfig
    freetype

    rustc
    rustfmt
    cargo
    clippy
    rust-analyzer
    libinput
    libxkbcommon
    cairo
    hyprcursor
    inputs.hyprutils.packages.${system}.default
    inputs.aquamarine.packages.${system}.default
    inputs.hyprwayland-scanner.packages.${system}.default
    libdrm.dev
    libgbm
    libunwind.dev
    libbacktrace
    hwdata
    libdisplay-info
    wayland
    wayland-protocols
    wayland-scanner
    udev
    seatd
  ];
  LD_LIBRARY_PATH = lib.makeLibraryPath packages;
  LIBCLANG_PATH = "${pkgs.llvmPackages_16.libclang.lib}/lib";
}
