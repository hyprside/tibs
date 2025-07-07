use crate::login::LoginManager;
use crate::tty::*;
use color_eyre::eyre::bail;
use color_eyre::eyre::OptionExt;
use freedesktop_entry_parser::parse_entry;
use nix::libc;
use nix::libc::initgroups;
use nix::libc::setgid;
use nix::libc::setsid;
use nix::libc::setuid;
use pam::putenv;
use pam::set_item;
use pam::Client;
use pam::PasswordConv;
use rustamarine::Rustamarine;
use uzers::os::unix::UserExt;
use uzers::User;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::env;
use std::ffi::CString;
use std::fs;
use std::fs::OpenOptions;
use std::mem::transmute;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;
use zbus_systemd::login1::{ManagerProxy, SessionProxy};
use zbus_systemd::zbus::zvariant::Value;
use zbus_systemd::zbus::Connection;

#[derive(Debug, Clone)]
pub struct DesktopEnvironmentFile {
	name: String,
	command: String,
}
impl DesktopEnvironmentFile {
	pub fn name(&self) -> &str {
		self.name.as_str()
	}
	pub fn command(&self) -> &str {
		self.command.as_str()
	}
}
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SessionStatus {
	Running,
	ShutdownGracefully,
	Crashed,
}
pub struct Session {
	process: RefCell<Child>,
	tty: TTYInfo,
	user_id: u32,
	_pam: Arc<RwLock<Client<'static, PasswordConv>>>
}

impl Session {
	fn new(
		user: User,
		tty: TTYInfo,
		session_file: &DesktopEnvironmentFile,
		pam: Arc<RwLock<Client<'static, PasswordConv>>>
	) -> color_eyre::Result<Session> {
		let user_id = user.uid();
		{
			let mut pam = pam.write().unwrap();

			pam.set_env("XDG_VTNR", &tty.number.to_string());
			pam.set_env("XDG_SEAT", "seat0");
			let tty_item = CString::new(format!("tty{}", tty.number)).unwrap();
			set_item(pam.handle, pam::PamItemType::TTY, unsafe{transmute(tty_item.as_ptr())});
			pam.open_session().unwrap();
		}
		// let session_file = session_file.clone();

		// while TTYInfo::get_active_tty_number() != tty.number {
		// 	std::hint::spin_loop();
		// }
		// let tty_fd = tty.fd.as_raw_fd();
		let process = RefCell::new(
			Command::new("bash")
				.args(["-c", &session_file.command])
				.env("XDG_SESSION_TYPE", "wayland")
				.env("XDG_VTNR", tty.number.to_string())
				.env("XDG_SEAT", "seat0")
				.before_exec(move || {
					unsafe {
						setsid();
					};

					// Clone just to make sure the fd exists
					let tty = TTYInfo::new(tty.number).unwrap();
					tty.make_current().unwrap();
					let fd = tty.fd.as_raw_fd();
					let ret = unsafe { libc::ioctl(fd, libc::TIOCSCTTY, 1) };
					if ret < 0 {
						return Err(std::io::Error::last_os_error());
					}
					unsafe {
						let username = CString::new(user.name().to_str().unwrap()).unwrap();
						initgroups(username.as_ptr(), user.primary_group_id());
						setgid(user.primary_group_id());
						setuid(user.uid());
					}
					std::env::set_current_dir(user.home_dir());
					Ok(())
				})
				.spawn()?,
		);
		Ok(Self {
			process,
			tty,
			user_id,
			_pam: pam
		})
	}
	pub fn status(&self) -> SessionStatus {
		match self.process.borrow_mut().try_wait() {
			Ok(Some(code)) if code.success() => SessionStatus::ShutdownGracefully,
			Ok(Some(_)) => SessionStatus::Crashed,
			Ok(None) => SessionStatus::Running,
			Err(_) => SessionStatus::Crashed,
		}
	}
	pub fn user_id(&self) -> u32 {
		self.user_id
	}
}

