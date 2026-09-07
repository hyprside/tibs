use color_eyre::Result;
use std::{any::Any, process::Command};

use crate::{
    authentication::AuthProof,
    desktop_sessions::DesktopSession,
    users::{UserAccount, UserId},
};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct SessionHandle(pub String);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SessionStatus {
    Running,
    ShutdownGracefully,
    Crashed,
}

pub trait SessionManager {
    fn create_session(&self) -> Result<SessionHandle>;
    fn go_to(&self, session: &SessionHandle, options: Option<Box<dyn Any + Send>>) -> Result<()>;
    fn start_compositor(
        &self,
        session: &SessionHandle,
        command: Command,
        user: &UserAccount,
        auth: AuthProof,
    ) -> Result<()>;
    fn session_status(&self, session: &SessionHandle) -> Result<Option<SessionStatus>>;
    fn start_desktop_session(
        &self,
        user: &UserAccount,
        desktop_session: &DesktopSession,
        auth: AuthProof,
    ) -> Result<SessionHandle>;
    fn user_session_status(&self, user: &UserId) -> Result<Option<SessionStatus>>;
    fn is_login_session_active(&self) -> bool;

    fn is_user_session_running(&self, user: &UserId) -> Result<bool> {
        Ok(self
            .user_session_status(user)?
            .is_some_and(|status| status == SessionStatus::Running))
    }
}
