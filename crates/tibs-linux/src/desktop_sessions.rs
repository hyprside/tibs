use std::{env, fs};

use color_eyre::Result;
use freedesktop_entry_parser::parse_entry;
use tibs_service_definitions::{DesktopSession, DesktopSessionId, DesktopSessionRepositoryService};

pub struct LinuxDesktopSessionRepository;

impl LinuxDesktopSessionRepository {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxDesktopSessionRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl DesktopSessionRepositoryService for LinuxDesktopSessionRepository {
    fn list_sessions(&self) -> Result<Vec<DesktopSession>> {
        log::info!("Loading Linux desktop sessions");
        let session_dirs = env::var("XDG_SESSION_DIRS")
            .map(|value| value.split(':').map(String::from).collect::<Vec<_>>())
            .unwrap_or_else(|_| {
                vec![
                    "/usr/share/wayland-sessions".into(),
                    "/run/current-system/sw/share/wayland-sessions".into(),
                ]
            });

        log::debug!("Desktop session directories: {}", session_dirs.join(":"));
        let sessions = session_dirs
            .iter()
            .filter_map(|dir| fs::read_dir(dir).ok())
            .flat_map(|entries| entries.filter_map(Result::ok))
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|extension| extension == "desktop")
                    .unwrap_or(false)
            })
            .filter_map(|entry| {
                let path = entry.path();
                let entry = parse_entry(&path).ok()?;
                let section = entry.section("Desktop Entry");
                let name = section.attr("Name")?.to_string();
                let command = section.attr("Exec")?.to_string();
                Some(DesktopSession {
                    id: DesktopSessionId(path.to_string_lossy().to_string()),
                    name,
                    command,
                })
            })
            .collect::<Vec<_>>();
        log::info!("Loaded {} Linux desktop sessions", sessions.len());
        Ok(sessions)
    }
}
