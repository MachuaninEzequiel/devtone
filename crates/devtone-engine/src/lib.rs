mod drum;
mod spectrum;
mod synth;

pub use spectrum::{compute_spectrum, dummy_spectrum, peak_hold};
pub use synth::Engine;
