use serde::{Deserialize, Serialize};

use crate::dsp::{Envelope, EnvelopeStage, Filter, FilterMode, Oscillator, Waveform};

/// Convert a MIDI note number to frequency in Hz.
/// A4 = MIDI 69 = 440 Hz.
pub fn midi_note_to_freq(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((note as f64 - 69.0) / 12.0)
}

/// All user-facing parameters in one flat struct.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Patch {
    // Oscillator
    pub waveform: Waveform,
    pub pulse_width: f64, // only relevant for Square, 0.0–1.0

    // Unison
    pub unison: bool,
    pub unison_detune: f64, // detune amount in cents, 0.0–50.0

    // Portamento
    pub portamento: bool,
    pub portamento_time: f64,

    // Filter
    pub filter_mode: FilterMode,
    pub filter_cutoff: f64,    // base cutoff Hz
    pub filter_resonance: f64, // 0.0–1.0

    // Amplitude envelope
    pub amp_attack: f64,
    pub amp_decay: f64,
    pub amp_sustain: f64,
    pub amp_release: f64,

    // Filter envelope
    pub flt_attack: f64,
    pub flt_decay: f64,
    pub flt_sustain: f64,
    pub flt_release: f64,
    /// How far the filter envelope opens the cutoff above the base, in Hz.
    pub flt_env_amount: f64,
    /// Master output gain, 0.0–1.0.
    pub gain: f64,
}

impl Default for Patch {
    fn default() -> Self {
        Self {
            waveform: Waveform::Sawtooth,
            pulse_width: 0.5,
            unison: false,
            unison_detune: 0.0,
            portamento: false,
            portamento_time: 0.1,
            filter_mode: FilterMode::LowPass,
            filter_cutoff: 10.0,
            filter_resonance: 0.3,
            amp_attack: 0.005,
            amp_decay: 0.15,
            amp_sustain: 0.7,
            amp_release: 0.3,
            flt_attack: 0.005,
            flt_decay: 0.25,
            flt_sustain: 0.1,
            flt_release: 0.3,
            flt_env_amount: 4000.0,
            gain: 0.5,
        }
    }
}

impl Patch {
    pub fn random() -> Self {
        let waveform = if fastrand::bool() {
            Waveform::Sawtooth
        } else {
            Waveform::Square
        };
        let pulse_width = if fastrand::bool() { 0.25 } else { 0.5 };
        let filter_mode = if fastrand::f64() < 0.7 {
            FilterMode::LowPass
        } else {
            FilterMode::LowPass2Pole
        };

        let r = |min: f64, max: f64| min + fastrand::f64() * (max - min);

        Self {
            waveform,
            pulse_width,
            unison: fastrand::bool(),
            unison_detune: if fastrand::bool() { r(5.0, 30.0) } else { 0.0 },
            portamento: fastrand::bool(),
            portamento_time: r(0.05, 0.5),
            filter_mode,
            filter_cutoff: r(200.0, 8000.0),
            filter_resonance: r(0.0, 0.7),
            flt_env_amount: r(0.0, 6000.0),
            amp_attack: r(0.001, 0.5),
            amp_decay: r(0.05, 1.0),
            amp_sustain: r(0.3, 1.0),
            amp_release: r(0.05, 2.0),
            flt_attack: r(0.001, 1.0),
            flt_decay: r(0.05, 2.0),
            flt_sustain: r(0.0, 0.8),
            flt_release: r(0.05, 2.0),
            gain: r(0.3, 0.7),
        }
    }
}

/// Monophonic DSP engine.
///
/// Owns one oscillator, one filter, and two envelopes (amplitude + filter).
/// It is intentionally not `Send` - it lives entirely on the audio
/// thread.
pub struct Engine {
    pub patch: Patch,

    osc: Oscillator,
    osc2: Oscillator, // detuned up
    osc3: Oscillator, // detuned down
    filter: Filter,
    amp_env: Envelope,
    flt_env: Envelope,

    sample_rate: f64,

