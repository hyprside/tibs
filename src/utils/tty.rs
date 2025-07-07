use color_eyre::eyre::bail;
use linux_raw_sys::ioctl::{KDSETMODE, VT_ACTIVATE, VT_GETSTATE, VT_WAITACTIVE};
use nix::libc;
use rustamarine::Rustamarine;
use std::{
	fs::{File, OpenOptions},
	mem::MaybeUninit,
	os::fd::AsRawFd,
};

pub struct TTYInfo {
	pub fd: File,
	pub number: u16,
}
impl TTYInfo {
	pub fn new(i: u16) -> Option<Self> {
		OpenOptions::new()
			.read(true)
			.write(true)
			.open(&format!("/dev/tty{i}"))
			.ok()
			.map(|f| TTYInfo { fd: f, number: i })
	}
	pub fn make_current(&self) -> color_eyre::Result<()> {
		let root_tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;

		let fd = root_tty.as_raw_fd();
		dbg!(self.number);
		// pede ao kernel para mudar para este VT
		let ret = unsafe { libc::ioctl(fd, VT_ACTIVATE as u64, self.number as libc::c_int) };
		if ret != 0 {
			bail!("VT_ACTIVATE failed: {}", std::io::Error::last_os_error());
		}

		// espera até este VT ficar ativo
		let ret = unsafe { libc::ioctl(fd, VT_WAITACTIVE as u64, self.number as libc::c_int) };
		if ret != 0 {
			bail!("VT_WAITACTIVE failed: {}", std::io::Error::last_os_error());
		}

		// mete o VT em modo gráfico (opcional, mas recomendado para DMs/Wayland)
		let ret = unsafe { libc::ioctl(fd, KDSETMODE as u64, 1) };
		if ret != 0 {
			bail!("KDSETMODE failed: {}", std::io::Error::last_os_error());
		}

		Ok(())
	}
	pub fn get_active_tty_number() -> u16 {
		#[repr(C)]
		#[derive(Debug)]
		struct vt_stat {
			v_active: libc::c_ushort,
			v_signal: libc::c_ushort,
			v_state: libc::c_ushort,
		}
		let Ok(file) = File::open("/dev/console") else {
			return 2;
		};
		let fd = file.as_raw_fd();
		let mut vt: MaybeUninit<vt_stat> = MaybeUninit::uninit();
		unsafe { libc::ioctl(fd, VT_GETSTATE as u64, vt.as_mut_ptr()) };
		unsafe { vt.assume_init().v_active }
	}
}
