use std::collections::HashMap;

use futures_util::{FutureExt as _, StreamExt};
use tibs_service_definitions::{InitProgress, SystemInitProgressService};
use zbus_systemd::systemd1::{ManagerProxy, UnitProxy};
use zbus_systemd::zbus::{self, Connection};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum ServiceState {
    Loading,
    Failed,
    Loaded,
}

#[derive(Clone, Debug, Default)]
struct LinuxProgressData {
    services: HashMap<String, ServiceState>,
    finished: bool,
}

impl LinuxProgressData {
    fn to_init_progress(&self) -> InitProgress {
        let mut init_progress = InitProgress {
            finished: self.finished,
            ..Default::default()
        };

        for state in self.services.values() {
            match state {
                ServiceState::Loading => init_progress.pending_services += 1,
                ServiceState::Failed => init_progress.failed_services += 1,
                ServiceState::Loaded => init_progress.loaded_services += 1,
            }
        }

        init_progress
    }
}

pub struct LinuxSystemInitProgressService;

impl LinuxSystemInitProgressService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LinuxSystemInitProgressService {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemInitProgressService for LinuxSystemInitProgressService {
    fn watch_progress(&self) -> smol::channel::Receiver<InitProgress> {
        let (tx, rx) = smol::channel::unbounded();

        std::thread::spawn(move || {
            smol::block_on(async move {
                let mut progress_data = LinuxProgressData::default();
                let _ = tx.send(progress_data.to_init_progress()).await;

                let connection = Connection::system().await?;
                let manager = ManagerProxy::new(&connection).await?;
                let mut job_new_stream = manager.receive_job_new().await?;
                let mut job_removed_stream = manager.receive_job_removed().await?;
                let mut system_started_up = manager.receive_startup_finished().await?;
                let default_target_path = manager
                    .get_unit(manager.get_default_target().await?)
                    .await?;
                let default_target = UnitProxy::new(&connection, default_target_path).await?;

                if default_target.active_state().await? == "active" {
                    progress_data.finished = true;
                    let _ = tx.send(progress_data.to_init_progress()).await;
                    return Ok(());
                }

                let jobs = manager.list_jobs().await?;
                progress_data.services = jobs
                    .iter()
                    .filter(|job| job.3 != "done")
                    .map(|job| (job.2.clone(), ServiceState::Loading))
                    .collect();

                if tx.send(progress_data.to_init_progress()).await.is_err() {
                    return Ok(());
                }

                loop {
                    futures_util::select! {
                        new_event = job_new_stream.next().fuse() => {
                            let Some(new_event) = new_event else {
                                break;
                            };
                            let Ok(args) = new_event.args() else {
                                eprintln!("Failed to get JobNew event args");
                                continue;
                            };
                            progress_data
                                .services
                                .entry(args.unit)
                                .or_insert(ServiceState::Loading);
                            if tx.send(progress_data.to_init_progress()).await.is_err() {
                                break;
                            }
                        },
                        removed_event = job_removed_stream.next().fuse() => {
                            let Some(removed_event) = removed_event else {
                                break;
                            };
                            let Ok(args) = removed_event.args() else {
                                eprintln!("Failed to get JobRemoved event args");
                                continue;
                            };

                            progress_data.services.insert(args.unit, match args.result.as_str() {
                                "done" | "dependency" | "skipped" => ServiceState::Loaded,
                                "canceled" | "timeout" | "failed" => ServiceState::Failed,
                                _ => ServiceState::Failed,
                            });
                            if tx.send(progress_data.to_init_progress()).await.is_err() {
                                break;
                            }
                        },
                        system_started_up_event = system_started_up.next().fuse() => {
                            if system_started_up_event.is_some() {
                                progress_data.finished = true;
                                if tx.send(progress_data.to_init_progress()).await.is_err() {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                    }
                }

                zbus::Result::Ok(())
            })
            .unwrap_or_else(|error| eprintln!("Failed to watch systemd progress: {error:#?}"));
        });

        rx
    }
}