    /// The MIDI note currently sounding, if any.
    current_note: Option<u8>,
    pitch_bend_semitones: f64,
    mod_wheel: f64,
    current_freq: f64,
    target_freq: f64,
}

impl Engine {
    pub fn new(sample_rate: f64) -> Self {
        let p = Patch::default();

        let mut osc = Oscillator::new(p.waveform, 440.0, sample_rate);
        osc.pulse_width = p.pulse_width;

        let osc2 = osc.clone();
        let osc3 = osc.clone();

        let mut filter = Filter::new(p.filter_cutoff, p.filter_resonance, sample_rate);
        filter.mode = p.filter_mode;

        let amp_env = Envelope::new(
            p.amp_attack,
            p.amp_decay,
            p.amp_sustain,
            p.amp_release,
            sample_rate,
        );

        let flt_env = Envelope::new(
            p.flt_attack,
            p.flt_decay,
            p.flt_sustain,
            p.flt_release,
            sample_rate,
        );

        Self {
            patch: p,
            osc,
            osc2,
            osc3,
            filter,
            amp_env,
            flt_env,
            sample_rate,
            current_note: None,
            pitch_bend_semitones: 0.0,
            mod_wheel: 0.0,
            current_freq: 440.0,
            target_freq: 440.0,
        }
    }

    /// Start a note. Frequency is derived from the MIDI note number.
    pub fn note_on(&mut self, note: u8, _velocity: u8) {
        self.current_note = Some(note);
        self.target_freq = midi_note_to_freq(note);
        if !self.patch.portamento || self.amp_env.stage() == EnvelopeStage::Idle {
            self.current_freq = self.target_freq;
        }
        self.amp_env.gate_on();
        self.flt_env.gate_on();
    }

    /// Release the current note. If `note` doesn't match the sounding note
    /// we ignore it — handles the case where a new note was struck before
    /// the old one was released (MIDI note-stealing).
    pub fn note_off(&mut self, note: u8) {
        if self.current_note == Some(note) {
            self.current_note = None;
            self.amp_env.gate_off();
            self.flt_env.gate_off();
        }
    }

    pub fn pitch_bend(&mut self, semitones: f64) {
        self.pitch_bend_semitones = semitones;
    }

    pub fn set_mod_wheel(&mut self, value: f64) {
        self.mod_wheel = value.clamp(0.0, 1.0);
        self.patch.filter_cutoff = 10.0 + self.mod_wheel * (18000.0 - 10.0);
    }

    /// Hard-reset oscillator phase to zero. Call before `note_on` for a staccato.
    pub fn reset_phase(&mut self) {
        self.osc.reset_phase();
    }

    /// Returns true if the engine is producing non-silent output
    pub fn is_active(&self) -> bool {
        self.current_note.is_some() || !matches!(self.amp_env.stage(), EnvelopeStage::Idle)
    }

    pub fn set_patch(&mut self, p: Patch) {
        self.osc.waveform = p.waveform;
        self.osc.pulse_width = p.pulse_width;
        self.osc2.waveform = p.waveform;
        self.osc2.pulse_width = p.pulse_width;
        self.osc3.waveform = p.waveform;
        self.osc3.pulse_width = p.pulse_width;

        self.filter.mode = p.filter_mode;
        self.filter.set_cutoff(p.filter_cutoff);
        self.filter.set_resonance(p.filter_resonance);

        self.amp_env.set_attack(p.amp_attack);
        self.amp_env.set_decay(p.amp_decay);
        self.amp_env.set_sustain(p.amp_sustain);
        self.amp_env.set_release(p.amp_release);

        self.flt_env.set_attack(p.flt_attack);
        self.flt_env.set_decay(p.flt_decay);
        self.flt_env.set_sustain(p.flt_sustain);
        self.flt_env.set_release(p.flt_release);

        self.patch = p;
    }

