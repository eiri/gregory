use gregory::dsp::{Envelope, Filter, FilterMode, Oscillator, Waveform};
use hound::{SampleFormat, WavSpec, WavWriter};

const SAMPLE_RATE: u32 = 44_100;

fn make_spec() -> WavSpec {
    WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    }
}

fn to_pcm(s: f64) -> i16 {
    (s as f32 * i16::MAX as f32) as i16
}

fn main() {
    let sample_rate = SAMPLE_RATE as f64;
    let duration_secs = 1.0;
    let frequency = 110.0; // A2
    let filter_from = 80.0;
    let filter_to = 8000.0;
    let resonance = 0.6;
    let amplitude = 0.5;
    let total_samples = (sample_rate * duration_secs) as usize;

    // Dry sawtooth
    {
        let mut writer = WavWriter::create("gregory_saw_raw.wav", make_spec()).unwrap();
        let mut osc = Oscillator::new(Waveform::Sawtooth, frequency, sample_rate);

        for _ in 0..total_samples {
            let sample = osc.next_sample() * amplitude;
            writer.write_sample(to_pcm(sample)).unwrap();
        }

        writer.finalize().unwrap();
        println!("Wrote gregory_saw_raw.wav");
    }

    // Filtered sawtooth — cutoff sweeps 80 Hz to 8 kHz and back
    {
        let mut writer = WavWriter::create("gregory_saw_filtered.wav", make_spec()).unwrap();
        let mut osc = Oscillator::new(Waveform::Sawtooth, frequency, sample_rate);
        let mut filter = Filter::new(filter_from, resonance, sample_rate);
        filter.mode = FilterMode::LowPass;

        for i in 0..total_samples {
            // Slow sinusoidal sweep of cutoff: 80 Hz to 8000 Hz and back.
            let t = i as f64 / total_samples as f64;
            let sweep = (std::f64::consts::PI * t).sin();
            let cutoff = filter_from + sweep * (filter_to - filter_from);
            filter.set_cutoff(cutoff);

            let raw = osc.next_sample();
            let filtered = filter.process(raw) * amplitude;
            writer.write_sample(to_pcm(filtered)).unwrap();
        }

        writer.finalize().unwrap();
        println!("Wrote gregory_saw_filtered.wav");
    }

    // Three notes — oscillator + filter + envelope
    {
        // Notes: (frequency_hz, note_on_sec, note_off_sec)
        let notes: &[(f64, f64, f64)] = &[
            (110.0, 0.1, 0.9), // A2
            (146.8, 1.1, 1.9), // D3
            (164.8, 2.1, 3.5), // E3 — held longer
        ];
        let total_secs = 4.5;
        let total = (sample_rate * total_secs) as usize;

        let mut writer = WavWriter::create("gregory_adsr.wav", make_spec()).unwrap();
        let mut osc = Oscillator::new(Waveform::Sawtooth, frequency, sample_rate);
        let mut filter = Filter::new(filter_from, resonance, sample_rate);

        // Amplitude envelope: snappy attack, medium decay, full sustain.
        let mut amp_env = Envelope::new(0.005, 0.15, 0.6, 0.4, sample_rate);
        // Filter envelope: fast attack, slower decay, low sustain → pluck shape.
        let mut flt_env = Envelope::new(0.005, 0.25, 0.1, 0.4, sample_rate);

        let base_cutoff = 300.0_f64;
        let cutoff_range = 4000.0_f64; // filter opens by this much at peak

        for i in 0..total {
            let t = i as f64 / sample_rate;

            // Gate management.
            for &(freq, on, off) in notes {
                if (t - on).abs() < 1.0 / sample_rate {
                    osc.set_frequency(freq);
                    amp_env.gate_on();
                    flt_env.gate_on();
                }
                if (t - off).abs() < 1.0 / sample_rate {
                    amp_env.gate_off();
                    flt_env.gate_off();
                }
            }

            let amp = amp_env.next_sample();
            let flt = flt_env.next_sample();
            let cutoff = base_cutoff + flt * cutoff_range;

            filter.set_cutoff(cutoff);
            let raw = osc.next_sample();
            let filtered = filter.process(raw);
            let out = (filtered * amp) * amplitude;

            writer.write_sample(to_pcm(out)).unwrap();
        }
        writer.finalize().unwrap();
        println!("Wrote gregory_adsr.wav")
    }
}
