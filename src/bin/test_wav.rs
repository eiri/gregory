use gregory::dsp::{Engine, Patch};
use gregory::dsp::{FilterMode, Waveform};
use hound::{SampleFormat, WavSpec, WavWriter};

const SAMPLE_RATE: u32 = 44_100;
const SR: f64 = SAMPLE_RATE as f64;

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
    // Dry sawtooth
    {
        let mut engine = Engine::new(SR);
        engine.set_patch(Patch {
            waveform: Waveform::Sawtooth,
            filter_cutoff: 20000.0,
            filter_resonance: 0.0,
            flt_env_amount: 0.0,
            amp_attack: 0.0,
            amp_decay: 0.0,
            amp_sustain: 1.0,
            amp_release: 0.0,
            gain: 0.5,
            ..Patch::default()
        });

        let mut writer = WavWriter::create("gregory_saw_raw.wav", make_spec()).unwrap();
        engine.note_on(45, 100); // A2
        for _ in 0..(SR * 1.0) as usize {
            writer.write_sample(to_pcm(engine.process())).unwrap();
        }
        writer.finalize().unwrap();
        println!("Wrote gregory_saw_raw.wav");
    }

    // Filtered sawtooth — cutoff sweeps 80 Hz to 8 kHz and back
    {
        let total = (SR * 2.0) as usize;
        let mut engine = Engine::new(SR);
        engine.set_patch(Patch {
            waveform: Waveform::Sawtooth,
            filter_resonance: 0.6,
            flt_env_amount: 0.0,
            amp_attack: 0.0,
            amp_decay: 0.0,
            amp_sustain: 1.0,
            amp_release: 0.0,
            gain: 0.5,
            ..Patch::default()
        });

        let mut writer = WavWriter::create("gregory_saw_filtered.wav", make_spec()).unwrap();
        engine.note_on(45, 100); // A2
        for i in 0..total {
            let t = i as f64 / total as f64; // 0.0 → 1.0
            let sweep = (std::f64::consts::PI * t).sin(); // 0 → 1 → 0
            let cutoff = 80.0 + sweep * (8000.0 - 80.0);
            engine.patch.filter_cutoff = cutoff;
            writer.write_sample(to_pcm(engine.process())).unwrap();
        }
        writer.finalize().unwrap();
        println!("Wrote gregory_saw_filtered.wav");
    }

    // Three notes — oscillator + filter + envelope
    {
        let notes: &[(u8, f64, f64)] = &[
            (45, 0.1, 0.9), // A2
            (50, 1.1, 1.9), // D3
            (52, 2.1, 3.0), // E3 — held longer
        ];
        let total = (SR * 4.0) as usize;

        let mut engine = Engine::new(SR);
        engine.set_patch(Patch {
            waveform: Waveform::Sawtooth,
            filter_mode: FilterMode::LowPass,
            filter_cutoff: 300.0,
            filter_resonance: 0.5,
            flt_env_amount: 4000.0,
            flt_attack: 0.005,
            flt_decay: 0.25,
            flt_sustain: 0.1,
            flt_release: 0.4,
            amp_attack: 0.005,
            amp_decay: 0.15,
            amp_sustain: 0.6,
            amp_release: 0.4,
            gain: 0.5,
            ..Patch::default()
        });

        let mut writer = WavWriter::create("gregory_adsr.wav", make_spec()).unwrap();
        for i in 0..total {
            let t = i as f64 / SR;
            for &(note, on, off) in notes {
                if (t - on).abs() < 1.0 / SR {
                    engine.note_on(note, 100);
                }
                if (t - off).abs() < 1.0 / SR {
                    engine.note_off(note);
                }
            }
            writer.write_sample(to_pcm(engine.process())).unwrap();
        }
        writer.finalize().unwrap();
        println!("Wrote gregory_adsr.wav");
    }
}
