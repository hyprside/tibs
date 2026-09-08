use crate::gl;
use crate::input::KeyboardState;
use ::input::Libinput;
use drm::control::connector;
use easydrm::EasyDRM;
use input::{InputInterface, MouseState};
use libc::{c_char, c_int, c_short, ioctl, SIGUSR1, SIGUSR2};
use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::LazyLock;

use super::GlesContext;

static TTY_FOCUS: AtomicBool = AtomicBool::new(true);
static TTY: LazyLock<std::fs::File> = LazyLock::new(|| {
    log::info!("Opening controlling TTY for DRM context");
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .expect("Failed to open /dev/tty")
});

#[repr(C)]
struct vt_mode {
    mode: c_char,
    waitv: c_char,
    relsig: c_short,
    acqsig: c_short,
    frsig: c_short,
}

const VT_PROCESS: c_char = 0x01;
const VT_SETMODE: u64 = 0x5602;
const VT_RELDISP: u64 = 0x5605;

unsafe extern "C" fn handle_release(_sig: i32) {
    log::info!("DRM VT release signal received");
    TTY_FOCUS.store(false, Ordering::Relaxed);
    libc::ioctl(TTY.as_raw_fd(), VT_RELDISP, 1);
    set_tty_text_mode(TTY.as_raw_fd())
        .map_err(|e| log::error!("Failed to set text mode: {e}"))
        .ok();
}

unsafe extern "C" fn handle_acquire(_sig: i32) {
    log::info!("DRM VT acquire signal received");
    TTY_FOCUS.store(true, Ordering::Relaxed);
    set_tty_graphics_mode(TTY.as_raw_fd())
        .map_err(|e| log::error!("Failed to set graphics mode: {e}"))
        .ok();
}

const KDSETMODE: u64 = 0x4B3A;
const KD_TEXT: c_int = 0;
const KD_GRAPHICS: c_int = 1;

