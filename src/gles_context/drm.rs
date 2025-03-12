pub use drm::control::Device as ControlDevice;
pub use drm::Device;
use std::ffi::CString;
use std::ptr::NonNull;
use std::time::Duration;

use drm::control::{connector, crtc, Mode};
use gbm::{AsRaw, BufferObjectFlags};
use glutin::api::egl;
use glutin::config::ConfigTemplateBuilder;
use glutin::context::ContextAttributesBuilder;
use glutin::prelude::*;
use glutin::surface::{SurfaceAttributesBuilder, WindowSurface};
use raw_window_handle::{GbmDisplayHandle, GbmWindowHandle, RawDisplayHandle, RawWindowHandle};

use crate::gl;

use super::GlesContext;

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
    pub fn open(path: &str) -> std::io::Result<Self> {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        options.write(true);
        Ok(Self(options.open(path)?))
    }

    pub fn open_global() -> Self {
        let mut devices = egl::device::Device::query_devices().expect("Failed to query devices");
        let started_time = std::time::Instant::now();
        loop {
            let Some(egl_device) = devices.next() else {
                if started_time.elapsed().as_secs() < 5 {
                    println!("Failed to find device, trying again in 50ms");
                    devices =
                        egl::device::Device::query_devices().expect("Failed to query devices");

                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                panic!("No device found (waited for 5s)");
            };
            let Some(drm) = egl_device.drm_device_node_path() else {
                continue;
            };
            break Self::open(drm.as_os_str().to_str().unwrap()).unwrap();
        }
    }

    pub fn initialize_egl(self) -> DrmGlesContext {
        let (connector, crtc, mode) = self.get_connector_and_crtc();
        let (disp_width, disp_height) = mode.size();
        let gbm = gbm::Device::new(self).unwrap();
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
        let mut context = DrmGlesContext {
            display: egl_display,
            gbm,
            gbm_surface,
            surface,
            context,
            connector,
            crtc,
            mode,
        };
        gl::load_with(|symbol| context.get_proc_address(symbol));
        context
    }

    fn get_connector_and_crtc(&self) -> (connector::Info, crtc::Info, Mode) {
        let res = self
            .resource_handles()
            .expect("Could not load normal resource ids.");
        let coninfo: Vec<connector::Info> = res
            .connectors()
            .iter()
            .flat_map(|con| self.get_connector(*con, true))
            .collect();

        let con = coninfo
            .iter()
            .find(|&i| i.state() == connector::State::Connected)
            .expect("No connected connectors");

        let crtcinfo: Vec<crtc::Info> = res
            .crtcs()
            .iter()
            .flat_map(|crtc| self.get_crtc(*crtc))
            .collect();
        let &mode = con.modes().first().expect("No modes found on connector");

        let crtc = crtcinfo.first().expect("No crtcs found");

        (con.clone(), crtc.clone(), mode)
    }
}
pub struct DrmGlesContext {
    display: egl::display::Display,
    gbm: gbm::Device<Card>,
    gbm_surface: gbm::Surface<()>,
    surface: egl::surface::Surface<WindowSurface>,
    context: egl::context::PossiblyCurrentContext,
    connector: connector::Info,
    crtc: crtc::Info,
    mode: Mode,
}

fn find_egl_config(egl_display: &egl::display::Display) -> egl::config::Config {
    unsafe { egl_display.find_configs(ConfigTemplateBuilder::new().build()) }
        .unwrap()
        .reduce(|config, acc| {
            if config.num_samples() > acc.num_samples() {
                config
            } else {
                acc
            }
        })
        .expect("No available configs")
}

impl GlesContext for DrmGlesContext {
    fn get_proc_address(&mut self, fn_name: &str) -> *const std::ffi::c_void {
        let symbol = CString::new(fn_name).unwrap();
        self.display.get_proc_address(symbol.as_c_str())
    }
    fn swap_buffers(&self) {
        unsafe {
            self.surface.swap_buffers(&self.context).unwrap();
            let frontbuffer = self.gbm_surface.lock_front_buffer().unwrap();
            let fb = self.gbm.add_framebuffer(&frontbuffer, 24, 32).unwrap();
            self.gbm
                .set_crtc(
                    self.crtc.handle(),
                    Some(fb),
                    (0, 0),
                    &[self.connector.handle()],
                    Some(self.mode),
                )
                .unwrap();
        }
    }

    fn size(&self) -> (u32, u32) {
        (self.mode.size().0 as u32, self.mode.size().1 as u32)
    }
}
impl DrmGlesContext {
    pub fn new_from_default_card() -> Self {
        let card = Card::open_global();
        card.initialize_egl()
    }
}
