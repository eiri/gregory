use gregory::dsp::{Oscillator, Waveform};
use hound::{SampleFormat, WavSpec, WavWriter};

fn main() {
    let sample_rate = 44_100u32;
    let duration_secs = 1.0f64;
    let frequency = 220.0; // A3
    let amplitude = 0.5f32;

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let path = "gregory_test.wav";
    let mut writer = WavWriter::create(path, spec).expect("Failed to create WAV file");

    let mut osc = Oscillator::new(Waveform::Sawtooth, frequency, sample_rate as f64);

    let total_samples = (sample_rate as f64 * duration_secs) as usize;

    for _ in 0..total_samples {
        let sample = osc.next_sample() as f32 * amplitude;
        // Convert f32 [-1, 1] to i16
        let pcm = (sample * i16::MAX as f32) as i16;
        writer.write_sample(pcm).expect("Failed to write sample");
    }

    writer.finalize().expect("Failed to finalize WAV file");
    println!("Wrote {duration_secs}s of {frequency}Hz sawtooth to '{path}'");
}
