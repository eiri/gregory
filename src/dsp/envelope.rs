#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// An ADSR envelope generator.
///
/// Rather than stepping through a linear ramp, each stage uses an
/// exponential approach.
///
/// Call gate_on() to start the attack phase from wherever the envelope
/// currently is, call gate_off() to enter release from the current level.
pub struct Envelope {
    // All times in seconds, sustain is level 0.0–1.0
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,

    sample_rate: f64,
    stage: EnvelopeStage,
    level: f64,

    // Per-stage exponential coefficients, recomputed when params change.
    attack_coeff: f64,
    decay_coeff: f64,
    release_coeff: f64,

    dirty: bool,
}

impl Envelope {
    pub fn new(attack: f64, decay: f64, sustain: f64, release: f64, sample_rate: f64) -> Self {
        let mut env = Self {
            attack,
            decay,
            sustain: sustain.clamp(0.0, 1.0), // this is a level 0.0–1.0
            release,
            sample_rate,
            stage: EnvelopeStage::Idle,
            level: 0.0,
            attack_coeff: 0.0,
            decay_coeff: 0.0,
            release_coeff: 0.0,
            dirty: true,
        };
        env.update_coefficients();
        env
    }

    /// Begin attack from the current level.
    /// Starting from the current level rather than 0.0 to avoid clicks on retrig.
    pub fn gate_on(&mut self) {
        if self.dirty {
            self.update_coefficients();
        }
        self.stage = EnvelopeStage::Attack;
    }

    /// Begin release from the current level.
    pub fn gate_off(&mut self) {
        if self.stage != EnvelopeStage::Idle {
            self.stage = EnvelopeStage::Release;
        }
    }

    pub fn stage(&self) -> EnvelopeStage {
        self.stage
    }

    pub fn level(&self) -> f64 {
        self.level
    }

    pub fn set_attack(&mut self, secs: f64) {
        self.attack = secs.max(0.001);
        self.dirty = true;
    }

    pub fn set_decay(&mut self, secs: f64) {
        self.decay = secs.max(0.001);
        self.dirty = true;
    }

    pub fn set_sustain(&mut self, level: f64) {
        self.sustain = level.clamp(0.0, 1.0);
    }

    pub fn set_release(&mut self, secs: f64) {
        self.release = secs.max(0.001);
        self.dirty = true;
    }

    pub fn next_sample(&mut self) -> f64 {
        if self.dirty {
            self.update_coefficients();
        }

        match self.stage {
            EnvelopeStage::Idle => {
                self.level = 0.0;
            }

            EnvelopeStage::Attack => {
                // Approach attack_target (1.1) exponentially.
                self.level = 1.1 + (self.level - 1.1) * self.attack_coeff;

                // Transition to decay once we've crossed 1.0.
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = EnvelopeStage::Decay;
                }
            }

            EnvelopeStage::Decay => {
                // Approach sustain level exponentially.
                self.level = self.sustain + (self.level - self.sustain) * self.decay_coeff;

                // Transition to sustain when close enough.
                if (self.level - self.sustain).abs() < 1e-3 {
                    self.level = self.sustain;
                    self.stage = EnvelopeStage::Sustain;
                }
            }

            // Sustain sustains ;)
            EnvelopeStage::Sustain => {
                self.level = self.sustain;
            }

            EnvelopeStage::Release => {
                // Approach release_target (-0.1) exponentially.
                self.level = -0.1 + (self.level + 0.1) * self.release_coeff;

                // Transition to idle once close enough to silence.
                if self.level <= 1e-3 {
                    self.level = 0.0;
                    self.stage = EnvelopeStage::Idle;
                }
            }
        }

        self.level
    }

    /// Compute exponential coefficients for each stage.
    fn update_coefficients(&mut self) {
        self.attack_coeff = exp_coeff(self.attack, self.sample_rate);
        self.decay_coeff = exp_coeff(self.decay, self.sample_rate);
        self.release_coeff = exp_coeff(self.release, self.sample_rate);
        self.dirty = false;
    }
}

