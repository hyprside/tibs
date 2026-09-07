use smol::channel;
use tibs_service_definitions::{InitProgress, SystemInitProgressService};

pub struct ProgressWatcher {
    progress_rx: channel::Receiver<InitProgress>,
    progress_data: InitProgress,
}

impl ProgressWatcher {
    pub fn new(progress_service: &dyn SystemInitProgressService) -> Self {
        Self {
            progress_rx: progress_service.watch_progress(),
            progress_data: InitProgress::default(),
        }
    }

    pub fn poll_progress(&mut self) -> &InitProgress {
        while let Ok(new_progress) = self.progress_rx.try_recv() {
            self.progress_data = new_progress;
        }
        &self.progress_data
    }
}
