//! The MIDI thread receives raw bytes from `midir`, parses them with `wmidi`,
//! and pushes `NoteEvent`s into a ring buffer that the audio thread drains.

use midir::{MidiInput, MidiInputConnection};
use ringbuf::HeapProd;
use ringbuf::traits::Producer;
use wmidi::MidiMessage;

#[derive(Debug, Clone, Copy)]
pub enum NoteEvent {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    PitchBend { semitones: f64 },
    ModWheel { value: f64 },
}

pub struct MidiInputHandle {
    /// Kept alive purely to maintain the connection — never read directly.
    _connection: MidiInputConnection<()>,
}

impl MidiInputHandle {
    pub fn list_ports() -> Vec<String> {
        let midi_in = MidiInput::new("gregory-list").expect("Failed to create MIDI input");
        let ports = midi_in.ports();
        ports
            .iter()
            .filter_map(|p| midi_in.port_name(p).ok())
            .collect()
    }

    /// Connect to the port whose name contains `port_hint` (case-insensitive
    /// substring match). Pass an empty string to connect to the first available port.
    pub fn connect(
        port_hint: &str,
        producer: HeapProd<NoteEvent>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let midi_in = MidiInput::new("gregory")?;
        let ports = midi_in.ports();

        if ports.is_empty() {
            return Err("No MIDI input ports found".into());
        }

        let port = ports
            .iter()
            .find(|p| {
                midi_in
                    .port_name(p)
                    .map(|name| name.to_lowercase().contains(&port_hint.to_lowercase()))
                    .unwrap_or(false)
            })
            .or_else(|| ports.first())
            .ok_or("No MIDI ports available")?;

        let port_name = midi_in.port_name(port).unwrap_or_default();
        println!("MIDI input: {port_name}");

        // `producer` is moved into the callback
        let mut prod = producer;

        let connection = midi_in.connect(
            port,
            "gregory-input",
            move |_timestamp_us, raw, _| {
                parse_and_push(raw, &mut prod);
            },
            (),
        )?;

        Ok(Self {
            _connection: connection,
        })
    }
}

fn parse_and_push(raw: &[u8], prod: &mut HeapProd<NoteEvent>) {
    let Ok(msg) = MidiMessage::try_from(raw) else {
        return;
    };

    let event = match msg {
        MidiMessage::NoteOn(_ch, note, vel) => {
            let velocity = u8::from(vel);
            if velocity == 0 {
                // MIDI convention: NoteOn with velocity 0 == NoteOff.
                NoteEvent::NoteOff {
                    note: u8::from(note),
                }
            } else {
                NoteEvent::NoteOn {
                    note: u8::from(note),
                    velocity,
                }
            }
        }
        MidiMessage::NoteOff(_ch, note, _vel) => NoteEvent::NoteOff {
            note: u8::from(note),
        },
        MidiMessage::PitchBendChange(_ch, bend) => {
            // wmidi PitchBend is a 14-bit value, center = 8192, range 0–16383.
            let raw = u16::from(bend) as f64;
            let normalized = (raw - 8192.0) / 8192.0;
            let semitones = normalized * 2.0; // +/- 2 semitone
            NoteEvent::PitchBend { semitones }
        }
        MidiMessage::ControlChange(_ch, control, value) => {
            use wmidi::ControlFunction;
            if control == ControlFunction::MODULATION_WHEEL {
                NoteEvent::ModWheel {
                    value: u8::from(value) as f64 / 127.0,
                }
            } else {
                return;
            }
        }
        // Ignore all other messages for now (CC, etc.).
        _ => return,
    };

    // Best-effort push — if the ring buffer is full we drop.
    let _ = prod.try_push(event);
}
