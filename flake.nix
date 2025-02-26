{
  description = "Tiago's Incredible Boot Screen";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell rec {
          # OVMF_PATH = "${pkgs.OVMF.fd}/FV/OVMF.fd";
          # AAVMF_PATH = "${pkgs.OVMF.fd}/FV/AAVMF.fd";
          buildInputs = [
            # qemu_full
            # xorriso
            # gnumake
            # git
            mesa
            libGL
            cmake
            glfw
            pkg-config
            xorg.libX11
            xorg.libXrandr
            xorg.libXinerama
            xorg.libXcursor
            xorg.libXi
            python3
            ninja
            fontconfig
            freetype
            (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
          ];
          LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
        };
      }
    );
}
