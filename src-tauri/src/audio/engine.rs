use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::error::{AppError, AppResult};

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
    /// Notifies when a `PlayForPad` sound finishes on its own (not via a
    /// manual `StopPad`). Taken exactly once by `take_finished_receiver`,
    /// which the app calls during setup to drive LED / playing-state cleanup.
    finished_rx: std::sync::Mutex<Option<mpsc::Receiver<String>>>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<AudioCommand>(32);
        let (finished_tx, finished_rx) = mpsc::channel::<String>();

        thread::Builder::new()
            .name("soundbox-audio".into())
            .spawn(move || audio_thread(rx, finished_tx))
            .expect("failed to spawn audio thread");

        Self {
            sender: tx,
            finished_rx: std::sync::Mutex::new(Some(finished_rx)),
        }
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

fn audio_thread(rx: mpsc::Receiver<AudioCommand>, finished_tx: mpsc::Sender<String>) {
    let Ok((_stream, handle)): Result<(OutputStream, OutputStreamHandle), _> =
        OutputStream::try_default()
    else {
        tracing::error!("Failed to open default audio output device");
        return;
    };

    let mut active_sinks: Vec<Sink> = Vec::new();
    let mut pad_sinks: HashMap<String, Sink> = HashMap::new();

    loop {
        let cmd = match rx.recv_timeout(Duration::from_millis(80)) {
            Ok(cmd) => Some(cmd),
            Err(RecvTimeoutError::Timeout) => None,
            // Sender was dropped (app shutdown) — exit cleanly.
            Err(RecvTimeoutError::Disconnected) => break,
        };

        match cmd {
            Some(AudioCommand::PlayFile { path, volume }) => {
                match File::open(&path) {
                    Ok(file) => {
                        let reader = BufReader::new(file);
                        match Decoder::new(reader) {
                            Ok(source) => match Sink::try_new(&handle) {
                                Ok(sink) => {
                                    sink.set_volume(volume.clamp(0.0, 2.0));
                                    sink.append(source);
                                    active_sinks.push(sink);
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
                if let Some(old_sink) = pad_sinks.remove(&pad_id) {
                    old_sink.stop();
                }
                match File::open(&path) {
                    Ok(file) => {
                        let reader = BufReader::new(file);
                        match Decoder::new(reader) {
                            Ok(source) => match Sink::try_new(&handle) {
                                Ok(sink) => {
                                    sink.set_volume(volume.clamp(0.0, 2.0));
                                    sink.append(source);
                                    pad_sinks.insert(pad_id, sink);
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
                if let Some(sink) = pad_sinks.remove(&pad_id) {
                    sink.stop();
                }
            }
            Some(AudioCommand::StopAll) => {
                active_sinks.iter().for_each(|s| s.stop());
                active_sinks.clear();
                pad_sinks.values().for_each(|s| s.stop());
                pad_sinks.clear();
            }
            None => {}
        }

        // Prune finished generic sinks, and notify + prune pad sinks that
        // finished on their own (a manual `StopPad` already removed its
        // entry above, so this only fires for natural completion).
        active_sinks.retain(|s| !s.empty());
        pad_sinks.retain(|pad_id, sink| {
            if sink.empty() {
                let _ = finished_tx.send(pad_id.clone());
                false
            } else {
                true
            }
        });
    }
}

