use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadConfig {
    pub note: u8,
    pub label: String,
    pub color: String,
    pub sound_path: Option<String>,
    pub volume: f32,
}

impl Default for PadConfig {
    fn default() -> Self {
        Self {
            note: 0,
            label: String::new(),
            color: "#1F2937".to_string(),
            sound_path: None,
            volume: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub device_name: String,
    pub rows: u8,
    pub cols: u8,
    pub pads: Vec<PadConfig>,
}

impl Profile {
    pub fn new(name: String, device_name: String, rows: u8, cols: u8) -> Self {
        let pad_count = rows as usize * cols as usize;
        Self {
            id: Uuid::new_v4().to_string(),
            name,
            device_name,
            rows,
            cols,
            pads: (0..pad_count)
                .map(|i| PadConfig {
                    note: i as u8,
                    ..Default::default()
                })
                .collect(),
        }
    }
}
