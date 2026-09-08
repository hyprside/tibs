pub mod authentication;
pub mod desktop_sessions;
pub mod session_manager;
pub mod systemd_progress;
pub mod users;

mod logind;
mod pam_session;
mod session;

#[cfg(test)]
mod session_dbus_tests;

pub use authentication::{LinuxAuthenticationService, LinuxPamAuthProof};
pub use desktop_sessions::LinuxDesktopSessionRepository;
pub use session_manager::LinuxSessionManager;
pub use systemd_progress::LinuxSystemInitProgressService;
pub use users::LinuxUserRepository;

use tibs_service_definitions::PlatformServices;
use zbus_systemd::{login1, systemd1, zbus::Connection};

#[derive(thiserror::Error, Debug)]
pub enum CreatePlatformLinuxError {
    #[error("failed to connect to dbus: {cause:#?}")]
    DBusConnectionError { cause: color_eyre::eyre::Error },
    #[error("failed to connect to systemd: {cause:#?}")]
    SystemDConnectionError { cause: color_eyre::eyre::Error },
    #[error("failed to connect to logind: {cause:#?}")]
    LoginDConnectionError { cause: color_eyre::eyre::Error },
}

async fn platform_services_layer_async() -> Result<PlatformServices, CreatePlatformLinuxError> {
    log::info!("Creating Linux platform services");

    let dbus = Connection::system()
        .await
        .map_err(Into::into)
        .map_err(|cause| CreatePlatformLinuxError::DBusConnectionError { cause })?;
    log::info!("Connected to dbus");
    let systemd = systemd1::ManagerProxy::new(&dbus)
        .await
        .map_err(Into::into)
        .map_err(|cause| CreatePlatformLinuxError::SystemDConnectionError { cause })?;
    log::info!("Connected to systemd");
    let login = login1::ManagerProxy::new(&dbus)
        .await
        .map_err(Into::into)
        .map_err(|cause| CreatePlatformLinuxError::LoginDConnectionError { cause })?;
    let sessions = LinuxSessionManager::new(&login, &systemd)
        .await
        .map_err(|cause| CreatePlatformLinuxError::LoginDConnectionError { cause })?;
    log::info!("Connected to logind");
    Ok(PlatformServices {
        init_progress: Box::new(LinuxSystemInitProgressService::new(&dbus, &systemd)),
        users: Box::new(LinuxUserRepository::new()),
        desktop_sessions: Box::new(LinuxDesktopSessionRepository::new()),
        authentication: Box::new(LinuxAuthenticationService::new()),
        sessions: Box::new(sessions),
    })
}

pub fn platform_services_layer() -> Result<PlatformServices, CreatePlatformLinuxError> {
    smol::block_on(platform_services_layer_async())
}
