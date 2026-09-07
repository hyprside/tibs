pub mod authentication;
pub mod desktop_sessions;
pub mod session_manager;
pub mod systemd_progress;
pub mod users;

mod session;
mod tty;

pub use authentication::{LinuxAuthenticationService, LinuxPamAuthProof};
pub use desktop_sessions::LinuxDesktopSessionRepository;
pub use session_manager::LinuxSessionManager;
pub use systemd_progress::LinuxSystemInitProgressService;
pub use users::LinuxUserRepository;

use tibs_service_definitions::PlatformServices;

pub fn create_platform_services() -> PlatformServices {
    PlatformServices {
        init_progress: Box::new(LinuxSystemInitProgressService::new()),
        users: Box::new(LinuxUserRepository::new()),
        desktop_sessions: Box::new(LinuxDesktopSessionRepository::new()),
        authentication: Box::new(LinuxAuthenticationService::new()),
        sessions: Box::new(LinuxSessionManager::new()),
    }
}
