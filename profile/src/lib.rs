#[cfg(feature = "optick")]
pub use optick;

#[cfg(feature = "optick")]
pub fn init() {}

#[cfg(feature = "optick")]
pub fn next_frame() {
    optick::next_frame();
}

#[cfg(feature = "optick")]
#[macro_export]
macro_rules! scope {
    ($name:expr) => {
        optick::event!($name);
    };
}

#[cfg(feature = "tracy")]
pub use tracy_client;

#[cfg(feature = "tracy")]
pub fn init() {
    tracy_client::Client::start();
}

#[cfg(feature = "tracy")]
pub fn next_frame() {
    tracy_client::Client::running().unwrap().frame_mark();
}

#[cfg(feature = "tracy")]
#[macro_export]
macro_rules! scope {
    ($name:expr) => {
        let _span = $crate::tracy_client::span!($name);
    };
}

#[cfg(not(any(feature = "optick", feature = "tracy",)))]
pub fn init() {}

#[cfg(not(any(feature = "optick", feature = "tracy",)))]
pub fn next_frame() {}

#[cfg(not(any(feature = "optick", feature = "tracy",)))]
#[macro_export]
macro_rules! scope {
    ($name:expr) => {};
    ($name:expr, $data:expr) => {};
}
