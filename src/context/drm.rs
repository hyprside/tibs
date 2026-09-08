use crate::gl;
use crate::input::KeyboardState;
use ::input::Libinput;
use drm::control::connector;
use easydrm::EasyDRM;
use input::{InputInterface, MouseState};

use super::GlesContext;

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
    commit_allowed: bool,
}

impl GlesContext for DrmContext {
    fn get_proc_address(&mut self, fn_name: &str) -> *const std::ffi::c_void {
        self.easydrm.get_proc_address(fn_name)
    }
    fn render_target_origin(&self) -> crate::gles_context::RenderTargetOrigin {
        crate::gles_context::RenderTargetOrigin::TopLeft
    }
    fn swap_buffers(&mut self) -> bool {
        if !self.commit_allowed {
            return false;
        }
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

    fn set_commit_allowed(&mut self, allowed: bool) {
        self.commit_allowed = allowed;
        if allowed {
            self.make_target_current();
        }
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

        // let tty_fd = TTY.as_raw_fd();
        // set_vt_mode(tty_fd).expect("Failed to set VT mode");
        // log::info!("Configured DRM VT process mode");
        // unsafe {
        //     libc::signal(SIGUSR1, handle_release as *const () as usize);
        //     libc::signal(SIGUSR2, handle_acquire as *const () as usize);
        // }
        // log::info!("Installed DRM VT signal handlers");

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
            commit_allowed: true,
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
        // EasyDRM's poll_events_ex blocks until a DRM page-flip or hotplug
        // event. Once logind switches away from TIBS no page-flips are
        // submitted, so blocking here would prevent the app loop from
        // observing the session becoming active again.
        if !self.commit_allowed {
            return;
        }
        if !self.has_swapped_once {
            self.make_target_current();
            return;
        }

        if let Err(e) = self.easydrm.poll_events_ex([]) {
            log::error!("Failed to poll EasyDRM events: {e}");
        }
        self.make_target_current();
    }
}

mod input;
