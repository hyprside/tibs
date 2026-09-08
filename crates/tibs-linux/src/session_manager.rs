use color_eyre::{
    eyre::{ensure, OptionExt},
    Result,
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, HashSet},
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tibs_service_definitions::{
    AuthProof, DesktopSession, SessionHandle, SessionManager, SessionStatus, UserAccount, UserId,
};
use zbus_systemd::{login1, systemd1, zbus};

use crate::{authentication::LinuxPamAuthProof, logind::Logind, session::LinuxSession};

/// Platform-neutral handles are resolved to logind IDs only after PAM startup.
/// All discovery, activation and process ownership remain inside tibs-linux.
pub struct LinuxSessionManager {
    backend: Box<dyn SessionBackend>,
    state: RefCell<SessionState>,
}

#[derive(Default)]
struct SessionState {
    pending: HashSet<SessionHandle>,
    resolved: HashMap<SessionHandle, SessionHandle>,
    users: HashMap<UserId, SessionHandle>,
    next_id: u64,
}

trait SessionBackend {
    fn start(
        &self,
        user: &UserAccount,
        command: Command,
        auth: &AuthProof,
    ) -> Result<SessionHandle>;
    fn activate(&self, id: &SessionHandle) -> Result<()>;
    fn status(&self, id: &SessionHandle) -> Result<Option<SessionStatus>>;
    fn find_user(&self, user: &UserId) -> Result<Option<SessionHandle>>;
    fn login_active(&self) -> bool;
}

impl LinuxSessionManager {
    pub async fn new(
        login: &login1::ManagerProxy<'static>,
        systemd: &systemd1::ManagerProxy<'static>,
    ) -> Result<Self> {
        // systemd only emits unit state changes to subscribed clients.
        match systemd.subscribe().await {
            Err(zbus::Error::MethodError(name, _, _))
                if name.as_str() == "org.freedesktop.systemd1.AlreadySubscribed" => {}
            result => result?,
        }
        let logind = Logind::new(login.clone(), systemd).await?;
        Ok(Self {
            backend: Box::new(SystemSessionBackend {
                logind,
                systemd: systemd.clone(),
                sessions: RefCell::new(HashMap::new()),
                next_unit: Cell::new(0),
                active_error_logged: Cell::new(false),
                unit_prefix: format!(
                    "tibs-desktop-{}-{}",
                    std::process::id(),
                    SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
                ),
                discovered: RefCell::new(HashMap::new()),
                external: RefCell::new(HashMap::new()),
                external_watches: RefCell::new(HashMap::new()),
            }),
            state: RefCell::new(SessionState::default()),
        })
    }
}

impl SessionManager for LinuxSessionManager {
    fn create_session(&self) -> Result<SessionHandle> {
        let mut state = self.state.borrow_mut();
        state.next_id += 1;
        let handle = SessionHandle(format!("pending-{}", state.next_id));
        state.pending.insert(handle.clone());
        Ok(handle)
    }

    fn go_to(
        &self,
        session: &SessionHandle,
        _options: Option<Box<dyn std::any::Any + Send>>,
    ) -> Result<()> {
        let state = self.state.borrow();
        ensure!(
            !state.pending.contains(session),
            "Session has not been started yet"
        );
        let id = state
            .resolved
            .get(session)
            .ok_or_eyre("Unknown Linux session handle")?;
        self.backend.activate(id)
    }

    fn start_compositor(
        &self,
        session: &SessionHandle,
        command: Command,
        user: &UserAccount,
        auth: AuthProof,
    ) -> Result<()> {
        ensure!(
            self.state.borrow().pending.contains(session),
            "Unknown or already-started Linux session handle"
        );
        // Preserve the pending handle on failure, so callers can retry it.
        let id = self.backend.start(user, command, &auth)?;
        let mut state = self.state.borrow_mut();
        state.pending.remove(session);
        state.resolved.insert(session.clone(), id);
        state.users.insert(user.id.clone(), session.clone());
        Ok(())
    }

    fn session_status(&self, session: &SessionHandle) -> Result<Option<SessionStatus>> {
        let state = self.state.borrow();
        match state.resolved.get(session) {
            Some(id) => self.backend.status(id),
            None => Ok(None),
        }
    }

    fn start_desktop_session(
        &self,
        user: &UserAccount,
        desktop: &DesktopSession,
        auth: AuthProof,
    ) -> Result<SessionHandle> {
        let handle = self.create_session()?;
        let mut command = Command::new("bash");
        command.args(["-c", &desktop.command]);
        if let Err(error) = self.start_compositor(&handle, command, user, auth) {
            self.state.borrow_mut().pending.remove(&handle);
            return Err(error);
        }
        Ok(handle)
    }

