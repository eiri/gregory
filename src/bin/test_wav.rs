use gregory::dsp::{Oscillator, Waveform};
use hound::{SampleFormat, WavSpec, WavWriter};

const SAMPLE_RATE: u32 = 44_100;

fn to_pcm(s: f64) -> i16 {
    (s as f32 * i16::MAX as f32) as i16
}

fn main() {
    let sample_rate = SAMPLE_RATE as f64;
    let duration_secs = 1.0;
    let frequency = 220.0; // A3
    let amplitude = 0.5;

    let spec = WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let path = "gregory_test.wav";
    let mut writer = WavWriter::create(path, spec).expect("Failed to create WAV file");

    let mut osc = Oscillator::new(Waveform::Sawtooth, frequency, sample_rate);

    let total_samples = (sample_rate * duration_secs) as usize;

    for _ in 0..total_samples {
        let sample = osc.next_sample() * amplitude;
        writer
            .write_sample(to_pcm(sample))
            .expect("Failed to write sample");
    }

    writer.finalize().expect("Failed to finalize WAV file");
    println!("Wrote {duration_secs}s of {frequency}Hz sawtooth to '{path}'");
}
