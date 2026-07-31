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
    /// Whether playback should apply each pad's cached
    /// `PadConfig::normalized_gain` (see `commands::trigger_pad`). Mirrors
    /// `AppSettings.normalize_volumes`, applied at startup and whenever the
    /// user toggles the "normalize volumes" setting.
    pub normalize_volumes: AtomicBool,
    /// Mirrors `AppSettings.launch_at_startup`. Kept in `AppState` (in
    /// addition to the OS-level registration itself) so
    /// `set_minimize_to_tray` can cheaply check it without re-reading
    /// settings.json.
    pub launch_at_startup: AtomicBool,
    /// Whether closing the main window should hide it to the system tray
    /// instead of quitting the app (see `lib.rs`'s window `CloseRequested`
    /// handler). Forced to `true` whenever `launch_at_startup` is enabled —
    /// otherwise an app that auto-launches at boot would have no way to be
    /// reached again after its first close.
    pub minimize_to_tray_on_close: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            midi: Mutex::new(MidiPort::new()),
            audio: Mutex::new(AudioEngine::new()),
            active_profile: Mutex::new(None),
            device_controller: Arc::new(Mutex::new(None)),
            playing_pads: Mutex::new(HashMap::new()),
            normalize_volumes: AtomicBool::new(false),
            launch_at_startup: AtomicBool::new(false),
            minimize_to_tray_on_close: AtomicBool::new(false),
        }
    }
}


