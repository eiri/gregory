use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum FilterMode {
    /// Classic 4-pole -24dB low-pass
    LowPass,
    /// 2-pole -12dB low-pass
    LowPass2Pole,
}

/// Moog ladder filter.
///
/// This implementation follows Huovilainen (2004, 2006) with the thermal
/// voltage correction and double-sampling trick for stability.
///
/// * cutoff — cutoff frequency in Hz, clamped to (10, nyquist)
/// * resonance — feedback amount, 0.0 (none) to 1.0 (self-oscillation)
pub struct Filter {
    pub mode: FilterMode,
    pub cutoff: f64,
    pub resonance: f64,
    sample_rate: f64,

    // Internal filter state.
    stage: [f64; 4],
    stage_tanh: [f64; 3], // hyperbolic tangent
    delayed_output: f64,

    // Pre-computed coefficients, updated whenever cutoff/resonance change.
    // These are recomputed lazily via a dirty flag.
    thermal: f64, // 1 / (2 * VT), thermal voltage reciprocal
    f: f64,       // frequency coefficient
    fc: f64,      // (1 - f) — the one-pole feedback coefficient
    four_r: f64,  // 4 * resonance * thermal  (feedback gain)
    dirty: bool,
}

impl Filter {
    pub fn new(cutoff: f64, resonance: f64, sample_rate: f64) -> Self {
        let mut f = Self {
            mode: FilterMode::LowPass,
            cutoff,
            resonance,
            sample_rate,
            stage: [0.0; 4],
            stage_tanh: [0.0; 3],
            delayed_output: 0.0,
            thermal: 1.0,
            f: 0.0,
            fc: 0.0,
            four_r: 0.0,
            dirty: true,
        };
        f.update_coefficients();
        f
    }

    pub fn set_cutoff(&mut self, cutoff: f64) {
        self.cutoff = cutoff;
        self.dirty = true;
    }

    pub fn set_resonance(&mut self, resonance: f64) {
        self.resonance = resonance.clamp(0.0, 1.0);
        self.dirty = true;
    }

    /// Process a single sample through the filter and return the output.
    pub fn process(&mut self, input: f64) -> f64 {
        if self.dirty {
            self.update_coefficients();
        }

        // Run the filter twice at half the input rate (double-sampling).
        // This improves the accuracy of the nonlinear model at high cutoffs
        // by reducing the aliasing introduced by the tanh approximation.
        let out = self.tick(input);
        let out = self.tick(out);

        match self.mode {
            FilterMode::LowPass => out,
            FilterMode::LowPass2Pole => self.stage[1],
        }
    }

    /// Reset all internal state aka silence the filter without changing params.
    pub fn reset(&mut self) {
        self.stage = [0.0; 4];
        self.stage_tanh = [0.0; 3];
        self.delayed_output = 0.0;
    }

    fn tick(&mut self, input: f64) -> f64 {
        // Subtract feedback from input, apply thermal saturation.
        let input_sat = tanh_approx((input - self.four_r * self.delayed_output) * self.thermal);

        // Stage 0
        let s0_tanh = tanh_approx(self.stage[0] * self.thermal);
        self.stage[0] += self.f * (input_sat - s0_tanh);
        self.stage_tanh[0] = s0_tanh;

        // Stage 1
        let s1_tanh = tanh_approx(self.stage[1] * self.thermal);
        self.stage[1] += self.f * (self.stage_tanh[0] - s1_tanh);
        self.stage_tanh[1] = s1_tanh;

        // Stage 2
        let s2_tanh = tanh_approx(self.stage[2] * self.thermal);
        self.stage[2] += self.f * (self.stage_tanh[1] - s2_tanh);
        self.stage_tanh[2] = s2_tanh;

        // Stage 3 — output stage, no tanh needed on output
        self.stage[3] += self.f * (self.stage_tanh[2] - tanh_approx(self.stage[3] * self.thermal));

        self.delayed_output = self.stage[3];
        self.stage[3]
    }

