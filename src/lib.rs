use notify::{Event, ReadDirectoryChangesWatcher};

pub mod middleware;
pub mod ws;

pub struct WatchingData {
    pub watcher : ReadDirectoryChangesWatcher,
    pub rx : tokio::sync::broadcast::Receiver<Event>
}