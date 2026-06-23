use cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::sync::mpsc;
use std::thread;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

enum AudioCommand {
    PlayFile { path: String, volume: f32 },
    StopAll,
}

/// Audio engine backed by a dedicated OS thread that owns the `OutputStream`.
/// This design ensures `AudioEngine` is `Send + Sync` regardless of whether
/// `OutputStream` itself implements those traits.
pub struct AudioEngine {
    sender: mpsc::SyncSender<AudioCommand>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<AudioCommand>(32);

        thread::Builder::new()
            .name("soundbox-audio".into())
            .spawn(move || audio_thread(rx))
            .expect("failed to spawn audio thread");

        Self { sender: tx }
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

    pub fn stop_all_sounds(&self) -> AppResult<()> {
        self.sender
            .send(AudioCommand::StopAll)
            .map_err(|e| AppError::Audio(e.to_string()))
    }
}

fn audio_thread(rx: mpsc::Receiver<AudioCommand>) {
    let Ok((_stream, handle)): Result<(OutputStream, OutputStreamHandle), _> =
        OutputStream::try_default()
    else {
        tracing::error!("Failed to open default audio output device");
        return;
    };

    let mut active_sinks: Vec<Sink> = Vec::new();

    loop {
        match rx.recv() {
            Ok(AudioCommand::PlayFile { path, volume }) => {
                // Prune finished sinks to avoid unbounded growth.
                active_sinks.retain(|s| !s.empty());

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
            Ok(AudioCommand::StopAll) => {
                active_sinks.iter().for_each(|s| s.stop());
                active_sinks.clear();
            }
            // Sender was dropped (app shutdown) — exit cleanly.
            Err(_) => break,
        }
    }
}
