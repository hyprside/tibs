use futures_util::{FutureExt as _, StreamExt};
use rand::Rng;
use smol::channel;
use std::thread;
use std::time::Duration;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use zbus_systemd::systemd1::{ManagerProxy, UnitProxy};
use zbus_systemd::zbus::{self, Connection};
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ServiceState {
    Loading,
    Failed,
    Loaded,
}
#[derive(Clone, Debug, Default)]
pub struct ProgressData {
    pub services: HashMap<String, ServiceState>,
    pub finished: bool,
}

impl ProgressData {
    pub fn get_percentage(&self) -> f32 {
        if self.finished {
            1.0
        } else if self.services.is_empty() {
            0.0
        } else {
            self.services
                .iter()
                .filter(|(_, &s)| s > ServiceState::Loading)
                .count() as f32
                / self.services.len() as f32
        }
    }
    pub fn has_failed_services(&self) -> bool {
        self.services
            .iter()
            .any(|(_, &s)| s == ServiceState::Failed)
    }
}

pub struct ProgressWatcher {
    progress_rx: channel::Receiver<ProgressData>,
    progress_data: ProgressData,
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ProgressWatcher {
    pub fn new() -> Self {
        let (tx, rx) = channel::unbounded::<ProgressData>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            smol::block_on(async {

                let mut progress_data = ProgressData::default();
                if matches!(std::env::var("TIBS_DEBUG_FAKE_PROGRESS_BAR"), Ok(s) if s == "1") {
                    fake_progress_bar(&tx, &shutdown_clone, &mut progress_data).await;
                    return Ok(());
                }
                let connection = Connection::system().await?;
                let manager = ManagerProxy::new(&connection).await?;
                // Subscribe to job new and job removed signals.
                let mut job_new_stream = manager.receive_job_new().await?;
                let mut job_removed_stream = manager.receive_job_removed().await?;
                let mut system_started_up = manager.receive_startup_finished().await?;
                let default_target_path = manager.get_unit(manager.get_default_target().await?).await?;
                let default_target = UnitProxy::new(&connection, default_target_path).await?;
                if default_target.active_state().await? == "active" {
                    tx.send(ProgressData { finished: true, ..Default::default() }).await.unwrap();
                    return Ok(());
                }
                let jobs = manager.list_jobs().await?;
                progress_data.services = jobs
                    .iter()
                    .filter(|job| job.3 != "done")
                    .map(|job| (job.2.clone(), ServiceState::Loading))
                    .collect();
                let _ = tx.send(progress_data.clone()).await;


                loop {
                    if shutdown_clone.load(Ordering::Relaxed) {
                        break;
                    }
                    futures_util::select! {
                        new_event = job_new_stream.next().fuse() => {
                            if let Some(new_event) = new_event {
                                let Ok(args) = new_event.args() else {
                                    eprintln!("Failed to get JobNew event args");
                                    continue;
                                };
                                if !progress_data.services.contains_key(&args.unit) {
                                    progress_data.services.insert(args.unit, ServiceState::Loading);
                                }
                                if tx.send(progress_data.clone()).await.is_err() {
                                    break;
                                };
                            }
                        },
                        removed_event = job_removed_stream.next().fuse() => {
                            if let Some(removed_event) = removed_event {
                                let Ok(args) = removed_event.args() else {
                                    eprintln!("Failed to get JobRemoved event args");
                                    continue;
                                };

                                progress_data.services.insert(args.unit, match args.result.as_str() {
                                    "done" | "dependency" | "skipped" => ServiceState::Loaded,
                                    "canceled" | "timeout" | "failed" => ServiceState::Failed,
                                    _ => panic!("WOT??")
                                });
                                if tx.send(progress_data.clone()).await.is_err() {
                                    break;
                                }
                            }
                        },
                        system_started_up_event = system_started_up.next().fuse() => {
                            if system_started_up_event.is_some() {
                                progress_data.finished = true;
                                if tx.send(progress_data.clone()).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
                zbus::Result::Ok(())
            }).unwrap();
        });

        ProgressWatcher {
            progress_rx: rx,
            progress_data: ProgressData::default(),
            shutdown,
            handle: Some(handle),
        }
    }
    pub fn poll_progress(&mut self) -> &ProgressData {
        if let Some(new_progress) = self.progress_rx.try_recv().ok() {
            self.progress_data = new_progress;
        }
        &self.progress_data
    }
}

async fn fake_progress_bar(
    tx: &channel::Sender<ProgressData>,
    shutdown_clone: &Arc<AtomicBool>,
    progress_data: &mut ProgressData,
) {
    let service_names = (0..20)
        .map(|i| format!("fake{i}.service"))
        .collect::<Vec<String>>();

    if progress_data.services.is_empty() {
        progress_data.services = service_names
            .into_iter()
            .map(|s| (s.to_string(), ServiceState::Loading))
            .collect();
    }
    let simulate_failure = std::env::var("TIBS_SIMULATE_BOOT_FAILURE").is_ok_and(|s| s == "1");
    let mut rng = rand::rng();

    while !shutdown_clone.load(Ordering::Relaxed) && !progress_data.finished {
        for (_, state) in progress_data.services.iter_mut() {
            if *state == ServiceState::Loading {
                let chance: u8 = rng.random_range(0..100);
                if simulate_failure {
                    if chance < 10 {
                        *state = ServiceState::Failed;
                    } else if chance < 50 {
                        *state = ServiceState::Loaded;
                    }
                } else if chance < 50 {
                    *state = ServiceState::Loaded;
                }
            }
        }
        if progress_data
            .services
            .values()
            .all(|s| *s != ServiceState::Loading)
        {
            progress_data.finished = true;
        }
        if tx.send(progress_data.clone()).await.is_err() {
            break;
        }
        smol::Timer::after(Duration::from_secs(1)).await;
    }
}

impl Drop for ProgressWatcher {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
