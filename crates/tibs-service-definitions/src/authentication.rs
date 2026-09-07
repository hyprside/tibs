use color_eyre::Result;
use smol::channel;
use std::{any::Any, sync::Arc};

use crate::users::UserAccount;

#[derive(Clone)]
pub struct AuthProof {
    inner: Arc<dyn Any + Send + Sync>,
}

impl AuthProof {
    pub fn new<T: Any + Send + Sync>(inner: T) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn downcast_ref<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.inner.downcast_ref::<T>()
    }
}

#[derive(Clone)]
pub enum AuthState {
    WaitingForInteraction,
    Checking,
    Failed(String),
    Authenticated(AuthProof),
    Cancelled,
}

pub trait AuthenticationSession: Send {
    fn user(&self) -> &UserAccount;
    fn watch_state(&self) -> channel::Receiver<AuthState>;
    fn submit_password(&self, password: String) -> Result<()>;
    fn cancel(&self);
}

pub trait AuthenticationService: Send + Sync {
    fn create_session(&self, user: UserAccount) -> Result<Box<dyn AuthenticationSession>>;
}
