use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppSettings {
    pub merge_open_after: bool,
    pub split_open_after: bool,
    pub compress_open_after: bool,
    pub split_at_page: i32,
    pub compress_quality: i32,
    pub last_dir: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            merge_open_after: false,
            split_open_after: true,
            compress_open_after: true,
            split_at_page: 1,
            compress_quality: 80,
            last_dir: home,
        }
    }
}

impl AppSettings {
    fn get_path() -> PathBuf {
        let config_base = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                PathBuf::from(home).join(".config")
            });

        let dir = config_base.join("pdftools");
        if !dir.exists() {
            let _ = fs::create_dir_all(&dir);
        }
        dir.join("settings.json")
    }

    pub fn load() -> Self {
        let p = Self::get_path();
        if p.exists() {
            if let Ok(data) = fs::read_to_string(&p) {
                if let Ok(settings) = serde_json::from_str(&data) {
                    return settings;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let p = Self::get_path();
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(p, json);
        }
    }
}
