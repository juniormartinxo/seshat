pub mod cli;
pub mod config;
pub mod core;
pub mod flow;
pub mod git;
pub mod json_output;
pub mod providers;
pub mod review;
pub mod tooling;
pub mod ui;
pub mod utils;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
pub(crate) mod test_env {
    use std::sync::Mutex;

    pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());
}
