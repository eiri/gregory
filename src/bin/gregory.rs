use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample, SampleFormat, StreamConfig};

use gregory::dsp::{FilterMode, Waveform};
use gregory::{Engine, Patch};

fn main() {
    let host = cpal::default_host();

    let device = host
        .default_output_device()
        .expect("No output device found");

    println!("Output device: {}", device.description().unwrap());

    let config = device
        .default_output_config()
        .expect("Failed to get default output config");

    println!(
        "Sample rate: {}  Channels: {}  Format: {:?}",
        config.sample_rate(),
        config.channels(),
        config.sample_format()
    );

    let sample_rate = config.sample_rate() as f64;

    let mut engine = Engine::new(sample_rate);
    engine.set_patch(Patch {
        waveform: Waveform::Sawtooth,
        filter_mode: FilterMode::LowPass,
        filter_cutoff: 600.0,
        filter_resonance: 0.4,
        flt_env_amount: 3000.0,
        flt_attack: 0.01,
        flt_decay: 0.3,
        flt_sustain: 0.2,
        flt_release: 0.5,
        amp_attack: 0.01,
        amp_decay: 0.1,
        amp_sustain: 0.7,
        amp_release: 0.5,
        gain: 0.5,
        ..Patch::default()
    });

    // Wrap engine in Arc<Mutex> so it can be shared between the audio
    // callback thread and the main thread (for note scheduling below).
    let engine = Arc::new(Mutex::new(engine));

    let stream = build_stream(&device, &config.into(), Arc::clone(&engine))
        .expect("Failed to build audio stream");

    stream.play().expect("Failed to start audio stream");
    println!("Audio stream running. Playing demo sequence...");

    // Demo notes: (midi_note, on_ms, off_ms) relative to sequence start.
    let sequence: &[(u8, u64, u64)] = &[
        (45, 0, 700),     // A2
        (50, 800, 1500),  // D3
        (52, 1600, 2300), // E3
        (48, 2400, 3100), // C3
        (45, 3200, 4500), // A2 — held
    ];

    let start = std::time::Instant::now();
    let mut next_event: usize = 0;

    // Flatten sequence into a sorted event list.
    let mut events: Vec<(u64, bool, u8)> = Vec::new(); // (time_ms, is_on, note)
    for &(note, on_ms, off_ms) in sequence {
        events.push((on_ms, true, note));
        events.push((off_ms, false, note));
    }
    events.sort_by_key(|e| e.0);

    while next_event < events.len() {
        let elapsed_ms = start.elapsed().as_millis() as u64;
        while next_event < events.len() && events[next_event].0 <= elapsed_ms {
            let (_, is_on, note) = events[next_event];
            let mut eng = engine.lock().unwrap();
            if is_on {
                println!("note_on  {note}");
                eng.note_on(note, 100);
            } else {
                println!("note_off {note}");
                eng.note_off(note);
            }
            next_event += 1;
        }
        std::thread::sleep(Duration::from_millis(1));
    }

    // Let the last note release fully before exiting.
    std::thread::sleep(Duration::from_millis(2000));
    println!("Done");
}

fn build_stream(
    device: &Device,
    config: &StreamConfig,
    engine: Arc<Mutex<Engine>>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = config.channels as usize;
    let sample_format = device.default_output_config().unwrap().sample_format();

    match sample_format {
        SampleFormat::F32 => build_stream_typed::<f32>(device, config, engine, channels),
        SampleFormat::F64 => build_stream_typed::<f64>(device, config, engine, channels),
        SampleFormat::I16 => build_stream_typed::<i16>(device, config, engine, channels),
        SampleFormat::U16 => build_stream_typed::<u16>(device, config, engine, channels),
        _ => panic!("Unsupported sample format: {sample_format}"),
    }
}

fn build_stream_typed<T>(
    device: &Device,
    config: &StreamConfig,
    engine: Arc<Mutex<Engine>>,
    channels: usize,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: Sample + FromSample<f32> + cpal::SizedSample,
{
    device.build_output_stream(
        config,
        move |data: &mut [T], _info: &cpal::OutputCallbackInfo| {
            // Lock the engine for the duration of this callback.
            let mut eng = match engine.try_lock() {
                Ok(e) => e,
                Err(_) => return, // skip buffer if contended
            };
            // Produce one mono sample per frame and copy it to all channels.
            for frame in data.chunks_mut(channels) {
                let sample = eng.process() as f32;
                let value = T::from_sample(sample);
                for channel_sample in frame.iter_mut() {
                    *channel_sample = value;
                }
            }
        },
        |err| eprintln!("Audio stream error: {err}"),
        None,
    )
}
