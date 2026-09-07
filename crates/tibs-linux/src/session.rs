use color_eyre::eyre::OptionExt;
use color_eyre::Result;
use nix::libc::{initgroups, setgid, setsid, setuid};
use std::{
    cell::RefCell,
    ffi::CString,
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command},
};
use tibs_service_definitions::{AuthProof, DesktopSession, SessionStatus, UserAccount};
use uzers::os::unix::UserExt;

use crate::{authentication::LinuxPamAuthProof, tty::TtyInfo};

pub(crate) struct LinuxSession {
    process: RefCell<Child>,
    pub(crate) tty: TtyInfo,
    _auth: AuthProof,
}

impl LinuxSession {
    pub(crate) fn new_for_desktop_session(
        user: &UserAccount,
        tty: TtyInfo,
        session: &DesktopSession,
        auth: AuthProof,
    ) -> Result<Self> {
        let mut command = Command::new("bash");
        command.args(["-c", &session.command]);
        Self::new(user, tty, command, auth)
    }

    pub(crate) fn new(
        user: &UserAccount,
        tty: TtyInfo,
        mut command: Command,
        auth: AuthProof,
    ) -> Result<Self> {
        let pam = auth
            .downcast_ref::<LinuxPamAuthProof>()
            .ok_or_eyre("Linux session startup requires a Linux PAM auth proof")?;
        pam.prepare_tty_session(tty.number)?;

        let linux_user = uzers::get_user_by_name(&user.username)
            .ok_or_eyre("Could not find authenticated user in Linux user database")?;
        command
            .env("XDG_SESSION_TYPE", "wayland")
            .env("XDG_VTNR", tty.number.to_string())
            .env("XDG_SEAT", "seat0");
        unsafe {
            command.pre_exec(move || {
                setsid();

                let tty = TtyInfo::new(tty.number).unwrap();
                tty.make_current().unwrap();
                let fd = tty.fd.as_raw_fd();
                let ret = libc::ioctl(fd, libc::TIOCSCTTY, 1);
                if ret < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                let username = CString::new(linux_user.name().to_str().unwrap()).unwrap();
                initgroups(username.as_ptr(), linux_user.primary_group_id());
                setgid(linux_user.primary_group_id());
                setuid(linux_user.uid());
                std::env::set_current_dir(linux_user.home_dir())?;
                Ok(())
            });
        }
        let process = RefCell::new(command.spawn()?);

        Ok(Self {
            process,
            tty,
            _auth: auth,
        })
    }

    pub(crate) fn status(&self) -> SessionStatus {
        match self.process.borrow_mut().try_wait() {
            Ok(Some(code)) if code.success() => SessionStatus::ShutdownGracefully,
            Ok(Some(_)) => SessionStatus::Crashed,
            Ok(None) => SessionStatus::Running,
            Err(_) => SessionStatus::Crashed,
        }
    }
}

impl Drop for LinuxSession {
    fn drop(&mut self) {
        if matches!(self.status(), SessionStatus::Running) {
            self.process.borrow_mut().kill().ok();
            let current_tty = TtyInfo::active_number();
            if self.tty.number == current_tty {
                println!(
                    "[WARN] Dropped session while still inside the session's tty: {current_tty}"
                );
            }
        }
    }
}
