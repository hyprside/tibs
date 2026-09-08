//! Desktop process lifecycle, delegated to the system service manager.

use color_eyre::{
    eyre::{bail, ensure, eyre, OptionExt, WrapErr},
    Result,
};
use futures_util::{FutureExt, StreamExt};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};
use tibs_service_definitions::{SessionStatus, UserAccount};
use zbus_systemd::{
    systemd1,
    zbus::proxy::CacheProperties,
    zvariant::{OwnedValue, Value},
};

use crate::{authentication::PAM_SERVICE, logind::Logind};

pub(crate) struct LinuxSession {
    pub(crate) id: String,
    status: Arc<Mutex<std::result::Result<SessionStatus, String>>>,
    watch: Option<smol::Task<()>>,
}

impl LinuxSession {
    pub(crate) async fn start(
        systemd: &systemd1::ManagerProxy<'static>,
        logind: &Logind,
        unit_name: String,
        user: &UserAccount,
        command: Command,
    ) -> Result<Self> {
        let vt = logind.next_vt().await?;
        let properties = service_properties(user, &command, &logind.seat_id, vt)?;
        let connection = systemd.inner().connection();
        let login_id = logind.login.id().await?;
        let mut created_session = None;
        let mut jobs = systemd.receive_job_removed().await?;
        log::info!("Starting desktop for '{}' in {unit_name}", user.username);
        let job = systemd
            .start_transient_unit(unit_name.clone(), "fail".into(), properties, vec![])
            .await?;

        let started = async {
            let path = systemd.get_unit(unit_name.clone()).await?;
            let unit = systemd1::UnitProxy::new(connection, path.clone()).await?;
            let service = systemd1::ServiceProxy::builder(connection).path(path)?
                .cache_properties(CacheProperties::No).build().await?;
            let deadline = FutureExt::fuse(smol::Timer::after(Duration::from_secs(30)));
            futures_util::pin_mut!(deadline);
            loop {
                futures_util::select! {
                    signal = jobs.next().fuse() => {
                        let signal = signal.ok_or_eyre("systemd job stream ended during session startup")?;
                        let args = signal.args()?;
                        if args.job != job { continue; }
                        ensure!(args.result == "done", "{unit_name} startup job {}: {}", args.result, service.result().await?);
                        break;
                    }
                    _ = deadline => bail!("Timed out starting {unit_name}"),
                }
            }
            let pid = service.main_pid().await?;
            ensure!(pid != 0, "{unit_name} exited before registering a desktop session");
            let path = logind.manager.get_session_by_pid(pid).await.wrap_err(
                "The desktop did not register with logind; the login PAM session stack must include pam_systemd",
            )?;
            let session = Logind::session_proxy(connection, path).await?;
            ensure!(session.user().await?.0 == user.id.0.parse::<u32>()?, "Desktop session UID mismatch");
            ensure!(session.seat().await?.0 == logind.seat_id, "Desktop session seat mismatch");
            let id = session.id().await?;
            ensure!(id != login_id, "Desktop was registered in the greeter session");
            created_session = Some(id.clone());
            let changes = unit.receive_active_state_changed().await;
            logind.manager.activate_session(id.clone()).await?;
            Ok::<_, color_eyre::Report>((id, unit, service, changes))
        }.await;

        let (id, unit, service, mut changes) = match started {
            Ok(started) => started,
            Err(error) => {
                // Only stop the unit created here, never an adopted session.
                if let Some(id) = created_session {
                    if let Err(cleanup) = logind.manager.terminate_session(id).await {
                        log::warn!("Failed to terminate the new logind session: {cleanup}");
                    }
                }
                if let Err(cleanup) = systemd.stop_unit(unit_name.clone(), "replace".into()).await {
                    log::error!("Failed to clean up {unit_name} after startup failure: {cleanup}");
                }
                let _ = systemd.unref_unit(unit_name).await;
                return Err(error);
            }
        };
        let status = Arc::new(Mutex::new(Ok(SessionStatus::Running)));
        let current = Arc::clone(&status);
        let manager = logind.manager.clone();
        let systemd = systemd.clone();
        let seat = logind.seat.clone();
        let desktop_id = id.clone();
        let watch = smol::spawn(async move {
            let result = async {
                loop {
                    let state = unit.active_state().await?;
                    if matches!(state.as_str(), "inactive" | "failed") {
                        let terminal = terminal_status(&state, &service.result().await?)
                            .expect("terminal service state");
                        *current.lock().unwrap() = Ok(terminal);
                        log::info!("Desktop session {desktop_id} ended: {terminal:?}");
                        let (active, _) = seat.active_session().await?;
                        // Do not interrupt a different session the user switched to.
                        if active.is_empty() || active == desktop_id {
                            if let Err(error) = manager.activate_session(login_id).await {
                                log::warn!("Could not return to the login session: {error}");
                            }
                        }
                        break;
                    }
                    if changes.next().await.is_none() {
                        bail!("systemd session state stream ended");
                    }
                }
                Ok::<_, color_eyre::Report>(())
            }
            .await;
            if let Err(error) = result {
                log::error!("Monitoring {unit_name}: {error:#}");
                *current.lock().unwrap() = Err(error.to_string());
            }
            let _ = systemd.unref_unit(unit_name).await;
        });
        Ok(Self {
            id,
            status,
            watch: Some(watch),
        })
    }

