use std::collections::HashMap;

use futures_util::{FutureExt as _, StreamExt};
use tibs_service_definitions::{InitProgress, SystemInitProgressService};
use zbus_systemd::systemd1::{self, UnitProxy};
use zbus_systemd::zbus;

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
        self.services.values().fold(
            InitProgress {
                finished: self.finished,
                ..Default::default()
            },
            |init, state| InitProgress {
                pending_services: if matches!(state, ServiceState::Loading) {
                    init.pending_services + 1
                } else {
                    init.pending_services
                },
                failed_services: if matches!(state, ServiceState::Failed) {
                    init.failed_services + 1
                } else {
                    init.pending_services
                },
                loaded_services: if matches!(state, ServiceState::Loaded) {
                    init.loaded_services + 1
                } else {
                    init.loaded_services
                },
                ..init
            },
        )
    }
}

pub struct LinuxSystemInitProgressService {
    systemd: systemd1::ManagerProxy<'static>,
    dbus: zbus::Connection,
}

impl LinuxSystemInitProgressService {
    pub fn new(dbus: &zbus::Connection, systemd: &systemd1::ManagerProxy<'static>) -> Self {
        (
            Self {
                systemd: systemd.clone(),
                dbus: dbus.clone(),
            },
            log::info!("Initialized linux implementation of SystemInitProgressService"),
        )
            .0
    }
}

impl SystemInitProgressService for LinuxSystemInitProgressService {
    fn watch_progress(&self) -> smol::channel::Receiver<InitProgress> {
        let (tx, rx) = smol::channel::unbounded();
        log::info!("Starting systemd init progress watcher");
        let dbus = self.dbus.clone();
        let systemd = self.systemd.clone();
        std::thread::spawn(move || {
            smol::block_on(async move {
                let mut progress_data = LinuxProgressData::default();
                let _ = tx.send(progress_data.to_init_progress()).await;

                log::info!("Connecting to system bus for systemd progress");
                let mut job_new_stream = systemd.receive_job_new().await?;
                let mut job_removed_stream = systemd.receive_job_removed().await?;
                let mut system_started_up = systemd.receive_startup_finished().await?;
                let default_target_path = systemd
                    .get_unit(systemd.get_default_target().await?)
                    .await?;
                let default_target = UnitProxy::new(&dbus, default_target_path).await?;

                if default_target.active_state().await? == "active" {
                    progress_data.finished = true;
                    log::info!("systemd default target is already active");
                    let _ = tx.send(progress_data.to_init_progress()).await;
                    return Ok(());
                }

                let jobs = systemd.list_jobs().await?;
                log::info!("Loaded {} initial systemd jobs", jobs.len());
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
                                log::warn!("Failed to get JobNew event args");
                                continue;
                            };
                            progress_data
                                .services
                                .entry(args.unit)
                                .or_insert(ServiceState::Loading);
                            log::debug!(
                                "systemd job started; tracked pending services: {}",
                                progress_data.to_init_progress().pending_services
                            );
                            if tx.send(progress_data.to_init_progress()).await.is_err() {
                                break;
                            }
                        },
                        removed_event = job_removed_stream.next().fuse() => {
                            let Some(removed_event) = removed_event else {
                                break;
                            };
                            let Ok(args) = removed_event.args() else {
                                log::warn!("Failed to get JobRemoved event args");
                                continue;
                            };

                            progress_data.services.insert(args.unit, match args.result.as_str() {
                                "done" | "dependency" | "skipped" => ServiceState::Loaded,
                                "canceled" | "timeout" | "failed" => ServiceState::Failed,
                                _ => ServiceState::Failed,
                            });
                            let progress = progress_data.to_init_progress();
                            log::debug!(
                                "systemd job removed with result {}; loaded={}, failed={}, pending={}",
                                args.result,
                                progress.loaded_services,
                                progress.failed_services,
                                progress.pending_services
                            );
                            if tx.send(progress_data.to_init_progress()).await.is_err() {
                                break;
                            }
                        },
                        system_started_up_event = system_started_up.next().fuse() => {
                            if system_started_up_event.is_some() {
                                progress_data.finished = true;
                                log::info!("systemd startup finished");
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
            .unwrap_or_else(|error| log::error!("Failed to watch systemd progress: {error:#?}"));
        });

        rx
    }
}
