use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::error::{AppError, AppResult};

/// Clamp applied to every per-sound volume/gain before it reaches rodio —
/// generous enough to let normalization boost a very quiet recording, but
/// not so generous that a bad value could blast the output unexpectedly.
const VOLUME_CLAMP: std::ops::RangeInclusive<f32> = 0.0..=2.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

enum AudioCommand {
    PlayFile { path: String, volume: f32 },
    /// Plays a file as the tracked sound for a given pad, replacing whatever
    /// was previously playing on that same pad (if anything). Unlike
    /// `PlayFile`, completion is watched so `AudioEngine`'s finished-pad
    /// channel can notify when it ends naturally (see `take_finished_receiver`).
    PlayForPad {
        pad_id: String,
        path: String,
        volume: f32,
    },
    /// Immediately stops the sound tracked for `pad_id`, if any.
    StopPad { pad_id: String },
    StopAll,
}

/// Audio engine backed by a dedicated OS thread that owns the `OutputStream`.
/// This design ensures `AudioEngine` is `Send + Sync` regardless of whether
/// `OutputStream` itself implements those traits.
pub struct AudioEngine {
    sender: mpsc::SyncSender<AudioCommand>,
    /// Global output gain multiplier (1.0 = unity), stored as `f32::to_bits`
    /// so it can be read/written from any thread without a lock. Shared with
    /// the audio thread, which re-applies it to every currently-playing sink
    /// (not just future ones) whenever it changes.
    master_volume: Arc<AtomicU32>,
    /// Notifies when a `PlayForPad` sound finishes on its own (not via a
    /// manual `StopPad`). Taken exactly once by `take_finished_receiver`,
    /// which the app calls during setup to drive LED / playing-state cleanup.
    finished_rx: std::sync::Mutex<Option<mpsc::Receiver<String>>>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<AudioCommand>(32);
        let (finished_tx, finished_rx) = mpsc::channel::<String>();
        let master_volume = Arc::new(AtomicU32::new(1.0f32.to_bits()));

        let thread_master_volume = Arc::clone(&master_volume);
        thread::Builder::new()
            .name("soundbox-audio".into())
            .spawn(move || audio_thread(rx, finished_tx, thread_master_volume))
            .expect("failed to spawn audio thread");

        Self {
            sender: tx,
            master_volume,
            finished_rx: std::sync::Mutex::new(Some(finished_rx)),
        }
    }

    /// Sets the global output gain multiplier applied on top of every
    /// pad/sound's own volume. Takes effect immediately for both already-
    /// playing sounds (rescaled by the audio thread within one tick, ~80ms)
    /// and anything played afterwards.
    pub fn set_master_volume(&self, volume: f32) {
        let clamped = volume.clamp(*VOLUME_CLAMP.start(), *VOLUME_CLAMP.end());
        self.master_volume.store(clamped.to_bits(), Ordering::Relaxed);
    }

    pub fn list_output_devices() -> AppResult<Vec<AudioDevice>> {
        let host = cpal::default_host();

        let default_name = host
            .default_output_device()
            .and_then(|d| d.name().ok())
            .unwrap_or_default();

        let devices = host
            .output_devices()
            .map_err(|e| AppError::Audio(e.to_string()))?
            .filter_map(|d| {
                d.name().ok().map(|name| AudioDevice {
                    is_default: name == default_name,
                    id: name.clone(),
                    name,
                })
            })
            .collect();

        Ok(devices)
    }

    pub fn play_sound_file(&self, file_path: &str, volume: f32) -> AppResult<()> {
        self.sender
            .send(AudioCommand::PlayFile {
                path: file_path.to_owned(),
                volume,
            })
            .map_err(|e| AppError::Audio(e.to_string()))
    }

    /// Plays `file_path` as the tracked sound for `pad_id`. If `pad_id` was
    /// already playing something, that sound is stopped first — a pad only
    /// ever has one sound in flight at a time.
    pub fn play_for_pad(&self, pad_id: String, file_path: &str, volume: f32) -> AppResult<()> {
        self.sender
            .send(AudioCommand::PlayForPad {
                pad_id,
                path: file_path.to_owned(),
                volume,
            })
            .map_err(|e| AppError::Audio(e.to_string()))
    }

    /// Stops the sound currently tracked for `pad_id`, if any.
    pub fn stop_pad(&self, pad_id: String) -> AppResult<()> {
        self.sender
            .send(AudioCommand::StopPad { pad_id })
            .map_err(|e| AppError::Audio(e.to_string()))
    }

    pub fn stop_all_sounds(&self) -> AppResult<()> {
        self.sender
            .send(AudioCommand::StopAll)
            .map_err(|e| AppError::Audio(e.to_string()))
    }

    /// Takes ownership of the "pad finished naturally" notification channel.
    /// Returns `None` if already taken. Meant to be called exactly once,
    /// during app setup, to start the completion watcher thread.
    pub fn take_finished_receiver(&self) -> Option<mpsc::Receiver<String>> {
        self.finished_rx.lock().ok()?.take()
    }
}

