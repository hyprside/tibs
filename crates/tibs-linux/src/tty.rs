use color_eyre::eyre::bail;
use color_eyre::Result;
use linux_raw_sys::ioctl::{KDSETMODE, VT_ACTIVATE, VT_GETSTATE, VT_WAITACTIVE};
use std::{
    fs::{File, OpenOptions},
    mem::MaybeUninit,
    os::fd::AsRawFd,
};

pub(crate) struct TtyInfo {
    pub(crate) fd: File,
    pub(crate) number: u16,
}

impl TtyInfo {
    pub(crate) fn new(number: u16) -> Option<Self> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!("/dev/tty{number}"))
            .ok()
            .map(|fd| TtyInfo { fd, number })
    }

    pub(crate) fn make_current(&self) -> Result<()> {
        let root_tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
        let fd = root_tty.as_raw_fd();

        let ret = unsafe { libc::ioctl(fd, VT_ACTIVATE as u64, self.number as libc::c_int) };
        if ret != 0 {
            bail!("VT_ACTIVATE failed: {}", std::io::Error::last_os_error());
        }

        let ret = unsafe { libc::ioctl(fd, VT_WAITACTIVE as u64, self.number as libc::c_int) };
        if ret != 0 {
            bail!("VT_WAITACTIVE failed: {}", std::io::Error::last_os_error());
        }

        let ret = unsafe { libc::ioctl(fd, KDSETMODE as u64, 1) };
        if ret != 0 {
            bail!("KDSETMODE failed: {}", std::io::Error::last_os_error());
        }

        Ok(())
    }

    pub(crate) fn active_number() -> u16 {
        #[repr(C)]
        #[derive(Debug)]
        struct VtStat {
            v_active: libc::c_ushort,
            v_signal: libc::c_ushort,
            v_state: libc::c_ushort,
        }

        let Ok(file) = File::open("/dev/console") else {
            return 2;
        };
        let fd = file.as_raw_fd();
        let mut vt: MaybeUninit<VtStat> = MaybeUninit::uninit();
        unsafe { libc::ioctl(fd, VT_GETSTATE as u64, vt.as_mut_ptr()) };
        unsafe { vt.assume_init().v_active }
    }
}
