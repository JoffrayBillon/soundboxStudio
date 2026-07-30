use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::audio::AudioEngine;
use crate::midi::controller::MidiPort;
use crate::midi::traits::MidiController;
use crate::profile::Profile;

pub struct AppState {
    pub midi: Mutex<MidiPort>,
    pub audio: Mutex<AudioEngine>,
    pub active_profile: Mutex<Option<Profile>>,
    /// Shared with the MIDI input callback so the callback can translate
    /// raw messages into `DeviceAction` events without extra indirection.
    pub device_controller: Arc<Mutex<Option<Box<dyn MidiController>>>>,
    /// Pads currently playing a sound, keyed by pad id. Each flag signals
    /// that pad's LED-blink thread to stop (set to `false` on a manual
    /// re-click or once the sound finishes naturally); presence in this map
    /// is also the source of truth for "is this pad currently playing".
    pub playing_pads: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            midi: Mutex::new(MidiPort::new()),
            audio: Mutex::new(AudioEngine::new()),
            active_profile: Mutex::new(None),
            device_controller: Arc::new(Mutex::new(None)),
            playing_pads: Mutex::new(HashMap::new()),
        }
    }
}