fn audio_thread(
    rx: mpsc::Receiver<AudioCommand>,
    finished_tx: mpsc::Sender<String>,
    master_volume: Arc<AtomicU32>,
) {
    let Ok((_stream, handle)): Result<(OutputStream, OutputStreamHandle), _> =
        OutputStream::try_default()
    else {
        tracing::error!("Failed to open default audio output device");
        return;
    };

    // Sinks are kept alongside their own *base* volume (pre-master-gain) so
    // that whenever the master volume changes, every currently-playing sink
    // can be rescaled from its original value instead of compounding gains.
    let mut active_sinks: Vec<(Sink, f32)> = Vec::new();
    let mut pad_sinks: HashMap<String, (Sink, f32)> = HashMap::new();
    let mut last_master = f32::from_bits(master_volume.load(Ordering::Relaxed));

    loop {
        let cmd = match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(cmd) => Some(cmd),
            Err(RecvTimeoutError::Timeout) => None,
            // Sender was dropped (app shutdown) — exit cleanly.
            Err(RecvTimeoutError::Disconnected) => break,
        };

        let master = f32::from_bits(master_volume.load(Ordering::Relaxed));

        match cmd {
            Some(AudioCommand::PlayFile { path, volume }) => {
                let base = volume.clamp(*VOLUME_CLAMP.start(), *VOLUME_CLAMP.end());
                match File::open(&path) {
                    Ok(file) => {
                        let reader = BufReader::new(file);
                        match Decoder::new(reader) {
                            Ok(source) => match Sink::try_new(&handle) {
                                Ok(sink) => {
                                    sink.set_volume(scaled_volume(base, master));
                                    sink.append(source);
                                    active_sinks.push((sink, base));
                                }
                                Err(e) => tracing::error!("Sink error: {e}"),
                            },
                            Err(e) => tracing::warn!("Decode error for {path}: {e}"),
                        }
                    }
                    Err(e) => tracing::warn!("Cannot open audio file {path}: {e}"),
                }
            }
            Some(AudioCommand::PlayForPad { pad_id, path, volume }) => {
                if let Some((old_sink, _)) = pad_sinks.remove(&pad_id) {
                    old_sink.stop();
                }
                let base = volume.clamp(*VOLUME_CLAMP.start(), *VOLUME_CLAMP.end());
                match File::open(&path) {
                    Ok(file) => {
                        let reader = BufReader::new(file);
                        match Decoder::new(reader) {
                            Ok(source) => match Sink::try_new(&handle) {
                                Ok(sink) => {
                                    sink.set_volume(scaled_volume(base, master));
                                    sink.append(source);
                                    pad_sinks.insert(pad_id, (sink, base));
                                }
                                Err(e) => tracing::error!("Sink error: {e}"),
                            },
                            Err(e) => tracing::warn!("Decode error for {path}: {e}"),
                        }
                    }
                    Err(e) => tracing::warn!("Cannot open audio file {path}: {e}"),
                }
            }
            Some(AudioCommand::StopPad { pad_id }) => {
                if let Some((sink, _)) = pad_sinks.remove(&pad_id) {
                    sink.stop();
                }
            }
            Some(AudioCommand::StopAll) => {
                active_sinks.iter().for_each(|(s, _)| s.stop());
                active_sinks.clear();
                pad_sinks.values().for_each(|(s, _)| s.stop());
                pad_sinks.clear();
            }
            None => {}
        }

        // The master volume can change between ticks even without a new
        // play/stop command (the frontend calls `set_master_volume`
        // directly, bypassing this channel) — rescale every currently
        // playing sink from its stored base volume when that happens, so a
        // slider drag is heard immediately instead of only affecting the
        // *next* sound played.
        if master != last_master {
            for (sink, base) in active_sinks.iter() {
                sink.set_volume(scaled_volume(*base, master));
            }
            for (sink, base) in pad_sinks.values() {
                sink.set_volume(scaled_volume(*base, master));
            }
            last_master = master;
        }

        // Prune finished generic sinks, and notify + prune pad sinks that
        // finished on their own (a manual `StopPad` already removed its
        // entry above, so this only fires for natural completion).
        active_sinks.retain(|(s, _)| !s.empty());
        pad_sinks.retain(|pad_id, (sink, _)| {
            if sink.empty() {
                let _ = finished_tx.send(pad_id.clone());
                false
            } else {
                true
            }
        });
    }
}

