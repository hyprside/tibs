//! Exercise the real proxies on an isolated bus, without PAM or VT changes.

use crate::{logind::Logind, session::LinuxSession};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tibs_service_definitions::{SessionStatus, UserAccount, UserId};
use zbus_systemd::{
    login1, systemd1, zbus,
    zvariant::{OwnedObjectPath, OwnedValue},
};

const LOGIN: &str = "/org/freedesktop/login1/session/greeter";
const DESKTOP: &str = "/org/freedesktop/login1/session/c42";
const SEAT: &str = "/org/freedesktop/login1/seat/seat0";
const UNIT: &str = "/org/freedesktop/systemd1/unit/desktop";

fn path(value: &str) -> OwnedObjectPath {
    value.try_into().unwrap()
}

struct Bus(Child);
impl Bus {
    fn start() -> (Self, String) {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("D-Bus integration tests require dbus-daemon");
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        (Self(child), address.trim().to_owned())
    }
}
impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct State {
    active: String,
    unit_state: String,
    result: String,
    started: bool,
    fail_start: bool,
    fail_activate: bool,
    activated: Vec<String>,
    terminated: Vec<String>,
    stopped: usize,
    unreferenced: usize,
    properties: Vec<(String, OwnedValue)>,
}
type Shared = Arc<Mutex<State>>;

struct LoginManager(Shared);
#[zbus::interface(name = "org.freedesktop.login1.Manager", crate = "zbus_systemd::zbus")]
impl LoginManager {
    #[zbus(name = "GetSessionByPID")]
    fn get_session_by_pid(&self, pid: u32) -> zbus::fdo::Result<OwnedObjectPath> {
        if pid == std::process::id() {
            Ok(path(LOGIN))
        } else if pid == 4242 && self.0.lock().unwrap().started {
            Ok(path(DESKTOP))
        } else {
            Err(zbus::fdo::Error::UnknownObject("No session".into()))
        }
    }
    fn get_session(&self, id: &str) -> zbus::fdo::Result<OwnedObjectPath> {
        match id {
            "greeter" => Ok(path(LOGIN)),
            "c42" if self.0.lock().unwrap().started => Ok(path(DESKTOP)),
            _ => Err(zbus::fdo::Error::UnknownObject("No session".into())),
        }
    }
    fn list_sessions(&self) -> Vec<(String, u32, String, String, OwnedObjectPath)> {
        let mut sessions = vec![(
            "greeter".into(),
            0,
            "root".into(),
            "seat0".into(),
            path(LOGIN),
        )];
        if self.0.lock().unwrap().started {
            sessions.push((
                "c42".into(),
                1000,
                "alice".into(),
                "seat0".into(),
                path(DESKTOP),
            ));
        }
        sessions
    }
    fn terminate_session(&self, id: String) {
        self.0.lock().unwrap().terminated.push(id);
    }
    #[zbus(signal)]
    async fn session_removed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        session_id: String,
        object_path: OwnedObjectPath,
    ) -> zbus::Result<()>;
    async fn activate_session(
        &self,
        id: String,
        #[zbus(connection)] connection: &zbus::Connection,
    ) -> zbus::fdo::Result<()> {
        {
            let mut state = self.0.lock().unwrap();
            if state.fail_activate {
                return Err(zbus::fdo::Error::AccessDenied("Denied".into()));
            }
            state.active = id.clone();
            state.activated.push(id);
        }
        let interface = connection
            .object_server()
            .interface::<_, Session>(LOGIN)
            .await?;
        interface
            .get()
            .await
            .active_changed(interface.signal_emitter())
            .await?;
        let seat = connection
            .object_server()
            .interface::<_, Seat>(SEAT)
            .await?;
        seat.get()
            .await
            .active_session_changed(seat.signal_emitter())
            .await?;
        Ok(())
    }
}

struct Session {
    shared: Shared,
    id: &'static str,
    uid: u32,
    vt: u32,
}
#[zbus::interface(name = "org.freedesktop.login1.Session", crate = "zbus_systemd::zbus")]
impl Session {
    #[zbus(property)]
    fn id(&self) -> &str {
        self.id
    }
    #[zbus(property)]
    fn user(&self) -> (u32, OwnedObjectPath) {
        (self.uid, path("/user"))
    }
    #[zbus(property)]
    fn seat(&self) -> (String, OwnedObjectPath) {
        ("seat0".into(), path(SEAT))
    }
    #[zbus(property, name = "VTNr")]
    fn vt_nr(&self) -> u32 {
        self.vt
    }
    #[zbus(property)]
    fn active(&self) -> bool {
        self.shared.lock().unwrap().active == self.id
    }
    #[zbus(property)]
    fn state(&self) -> &str {
        "online"
    }
    #[zbus(property)]
    fn class(&self) -> &str {
        if self.uid == 0 {
            "greeter"
        } else {
            "user"
        }
    }
    #[zbus(property, name = "Type")]
    fn type_property(&self) -> &str {
        "wayland"
    }
}

