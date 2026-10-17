//! Real-time mixing primitives shared by the audio thread (see `engine.rs`):
//! decoding a sound file into the engine's fixed internal format, streaming
//! samples from a currently-playing pad/one-shot sound into an output
//! buffer, and converting between arbitrary device sample rates/channel
//! counts and the engine's fixed bus without allocating inside a real-time
//! audio callback.
//!
//! The whole engine runs its mixing math at one fixed format — 48kHz
//! stereo f32 — regardless of what any individual input/output device
//! actually uses. Every device only ever touches the bus through a
//! [`StreamResampler`] at its own edge, which keeps the mixing code itself
//! (voice summing, gain) trivial and format-agnostic.

use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;

use rodio::{Decoder, Source};

use crate::error::{AppError, AppResult};

pub const ENGINE_SAMPLE_RATE: u32 = 48_000;
pub const ENGINE_CHANNELS: usize = 2;
pub const TICK_MS: u64 = 20;
pub const TICK_FRAMES: usize = (ENGINE_SAMPLE_RATE as usize * TICK_MS as usize) / 1000;

/// A single currently-playing sound, pre-decoded and pre-resampled to the
/// engine's fixed format so mixing it in is just an add-and-advance per
/// tick — no decoding or resampling happens on the audio thread's hot path.
pub struct Voice {
    samples: Arc<[f32]>,
    position: usize,
    gain: f32,
}

impl Voice {
    pub fn new(samples: Arc<[f32]>, gain: f32) -> Self {
        Self { samples, position: 0, gain }
    }

    /// Adds this voice's next `out.len()` interleaved-stereo samples into
    /// `out` (does not overwrite — callers mix multiple voices into the
    /// same buffer). Returns `false` once the voice has no samples left.
    pub fn mix_into(&mut self, out: &mut [f32]) -> bool {
        let remaining = self.samples.len().saturating_sub(self.position);
        let n = remaining.min(out.len());
        for i in 0..n {
            out[i] += self.samples[self.position + i] * self.gain;
        }
        self.position += n;
        self.position < self.samples.len()
    }
}

/// Decodes `path` fully and converts it to the engine's fixed 48kHz-stereo
/// format, ready to be played by a [`Voice`]. Decoding/resampling the whole
/// file upfront (rather than streaming it live) keeps the real-time mixing
/// tick simple and glitch-free at the cost of a small one-time delay when a
/// pad is first triggered — acceptable for soundboard clips, which are
/// always short.
pub fn decode_to_engine_format(path: &str) -> AppResult<Arc<[f32]>> {
    let file = File::open(path).map_err(|e| AppError::Audio(e.to_string()))?;
    let reader = BufReader::new(file);
    let decoder = Decoder::new(reader).map_err(|e| AppError::Audio(e.to_string()))?;

    let channels = decoder.channels() as usize;
    let sample_rate = decoder.sample_rate();
    let samples: Vec<f32> = decoder.map(|s| s as f32 / i16::MAX as f32).collect();

    let stereo = to_stereo(&samples, channels);
    Ok(resample_stereo_buffer(&stereo, sample_rate, ENGINE_SAMPLE_RATE).into())
}

/// Upmixes mono to stereo (by duplication) or downmixes >2 channels to
/// stereo (by keeping the first two). Already-stereo input passes through.
fn to_stereo(samples: &[f32], channels: usize) -> Vec<f32> {
    match channels {
        1 => samples.iter().flat_map(|&s| [s, s]).collect(),
        2 => samples.to_vec(),
        0 => Vec::new(),
        n => samples
            .chunks(n)
            .flat_map(|frame| [frame[0], frame.get(1).copied().unwrap_or(frame[0])])
            .collect(),
    }
}

