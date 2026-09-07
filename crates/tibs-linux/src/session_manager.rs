use color_eyre::eyre::OptionExt;
use color_eyre::Result;
use std::{cell::RefCell, collections::HashMap, collections::HashSet, process::Command, rc::Rc};
use tibs_service_definitions::{
    AuthProof, DesktopSession, SessionHandle, SessionManager, SessionStatus, UserAccount, UserId,
};

use crate::{session::LinuxSession, tty::TtyInfo};

pub struct LinuxSessionManager {
    state: RefCell<LinuxSessionManagerState>,
    tibs_tty: u16,
}

#[derive(Default)]
struct LinuxSessionManagerState {
    sessions: HashMap<UserId, Rc<LinuxSession>>,
    sessions_by_handle: HashMap<SessionHandle, Rc<LinuxSession>>,
    pending_sessions: HashMap<SessionHandle, TtyInfo>,
    next_session_id: u64,
}

impl LinuxSessionManager {
    pub fn new() -> Self {
        Self {
            state: RefCell::new(LinuxSessionManagerState::default()),
            tibs_tty: TtyInfo::active_number(),
        }
    }

    fn next_tty(&self, state: &LinuxSessionManagerState) -> Option<TtyInfo> {
        let used_ttys = state
            .sessions
            .values()
            .filter(|session| matches!(session.status(), SessionStatus::Running))
            .map(|session| session.tty.number)
            .collect::<HashSet<_>>();

        (1..64u16)
            .find_map(|i| (i != self.tibs_tty && !used_ttys.contains(&i)).then(|| TtyInfo::new(i)))
            .flatten()
    }

    pub fn get_session_state_of_user(&self, user_id: &UserId) -> Option<SessionStatus> {
        self.state
            .borrow()
            .sessions
            .get(user_id)
            .map(|session| session.status())
    }

    pub fn is_running(&self, user_id: &UserId) -> bool {
        self.state
            .borrow()
            .sessions
            .get(user_id)
            .is_some_and(|session| matches!(session.status(), SessionStatus::Running))
    }

    pub fn has_crashed(&self, user_id: &UserId) -> bool {
        self.state
            .borrow()
            .sessions
            .get(user_id)
            .is_some_and(|session| matches!(session.status(), SessionStatus::Crashed))
    }

    pub fn is_on_tibs_tty(&self) -> bool {
        self.tibs_tty == TtyInfo::active_number()
    }
}

impl Default for LinuxSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager for LinuxSessionManager {
    fn create_session(&self) -> Result<SessionHandle> {
        let mut state = self.state.borrow_mut();
        let free_tty = self
            .next_tty(&state)
            .ok_or_eyre("There's no free tty's left for this session.")?;
        state.next_session_id += 1;
        let handle = SessionHandle(format!("linux-tty-{}", state.next_session_id));
        state.pending_sessions.insert(handle.clone(), free_tty);
        Ok(handle)
    }

    fn go_to(
        &self,
        session: &SessionHandle,
        _options: Option<Box<dyn std::any::Any + Send>>,
    ) -> Result<()> {
        let state = self.state.borrow();
        let tty = state
            .pending_sessions
            .get(session)
            .map(|tty| tty.number)
            .or_else(|| {
                state
                    .sessions_by_handle
                    .get(session)
                    .map(|session| session.tty.number)
            })
            .ok_or_eyre("Unknown Linux session handle")?;

        TtyInfo::new(tty)
            .ok_or_eyre("Could not reopen Linux session TTY")?
            .make_current()
    }

    fn start_compositor(
        &self,
        session: &SessionHandle,
        command: Command,
        user: &UserAccount,
        auth: AuthProof,
    ) -> Result<()> {
        let mut state = self.state.borrow_mut();
        let tty = state
            .pending_sessions
            .remove(session)
            .ok_or_eyre("Unknown or already-started Linux session handle")?;
        let running_session = LinuxSession::new(user, tty, command, auth).map(Rc::new)?;
        state
            .sessions
            .insert(user.id.clone(), Rc::clone(&running_session));
        state
            .sessions_by_handle
            .insert(session.clone(), running_session);
        Ok(())
    }

    fn session_status(&self, session: &SessionHandle) -> Result<Option<SessionStatus>> {
        Ok(self
            .state
            .borrow()
            .sessions_by_handle
            .get(session)
            .map(|session| session.status()))
    }

    fn start_desktop_session(
        &self,
        user: &UserAccount,
        desktop_session: &DesktopSession,
        auth: AuthProof,
    ) -> Result<SessionHandle> {
        let handle = self.create_session()?;
        let mut state = self.state.borrow_mut();
        let tty = state
            .pending_sessions
            .remove(&handle)
            .ok_or_eyre("Created Linux session handle had no reserved TTY")?;
        let session =
            LinuxSession::new_for_desktop_session(user, tty, desktop_session, auth).map(Rc::new)?;
        state.sessions.insert(user.id.clone(), Rc::clone(&session));
        state
            .sessions_by_handle
            .insert(handle.clone(), Rc::clone(&session));
        Ok(handle)
    }

    fn user_session_status(&self, user: &UserId) -> Result<Option<SessionStatus>> {
        Ok(self.get_session_state_of_user(user))
    }

    fn is_login_session_active(&self) -> bool {
        self.is_on_tibs_tty()
    }
}
