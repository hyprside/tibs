use std::{collections::HashSet, sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
}};
use zbus_systemd::systemd1::{ManagerProxy, TargetProxy, UnitProxy};
use zbus_systemd::zbus::{self, Connection};
use std::thread;
use futures_util::{FutureExt as _, StreamExt};
use smol::channel;
#[derive(Clone, Debug, Default)]
pub struct ProgressData {
    pub total: HashSet<String>,
    pub active: HashSet<String>,
    pub failed: HashSet<String>,
    pub finished: bool
}


impl ProgressData {
    pub fn get_percentage(&self) -> f64 {
        if self.total.is_empty() {
            0.0
        } else {
            (((self.active.len() + self.failed.len()) as f64) / self.total.len() as f64) * 100.0
        }
    }
}

pub struct ProgressWatcher {
    progress_rx: channel::Receiver<ProgressData>,
    progress_data: ProgressData,
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl ProgressWatcher {
    pub fn new() -> zbus::Result<Self> {
        let (tx, rx) = channel::unbounded::<ProgressData>();
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            smol::block_on(async {
                let mut progress_data = ProgressData::default();
                let connection = Connection::system().await?;
                let manager = ManagerProxy::new(&connection).await?;
                let default_target_path = manager.get_unit(manager.get_default_target().await?).await?;
                let default_target = UnitProxy::new(&connection, default_target_path).await?;
                if default_target.active_state().await? == "active" {
                    tx.send(ProgressData { finished: true, ..Default::default() }).await.unwrap();
                    return Ok(());
                }
                let jobs = manager.list_jobs().await?;
                progress_data.total = jobs
                    .iter()
                    .filter(|job| job.3 != "done")
                    .map(|job| job.2.clone())
                    .collect();
                let _ = tx.send(progress_data.clone()).await;

                // Subscribe to job new and job removed signals.
                let mut job_new_stream = manager.receive_job_new().await?;
                let mut job_removed_stream = manager.receive_job_removed().await?;
                let mut system_started_up = manager.receive_startup_finished().await?;

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
                                progress_data.total.insert(args.unit);
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
                                progress_data.active.insert(args.unit);
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

        Ok(ProgressWatcher {
            progress_rx: rx,
            progress_data: ProgressData {
                total: HashSet::new(),
                active: HashSet::new(),
                failed: HashSet::new(),
                finished: true
            },
            shutdown,
            handle: Some(handle),
        })
    }
    pub fn poll_progress(&mut self) -> &ProgressData {
        if let Some(new_progress) = self.progress_rx.try_recv().ok() {
            self.progress_data = new_progress;
        }
        &self.progress_data
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
