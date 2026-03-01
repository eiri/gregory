# gregory

Monophonic software synthesizer

![Gregory UI](ui.png)

## Summary

Gregory is a one-oscillator subtractive monosynth written in Rust.
It has a band-limited sawtooth and square oscillator with optional unison mode,
a four-pole Moog-style low-pass filter, dual ADSR envelopes for amplitude and
filter modulation, plus portamento for smooth pitch gliding between notes.
It accepts input from any connected MIDI keyboard and runs on macOS, Linux and
Windows via native audio APIs.

## Usage

Run in terminal:

```bash
$ gregory
Output device: MiniFuse 4
Sample rate: 48000  Channels: 6  Format: F32
Available MIDI ports:
  [0] KeyStep Pro
  [1] MiniFuse 4
MIDI input: KeyStep Pro
MIDI connected
```

Then tweak the knobs and faders or just click dice to create a random patch.
Save patches with Cmd+S and load with Cmd+O. Create a new default patch with Cmd+N.
By default patches saved in `~/.config/gregory/patches` as TOML files.

## Installation

Prebuilt binaries for macOS, Linux and Windows are available on the
[releases page](https://github.com/eiri/gregory/releases).

To build and install from source with Cargo:

```bash
cargo install --git https://github.com/eiri/gregory
```

### Motivation

This is an educational project with two goals.

First, to explore writing audio software in Rust with a look at options
for lock-free concurrency between audio and MIDI threads.

Second, to learn software sound synthesis practically by writing
from the ground up oscillators, filters and envelopes.

The implementation is intentionally kept simple.

#### Name

**Gregory** is named after Gregorian chant, since it's monophonic, one voice at a time.

## License

Licensed under either of [MIT](https://github.com/eiri/gregory/blob/main/LICENSE-MIT)
or [Apache 2.0](https://github.com/eiri/gregory/blob/main/LICENSE-APACHE)
at your option.
