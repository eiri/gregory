pub mod dsp;
pub mod midi;
pub mod patch_manager;
pub mod ui;

pub use dsp::{Engine, Patch, midi_note_to_freq};
pub use dsp::{FilterMode, Waveform};
