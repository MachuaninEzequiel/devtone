mod draw;
#[cfg(feature = "window")]
mod window;

pub use draw::{draw_notch, hit_bars, hit_quit, NOTCH_H, NOTCH_W};
#[cfg(feature = "window")]
pub use window::run_notch;
