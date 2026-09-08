//! PAM registration for a greeter started by a system service without PAMName=.
//! Desktop PAM sessions are opened and closed by systemd in the desktop process.

use color_eyre::{eyre::ensure, Result};
use pam::{Client, PamItemType, PamReturnCode, PasswordConv};
use std::ffi::CString;

use crate::authentication::PAM_SERVICE;

pub(crate) struct GreeterPamSession(Client<'static, PasswordConv>);

impl GreeterPamSession {
    pub(crate) fn open(username: &str, vt: u32) -> Result<Self> {
        let client = Client::with_password(PAM_SERVICE)?;
        for (item, value) in [
            (PamItemType::User, username.to_owned()),
            (PamItemType::TTY, format!("tty{vt}")),
        ] {
            let value = CString::new(value)?;
            // PAM copies these strings during pam_set_item.
            let result: PamReturnCode = unsafe {
                pam::ffi::pam_set_item(client.handle, item as i32, value.as_ptr().cast())
            }
            .into();
            ensure!(
                result == PamReturnCode::Success,
                "Setting greeter PAM item: {result:?}"
            );
        }
        for value in [
            "XDG_SESSION_CLASS=greeter".to_owned(),
            "XDG_SESSION_TYPE=wayland".to_owned(),
            "XDG_SEAT=seat0".to_owned(),
            format!("XDG_VTNR={vt}"),
        ] {
            // Do not use Client::set_env: it also mutates the process environment.
            pam::putenv(client.handle, &value)?;
        }
        // This registers the already-running greeter's OS identity. It never
        // authenticates a desktop user or uses a desktop authentication proof.
        let result = pam::open_session(client.handle, false);
        ensure!(
            result == PamReturnCode::Success,
            "Opening greeter PAM session: {result:?}"
        );
        Ok(Self(client))
    }
}

impl Drop for GreeterPamSession {
    fn drop(&mut self) {
        let result = pam::close_session(self.0.handle, false);
        if result != PamReturnCode::Success {
            log::warn!("Closing greeter PAM session: {result:?}");
        }
    }
}
