//! Non-secret helper progress, independent of transport and terminal rendering.
#[derive(Default, Debug)]
pub struct Counts {
    pub admitted: u32,
    pub authorized: bool,
    pub fetched: u32,
    pub failed: u32,
    monitor: Option<tokio::sync::watch::Sender<Progress>>,
}
#[derive(Clone, Copy, Default, Debug)]
pub struct Progress {
    pub admitted: u32,
    pub authorized: bool,
    pub fetched: u32,
    pub failed: u32,
}
impl Counts {
    pub fn monitored() -> (Self, tokio::sync::watch::Receiver<Progress>) {
        let (sender, receiver) = tokio::sync::watch::channel(Progress::default());
        (
            Self {
                monitor: Some(sender),
                ..Self::default()
            },
            receiver,
        )
    }
    pub(crate) fn publish(&self) {
        if let Some(sender) = &self.monitor {
            sender.send_replace(Progress {
                admitted: self.admitted,
                authorized: self.authorized,
                fetched: self.fetched,
                failed: self.failed,
            });
        }
    }
}
