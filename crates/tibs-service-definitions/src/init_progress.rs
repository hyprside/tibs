use smol::channel;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InitProgress {
    pub loaded_services: usize,
    pub failed_services: usize,
    pub pending_services: usize,
    pub finished: bool,
}

impl InitProgress {
    pub fn total_services(&self) -> usize {
        self.loaded_services + self.failed_services + self.pending_services
    }

    pub fn percentage(&self) -> f32 {
        if self.finished {
            1.0
        } else {
            let total = self.total_services();
            if total == 0 {
                0.0
            } else {
                (self.loaded_services + self.failed_services) as f32 / total as f32
            }
        }
    }

    pub fn has_failed_services(&self) -> bool {
        self.failed_services > 0
    }
}

pub trait SystemInitProgressService {
    fn watch_progress(&self) -> channel::Receiver<InitProgress>;
}
