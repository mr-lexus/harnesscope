pub mod codex;
pub mod copilot;
pub mod discovery;
pub mod opencode;
pub mod wrapper;

pub use wrapper::{execute_wrapper, execute_wrapper_generic, spawn_background_server};