    fn user_session_status(&self, user: &UserId) -> Result<Option<SessionStatus>> {
        let previous = match self.state.borrow().users.get(user).cloned() {
            Some(handle) => self.session_status(&handle)?,
            None => None,
        };
        if previous == Some(SessionStatus::Running) {
            return Ok(previous);
        }
        match self.backend.find_user(user)? {
            Some(id) => self.backend.status(&id),
            // Keep a known exit outcome unless a new desktop superseded it.
            None => Ok(previous),
        }
    }

    fn is_login_session_active(&self) -> bool {
        self.backend.login_active()
    }
}

struct SystemSessionBackend {
    logind: Logind,
    systemd: systemd1::ManagerProxy<'static>,
    sessions: RefCell<HashMap<String, LinuxSession>>,
    next_unit: Cell<u64>,
    unit_prefix: String,
    active_error_logged: Cell<bool>,
    discovered: RefCell<HashMap<UserId, (Instant, Option<SessionHandle>)>>,
    external: RefCell<HashMap<String, login1::SessionProxy<'static>>>,
    external_watches: RefCell<HashMap<String, smol::Task<()>>>,
}

impl SessionBackend for SystemSessionBackend {
    fn start(
        &self,
        user: &UserAccount,
        command: Command,
        auth: &AuthProof,
    ) -> Result<SessionHandle> {
        auth.downcast_ref::<LinuxPamAuthProof>()
            .ok_or_eyre("Linux session startup requires a Linux PAM authentication proof")?
            .verify_user(&user.username)?;
        let uid = user.id.0.parse::<u32>()?;
        let account =
            uzers::get_user_by_name(&user.username).ok_or_eyre("User no longer exists")?;
        ensure!(
            account.uid() == uid,
            "User ID does not match the authenticated account"
        );
        smol::block_on(async {
            if let Some(session) = self.logind.user_session(uid).await? {
                let id = session.id().await?;
                let watch = if !self.sessions.borrow().contains_key(&id)
                    && !self.external_watches.borrow().contains_key(&id)
                {
                    Some(self.logind.watch_session_end(session.clone()).await?)
                } else {
                    None
                };
                self.logind.manager.activate_session(id.clone()).await?;
                if let Some(watch) = watch {
                    self.external_watches.borrow_mut().insert(id.clone(), watch);
                }
                self.external.borrow_mut().insert(id.clone(), session);
                self.discovered.borrow_mut().remove(&user.id);
                log::info!(
                    "Resuming existing logind session {id} for '{}'",
                    user.username
                );
                return Ok(SessionHandle(id));
            }
            self.next_unit.set(self.next_unit.get() + 1);
            let unit = format!("{}-{}.service", self.unit_prefix, self.next_unit.get());
            let session =
                LinuxSession::start(&self.systemd, &self.logind, unit, user, command).await?;
            let id = session.id.clone();
            self.sessions.borrow_mut().insert(id.clone(), session);
            self.discovered.borrow_mut().remove(&user.id);
            Ok(SessionHandle(id))
        })
    }

    fn activate(&self, id: &SessionHandle) -> Result<()> {
        smol::block_on(self.logind.manager.activate_session(id.0.clone()))?;
        Ok(())
    }

    fn status(&self, id: &SessionHandle) -> Result<Option<SessionStatus>> {
        if let Some(session) = self.sessions.borrow().get(&id.0) {
            return session.status().map(Some);
        }
        smol::block_on(async {
            let cached = self.external.borrow().get(&id.0).cloned();
            let session = match cached {
                Some(session) => Some(session),
                None => self.logind.session(&id.0).await?,
            };
            match session {
                Some(session) => Ok(match session.state().await?.as_str() {
                    "active" | "online" => Some(SessionStatus::Running),
                    _ => None,
                }),
                // logind does not provide exit codes for sessions we did not launch.
                None => Ok(None),
            }
        })
    }

    fn find_user(&self, user: &UserId) -> Result<Option<SessionHandle>> {
        // The UI asks every frame. Bound discovery traffic; known session
        // properties continue to update immediately through zbus's signal cache.
        if let Some((checked, id)) = self.discovered.borrow().get(user) {
            if checked.elapsed() < Duration::from_millis(500) {
                return Ok(id.clone());
            }
        }
        smol::block_on(async {
            let id = match self.logind.user_session(user.0.parse()?).await? {
                Some(session) => {
                    let id = session.id().await?;
                    self.external.borrow_mut().insert(id.clone(), session);
                    Some(SessionHandle(id))
                }
                None => None,
            };
            self.discovered
                .borrow_mut()
                .insert(user.clone(), (Instant::now(), id.clone()));
            Ok(id)
        })
    }

    fn login_active(&self) -> bool {
        // Read the seat's active-session tuple rather than relying solely on
        // Session.Active's property cache. VT switches are represented by a
        // seat-level PropertiesChanged(ActiveSession) update, and this direct
        // comparison remains correct after a desktop exits and logind returns
        // the seat to the greeter.
        match smol::block_on(self.logind.seat.active_session()) {
            Ok((active_id, _)) => {
                self.active_error_logged.set(false);
                match smol::block_on(self.logind.login.id()) {
                    Ok(login_id) => active_id == login_id,
                    Err(error) => {
                        if !self.active_error_logged.replace(true) {
                            log::error!("Could not read the login session ID: {error}");
                        }
                        false
                    }
                }
            }
            Err(error) => {
                if !self.active_error_logged.replace(true) {
                    log::error!("Could not read the login session's active state: {error}");
                }
                false
            }
        }
    }
}

