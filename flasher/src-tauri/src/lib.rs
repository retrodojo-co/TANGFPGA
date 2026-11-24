#[cfg(feature = "desktop")]
pub mod commands;
pub mod firmware;
pub mod hardware;
pub mod platform;
pub mod storage;
pub mod utils;
#[cfg(feature = "desktop")]
pub fn run() {
    commands::run();
}
