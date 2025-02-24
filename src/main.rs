#![allow(unsafe_op_in_unsafe_fn)]
use std::ffi::CString;
use std::os::fd::{AsFd, AsRawFd};
use std::ptr::NonNull;
pub mod gl;
pub use drm::Device;
pub use drm::control::Device as ControlDevice;

#[derive(Debug)]
/// A simple wrapper for a device node.
pub struct Card(std::fs::File);

/// Implementing `AsFd` is a prerequisite to implementing the traits found
/// in this crate. Here, we are just calling `as_fd()` on the inner File.
impl std::os::unix::io::AsFd for Card {
    fn as_fd(&self) -> std::os::unix::io::BorrowedFd<'_> {
        self.0.as_fd()
    }
}

/// With `AsFd` implemented, we can now implement `drm::Device`.
impl Device for Card {}
impl ControlDevice for Card {}

/// Simple helper methods for opening a `Card`.
impl Card {
    pub fn open(path: &str) -> Self {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        options.write(true);
        Card(options.open(path).unwrap())
    }

    pub fn open_global() -> (egl::device::Device, Self) {
        let mut devices = egl::device::Device::query_devices().expect("Query EGL devices");
        loop {
            let Some(egl_device) = devices.next() else {
                panic!("No EGL devices found");
            };
            dbg!(&egl_device);
            dbg!(egl_device.drm_render_device_node_path());
            let Some(drm) = dbg!(egl_device.drm_device_node_path()) else {
                continue;
            };
            break (egl_device, Self::open(drm.as_os_str().to_str().unwrap()));
        }
    }
}

use drm::control::{connector, crtc};
use gbm::{AsRaw, BufferObjectFlags};
use glutin::api::egl;
use glutin::config::{ConfigSurfaceTypes, ConfigTemplateBuilder};
use glutin::context::ContextAttributesBuilder;
use glutin::prelude::*;
use glutin::surface::{PbufferSurface, SurfaceAttributesBuilder, WindowSurface};
use raw_window_handle::{
    DrmDisplayHandle, GbmDisplayHandle, GbmWindowHandle, RawDisplayHandle, RawWindowHandle,
};

fn find_egl_config(egl_display: &egl::display::Display) -> egl::config::Config {
    unsafe { egl_display.find_configs(ConfigTemplateBuilder::new().build()) }
        .unwrap()
        .reduce(|config, acc| {
            println!("{:#?}", config.config_surface_types());
            if config.num_samples() > acc.num_samples() {
                config
            } else {
                acc
            }
        })
        .expect("No available configs")
}

pub fn main() {
    let (egl_device, card) = Card::open_global();

    card.set_client_capability(drm::ClientCapability::UniversalPlanes, true)
        .expect("Unable to request UniversalPlanes capability");
    card.set_client_capability(drm::ClientCapability::Atomic, true)
        .expect("Unable to request Atomic capability");

    // Load the information.
    let res = card
        .resource_handles()
        .expect("Could not load normal resource ids.");
    let coninfo: Vec<connector::Info> = res
        .connectors()
        .iter()
        .flat_map(|con| card.get_connector(*con, true))
        .collect();

    // Filter each connector until we find one that's connected.
    let con = coninfo
        .iter()
        .find(|&i| i.state() == connector::State::Connected)
        .expect("No connected connectors");

    let crtcinfo: Vec<crtc::Info> = res
        .crtcs()
        .iter()
        .flat_map(|crtc| card.get_crtc(*crtc))
        .collect();
    // Get the first (usually best) mode
    let &mode = con.modes().first().expect("No modes found on connector");

    let (disp_width, disp_height) = mode.size();

    // Find a crtc and FB
    let crtc = crtcinfo.first().expect("No crtcs found");

    println!("{:#?}", mode);
    let gbm = gbm::Device::new(card).unwrap();
    let rdh = RawDisplayHandle::Gbm(GbmDisplayHandle::new(
        NonNull::new(gbm.as_raw_mut()).unwrap().cast(),
    ));
    let egl_display = unsafe { egl::display::Display::new(rdh) }.expect("Create EGL Display");
    let config = find_egl_config(&egl_display);
    let gbm_surface = gbm
        .create_surface::<()>(
            disp_width.into(),
            disp_height.into(),
            gbm::Format::Xrgb8888,
            BufferObjectFlags::SCANOUT | BufferObjectFlags::RENDERING,
        )
        .unwrap();
    let rwh = RawWindowHandle::Gbm(GbmWindowHandle::new(
        NonNull::new(gbm_surface.as_raw_mut()).unwrap().cast(),
    ));
    let surface = unsafe {
        egl_display
            .create_window_surface(
                &config,
                &SurfaceAttributesBuilder::<WindowSurface>::new().build(
                    rwh,
                    (disp_width as u32).try_into().unwrap(),
                    (disp_height as u32).try_into().unwrap(),
                ),
            )
            .expect("Failed to create EGL surface")
    };
    let context = unsafe {
        egl_display
            .create_context(&config, &ContextAttributesBuilder::new().build(Some(rwh)))
            .expect("Failed to create EGL context")
            .make_current(&surface)
            .unwrap()
    };

    let gles = gl::Gles2::load_with(|symbol| {
        let symbol = CString::new(symbol).unwrap();
        egl_display.get_proc_address(symbol.as_c_str()).cast()
    });
    let mut should_close = false;
    let mut start_time = std::time::Instant::now();
    let mut frame_count = 0;

    while !should_close {
        unsafe {
            gles.Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            gles.ClearColor(1.0, 1.0, 1.0, 1.0);
            surface.swap_buffers(&context).unwrap();
            let frontbuffer = gbm_surface.lock_front_buffer().unwrap();
            let fb = gbm
                .add_framebuffer(&frontbuffer, 32, 32)
                .unwrap();
            gbm.set_crtc(crtc.handle(), Some(fb), (0, 0), &[con.handle()], Some(mode))
                .unwrap();
        }

        frame_count += 1;
        let elapsed = start_time.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            let fps = frame_count as f32 / elapsed;
            println!("FPS: {:.2}", fps);
            frame_count = 0;
            start_time = std::time::Instant::now();
        }
    }
}