fn set_tty_graphics_mode(fd: i32) -> std::io::Result<()> {
    let ret = unsafe { libc::ioctl(fd, KDSETMODE, KD_GRAPHICS) };
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn set_tty_text_mode(fd: i32) -> std::io::Result<()> {
    let ret = unsafe { libc::ioctl(fd, KDSETMODE, KD_TEXT) };
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn set_vt_mode(fd: i32) -> std::io::Result<()> {
    let mut vt = vt_mode {
        mode: VT_PROCESS,
        waitv: 0,
        relsig: SIGUSR1 as i16,
        acqsig: SIGUSR2 as i16,
        frsig: 0,
    };

    let ret = unsafe { ioctl(fd, VT_SETMODE, &mut vt) };
    if ret < 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub struct DrmContext {
    easydrm: EasyDRM<()>,
    target_connector: Option<connector::Handle>,
    libinput: Libinput,
    xkb_state: xkbcommon::xkb::State,
    keyboard_state: KeyboardState,
    mouse_state: MouseState,
    focused: bool,
    has_swapped_once: bool,
    gl_loaded: bool,
    framebuffer_id: u32,
}

impl GlesContext for DrmContext {
    fn get_proc_address(&mut self, fn_name: &str) -> *const std::ffi::c_void {
        self.easydrm.get_proc_address(fn_name)
    }
    fn render_target_origin(&self) -> crate::gles_context::RenderTargetOrigin {
        crate::gles_context::RenderTargetOrigin::TopLeft
    }
    fn swap_buffers(&mut self) -> bool {
        self.clear_non_target_monitors();
        let swapped = self
            .easydrm
            .swap_buffers()
            .map_err(|e| log::error!("Failed to swap DRM buffers: {e}"))
            .is_ok();
        self.has_swapped_once |= swapped;
        swapped
    }

    fn size(&self) -> (u32, u32) {
        self.target_monitor_size().unwrap_or((1, 1))
    }

    fn framebuffer_id(&self) -> u32 {
        self.framebuffer_id
    }
}

impl DrmContext {
    pub fn new() -> Self {
        log::info!("Initializing DRM context via EasyDRM");
        let easydrm = EasyDRM::init_empty().expect("Failed to initialize EasyDRM");

        let mut libinput = Libinput::new_with_udev(InputInterface);
        libinput.udev_assign_seat("seat0").unwrap();
        log::info!("libinput assigned to seat0");

        let xkb_context = xkbcommon::xkb::Context::new(xkbcommon::xkb::CONTEXT_NO_FLAGS);
        let xkb_keymap = xkbcommon::xkb::Keymap::new_from_names(
            &xkb_context,
            "evdev",
            "evdev",
            "",
            "",
            None,
            xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .unwrap();
        let xkb_state = xkbcommon::xkb::State::new(&xkb_keymap);

        let tty_fd = TTY.as_raw_fd();
        set_vt_mode(tty_fd).expect("Failed to set VT mode");
        log::info!("Configured DRM VT process mode");
        unsafe {
            libc::signal(SIGUSR1, handle_release as *const () as usize);
            libc::signal(SIGUSR2, handle_acquire as *const () as usize);
        }
        log::info!("Installed DRM VT signal handlers");

        let target_connector = easydrm
            .monitors()
            .next()
            .map(|monitor| monitor.connector_id());
        let mut context = DrmContext {
            target_connector,
            mouse_state: MouseState::new_at_middle(1, 1),
            easydrm,
            libinput,
            xkb_state,
            keyboard_state: KeyboardState::new(),
            focused: true,
            has_swapped_once: false,
            gl_loaded: false,
            framebuffer_id: 0,
        };

        if let Some((width, height)) = context.target_monitor_size() {
            context.mouse_state = MouseState::new_at_middle(width, height);
        }

        context.make_target_current();
        gl::load_with(|symbol| context.get_proc_address(symbol));
        context.gl_loaded = true;
        context.make_target_current();
        log::info!("OpenGL function pointers loaded for EasyDRM context");

        context
    }

    fn target_monitor_size(&self) -> Option<(u32, u32)> {
        self.target_connector
            .and_then(|connector| self.easydrm.get_monitor(connector))
            .map(|monitor| {
                let (width, height) = monitor.size();
                (width as u32, height as u32)
            })
    }

    fn ensure_target_connector(&mut self) {
        let target_is_valid = self
            .target_connector
            .and_then(|connector| self.easydrm.get_monitor(connector))
            .is_some();

        if !target_is_valid {
            self.target_connector = self.easydrm.monitors().next().map(|monitor| {
                log::info!(
                    "Selected EasyDRM connector {:?} as the TIBS render target",
                    monitor.connector_id()
                );
                monitor.connector_id()
            });
        }
    }

    pub(crate) fn make_target_current(&mut self) {
        self.ensure_target_connector();
        let Some(connector) = self.target_connector else {
            return;
        };
        let Some(monitor) = self.easydrm.get_monitor_mut(connector) else {
            return;
        };
        if !monitor.can_render() {
            return;
        }
        if let Err(e) = monitor.make_current() {
            log::error!("Failed to make EasyDRM monitor current: {e}");
            return;
        }
        self.update_framebuffer_id();
    }

    fn update_framebuffer_id(&mut self) {
        if !self.gl_loaded {
            return;
        }
        let mut framebuffer_id = 0;
        unsafe {
            gl::GetIntegerv(gl::FRAMEBUFFER_BINDING, &mut framebuffer_id);
        }
        self.framebuffer_id = framebuffer_id as u32;
    }

    fn clear_non_target_monitors(&mut self) {
        let target_connector = self.target_connector;
        for monitor in self.easydrm.monitors_mut() {
            if Some(monitor.connector_id()) == target_connector || !monitor.can_render() {
                continue;
            }
            if let Err(e) = monitor.make_current() {
                log::error!("Failed to make secondary EasyDRM monitor current: {e}");
                continue;
            }
            let gl = monitor.gl();
            unsafe {
                gl.ClearColor(0.0, 0.0, 0.0, 1.0);
                gl.Clear(easydrm::gl::COLOR_BUFFER_BIT);
            }
        }
    }

    pub(crate) fn poll_display_events(&mut self) {
        if !self.has_swapped_once {
            self.make_target_current();
            return;
        }

        if let Err(e) = self.easydrm.poll_events_ex([self.libinput.as_raw_fd()]) {
            log::error!("Failed to poll EasyDRM events: {e}");
        }
        self.make_target_current();
    }
}

mod input;