/// One-shot whole-buffer resample used only at decode time (never inside a
/// real-time callback, so allocating here is fine). Plain linear
/// interpolation — not broadcast-quality, but more than sufficient for
/// short soundboard clips, and `from_rate == to_rate` (by far the common
/// case, since most Windows audio endpoints default to 48kHz) is a
/// zero-cost passthrough.
fn resample_stereo_buffer(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.len() < 2 {
        return input.to_vec();
    }
    let in_frames = input.len() / 2;
    let ratio = from_rate as f64 / to_rate as f64;
    let out_frames = ((in_frames as f64) / ratio).round().max(1.0) as usize;

    let mut out = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let pos = i as f64 * ratio;
        let idx = (pos.floor() as usize).min(in_frames - 1);
        let next_idx = (idx + 1).min(in_frames - 1);
        let frac = (pos - idx as f64) as f32;
        let l = input[idx * 2] + (input[next_idx * 2] - input[idx * 2]) * frac;
        let r = input[idx * 2 + 1] + (input[next_idx * 2 + 1] - input[idx * 2 + 1]) * frac;
        out.push(l);
        out.push(r);
    }
    out
}

/// Reads device-native audio at `frame_idx` (0-based, in frames) as a
/// stereo `[l, r]` pair, regardless of the device's actual channel count.
/// Returns silence if `frame_idx` is out of bounds.
pub fn read_frame_as_stereo(data: &[f32], frame_idx: usize, channels: usize) -> [f32; 2] {
    if channels == 0 {
        return [0.0, 0.0];
    }
    let base = frame_idx * channels;
    if base + channels > data.len() {
        return [0.0, 0.0];
    }
    match channels {
        1 => [data[base], data[base]],
        _ => [data[base], data[base + 1]],
    }
}

/// Writes a stereo `[l, r]` frame into device-native audio at `frame_idx`,
/// averaging down to mono or zero-filling extra channels as needed.
pub fn write_stereo_frame(data: &mut [f32], frame_idx: usize, channels: usize, frame: [f32; 2]) {
    if channels == 0 {
        return;
    }
    let base = frame_idx * channels;
    if base + channels > data.len() {
        return;
    }
    match channels {
        1 => data[base] = (frame[0] + frame[1]) * 0.5,
        _ => {
            data[base] = frame[0];
            data[base + 1] = frame[1];
            for slot in &mut data[base + 2..base + channels] {
                *slot = 0.0;
            }
        }
    }
}

/// Streaming (allocation-free, per-frame) linear-interpolation resampler
/// used inside real-time audio callbacks to bridge between a device's
/// native sample rate and the engine's fixed 48kHz bus, in either
/// direction. See the module docs for why linear interpolation (not a
/// proper polyphase/sinc resampler) is an acceptable tradeoff here.
pub struct StreamResampler {
    ratio: f64,
    frac: f64,
    prev: [f32; 2],
    curr: [f32; 2],
}

impl StreamResampler {
    pub fn new(from_rate: u32, to_rate: u32) -> Self {
        Self {
            ratio: from_rate as f64 / to_rate.max(1) as f64,
            frac: 0.0,
            prev: [0.0; 2],
            curr: [0.0; 2],
        }
    }

    /// Produces the next output frame, pulling as many input frames as
    /// needed from `next_input` (which returns `[0.0, 0.0]` on underrun).
    pub fn next_frame(&mut self, mut next_input: impl FnMut() -> [f32; 2]) -> [f32; 2] {
        while self.frac >= 1.0 {
            self.prev = self.curr;
            self.curr = next_input();
            self.frac -= 1.0;
        }
        let t = self.frac as f32;
        let out = [
            self.prev[0] + (self.curr[0] - self.prev[0]) * t,
            self.prev[1] + (self.curr[1] - self.prev[1]) * t,
        ];
        self.frac += self.ratio;
        out
    }

    /// Number of output frames that correspond to `input_frames` at this
    /// resampler's rate ratio — used by the mic capture callback to know
    /// how many engine-rate frames to produce from one batch of raw input.
    pub fn estimate_output_frames(&self, input_frames: usize) -> usize {
        ((input_frames as f64) / self.ratio).round().max(0.0) as usize
    }
}
