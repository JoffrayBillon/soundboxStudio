pub mod schema;
pub use schema::{GridPadSpec, Profile};

use std::fs;
use std::path::PathBuf;

use crate::error::{AppError, AppResult};

fn profiles_dir() -> AppResult<PathBuf> {
    let dir = dirs::data_local_dir()
        .ok_or_else(|| AppError::Io("Cannot determine local data directory".into()))?
        .join("soundbox-studio")
        .join("profiles");

    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| AppError::Io(e.to_string()))?;
    }

    Ok(dir)
}

pub fn save_profile(profile: &Profile) -> AppResult<()> {
    let path = profiles_dir()?.join(format!("{}.json", profile.id));
    let json = serde_json::to_string_pretty(profile)?;
    fs::write(path, json).map_err(|e| AppError::Io(e.to_string()))
}

pub fn load_profile(id: &str) -> AppResult<Profile> {
    let path = profiles_dir()?.join(format!("{id}.json"));
    let json = fs::read_to_string(&path).map_err(|e| AppError::Io(e.to_string()))?;
    serde_json::from_str(&json).map_err(|e| AppError::Profile(e.to_string()))
}

pub fn list_profiles() -> AppResult<Vec<Profile>> {
    let dir = profiles_dir()?;
    let mut profiles = Vec::new();

    for entry in fs::read_dir(&dir)
        .map_err(|e| AppError::Io(e.to_string()))?
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Ok(json) = fs::read_to_string(&path) {
                if let Ok(profile) = serde_json::from_str::<Profile>(&json) {
                    profiles.push(profile);
                }
            }
        }
    }

    Ok(profiles)
}

pub fn delete_profile(id: &str) -> AppResult<()> {
    let path = profiles_dir()?.join(format!("{id}.json"));
    fs::remove_file(path).map_err(|e| AppError::Io(e.to_string()))
}