/// Compute the per-sample exponential decay coefficient for a given time.
/// The coefficient c = exp(ln(threshold) / (t * sample_rate)).
/// threshold of 0.001 means the curve reaches within 0.1% of the target
/// in time_secs. Should give about -60 dB decay.
fn exp_coeff(time_secs: f64, sample_rate: f64) -> f64 {
    let time_secs = time_secs.max(0.001); // time_secs should be >= 0
    (-8.0_f64.ln() / (time_secs * sample_rate)).exp() // -ln(8) ≈ -2.08 is an empirical magical number
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 44100.0;

    fn make_env() -> Envelope {
        Envelope::new(0.01, 0.1, 0.7, 0.3, SAMPLE_RATE)
    }

    #[test]
    fn test_idle_is_silent() {
        let mut env = make_env();
        for _ in 0..1000 {
            assert_eq!(env.next_sample(), 0.0);
        }
    }

    #[test]
    fn test_output_bounds() {
        let mut env = make_env();
        env.gate_on();
        // Hold for 0.5s then release.
        for i in 0..(SAMPLE_RATE as usize) {
            if i == (SAMPLE_RATE * 0.5) as usize {
                env.gate_off();
            }
            let s = env.next_sample();
            assert!(
                (0.0..=1.0).contains(&s),
                "Envelope out of bounds at sample {i}: {s}"
            );
        }
    }

    #[test]
    fn test_attack_reaches_peak() {
        let attack_secs = 0.05;
        let mut env = Envelope::new(attack_secs, 0.1, 0.7, 0.3, SAMPLE_RATE);
        env.gate_on();
        let attack_samples = (SAMPLE_RATE * attack_secs * 1.5) as usize;
        let mut peak = 0.0f64;
        for _ in 0..attack_samples {
            peak = peak.max(env.next_sample());
        }
        assert!(peak >= 0.99, "Attack did not reach peak: {peak}");
    }

    #[test]
    fn test_decay_settles_at_sustain() {
        let sustain = 0.6;
        let mut env = Envelope::new(0.001, 0.05, sustain, 0.3, SAMPLE_RATE);
        env.gate_on();
        // Run long enough for attack + decay to complete.
        let run = (SAMPLE_RATE * 0.5) as usize;
        let mut last = 0.0;
        for _ in 0..run {
            last = env.next_sample();
        }
        assert!(
            (last - sustain).abs() < 0.001,
            "Sustain level wrong: got {last}, expected {sustain}"
        );
    }

    #[test]
    fn test_release_reaches_silence() {
        let mut env = Envelope::new(0.001, 0.001, 0.7, 0.1, SAMPLE_RATE);
        env.gate_on();
        // Get to sustain quickly.
        for _ in 0..(SAMPLE_RATE * 0.05) as usize {
            env.next_sample();
        }
        env.gate_off();
        // Run for release + 50% margin.
        let release_samples = (SAMPLE_RATE * 0.15) as usize;
        let mut last = 1.0;
        for _ in 0..release_samples {
            last = env.next_sample();
        }
        assert!(last < 0.001, "Release did not reach silence: {last}");
    }

    #[test]
    fn test_legato_retrigger_no_click() {
        let mut env = Envelope::new(0.1, 0.1, 0.7, 0.3, SAMPLE_RATE);
        env.gate_on();
        // Get to sustain.
        for _ in 0..(SAMPLE_RATE * 0.5) as usize {
            env.next_sample();
        }
        env.gate_off();
        // Release for 50ms and level should be noticeably above 0.
        for _ in 0..(SAMPLE_RATE * 0.05) as usize {
            env.next_sample();
        }
        let level_at_retrigger = env.level();
        // Retrigger. Attack should start from current level, not from 0.
        env.gate_on();
        let first_sample_after_retrigger = env.next_sample();
        assert!(
            (first_sample_after_retrigger - level_at_retrigger).abs() < 0.05,
            "Retrigger caused level jump: was {level_at_retrigger}, jumped to {first_sample_after_retrigger}"
        );
    }

    #[test]
    fn test_stage_sequence() {
        let mut env = Envelope::new(0.001, 0.001, 0.5, 0.001, SAMPLE_RATE);
        assert_eq!(env.stage(), EnvelopeStage::Idle);
        env.gate_on();
        assert_eq!(env.stage(), EnvelopeStage::Attack);
        // Run past attack + decay — use generous margin (0.5s for 1ms stages).
        for _ in 0..(SAMPLE_RATE * 0.5) as usize {
            env.next_sample();
        }
        assert_eq!(env.stage(), EnvelopeStage::Sustain);
        env.gate_off();
        assert_eq!(env.stage(), EnvelopeStage::Release);
        // Run past release.
        for _ in 0..(SAMPLE_RATE * 0.5) as usize {
            env.next_sample();
        }
        assert_eq!(env.stage(), EnvelopeStage::Idle);
    }
}
