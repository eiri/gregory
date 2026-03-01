use std::sync::{Arc, Mutex};

use midir::{MidiInput, MidiInputConnection};
use ringbuf::HeapProd;
use ringbuf::traits::Producer;
use wmidi::MidiMessage;

/// MIDI channel filter — Omni listens on all channels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MidiChannel {
    Omni,
    Channel(u8), // 1–16
}

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
    pub channel: Arc<Mutex<MidiChannel>>,
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
        let channel = Arc::new(Mutex::new(MidiChannel::Omni));
        let channel_clone = Arc::clone(&channel);

        let connection = midi_in.connect(
            port,
            "gregory-input",
            move |_timestamp_us, raw, _| {
                let ch = *channel_clone.lock().unwrap();
                parse_and_push(raw, &mut prod, ch);
            },
            (),
        )?;

        Ok(Self {
            _connection: connection,
            channel,
        })
    }
}

fn parse_and_push(raw: &[u8], prod: &mut HeapProd<NoteEvent>, filter: MidiChannel) {
    let Ok(msg) = MidiMessage::try_from(raw) else {
        return;
    };

    // Check channel filter before parsing the message.
    let msg_channel = match &msg {
        MidiMessage::NoteOn(ch, _, _) => Some(ch.index() + 1),
        MidiMessage::NoteOff(ch, _, _) => Some(ch.index() + 1),
        MidiMessage::PitchBendChange(ch, _) => Some(ch.index() + 1),
        MidiMessage::ControlChange(ch, _, _) => Some(ch.index() + 1),
        _ => None,
    };

    if let Some(ch) = msg_channel {
        match filter {
            MidiChannel::Omni => {}
            MidiChannel::Channel(target) if ch == target => {}
            _ => return, // filtered out
        }
    }

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
