//! settings.json and history.json in the app data dir.
use sendit_core::{write_atomic, Settings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const SETTINGS: &str = "settings.json";
pub const HISTORY: &str = "history.json";
pub const HISTORY_CAP: usize = 500;

/// settings.json, and what the Settings page reads and writes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub alias: String,
    pub download_dir: PathBuf,
    pub pin: Option<String>,
    pub close_to_tray: bool,
}

impl AppSettings {
    pub fn core(&self) -> Settings {
        Settings {
            alias: self.alias.clone(),
            download_dir: self.download_dir.clone(),
            pin: self.pin.clone(),
        }
    }

    pub fn from_core(s: Settings, close_to_tray: bool) -> Self {
        Self {
            alias: s.alias,
            download_dir: s.download_dir,
            pin: s.pin,
            close_to_tray,
        }
    }
}

/// A missing, unreadable or hand-broken file means `defaults`.
pub fn load_settings(path: &Path, defaults: AppSettings) -> AppSettings {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<AppSettings>(&b).ok())
        .filter(|s| !s.alias.trim().is_empty())
        .unwrap_or(defaults)
}

pub fn save_settings(path: &Path, s: &AppSettings) -> std::io::Result<()> {
    write_atomic(path, &serde_json::to_vec_pretty(s)?)
}

/// Saved tickets, oldest first. A file that can't be read or parsed is
/// moved to `history.json.bad` so the next save doesn't destroy it.
pub fn load_history(path: &Path) -> Vec<Value> {
    let parsed = match std::fs::read(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => Err(e.to_string()),
        Ok(b) => serde_json::from_slice::<Vec<Value>>(&b).map_err(|e| e.to_string()),
    };
    parsed.unwrap_or_else(|e| {
        eprintln!("history.json unreadable ({e}); keeping it as history.json.bad");
        let mut bad = path.as_os_str().to_owned();
        bad.push(".bad");
        let _ = std::fs::rename(path, PathBuf::from(bad));
        Vec::new()
    })
}

/// Keeps the newest `HISTORY_CAP` items (the list is oldest first).
pub fn save_history(path: &Path, mut items: Vec<Value>) -> std::io::Result<()> {
    if items.len() > HISTORY_CAP {
        items.drain(..items.len() - HISTORY_CAP);
    }
    write_atomic(path, &serde_json::to_vec(&items)?)
}

/// The saved download folder if it can still be used (an unplugged drive
/// can't), otherwise `fallback`.
pub fn usable_dir(saved: &Path, fallback: PathBuf) -> PathBuf {
    if std::fs::create_dir_all(saved).is_ok() {
        saved.to_path_buf()
    } else {
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn defaults() -> AppSettings {
        AppSettings {
            alias: "host".into(),
            download_dir: "D:/down".into(),
            pin: None,
            close_to_tray: true,
        }
    }

    #[test]
    fn settings_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(SETTINGS);
        let s = AppSettings {
            alias: "studio".into(),
            pin: Some("1234".into()),
            close_to_tray: false,
            ..defaults()
        };
        save_settings(&p, &s).unwrap();
        assert_eq!(load_settings(&p, defaults()), s);
        let raw = std::fs::read_to_string(&p).unwrap();
        assert!(raw.contains("\"closeToTray\": false"), "{raw}");
    }

    #[test]
    fn missing_or_corrupt_settings_use_defaults() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(SETTINGS);
        assert_eq!(load_settings(&p, defaults()), defaults());
        std::fs::write(&p, "{nope").unwrap();
        assert_eq!(load_settings(&p, defaults()), defaults());
    }

    #[test]
    fn blank_alias_in_file_uses_defaults() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(SETTINGS);
        save_settings(
            &p,
            &AppSettings {
                alias: "  ".into(),
                ..defaults()
            },
        )
        .unwrap();
        assert_eq!(load_settings(&p, defaults()), defaults());
    }

    #[test]
    fn history_round_trip_keeps_the_newest_500() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(HISTORY);
        let items: Vec<_> = (0..520).map(|i| json!({ "key": i })).collect();
        save_history(&p, items).unwrap();
        let back = load_history(&p);
        assert_eq!(back.len(), HISTORY_CAP);
        assert_eq!(back[0]["key"], 20);
        assert_eq!(back[499]["key"], 519);
    }

    #[test]
    fn missing_history_is_empty() {
        let d = tempfile::tempdir().unwrap();
        assert!(load_history(&d.path().join(HISTORY)).is_empty());
    }

    #[test]
    fn corrupt_history_is_kept_as_bad_and_starts_empty() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(HISTORY);
        std::fs::write(&p, "[{\"key\":").unwrap();
        assert!(load_history(&p).is_empty());
        assert!(!p.exists());
        assert_eq!(
            std::fs::read_to_string(d.path().join("history.json.bad")).unwrap(),
            "[{\"key\":"
        );
    }

    #[test]
    fn unusable_saved_folder_falls_back() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("f");
        std::fs::write(&file, "x").unwrap();
        let fallback = d.path().join("fallback");
        assert_eq!(usable_dir(&file.join("sub"), fallback.clone()), fallback);
        let ok = d.path().join("ok");
        assert_eq!(usable_dir(&ok, fallback), ok);
        assert!(ok.is_dir());
    }
}