struct Seat(Shared);
#[zbus::interface(name = "org.freedesktop.login1.Seat", crate = "zbus_systemd::zbus")]
impl Seat {
    #[zbus(property, name = "CanTTY")]
    fn can_tty(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn active_session(&self) -> (String, OwnedObjectPath) {
        let id = self.0.lock().unwrap().active.clone();
        let object = if id == "greeter" { LOGIN } else { DESKTOP };
        (id, path(object))
    }
}

struct SystemManager(Shared);
#[zbus::interface(
    name = "org.freedesktop.systemd1.Manager",
    crate = "zbus_systemd::zbus"
)]
impl SystemManager {
    fn subscribe(&self) {}
    fn get_unit(&self, _name: String) -> OwnedObjectPath {
        path(UNIT)
    }
    async fn start_transient_unit(
        &self,
        name: String,
        _mode: String,
        properties: Vec<(String, OwnedValue)>,
        _aux: Vec<(String, Vec<(String, OwnedValue)>)>,
        #[zbus(signal_emitter)] emitter: zbus::object_server::SignalEmitter<'_>,
    ) -> zbus::fdo::Result<OwnedObjectPath> {
        let result = {
            let mut state = self.0.lock().unwrap();
            state.properties = properties;
            state.started = true;
            state.unit_state = if state.fail_start { "failed" } else { "active" }.into();
            state.result = if state.fail_start {
                "exit-code"
            } else {
                "success"
            }
            .into();
            if state.fail_start {
                "failed"
            } else {
                "done"
            }
        };
        // Emit before replying, exercising the startup subscription ordering.
        Self::job_removed(&emitter, 1, path("/job/1"), name, result.into()).await?;
        Ok(path("/job/1"))
    }
    #[zbus(signal)]
    async fn job_removed(
        emitter: &zbus::object_server::SignalEmitter<'_>,
        id: u32,
        job: OwnedObjectPath,
        unit: String,
        result: String,
    ) -> zbus::Result<()>;
    fn stop_unit(&self, _name: String, _mode: String) -> OwnedObjectPath {
        self.0.lock().unwrap().stopped += 1;
        path("/job/2")
    }
    fn unref_unit(&self, _name: String) {
        self.0.lock().unwrap().unreferenced += 1;
    }
}
struct Unit(Shared);
#[zbus::interface(name = "org.freedesktop.systemd1.Unit", crate = "zbus_systemd::zbus")]
impl Unit {
    #[zbus(property)]
    fn active_state(&self) -> String {
        self.0.lock().unwrap().unit_state.clone()
    }
}
struct Service(Shared);
#[zbus::interface(
    name = "org.freedesktop.systemd1.Service",
    crate = "zbus_systemd::zbus"
)]
impl Service {
    #[zbus(property, name = "MainPID")]
    fn main_pid(&self) -> u32 {
        4242
    }
    #[zbus(property)]
    fn result(&self) -> String {
        self.0.lock().unwrap().result.clone()
    }
}

async fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for D-Bus state change"
        );
        smol::Timer::after(Duration::from_millis(10)).await;
    }
}