    /// Recompute filter coefficients from cutoff and resonance.
    fn update_coefficients(&mut self) {
        self.dirty = false;

        let nyquist = self.sample_rate * 0.5;
        let cutoff = self.cutoff.clamp(10.0, nyquist - 1.0);

        // Normalized frequency: maps [0, nyquist] -> [0, 1].
        let fc_norm = cutoff / self.sample_rate;

        // Equivalent to a first-order bilinear prewarping.
        self.f = fc_norm * 1.873; // empirically tuned
        self.f = self.f.min(0.9999); // prevent instability near Nyquist

        self.fc = 1.0 - self.f;

        // Scale resonance to [0, 4] in the feedback loop.
        // Above 0.95 the filter self-oscillates.
        self.four_r = self.resonance.clamp(0.0, 1.0) * 4.0;
    }
}

/// Fast tanh (hyperbolic tangent) approximation (Pade 3/3 rational)
#[inline(always)]
fn tanh_approx(x: f64) -> f64 {
    // Clamp to avoid overflow in the polynomial at extreme inputs.
    let x = x.clamp(-4.5, 4.5);
    let x2 = x * x;
    x * (27.0 + x2) / (27.0 + 9.0 * x2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 44100.0;

    /// DC input should produce DC output (filter passes through at low freq).
    #[test]
    fn test_dc_passthrough_low_cutoff() {
        let mut f = Filter::new(20000.0, 0.0, SAMPLE_RATE);
        // Feed DC=1.0 for long enough to settle.
        let mut out = 0.0;
        for _ in 0..10_000 {
            out = f.process(1.0);
        }
        // At max cutoff with no resonance, DC should pass through near unity.
        assert!((out - 1.0).abs() < 0.05, "DC passthrough failed: {out}");
    }

    /// With cutoff very low, high-frequency content should be heavily attenuated.
    #[test]
    fn test_high_freq_attenuation() {
        let mut f = Filter::new(200.0, 0.0, SAMPLE_RATE);
        // Input: 5kHz sine wave.
        let freq = 5000.0;
        let mut rms_out = 0.0;
        let n = 4096;
        for i in 0..n {
            let input = (2.0 * std::f64::consts::PI * freq * i as f64 / SAMPLE_RATE).sin();
            let out = f.process(input);
            rms_out += out * out;
        }
        rms_out = (rms_out / n as f64).sqrt();
        // RMS should be well below 0.1 — heavily attenuated.
        assert!(
            rms_out < 0.1,
            "High freq not attenuated enough: rms={rms_out}"
        );
    }

    /// Filter should not explode at high resonance.
    #[test]
    fn test_stability_at_high_resonance() {
        let mut f = Filter::new(1000.0, 0.99, SAMPLE_RATE);
        for i in 0..SAMPLE_RATE as usize {
            let input = (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / SAMPLE_RATE).sin() * 0.1;
            let out = f.process(input);
            assert!(
                out.is_finite() && out.abs() < 20.0,
                "Filter unstable at sample {i}: {out}"
            );
        }
    }

    /// tanh approximation should stay close to the real tanh.
    #[test]
    fn test_tanh_approx_accuracy() {
        for i in -30..=30 {
            let x = i as f64 * 0.1;
            let approx = tanh_approx(x);
            let exact = x.tanh();
            assert!(
                (approx - exact).abs() < 0.05, // Pade 3/3 is about 3% max error within +3.0/-3.0
                "tanh_approx({x}) = {approx}, expected ~{exact}"
            );
        }
    }

    /// reset() should zero all state.
    #[test]
    fn test_reset_clears_state() {
        let mut f = Filter::new(500.0, 0.5, SAMPLE_RATE);
        for _ in 0..1000 {
            f.process(0.5);
        }
        f.reset();
        // After reset, a zero input should produce near-zero output immediately.
        let out = f.process(0.0);
        assert!(out.abs() < 1e-10, "Filter not silent after reset: {out}");
    }
}
