use parking_lot::Mutex;

use crate::audio::AudioEngine;
use crate::midi::MidiController;
use crate::profile::Profile;

pub struct AppState {
    pub midi: Mutex<MidiController>,
    pub audio: Mutex<AudioEngine>,
    pub active_profile: Mutex<Option<Profile>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            midi: Mutex::new(MidiController::new()),
            audio: Mutex::new(AudioEngine::new()),
            active_profile: Mutex::new(None),
        }
    }
}