#[test]
fn session_lifecycle_over_dbus() {
    let (_bus, address) = Bus::start();
    smol::block_on(async {
        let shared = Arc::new(Mutex::new(State {
            active: "greeter".into(),
            unit_state: "inactive".into(),
            result: "success".into(),
            started: false,
            fail_start: false,
            fail_activate: false,
            activated: vec![],
            terminated: vec![],
            stopped: 0,
            unreferenced: 0,
            properties: vec![],
        }));
        let server = zbus::connection::Builder::address(address.as_str())?
            .name("org.freedesktop.login1")?
            .name("org.freedesktop.systemd1")?
            .serve_at("/org/freedesktop/login1", LoginManager(shared.clone()))?
            .serve_at(
                LOGIN,
                Session {
                    shared: shared.clone(),
                    id: "greeter",
                    uid: 0,
                    vt: 1,
                },
            )?
            .serve_at(
                DESKTOP,
                Session {
                    shared: shared.clone(),
                    id: "c42",
                    uid: 1000,
                    vt: 2,
                },
            )?
            .serve_at(SEAT, Seat(shared.clone()))?
            .serve_at("/org/freedesktop/systemd1", SystemManager(shared.clone()))?
            .serve_at(UNIT, Unit(shared.clone()))?
            .serve_at(UNIT, Service(shared.clone()))?
            .build()
            .await?;
        let client = zbus::connection::Builder::address(address.as_str())?
            .build()
            .await?;
        let manager = login1::ManagerProxy::new(&client).await?;
        let systemd = systemd1::ManagerProxy::new(&client).await?;
        let logind = Logind::new(manager, &systemd).await?;
        assert!(logind.login.active().await?);
        assert_eq!(logind.next_vt().await?, Some(2));
        assert!(logind.session("missing").await?.is_none());
        let user = UserAccount {
            id: UserId("1000".into()),
            username: "alice".into(),
            display_name: "Alice".into(),
            home_dir: Some("/home/alice".into()),
            avatar_path: None,
        };
        let desktop = LinuxSession::start(
            &systemd,
            &logind,
            "test.service".into(),
            &user,
            Command::new("/bin/true"),
        )
        .await?;
        assert_eq!(desktop.id, "c42");
        assert_eq!(desktop.status()?, SessionStatus::Running);
        until(|| {
            logind
                .login
                .inner()
                .cached_property::<bool>("Active")
                .unwrap()
                == Some(false)
        })
        .await;
        assert_eq!(logind.user_session(1000).await?.unwrap().id().await?, "c42");

        // A crash updates status through PropertiesChanged and returns to greeter.
        {
            let mut state = shared.lock().unwrap();
            state.unit_state = "failed".into();
            state.result = "signal".into();
        }
        let unit = server.object_server().interface::<_, Unit>(UNIT).await?;
        unit.get()
            .await
            .active_state_changed(unit.signal_emitter())
            .await?;
        until(|| {
            desktop.status().unwrap() == SessionStatus::Crashed
                && shared.lock().unwrap().active == "greeter"
        })
        .await;
        until(|| shared.lock().unwrap().unreferenced == 1).await;
        assert_eq!(shared.lock().unwrap().stopped, 0);
        drop(desktop);

        // PAM/exec failure never activates a desktop and cleans up its unit.
        {
            let mut state = shared.lock().unwrap();
            state.started = false;
            state.fail_start = true;
        }
        let activated = shared.lock().unwrap().activated.len();
        assert!(LinuxSession::start(
            &systemd,
            &logind,
            "failed.service".into(),
            &user,
            Command::new("/bin/true")
        )
        .await
        .is_err());
        assert_eq!(shared.lock().unwrap().activated.len(), activated);
        assert_eq!(shared.lock().unwrap().stopped, 1);
        assert_eq!(shared.lock().unwrap().unreferenced, 2);

        // Failure to activate also tears down only the newly started service.
        {
            let mut state = shared.lock().unwrap();
            state.started = false;
            state.fail_start = false;
            state.fail_activate = true;
        }
        assert!(LinuxSession::start(
            &systemd,
            &logind,
            "denied.service".into(),
            &user,
            Command::new("/bin/true")
        )
        .await
        .is_err());
        assert_eq!(shared.lock().unwrap().stopped, 2);
        assert_eq!(shared.lock().unwrap().unreferenced, 3);
        assert_eq!(shared.lock().unwrap().terminated, ["c42"]);

        // A clean exit must not steal focus from another active session.
        {
            let mut state = shared.lock().unwrap();
            state.started = false;
            state.fail_activate = false;
        }
        let desktop = LinuxSession::start(
            &systemd,
            &logind,
            "clean.service".into(),
            &user,
            Command::new("/bin/true"),
        )
        .await?;
        logind.manager.activate_session("other".into()).await?;
        {
            let mut state = shared.lock().unwrap();
            state.unit_state = "inactive".into();
            state.result = "success".into();
        }
        unit.get()
            .await
            .active_state_changed(unit.signal_emitter())
            .await?;
        until(|| desktop.status().unwrap() == SessionStatus::ShutdownGracefully).await;
        until(|| shared.lock().unwrap().unreferenced == 4).await;
        assert_eq!(shared.lock().unwrap().active, "other");
        assert_eq!(shared.lock().unwrap().stopped, 2);

        // A desktop adopted from another manager also returns to the greeter
        // when logind removes it, without stopping any external systemd unit.
        let external = logind.session("c42").await?.unwrap();
        let watch = logind.watch_session_end(external).await?;
        logind.manager.activate_session("c42".into()).await?;
        let login_manager = server
            .object_server()
            .interface::<_, LoginManager>("/org/freedesktop/login1")
            .await?;
        LoginManager::session_removed(login_manager.signal_emitter(), "c42".into(), path(DESKTOP))
            .await?;
        until(|| shared.lock().unwrap().active == "greeter").await;
        watch.await;
        assert_eq!(shared.lock().unwrap().stopped, 2);
        Ok::<_, color_eyre::Report>(())
    })
    .unwrap();
}
