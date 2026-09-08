//! Session discovery and activation. No terminal descriptors or VT ioctls.

use color_eyre::{
    eyre::{bail, ensure, OptionExt, WrapErr},
    Result,
};
use futures_util::{FutureExt, StreamExt};
use std::collections::HashSet;
use zbus_systemd::{login1, systemd1, zbus, zvariant::OwnedObjectPath};

use crate::pam_session::GreeterPamSession;

pub(crate) struct Logind {
    pub(crate) manager: login1::ManagerProxy<'static>,
    pub(crate) login: login1::SessionProxy<'static>,
    pub(crate) seat: login1::SeatProxy<'static>,
    pub(crate) seat_id: String,
    // Keep registration alive until the greeter exits.
    _pam: Option<GreeterPamSession>,
}

impl Logind {
    pub(crate) async fn watch_session_end(
        &self,
        session: login1::SessionProxy<'static>,
    ) -> Result<smol::Task<()>> {
        let id = session.id().await?;
        let login_id = self.login.id().await?;
        let mut changes = session.receive_state_changed().await;
        let mut removed = self.manager.receive_session_removed().await?;
        let manager = self.manager.clone();
        let seat = self.seat.clone();
        Ok(smol::spawn(async move {
            let result = async {
                loop {
                    match session.state().await {
                        Ok(state) if state == "closing" => break,
                        Err(error) if is_missing_session(&error) => break,
                        Err(error) => return Err(error.into()),
                        _ => {}
                    }
                    futures_util::select! {
                        change = changes.next().fuse() => {
                            if change.is_none() { bail!("logind state stream ended"); }
                        }
                        signal = removed.next().fuse() => {
                            let signal = signal.ok_or_eyre("logind removal stream ended")?;
                            if signal.args()?.session_id == id { break; }
                        }
                    }
                }
                let (active, _) = seat.active_session().await?;
                if active.is_empty() || active == id {
                    manager.activate_session(login_id).await?;
                }
                Ok::<_, color_eyre::Report>(())
            }
            .await;
            if let Err(error) = result {
                log::warn!("Monitoring adopted logind session {id}: {error:#}");
            }
        }))
    }

    pub(crate) async fn new(
        manager: login1::ManagerProxy<'static>,
        systemd: &systemd1::ManagerProxy<'static>,
    ) -> Result<Self> {
        let connection = manager.inner().connection();
        let mut pam = None;
        let path = match manager.get_session_by_pid(std::process::id()).await {
            Ok(path) => path,
            Err(error) if is_missing_session(&error) => {
                // Our system service may only configure TTYPath, without PAMName.
                // Ask systemd for that metadata rather than querying /dev/console.
                let unit = systemd.get_unit_by_pid(std::process::id()).await?;
                let service = systemd1::ServiceProxy::new(connection, unit).await?;
                let tty = service.tty_path().await.wrap_err(
                    "TIBS needs a logind session or a systemd service with TTYPath=/dev/ttyN",
                )?;
                // A developer-launched GLFW process commonly belongs to no
                // logind session and its systemd unit has `TTYPath=""`. In
                // that case use the current seat session as the host session;
                // production greeters still take the PAM/TTY path below.
                if tty.is_empty() {
                    let seat_path = manager.get_seat("seat0".into()).await?;
                    let seat = login1::SeatProxy::builder(connection)
                        .path(seat_path)?
                        .build()
                        .await?;
                    let (_, active_path) = seat.active_session().await?;
                    ensure!(
                        active_path.as_str() != "/",
                        "seat0 has no active session for GLFW development mode"
                    );
                    log::warn!("TIBS is not running in a logind session; using active seat0 session as its host");
                    active_path
                } else {
                    let vt = vt_from_path(&tty)?;
                    let user = uzers::get_user_by_uid(uzers::get_current_uid())
                        .ok_or_eyre("Could not resolve the greeter's OS user")?;
                    let username = user
                        .name()
                        .to_str()
                        .ok_or_eyre("Non-UTF-8 greeter username")?;
                    pam = Some(GreeterPamSession::open(username, vt)?);
                    manager.get_session_by_pid(std::process::id()).await.wrap_err(
                    "PAM did not register the greeter with logind; the login PAM session stack must include pam_systemd",
                )?
                }
            }
            Err(error) => return Err(error.into()),
        };
        let login = Self::session_proxy(connection, path).await?;
        let (seat_id, seat_path) = login.seat().await?;
        ensure!(
            !seat_id.is_empty(),
            "The TIBS login session is not attached to a seat"
        );
        let seat = login1::SeatProxy::builder(connection)
            .path(seat_path)?
            .build()
            .await?;
        // Prime zbus's property cache. Subsequent reads use PropertiesChanged,
        // including switches initiated outside TIBS (e.g. Ctrl+Alt+Fn).
        let active = login.active().await?;
        log::info!(
            "Using logind session '{}' on {seat_id} (active={active})",
            login.id().await?
        );
        Ok(Self {
            manager,
            login,
            seat,
            seat_id,
            _pam: pam,
        })
    }

