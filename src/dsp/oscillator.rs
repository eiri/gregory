#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Waveform {
    Sawtooth,
    Square,
}

/// A single band-limited oscillator using the PolyBLEP technique.
pub struct Oscillator {
    pub waveform: Waveform,
    pub frequency: f64,
    sample_rate: f64,
    phase: f64,
    phase_increment: f64,
    pub pulse_width: f64, // Pulse width for square wave, 0.0–1.0. 0.5 = 50% duty cycle.
}

impl Oscillator {
    pub fn new(waveform: Waveform, frequency: f64, sample_rate: f64) -> Self {
        let phase_increment = frequency / sample_rate;
        Self {
            waveform,
            frequency,
            sample_rate,
            phase: 0.0,
            phase_increment,
            pulse_width: 0.5,
        }
    }

    pub fn set_frequency(&mut self, frequency: f64) {
        self.frequency = frequency;
        self.phase_increment = frequency / self.sample_rate;
    }

    /// Produce the next sample and advance the phase.
    pub fn next_sample(&mut self) -> f64 {
        let sample = match self.waveform {
            Waveform::Sawtooth => self.saw_sample(),
            Waveform::Square => self.square_sample(),
        };

        // Advance phase, wrapping to [0.0, 1.0).
        self.phase += self.phase_increment;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        sample
    }

    /// Naive sawtooth in [-1, 1] with PolyBLEP correction at the wrap point.
    fn saw_sample(&self) -> f64 {
        let naive = 2.0 * self.phase - 1.0;
        naive - poly_blep(self.phase, self.phase_increment)
    }

    /// Naive square in [-1, 1] with PolyBLEP correction at both transitions.
    fn square_sample(&self) -> f64 {
        let pw = self.pulse_width.clamp(0.01, 0.99);
        let naive = if self.phase < pw { 1.0 } else { -1.0 };

        // PolyBLEP at the rising edge (phase == 0).
        let correction_rise = poly_blep(self.phase, self.phase_increment);
        // PolyBLEP at the falling edge (phase == pulse_width).
        // We shift phase by -pw to reuse the same poly_blep function.
        let shifted_phase = (self.phase - pw).rem_euclid(1.0);
        let correction_fall = poly_blep(shifted_phase, self.phase_increment);

        naive + correction_rise - correction_fall
    }
}

fn poly_blep(t: f64, dt: f64) -> f64 {
    // Polynomial: 2t - t² - 1
    if t < dt {
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 44100.0;
    const FREQUENCY: f64 = 440.0;

    #[test]
    fn test_set_frequency() {
        let mut osc = Oscillator::new(Waveform::Sawtooth, 440.0, SAMPLE_RATE);
        osc.set_frequency(880.0);
        assert_eq!(osc.phase_increment, 880.0 / SAMPLE_RATE);
    }

    #[test]
    fn test_saw_amplitude_bounds() {
        let mut osc = Oscillator::new(Waveform::Sawtooth, FREQUENCY, SAMPLE_RATE);
        for _ in 0..SAMPLE_RATE as usize {
            let s = osc.next_sample();
            assert!(
                (-1.0..=1.0).contains(&s),
                "Sawtooth sample out of bounds: {s}"
            );
        }
    }

    #[test]
    fn test_square_amplitude_bounds() {
        let mut osc = Oscillator::new(Waveform::Square, FREQUENCY, SAMPLE_RATE);
        for _ in 0..SAMPLE_RATE as usize {
            let s = osc.next_sample();
            assert!(
                (-1.0..=1.0).contains(&s),
                "Square sample out of bounds: {s}"
            );
        }
    }

    #[test]
    fn test_saw_zero_mean() {
        let mut osc = Oscillator::new(Waveform::Sawtooth, FREQUENCY, SAMPLE_RATE);

        let n = SAMPLE_RATE as usize; // 1 second worth
        let sum: f64 = (0..n).map(|_| osc.next_sample()).sum();
        let mean = sum / n as f64;

        // Residual is bounded by (1 sample / N samples) * peak amplitude ≈ 2.3e-5
        assert!(mean.abs() < 1e-3, "Saw mean too large: {mean}");
    }
}