impl Drop for SystemSessionBackend {
    fn drop(&mut self) {
        for (_, watch) in self.external_watches.get_mut().drain() {
            watch.detach();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[derive(Default)]
    struct FakeBackend {
        fail: Cell<bool>,
        activated: RefCell<Vec<SessionHandle>>,
        exit: Cell<Option<SessionStatus>>,
        external_available: Cell<bool>,
    }
    impl SessionBackend for Rc<FakeBackend> {
        fn start(&self, _: &UserAccount, _: Command, _: &AuthProof) -> Result<SessionHandle> {
            ensure!(!self.fail.get(), "PAM/session startup failed");
            Ok(SessionHandle("c42".into()))
        }
        fn activate(&self, id: &SessionHandle) -> Result<()> {
            self.activated.borrow_mut().push(id.clone());
            Ok(())
        }
        fn status(&self, id: &SessionHandle) -> Result<Option<SessionStatus>> {
            Ok(Some(if id.0 == "c42" {
                self.exit.get().unwrap_or(SessionStatus::Running)
            } else {
                SessionStatus::Running
            }))
        }
        fn find_user(&self, _: &UserId) -> Result<Option<SessionHandle>> {
            Ok(self
                .external_available
                .get()
                .then(|| SessionHandle("external".into())))
        }
        fn login_active(&self) -> bool {
            false
        }
    }

    fn fixture() -> (LinuxSessionManager, Rc<FakeBackend>, UserAccount) {
        let backend = Rc::new(FakeBackend::default());
        backend.external_available.set(true);
        let manager = LinuxSessionManager {
            backend: Box::new(backend.clone()),
            state: RefCell::default(),
        };
        let user = UserAccount {
            id: UserId("1000".into()),
            username: "alice".into(),
            display_name: "Alice".into(),
            home_dir: None,
            avatar_path: None,
        };
        (manager, backend, user)
    }

    #[test]
    fn pending_handles_are_not_sent_to_logind_and_startup_is_retryable() {
        let (manager, backend, user) = fixture();
        let handle = manager.create_session().unwrap();
        assert!(manager.go_to(&handle, None).is_err());
        assert!(backend.activated.borrow().is_empty());
        backend.fail.set(true);
        assert!(manager
            .start_compositor(&handle, Command::new("test"), &user, AuthProof::new(()))
            .is_err());
        assert_eq!(manager.session_status(&handle).unwrap(), None);
        backend.fail.set(false);
        manager
            .start_compositor(&handle, Command::new("test"), &user, AuthProof::new(()))
            .unwrap();
        manager.go_to(&handle, None).unwrap();
        assert_eq!(*backend.activated.borrow(), [SessionHandle("c42".into())]);
        assert!(manager
            .start_compositor(&handle, Command::new("test"), &user, AuthProof::new(()))
            .is_err());
    }

    #[test]
    fn user_status_includes_sessions_created_outside_tibs() {
        let (manager, _, user) = fixture();
        assert_eq!(
            manager.user_session_status(&user.id).unwrap(),
            Some(SessionStatus::Running)
        );
        assert!(!manager.is_login_session_active());
    }

    #[test]
    fn failed_desktop_start_does_not_leave_an_unreachable_pending_handle() {
        let (manager, backend, user) = fixture();
        backend.fail.set(true);
        let desktop = DesktopSession {
            id: tibs_service_definitions::DesktopSessionId("test".into()),
            name: "Test".into(),
            command: "test".into(),
        };
        assert!(manager
            .start_desktop_session(&user, &desktop, AuthProof::new(()))
            .is_err());
        assert!(manager.state.borrow().pending.is_empty());
        assert!(manager.state.borrow().resolved.is_empty());
    }

    #[test]
    fn a_new_external_desktop_supersedes_an_old_local_exit_status() {
        let (manager, backend, user) = fixture();
        let handle = manager.create_session().unwrap();
        manager
            .start_compositor(&handle, Command::new("test"), &user, AuthProof::new(()))
            .unwrap();
        backend.exit.set(Some(SessionStatus::Crashed));
        backend.external_available.set(false);
        assert_eq!(
            manager.user_session_status(&user.id).unwrap(),
            Some(SessionStatus::Crashed)
        );
        backend.external_available.set(true);
        assert_eq!(
            manager.user_session_status(&user.id).unwrap(),
            Some(SessionStatus::Running)
        );
        assert_eq!(
            manager.session_status(&handle).unwrap(),
            Some(SessionStatus::Crashed)
        );
    }
}