impl Drop for Session {
	fn drop(&mut self) {
		match self.status() {
			SessionStatus::Running => {
				self.process.borrow_mut().kill().ok();
				let current_tty = TTYInfo::get_active_tty_number();
				if self.tty.number == current_tty {
					println!("[WARN] Dropped session while still inside the session's tty: {current_tty}");
				}
			}
			_ => {}
		}
	}
}

pub struct SessionManager {
	sessions: HashMap<u32, Rc<Session>>,
	tibs_tty: u16,
	wayland_desktop_environments_cache: Vec<DesktopEnvironmentFile>,
}

impl SessionManager {
	fn discover_wayland_desktop_environments() -> Vec<DesktopEnvironmentFile> {
		let session_dirs = env::var("XDG_SESSION_DIRS")
			.map(|v| v.split(':').map(String::from).collect::<Vec<_>>())
			.unwrap_or_else(|_| {
				vec![
					"/usr/share/wayland-sessions".into(),
					"/run/current-system/sw/share/wayland-sessions".into(),
				]
			});

		session_dirs
			.iter()
			.filter_map(|dir| fs::read_dir(dir).ok())
			.flat_map(|entries| entries.filter_map(Result::ok))
			.filter(|entry| {
				entry
					.path()
					.extension()
					.map(|e| e == "desktop")
					.unwrap_or(false)
			})
			.filter_map(|entry| {
				let path = entry.path();
				let entry = parse_entry(&path).ok()?;
				let section = entry.section("Desktop Entry");
				let name = section.attr("Name")?.to_string();
				let command = section.attr("Exec")?.to_string();
				Some(DesktopEnvironmentFile { name, command })
			})
			.collect()
	}
	pub fn update_desktop_environments_cache(&mut self) {
		self.wayland_desktop_environments_cache = Self::discover_wayland_desktop_environments();
	}
	pub fn get_desktop_environments_list(&self) -> &[DesktopEnvironmentFile] {
		&self.wayland_desktop_environments_cache
	}
	pub fn new() -> Self {
		Self {
			sessions: Default::default(),
			tibs_tty: TTYInfo::get_active_tty_number(),
			wayland_desktop_environments_cache: Self::discover_wayland_desktop_environments(),
		}
	}

	fn next_tty(&self) -> Option<TTYInfo> {
		let used_ttys = self
			.sessions
			.values()
			.filter(|s| matches!(s.status(), SessionStatus::Running))
			.map(|s| s.tty.number)
			.collect::<HashSet<_>>();
		(1..64u16)
			.into_iter()
			.find_map(|i| (i != self.tibs_tty && !used_ttys.contains(&i)).then(|| TTYInfo::new(i)))
			.flatten()
	}
	pub fn start_session(
		&mut self,
		login_manager: &LoginManager,
		username: &str,
		session_file: &DesktopEnvironmentFile,
	) -> color_eyre::Result<Rc<Session>> {
		let Some(crate::login::LoginState::Authenticated(pam)) =
			login_manager.get_current_login_state(username)
		else {
			bail!("Tried to start session without being authenticated (user={username})");
		};
		let free_tty = self
			.next_tty()
			.ok_or_eyre("There's no free tty's left for this session.")?;

		let user = uzers::get_user_by_name(username).unwrap();
		let uid = user.uid();
		let session = Session::new(user, free_tty, session_file, pam).map(Rc::new)?;
		self.sessions.insert(uid, Rc::clone(&session));
		Ok(session)
	}
	pub fn get_session_state_of_user(&self, uid: u32) -> Option<SessionStatus> {
		self.sessions.get(&uid).map(|s| s.status())
	}
	pub fn is_running(&self, uid: u32) -> bool {
		self
			.sessions
			.get(&uid)
			.is_some_and(|s| matches!(s.status(), SessionStatus::Running))
	}
	pub fn has_crashed(&self, uid: u32) -> bool {
		self
			.sessions
			.get(&uid)
			.is_some_and(|s| matches!(s.status(), SessionStatus::Crashed))
	}
	pub fn is_on_tibs_tty(&self) -> bool {
		self.tibs_tty == TTYInfo::get_active_tty_number()
	}
}
