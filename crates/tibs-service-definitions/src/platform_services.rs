use crate::{
    authentication::AuthenticationService, desktop_sessions::DesktopSessionRepositoryService,
    init_progress::SystemInitProgressService, session_manager::SessionManager,
    users::UserRepositoryService,
};

pub struct PlatformServices {
    pub init_progress: Box<dyn SystemInitProgressService>,
    pub users: Box<dyn UserRepositoryService>,
    pub desktop_sessions: Box<dyn DesktopSessionRepositoryService>,
    pub authentication: Box<dyn AuthenticationService>,
    pub sessions: Box<dyn SessionManager>,
}
