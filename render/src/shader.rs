use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use notify::Watcher;
pub use notify::{DebouncedEvent, RecursiveMode};

#[macro_export]
macro_rules! shader_path {
    ($shader:literal) => {
        concat!(env!("OUT_DIR"), "/shaders/", $shader)
    };
}

#[macro_export]
macro_rules! watch_crate_shaders {
    ($watcher:expr) => {
        let crate_shader_dir = concat!(env!("OUT_DIR"), "/shaders/");
        $watcher.watch(crate_shader_dir, crate::shader::RecursiveMode::Recursive);
    };
}

pub struct ShaderWatcher {
    receiver: Receiver<notify::DebouncedEvent>,
    watcher: notify::RecommendedWatcher,
}

impl ShaderWatcher {
    pub fn new() -> Self {
        // Create a channel to receive the events.
        let (sender, receiver) = channel();

        // Create a watcher object, delivering debounced events.
        // The notification back-end is selected based on the platform.
        let watcher = notify::watcher(sender, Duration::from_millis(16)).unwrap();

        Self { receiver, watcher }
    }

    pub fn watch<P: AsRef<std::path::Path>>(&mut self, path: P, recursive_mode: RecursiveMode) {
        self.watcher.watch(path, recursive_mode).unwrap();
    }

    pub fn unwatch<P: AsRef<std::path::Path>>(&mut self, path: P) {
        self.watcher.unwatch(path).unwrap();
    }

    pub fn update<T>(&mut self, cb: impl Fn(DebouncedEvent) -> Option<T>) -> Option<T> {
        if let Ok(event) = self.receiver.try_recv() {
            cb(event)
        } else {
            None
        }
    }
}

impl Default for ShaderWatcher {
    fn default() -> Self {
        Self::new()
    }
}