    pub(crate) fn status(&self) -> Result<SessionStatus> {
        self.status
            .lock()
            .unwrap()
            .clone()
            .map_err(|error| eyre!(error))
    }
}

impl Drop for LinuxSession {
    fn drop(&mut self) {
        // systemd owns the desktop, not the greeter. Keep watching until exit
        // so the unit reference is released even if the manager is replaced.
        if let Some(watch) = self.watch.take() {
            watch.detach();
        }
    }
}

fn terminal_status(state: &str, result: &str) -> Option<SessionStatus> {
    match state {
        "failed" => Some(SessionStatus::Crashed),
        "inactive" => Some(if result == "success" {
            SessionStatus::ShutdownGracefully
        } else {
            SessionStatus::Crashed
        }),
        _ => None,
    }
}

fn text(value: &OsStr) -> Result<&str> {
    let value = value
        .to_str()
        .ok_or_eyre("systemd D-Bus commands must be UTF-8")?;
    ensure!(
        !value.contains('\0'),
        "systemd D-Bus strings cannot contain NUL"
    );
    Ok(value)
}

fn resolve_program(command: &Command) -> Result<PathBuf> {
    let program = Path::new(command.get_program());
    if program.is_absolute() {
        return Ok(program.to_owned());
    }
    ensure!(
        program.components().count() == 1,
        "Use an absolute executable path, not a relative path with directories"
    );
    let path = command
        .get_envs()
        .find(|(key, _)| *key == "PATH")
        .map(|(_, value)| value.map(OsStr::to_owned))
        .unwrap_or_else(|| std::env::var_os("PATH"))
        .ok_or_eyre("PATH is required to resolve the compositor executable")?;
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&path)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(program))
        .find(|candidate| {
            candidate
                .metadata()
                .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        })
        .ok_or_eyre("Could not resolve compositor executable in PATH")
}

