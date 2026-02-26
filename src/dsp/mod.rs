pub mod envelope;
pub mod filter;
pub mod oscillator;

pub use envelope::{Envelope, EnvelopeStage};
pub use filter::{Filter, FilterMode};
pub use oscillator::{Oscillator, Waveform};