fn scaled_volume(base: f32, master: f32) -> f32 {
    // A hard ceiling above the individual 0..=2.0 clamps applied to `base`
    // and `master` on their own — two maxed-out sliders multiplying
    // together (2.0 * 2.0) shouldn't be allowed to reach 4x gain in
    // practice, but this still leaves headroom above unity for the common
    // case of one control being boosted.
    (base * master).clamp(0.0, 3.0)
}

/// Target peak amplitude (relative to full scale, 0.0–1.0) that
/// `analyze_normalization_gain` tries to bring each sound's loudest point
/// to. Deliberately below 1.0 to leave a little headroom so a normalized
/// sound doesn't clip if the master volume slider is later pushed above
/// unity.
const NORMALIZE_TARGET_PEAK: f32 = 0.8;
/// Gains outside this range aren't useful in practice — it would take an
/// enormous multiplier to make near-silence audible (almost always an
/// empty/corrupt/very-quiet file, not something worth blasting), and
/// nothing should ever need to be turned down to a twentieth of its
/// original volume just to match its peers.
const NORMALIZE_GAIN_RANGE: std::ops::RangeInclusive<f32> = 0.2..=4.0;

/// Decodes `path` fully and computes the per-sound gain that would bring
/// its loudest sample up (or down) to `NORMALIZE_TARGET_PEAK`, so quieter
/// and louder source recordings play back at a more consistent perceived
/// loudness once "normalize volumes" is enabled. The result is meant to be
/// cached (see `PadConfig::normalized_gain`) rather than recomputed on every
/// play. Returns unity (no adjustment) for near-silent files instead of
/// applying an extreme gain to what's likely silence or noise.
pub fn analyze_normalization_gain(path: &str) -> AppResult<f32> {
    let file = File::open(path).map_err(|e| AppError::Audio(e.to_string()))?;
    let reader = BufReader::new(file);
    let decoder = Decoder::new(reader).map_err(|e| AppError::Audio(e.to_string()))?;

    let peak = decoder
        .map(|sample| (sample as f32 / i16::MAX as f32).abs())
        .fold(0.0_f32, f32::max);

    if peak < 0.001 {
        return Ok(1.0);
    }

    let gain = NORMALIZE_TARGET_PEAK / peak;
    Ok(gain.clamp(*NORMALIZE_GAIN_RANGE.start(), *NORMALIZE_GAIN_RANGE.end()))
}

