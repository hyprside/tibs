use smol::channel;
use tibs_service_definitions::{InitProgress, SystemInitProgressService};

pub struct ProgressWatcher {
    progress_rx: channel::Receiver<InitProgress>,
    progress_data: InitProgress,
}

impl ProgressWatcher {
    pub fn new(progress_service: &dyn SystemInitProgressService) -> Self {
        log::info!("Creating progress watcher");
        Self {
            progress_rx: progress_service.watch_progress(),
            progress_data: InitProgress::default(),
        }
    }

    pub fn poll_progress(&mut self) -> &InitProgress {
        while let Ok(new_progress) = self.progress_rx.try_recv() {
            log::debug!(
                "Boot progress updated: loaded={}, failed={}, pending={}, finished={}",
                new_progress.loaded_services,
                new_progress.failed_services,
                new_progress.pending_services,
                new_progress.finished
            );
            self.progress_data = new_progress;
        }
        &self.progress_data
    }
}
