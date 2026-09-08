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

impl LoginState {
    fn label(&self) -> &'static str {
        match self {
            LoginState::WaitingForInteraction => "waiting_for_interaction",
            LoginState::Logging => "logging",
            LoginState::Failed(_) => "failed",
            LoginState::Authenticated(_) => "authenticated",
            LoginState::Cancelled => "cancelled",
        }
    }
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
        log::info!("Initializing login manager");
        Self {
            authentication_service,
            login_state_map: Default::default(),
        }
    }

    pub fn begin_authentication(&self, user: UserAccount) -> bool {
        let username = user.username.clone();
        log::debug!("Ensuring authentication session exists for user '{username}'");

        let Ok(mut login_map_lock) = self.login_state_map.lock() else {
            log::error!("Failed to lock login state map while beginning authentication");
            return false;
        };

        if let Some(entry) = login_map_lock.get_mut(&username) {
            Self::poll_entry(&username, entry);
            if !matches!(entry.state, LoginState::Cancelled) {
                log::debug!(
                    "Authentication session for user '{username}' already active with state {}",
                    entry.state.label()
                );
                return false;
            }
            log::info!(
                "Recreating authentication session for user '{username}' after state {}",
                entry.state.label()
            );
        }

        let entry = match self.authentication_service.create_session(user) {
            Ok(session) => {
                let state_rx = session.watch_state();
                log::info!("Authentication session created for user '{username}'");
                LoginEntry {
                    session: Some(session),
                    state_rx,
                    state: LoginState::WaitingForInteraction,
                }
            }
            Err(error) => {
                log::error!(
                    "Failed to create authentication session for user '{username}': {error:#?}"
                );
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
                log::info!("Cancelling replaced authentication session");
                session.cancel();
            }
        }
        true
    }

    pub fn submit_password(&self, name: impl Into<String>, password: impl Into<String>) -> bool {
        let name = name.into();
        log::info!("Submitting password authentication request for user '{name}'");
        let Ok(mut login_map_lock) = self.login_state_map.lock() else {
            log::error!("Failed to lock login state map while submitting password");
            return false;
        };
        let Some(entry) = login_map_lock.get_mut(&name) else {
            log::warn!("No authentication session exists for user '{name}'");
            return false;
        };
        Self::poll_entry(&name, entry);
        if matches!(
            entry.state,
            LoginState::Logging | LoginState::Authenticated(_)
        ) {
            log::warn!(
                "Ignoring password authentication request for user '{name}' because state is {}",
                entry.state.label()
            );
            return false;
        }
        let Some(session) = &entry.session else {
            log::warn!("Authentication entry for user '{name}' has no active session object");
            return false;
        };
        if let Err(error) = session.submit_password(password.into()) {
            log::error!(
                "Authentication service rejected password submission for user '{name}': {error:#?}"
            );
            entry.state = LoginState::Failed(error.to_string());
            return false;
        }
        Self::poll_entry(&name, entry);
        true
    }

    pub fn start_login(&self, user: UserAccount, password: impl Into<String>) -> bool {
        let username = user.username.clone();
        log::info!("Starting login flow for user '{username}'");
        let _ = self.begin_authentication(user);
        self.submit_password(username, password)
    }

    pub fn get_current_login_state(&self, name: impl Into<String>) -> Option<LoginState> {
        let mut login_map_lock = self.login_state_map.lock().ok()?;
        let name = name.into();
        let entry = login_map_lock.get_mut(&name)?;
        Self::poll_entry(&name, entry);
        Some(entry.state.clone())
    }

    pub fn reset_login_state(&self, name: impl Into<String>) {
        let name = name.into();
        log::info!("Resetting login state for user '{name}'");
        let Ok(mut login_map_lock) = self.login_state_map.lock() else {
            log::error!("Failed to lock login state map while resetting login state");
            return;
        };
        if let Some(entry) = login_map_lock.remove(&name) {
            if let Some(session) = entry.session {
                log::info!("Cancelling authentication session for user '{name}'");
                session.cancel();
            }
        }
    }

    fn poll_entry(name: &str, entry: &mut LoginEntry) {
        while let Ok(state) = entry.state_rx.try_recv() {
            let new_state: LoginState = state.into();
            if entry.state.label() != new_state.label() {
                match &new_state {
                    LoginState::Failed(message) => log::warn!(
                        "Authentication state for user '{name}' changed: {} -> failed: {message}",
                        entry.state.label()
                    ),
                    _ => log::info!(
                        "Authentication state for user '{name}' changed: {} -> {}",
                        entry.state.label(),
                        new_state.label()
                    ),
                }
            }
            entry.state = new_state;
        }
    }
}