    /// Produce one output sample. Call this once per sample on the audio thread.
    pub fn process(&mut self) -> f64 {
        if let Some(note) = self.current_note {
            let bent_target =
                midi_note_to_freq(note) * 2.0_f64.powf(self.pitch_bend_semitones / 12.0);
            self.target_freq = bent_target;

            if self.patch.portamento {
                let coeff = (-8.0_f64.ln() / (self.patch.portamento_time * self.sample_rate)).exp();
                self.current_freq =
                    self.target_freq + (self.current_freq - self.target_freq) * coeff;
            } else {
                self.current_freq = self.target_freq;
            }

            self.osc.set_frequency(self.current_freq);
            if self.patch.unison {
                let detune = 2.0_f64.powf(self.patch.unison_detune / 1200.0);
                self.osc2.set_frequency(self.current_freq * detune);
                self.osc3.set_frequency(self.current_freq / detune);
            }
        }

        let amp = self.amp_env.next_sample();
        let flt = self.flt_env.next_sample();

        // Map mod wheel [0.0, 1.0] to cutoff range [10, 18000] Hz.
        let base_cutoff = 10.0 + self.mod_wheel * (18000.0 - 10.0);
        let cutoff =
            (base_cutoff + flt * self.patch.flt_env_amount).clamp(10.0, self.sample_rate * 0.49);
        self.filter.set_cutoff(cutoff);

        let raw = if self.patch.unison {
            (self.osc.next_sample() + self.osc2.next_sample() + self.osc3.next_sample()) / 3.0
        } else {
            self.osc.next_sample()
        };
        let filtered = self.filter.process(raw);

        filtered * amp * self.patch.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 44100.0;

    #[test]
    fn test_idle_is_silent() {
        let mut engine = Engine::new(SAMPLE_RATE);
        for _ in 0..1000 {
            assert_eq!(engine.process(), 0.0);
        }
    }

    #[test]
    fn test_output_bounds() {
        let mut engine = Engine::new(SAMPLE_RATE);
        engine.note_on(60, 100); // middle C
        for i in 0..SAMPLE_RATE as usize {
            let s = engine.process();
            assert!(
                (-1.0..=1.0).contains(&s),
                "Engine output out of bounds at sample {i}: {s}"
            );
        }
    }

    #[test]
    fn test_note_off_wrong_note_ignored() {
        let mut engine = Engine::new(SAMPLE_RATE);
        engine.note_on(60, 100);
        // Run into sustain.
        for _ in 0..(SAMPLE_RATE * 0.5) as usize {
            engine.process();
        }
        engine.note_off(61); // wrong note
        assert!(engine.is_active(), "Engine went idle after wrong note_off");
    }

    #[test]
    fn test_becomes_idle_after_release() {
        let mut engine = Engine::new(SAMPLE_RATE);
        engine.note_on(60, 100);
        for _ in 0..(SAMPLE_RATE * 0.5) as usize {
            engine.process();
        }
        engine.note_off(60);
        // Default release is 0.3s — run for 2s to be safe.
        for _ in 0..(SAMPLE_RATE * 2.0) as usize {
            engine.process();
        }
        assert!(
            !engine.is_active(),
            "Engine still active after full release"
        );
    }

    /// Frequency should change correctly for a few MIDI notes.
    #[test]
    fn test_midi_note_to_freq() {
        assert!((midi_note_to_freq(69) - 440.0).abs() < 0.001); // A4
        assert!((midi_note_to_freq(60) - 261.626).abs() < 0.01); // C4
        assert!((midi_note_to_freq(57) - 220.0).abs() < 0.001); // A3
        assert!((midi_note_to_freq(81) - 880.0).abs() < 0.001); // A5
    }

    #[test]
    fn test_set_patch() {
        let mut engine = Engine::new(SAMPLE_RATE);
        let p = Patch {
            waveform: Waveform::Square,
            filter_cutoff: 1200.0,
            filter_resonance: 0.8,
            ..Patch::default()
        };
        engine.set_patch(p);
        engine.note_on(48, 100);
        for _ in 0..1000 {
            engine.process();
        }
    }
}
