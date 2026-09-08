use color_eyre::{
    eyre::{ensure, eyre},
    Result,
};
use pam::{Client, PasswordConv};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};

pub(crate) const PAM_SERVICE: &str = "login";
use tibs_service_definitions::{
    AuthProof, AuthState, AuthenticationService, AuthenticationSession, UserAccount,
};

#[derive(Clone)]
pub struct LinuxPamAuthProof {
    client: Arc<RwLock<Client<'static, PasswordConv>>>,
}

impl LinuxPamAuthProof {
    pub(crate) fn new(client: Client<'static, PasswordConv>) -> Self {
        Self {
            client: Arc::new(RwLock::new(client)),
        }
    }

    pub(crate) fn verify_user(&self, username: &str) -> Result<()> {
        let mut client = self
            .client
            .write()
            .map_err(|_| eyre!("PAM authentication proof lock poisoned"))?;
        ensure!(
            client.get_user()? == username,
            "PAM proof belongs to another user"
        );
        Ok(())
    }
}

pub struct LinuxAuthenticationService;

impl LinuxAuthenticationService {
    pub fn new() -> Self {
        log::info!("Initializing Linux authentication service");
        Self
    }
}

impl Default for LinuxAuthenticationService {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthenticationService for LinuxAuthenticationService {
    fn create_session(&self, user: UserAccount) -> Result<Box<dyn AuthenticationSession>> {
        log::info!(
            "Creating Linux PAM authentication session for user '{}'",
            user.username
        );
        Ok(Box::new(LinuxAuthenticationSession::new(user)))
    }
}

struct AuthStateBus {
    current: Mutex<AuthState>,
    watchers: Mutex<Vec<smol::channel::Sender<AuthState>>>,
}

impl AuthStateBus {
    fn new() -> Self {
        Self {
            current: Mutex::new(AuthState::WaitingForInteraction),
            watchers: Mutex::new(Vec::new()),
        }
    }

    fn subscribe(&self) -> smol::channel::Receiver<AuthState> {
        let (tx, rx) = smol::channel::unbounded();
        if let Ok(current) = self.current.lock() {
            let _ = tx.try_send(current.clone());
        }
        self.watchers.lock().unwrap().push(tx);
        rx
    }

    fn set(&self, state: AuthState) {
        log::debug!(
            "Publishing Linux authentication state: {}",
            auth_state_label(&state)
        );
        if let Ok(mut current) = self.current.lock() {
            *current = state.clone();
        }

        if let Ok(mut watchers) = self.watchers.lock() {
            watchers.retain(|watcher| watcher.try_send(state.clone()).is_ok());
        }
    }
}

struct LinuxAuthenticationSession {
    user: UserAccount,
    state: Arc<AuthStateBus>,
    cancelled: Arc<AtomicBool>,
}

impl LinuxAuthenticationSession {
    fn new(user: UserAccount) -> Self {
        Self {
            user,
            state: Arc::new(AuthStateBus::new()),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl AuthenticationSession for LinuxAuthenticationSession {
    fn user(&self) -> &UserAccount {
        &self.user
    }

    fn watch_state(&self) -> smol::channel::Receiver<AuthState> {
        self.state.subscribe()
    }

    fn submit_password(&self, password: String) -> Result<()> {
        let username = self.user.username.clone();
        let state = Arc::clone(&self.state);
        let cancelled = Arc::clone(&self.cancelled);

        log::info!("Starting PAM password authentication for user '{username}'");
        state.set(AuthState::Checking);
        std::thread::spawn(move || {
            let fail = |message: String| {
                if cancelled.load(Ordering::Relaxed) {
                    log::info!("PAM authentication for user '{username}' cancelled");
                    state.set(AuthState::Cancelled);
                } else {
                    log::warn!("PAM authentication for user '{username}' failed: {message}");
                    state.set(AuthState::Failed(message));
                }
            };

            let mut client = match Client::with_password(PAM_SERVICE) {
                Ok(client) => client,
                Err(error) => return fail(format!("Failed to create PAM client: {error:#?}")),
            };

            client
                .conversation_mut()
                .set_credentials(&username, &password);
            if let Err(error) = client.authenticate() {
                return fail(format!("Failed to authenticate: {error:#?}"));
            }

            if cancelled.load(Ordering::Relaxed) {
                log::info!("PAM authentication for user '{username}' succeeded after cancellation");
                state.set(AuthState::Cancelled);
                return;
            }

            log::info!("PAM authentication for user '{username}' succeeded");
            state.set(AuthState::Authenticated(AuthProof::new(
                LinuxPamAuthProof::new(client),
            )));
        });

        Ok(())
    }

    fn cancel(&self) {
        log::info!(
            "Cancelling Linux authentication session for user '{}'",
            self.user.username
        );
        self.cancelled.store(true, Ordering::Relaxed);
        self.state.set(AuthState::Cancelled);
    }
}

fn auth_state_label(state: &AuthState) -> &'static str {
    match state {
        AuthState::WaitingForInteraction => "waiting_for_interaction",
        AuthState::Checking => "checking",
        AuthState::Failed(_) => "failed",
        AuthState::Authenticated(_) => "authenticated",
        AuthState::Cancelled => "cancelled",
    }
}
