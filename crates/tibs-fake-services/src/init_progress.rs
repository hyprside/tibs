use rand::Rng;
use std::{collections::HashMap, time::Duration};
use tibs_service_definitions::{InitProgress, SystemInitProgressService};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FakeServiceState {
    Loading,
    Failed,
    Loaded,
}

#[derive(Clone, Debug)]
pub struct FakeSystemInitProgressService {
    service_count: usize,
    tick: Duration,
    simulate_failure: bool,
}

impl FakeSystemInitProgressService {
    pub fn new() -> Self {
        Self {
            service_count: 20,
            tick: Duration::from_secs(1),
            simulate_failure: false,
        }
    }

    pub fn with_failures(mut self, simulate_failure: bool) -> Self {
        self.simulate_failure = simulate_failure;
        self
    }

    pub fn from_environment() -> Self {
        Self::new().with_failures(matches!(
            std::env::var("TIBS_SIMULATE_BOOT_FAILURE"),
            Ok(value) if value == "1"
        ))
    }
}

impl Default for FakeSystemInitProgressService {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemInitProgressService for FakeSystemInitProgressService {
    fn watch_progress(&self) -> smol::channel::Receiver<InitProgress> {
        let (tx, rx) = smol::channel::unbounded();
        let service_count = self.service_count;
        let tick = self.tick;
        let simulate_failure = self.simulate_failure;
        log::info!(
            "Starting fake init progress service; service_count={service_count}, simulate_failure={simulate_failure}"
        );

        std::thread::spawn(move || {
            smol::block_on(async move {
                let mut services = (0..service_count)
                    .map(|i| (format!("fake{i}.service"), FakeServiceState::Loading))
                    .collect::<HashMap<_, _>>();
                let _ = tx.send(to_init_progress(&services, false)).await;

                let mut rng = rand::rng();
                loop {
                    for state in services.values_mut() {
                        if *state == FakeServiceState::Loading {
                            let chance: u8 = rng.random_range(0..100);
                            if simulate_failure {
                                if chance < 10 {
                                    *state = FakeServiceState::Failed;
                                } else if chance < 50 {
                                    *state = FakeServiceState::Loaded;
                                }
                            } else if chance < 50 {
                                *state = FakeServiceState::Loaded;
                            }
                        }
                    }

                    let finished = services
                        .values()
                        .all(|state| *state != FakeServiceState::Loading);
                    let progress = to_init_progress(&services, finished);
                    log::debug!(
                        "Fake init progress tick; loaded={}, failed={}, pending={}, finished={}",
                        progress.loaded_services,
                        progress.failed_services,
                        progress.pending_services,
                        progress.finished
                    );
                    if tx.send(progress).await.is_err() || finished {
                        break;
                    }
                    smol::Timer::after(tick).await;
                }
            });
        });

        rx
    }
}

fn to_init_progress(services: &HashMap<String, FakeServiceState>, finished: bool) -> InitProgress {
    let mut progress = InitProgress {
        finished,
        ..Default::default()
    };

    for state in services.values() {
        match state {
            FakeServiceState::Loading => progress.pending_services += 1,
            FakeServiceState::Failed => progress.failed_services += 1,
            FakeServiceState::Loaded => progress.loaded_services += 1,
        }
    }

    progress
}
