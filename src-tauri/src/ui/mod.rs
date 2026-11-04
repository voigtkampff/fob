//! User interface functionality
//!
//! This module handles UI-specific functionality, including window effects
//! and platform-specific visual customizations.

mod window;
#[cfg(target_os = "windows")]
pub use window::apply_mini_shape;
pub use window::{setup_mini_glass, setup_window_effects};