    pub(crate) async fn session_proxy(
        connection: &zbus::Connection,
        path: OwnedObjectPath,
    ) -> Result<login1::SessionProxy<'static>> {
        Ok(login1::SessionProxy::builder(connection)
            .path(path)?
            .build()
            .await?)
    }

    pub(crate) async fn session(&self, id: &str) -> Result<Option<login1::SessionProxy<'static>>> {
        match self.manager.get_session(id.to_owned()).await {
            Ok(path) => Ok(Some(
                Self::session_proxy(self.manager.inner().connection(), path).await?,
            )),
            Err(error) if is_missing_session(&error) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) async fn user_session(
        &self,
        uid: u32,
    ) -> Result<Option<login1::SessionProxy<'static>>> {
        let mut candidates = Vec::new();
        for (id, session_uid, _, seat, path) in self.manager.list_sessions().await? {
            if session_uid != uid || seat != self.seat_id {
                continue;
            }
            let session = Self::session_proxy(self.manager.inner().connection(), path).await?;
            let details = async {
                Ok::<_, zbus::Error>((
                    session.class().await?,
                    session.type_property().await?,
                    session.state().await?,
                    session.active().await?,
                ))
            }
            .await;
            let (class, kind, state, active) = match details {
                Ok(details) => details,
                Err(error) if is_missing_session(&error) => continue,
                Err(error) => return Err(error.into()),
            };
            if is_desktop_session(&class, &kind, &state) {
                candidates.push((!active, id, session));
            }
        }
        candidates.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
        Ok(candidates.into_iter().next().map(|(_, _, session)| session))
    }

    pub(crate) async fn next_vt(&self) -> Result<Option<u32>> {
        if !self.seat.can_tty().await? {
            return Ok(None);
        }
        let mut occupied = HashSet::new();
        for (_, _, _, seat, path) in self.manager.list_sessions().await? {
            if seat != self.seat_id {
                continue;
            }
            let session = Self::session_proxy(self.manager.inner().connection(), path).await?;
            match session.vt_nr().await {
                Ok(vt) => {
                    occupied.insert(vt);
                }
                Err(error) if is_missing_session(&error) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        // logind requires a VT number for seat0, but has no allocator API.
        // Actual acquisition uses systemd's tty-fail (never steal a terminal).
        (1..=63)
            .find(|vt| !occupied.contains(vt))
            .map(Some)
            .ok_or_eyre("No unoccupied virtual terminal reported by logind")
    }
}

pub(crate) fn is_missing_session(error: &zbus::Error) -> bool {
    matches!(error, zbus::Error::MethodError(name, _, _) if matches!(name.as_str(),
        "org.freedesktop.login1.NoSuchSession" | "org.freedesktop.login1.NoSessionForPID" |
        "org.freedesktop.DBus.Error.UnknownObject"))
}

fn vt_from_path(path: &str) -> Result<u32> {
    if let Some(vt) = path
        .strip_prefix("/dev/tty")
        .and_then(|s| s.parse::<u32>().ok())
    {
        if (1..=63).contains(&vt) {
            return Ok(vt);
        }
    }
    bail!("Expected a concrete systemd TTYPath=/dev/ttyN, got {path:?}")
}

fn is_desktop_session(class: &str, kind: &str, state: &str) -> bool {
    matches!(
        class,
        "user" | "user-early" | "user-light" | "user-early-light"
    ) && matches!(kind, "wayland" | "x11")
        && matches!(state, "active" | "online")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_live_user_desktops_are_reusable() {
        assert!(is_desktop_session("user", "wayland", "online"));
        assert!(is_desktop_session("user", "x11", "active"));
        for (class, kind, state) in [
            ("greeter", "wayland", "active"),
            ("user", "tty", "active"),
            ("user", "wayland", "closing"),
            ("manager", "unspecified", "online"),
        ] {
            assert!(!is_desktop_session(class, kind, state));
        }
    }

    #[test]
    fn requires_a_concrete_vt_in_service_metadata() {
        assert_eq!(vt_from_path("/dev/tty12").unwrap(), 12);
        for path in ["", "/dev/console", "/dev/tty0", "/dev/tty64", "/dev/pts/1"] {
            assert!(vt_from_path(path).is_err());
        }
    }
}