fn service_properties(
    user: &UserAccount,
    command: &Command,
    seat: &str,
    vt: Option<u32>,
) -> Result<Vec<(String, OwnedValue)>> {
    let program = resolve_program(command)?;
    let argv = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|arg| text(arg).map(str::to_owned))
        .collect::<Result<Vec<_>>>()?;
    let directory = command
        .get_current_dir()
        .or(user.home_dir.as_deref())
        .ok_or_eyre("Desktop user has no home directory")?;
    ensure!(
        directory.is_absolute(),
        "Desktop working directory must be absolute"
    );

    // Never inherit the greeter's identity, display socket or session address.
    let mut environment = BTreeMap::new();
    for key in [
        "PATH",
        "LANG",
        "LANGUAGE",
        "TZ",
        "XDG_DATA_DIRS",
        "XDG_CONFIG_DIRS",
    ] {
        if let Some(value) = std::env::var_os(key) {
            environment.insert(key.to_owned(), text(&value)?.to_owned());
        }
    }
    for (key, value) in command.get_envs() {
        let key = text(key)?;
        if let Some(value) = value {
            environment.insert(key.to_owned(), text(value)?.to_owned());
        } else {
            environment.remove(key);
        }
    }
    for key in [
        "XDG_SESSION_ID",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
        "XDG_VTNR",
        "HOME",
        "USER",
        "LOGNAME",
        "SHELL",
    ] {
        environment.remove(key);
    }
    environment.insert("XDG_SESSION_TYPE".into(), "wayland".into());
    environment.insert("XDG_SESSION_CLASS".into(), "user".into());
    environment.insert("XDG_SEAT".into(), seat.into());
    if let Some(vt) = vt {
        environment.insert("XDG_VTNR".into(), vt.to_string());
    }
    let environment: Vec<String> = environment
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();

    macro_rules! property {
        ($name:literal, $value:expr) => {
            ($name.to_owned(), OwnedValue::try_from(Value::from($value))?)
        };
    }
    let mut properties = vec![
        property!("Description", "TIBS desktop session"),
        property!("Type", "exec"),
        property!("AddRef", true),
        property!("CollectMode", "inactive-or-failed"),
        property!("User", user.username.as_str()),
        property!("PAMName", PAM_SERVICE),
        property!("WorkingDirectory", text(directory.as_os_str())?),
        property!("Environment", environment),
        // D-Bus argv is already tokenized. Preserve $FOO inside shell commands.
        property!(
            "ExecStartEx",
            vec![(
                text(program.as_os_str())?.to_owned(),
                argv,
                vec!["no-env-expand".to_owned()]
            )]
        ),
        property!("StandardOutput", "journal"),
        property!("StandardError", "journal"),
        property!("TimeoutStartUSec", 30_000_000u64),
        property!("TimeoutStopUSec", 10_000_000u64),
    ];
    if let Some(vt) = vt {
        properties.push(property!("TTYPath", format!("/dev/tty{vt}")));
        properties.push(property!("StandardInput", "tty-fail"));
        properties.push(property!("TTYReset", true));
    } else {
        properties.push(property!("StandardInput", "null"));
    }
    Ok(properties)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tibs_service_definitions::UserId;

    #[test]
    fn inactive_and_failed_services_preserve_exit_outcome() {
        assert_eq!(terminal_status("active", "success"), None);
        assert_eq!(terminal_status("deactivating", "exit-code"), None);
        assert_eq!(
            terminal_status("inactive", "success"),
            Some(SessionStatus::ShutdownGracefully)
        );
        assert_eq!(
            terminal_status("inactive", "signal"),
            Some(SessionStatus::Crashed)
        );
        assert_eq!(
            terminal_status("failed", "exit-code"),
            Some(SessionStatus::Crashed)
        );
    }

    #[test]
    fn service_preserves_argv_and_leaves_identity_and_runtime_to_pam() {
        let user = UserAccount {
            id: UserId("1000".into()),
            username: "alice".into(),
            display_name: "Alice".into(),
            home_dir: Some("/home/alice".into()),
            avatar_path: None,
        };
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "exec compositor --name '$literal %value'"])
            .env("XDG_RUNTIME_DIR", "/run/user/0")
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/0/bus")
            .env("CUSTOM", "a b");
        let mut properties: BTreeMap<_, _> = service_properties(&user, &command, "seat0", Some(3))
            .unwrap()
            .into_iter()
            .collect();
        let exec: Vec<(String, Vec<String>, Vec<String>)> = properties
            .remove("ExecStartEx")
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            exec[0].1,
            ["/bin/sh", "-c", "exec compositor --name '$literal %value'"]
        );
        assert_eq!(exec[0].2, ["no-env-expand"]);
        let env: Vec<String> = properties
            .remove("Environment")
            .unwrap()
            .try_into()
            .unwrap();
        assert!(env.contains(&"XDG_SESSION_CLASS=user".into()));
        assert!(env.contains(&"XDG_VTNR=3".into()));
        assert!(env.contains(&"CUSTOM=a b".into()));
        assert!(!env.iter().any(
            |s| s.starts_with("XDG_RUNTIME_DIR=") || s.starts_with("DBUS_SESSION_BUS_ADDRESS=")
        ));
        assert_eq!(
            String::try_from(properties.remove("StandardInput").unwrap()).unwrap(),
            "tty-fail"
        );
    }
}
