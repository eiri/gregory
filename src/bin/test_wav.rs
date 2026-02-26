use gregory::dsp::{Filter, FilterMode, Oscillator, Waveform};
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
        let mut filter = Filter::new(80.0, 0.6, sample_rate);
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
}
