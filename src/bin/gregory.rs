use std::sync::{Arc, Mutex};

use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample, SampleFormat, StreamConfig};
use ringbuf::traits::Consumer;
use ringbuf::traits::Split;
use ringbuf::{HeapCons, HeapRb};

use gregory::dsp::{FilterMode, Waveform};
use gregory::midi::{MidiInputHandle, NoteEvent};
use gregory::ui::GregoryApp;
use gregory::{Engine, Patch};

#[derive(Parser)]
#[command(name = "gregory", about = "Monophonic software synthesizer")]
struct Cli {
    /// MIDI input port name or substring to match (e.g. "KeyStep", "USB").
    /// Connects to the first available port if omitted.
    #[arg(short, long, default_value = "")]
    midi: String,
}

fn main() {
    let cli = Cli::parse();

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

    //
    let initial_patch = Patch {
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
    };

    let shared_patch: Arc<Mutex<Patch>> = Arc::new(Mutex::new(initial_patch.clone()));

    let sample_rate = config.sample_rate() as f64;

    let mut engine = Engine::new(sample_rate);
    engine.set_patch(initial_patch);

    // Wrap engine in Arc<Mutex> so it can be shared between the audio
    // callback thread and the main thread
    let engine = Arc::new(Mutex::new(engine));

    //  MIDI ring buffer
    //  Single-producer (MIDI thread) / single-consumer (audio thread)
    //  512 events should be enought for a monosynth
    let rb = HeapRb::<NoteEvent>::new(512);
    let (producer, consumer) = rb.split();

    println!("Available MIDI ports:");
    for (i, name) in MidiInputHandle::list_ports().iter().enumerate() {
        println!("  [{i}] {name}");
    }

    let port_hint = cli.midi;

    let _midi = match MidiInputHandle::connect(&port_hint, producer) {
        Ok(handle) => {
            println!("MIDI connected");
            Some(handle)
        }
        Err(e) => {
            eprintln!("MIDI unavailable: {e}");
            None
        }
    };

    let _stream = build_stream(
        &device,
        &config.into(),
        Arc::clone(&engine),
        Arc::clone(&shared_patch),
        consumer,
    )
    .expect("Failed to build audio stream");

    _stream.play().expect("Failed to start audio stream");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Gregory")
            .with_inner_size([936.0, 290.0])
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "Gregory",
        options,
        Box::new(move |cc| Ok(Box::new(GregoryApp::new(Arc::clone(&shared_patch), cc)))),
    )
    .expect("Failed to start UI");
}

fn build_stream(
    device: &Device,
    config: &StreamConfig,
    engine: Arc<Mutex<Engine>>,
    shared_patch: Arc<Mutex<Patch>>,
    consumer: HeapCons<NoteEvent>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = config.channels as usize;
    let sample_format = device.default_output_config().unwrap().sample_format();

    match sample_format {
        SampleFormat::F32 => {
            build_stream_typed::<f32>(device, config, engine, shared_patch, consumer, channels)
        }
        SampleFormat::F64 => {
            build_stream_typed::<f64>(device, config, engine, shared_patch, consumer, channels)
        }
        SampleFormat::I16 => {
            build_stream_typed::<i16>(device, config, engine, shared_patch, consumer, channels)
        }
        SampleFormat::U16 => {
            build_stream_typed::<u16>(device, config, engine, shared_patch, consumer, channels)
        }
        _ => panic!("Unsupported sample format: {sample_format}"),
    }
}

fn build_stream_typed<T>(
    device: &Device,
    config: &StreamConfig,
    engine: Arc<Mutex<Engine>>,
    shared_patch: Arc<Mutex<Patch>>,
    mut consumer: HeapCons<NoteEvent>,
    channels: usize,
) -> Result<cpal::Stream, cpal::BuildStreamError>
where
    T: Sample + FromSample<f32> + cpal::SizedSample,
{
    // Declared here so it's captured by the move closure below.
    let mut last_patch: Option<Patch> = None;

    device.build_output_stream(
        config,
        move |data: &mut [T], _info: &cpal::OutputCallbackInfo| {
            // Lock the engine for the duration of this callback.
            let mut eng = match engine.try_lock() {
                Ok(e) => e,
                Err(_) => return, // skip buffer if contended
            };

            // Apply patch changes from the UI thread — once per buffer, not per sample.
            if let Ok(p) = shared_patch.try_lock() {
                let changed = last_patch.as_ref().is_none_or(|lp| lp != &*p);
                if changed {
                    eng.set_patch(p.clone());
                    last_patch = Some(p.clone());
                }
            }

            // Drain all pending MIDI events before processing audio.
            while let Some(event) = consumer.try_pop() {
                match event {
                    NoteEvent::NoteOn { note, velocity } => {
                        eng.note_on(note, velocity);
                    }
                    NoteEvent::NoteOff { note } => {
                        eng.note_off(note);
                    }
                    NoteEvent::PitchBend { semitones } => eng.pitch_bend(semitones),
                    NoteEvent::ModWheel { value } => {
                        eng.set_mod_wheel(value);
                        if let Ok(mut p) = shared_patch.try_lock() {
                            p.mod_wheel = value;
                            // this kind of spills over, but that's ok for now
                            p.filter_cutoff = 10.0 + value * (18000.0 - 10.0);
                        }
                    }
                }
            }

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
