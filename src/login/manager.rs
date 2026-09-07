use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use tibs_service_definitions::{
    AuthProof, AuthState, AuthenticationService, AuthenticationSession, UserAccount,
};

#[derive(Clone)]
pub enum LoginState {
    WaitingForInteraction,
    Logging,
    Failed(String),
    Authenticated(AuthProof),
    Cancelled,
}

impl From<AuthState> for LoginState {
    fn from(state: AuthState) -> Self {
        match state {
            AuthState::WaitingForInteraction => LoginState::WaitingForInteraction,
            AuthState::Checking => LoginState::Logging,
            AuthState::Failed(message) => LoginState::Failed(message),
            AuthState::Authenticated(proof) => LoginState::Authenticated(proof),
            AuthState::Cancelled => LoginState::Cancelled,
        }
    }
}

struct LoginEntry {
    session: Option<Box<dyn AuthenticationSession>>,
    state_rx: smol::channel::Receiver<AuthState>,
    state: LoginState,
}

pub struct LoginManager {
    authentication_service: Box<dyn AuthenticationService>,
    login_state_map: Arc<Mutex<HashMap<String, LoginEntry>>>,
}

impl LoginManager {
    pub fn new(authentication_service: Box<dyn AuthenticationService>) -> Self {
        Self {
            authentication_service,
            login_state_map: Default::default(),
        }
    }

    pub fn begin_authentication(&self, user: UserAccount) -> bool {
        let username = user.username.clone();

        let Ok(mut login_map_lock) = self.login_state_map.lock() else {
            return false;
        };

        if let Some(entry) = login_map_lock.get_mut(&username) {
            Self::poll_entry(entry);
            if !matches!(entry.state, LoginState::Failed(_) | LoginState::Cancelled) {
                return false;
            }
        }

        let entry = match self.authentication_service.create_session(user) {
            Ok(session) => {
                let state_rx = session.watch_state();
                LoginEntry {
                    session: Some(session),
                    state_rx,
                    state: LoginState::WaitingForInteraction,
                }
            }
            Err(error) => {
                login_map_lock.insert(
                    username,
                    LoginEntry {
                        session: None,
                        state_rx: smol::channel::unbounded().1,
                        state: LoginState::Failed(error.to_string()),
                    },
                );
                return false;
            }
        };

        if let Some(old_entry) = login_map_lock.insert(username, entry) {
            if let Some(session) = old_entry.session {
                session.cancel();
            }
        }
        true
    }

    pub fn submit_password(&self, name: impl Into<String>, password: impl Into<String>) -> bool {
        let Ok(mut login_map_lock) = self.login_state_map.lock() else {
            return false;
        };
        let Some(entry) = login_map_lock.get_mut(&name.into()) else {
            return false;
        };
        Self::poll_entry(entry);
        if matches!(
            entry.state,
            LoginState::Logging | LoginState::Authenticated(_)
        ) {
            return false;
        }
        let Some(session) = &entry.session else {
            return false;
        };
        if let Err(error) = session.submit_password(password.into()) {
            entry.state = LoginState::Failed(error.to_string());
            return false;
        }
        Self::poll_entry(entry);
        true
    }

    pub fn start_login(&self, user: UserAccount, password: impl Into<String>) -> bool {
        let username = user.username.clone();
        let _ = self.begin_authentication(user);
        self.submit_password(username, password)
    }

    pub fn get_current_login_state(&self, name: impl Into<String>) -> Option<LoginState> {
        let mut login_map_lock = self.login_state_map.lock().ok()?;
        let entry = login_map_lock.get_mut(&name.into())?;
        Self::poll_entry(entry);
        Some(entry.state.clone())
    }

    pub fn reset_login_state(&self, name: impl Into<String>) {
        let Ok(mut login_map_lock) = self.login_state_map.lock() else {
            return;
        };
        if let Some(entry) = login_map_lock.remove(&name.into()) {
            if let Some(session) = entry.session {
                session.cancel();
            }
        }
    }

    fn poll_entry(entry: &mut LoginEntry) {
        while let Ok(state) = entry.state_rx.try_recv() {
            entry.state = state.into();
        }
    }
}
