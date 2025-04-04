# Tiago's Incredible Boot Screen (TIBS)

**Tiago's Incredible Boot Screen (TIBS)** is a boot animation program and display manager written in Rust. Designed to replace tools like Plymouth, SDDM, and GDM, TIBS delivers a smooth, modern boot experience by rendering animations with OpenGL.

---

## Features

- **Early Boot Animation:**  
  Provides a seamless and visually appealing boot screen right from the start of the boot process.

- **Direct DRM Rendering:**  
  Uses DRM for direct screen output, bypassing traditional window managers to reduce latency and jank.

- **OpenGL-Powered Graphics:**  
  Delivers high-quality, dynamic animations with hardware acceleration.

- **Integrated Input Handling:**  
  Processes keyboard and mouse input using libinput, ensuring responsive control throughout the boot sequence.

- **Scalable Cursor Rendering:**  
  Supports modern, vector-based cursor themes via the Hyprcursors library. Hyprcursors renders the cursor to a bitmap on demand, ensuring crisp visuals without the need for multiple bitmap sizes.

---


---

## Installation

### Building from Source

1. **Clone the Repository:**

   ```bash
   git clone https://github.com/coffeeispower/tibs.git
   cd tibs
   ```

2. **Build with Cargo:**

   ```bash
   cargo build --release
   ```

   The compiled binary will be located at `target/release/tibs`.

---

## NixOS Integration

TIBS includes a built-in NixOS module for easy integration. To activate TIBS on NixOS, import the TIBS NixOS module from the flake and enable it in your configuration:

```nix
{
    # ...
    imports = [
        inputs.tibs.nixosModules.tibs
    ];
    tibs.enable = true;

    # IMPORTANT: Disable all display managers and Plymouth to avoid conflicts.
    services.displayManager.sddm.enable = false;
    services.displayManager.gdm.enable = false;
    services.displayManager.ly.enable = false;
    boot.plymouth.enable = false;

    # Include the appropriate graphics driver in the initramfs.
    # This might be different depending on your gpu and what
    # kind of driver your using
    # This is to prevent tibs from starting with a dummy driver
    # and then crashing when the real driver loads
    boot.initrd.kernelModules = [ "i915" ]; 
    boot.initrd.systemd.enable = true;

    # OpenGL is, of course, also required for tibs to work properly
    hardware.graphics = {
        enable = true;
        extraPackages = with pkgs; [
            intel-media-driver # LIBVA_DRIVER_NAME=iHD
            libvdpau-va-gl
        ];
    };

    # ...
}
```

This configuration launches TIBS as a systemd service immediately after the initramfs stage, providing an early boot animation and handling input and cursor rendering before the desktop environment takes over.

---

## Usage

- **Boot Process:**  
  TIBS runs as a systemd service early in the boot sequence. Its configuration ensures that it operates before any display manager is started, presenting a smooth boot animation.

- **Customizing Animations & UI:**  
  Modify the assets within the designated assets directory to adjust animations, progress bars, logos, and other UI elements.

- **Cursor Rendering:**  
  TIBS leverages the Hyprcursors library to load scalable, vector-based cursor themes. The library converts vector data to a bitmap at the desired size, which is then loaded into OpenGL for rendering.

---

## License

TIBS is distributed under the AGPL-3.0 License. See the [LICENSE](LICENSE) file for details.

---

## Contact

For issues, feature requests, or contributions, please open an issue on GitHub or contact me at [tiagodinis33@proton.me](mailto:tiagodinis33@proton.me) or chat with me on discord (username: coffeeispower).
