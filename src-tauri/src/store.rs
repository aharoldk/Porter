use crate::browsers::Browser;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub label: String,
    pub url: String,
    pub browser_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Store {
    #[serde(default)]
    pub browsers: Vec<Browser>,
    #[serde(default)]
    pub presets: Vec<Preset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_browser: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
}

pub fn config_file(dir: &Path) -> PathBuf {
    dir.join("porter.json")
}

pub fn load(path: &Path) -> Store {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, store: &Store) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let text = serde_json::to_string_pretty(store).map_err(|err| err.to_string())?;
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    fs::write(&tmp, text).map_err(|err| err.to_string())?;
    fs::rename(&tmp, path).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        err.to_string()
    })
}

pub fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_round_trips_through_tmp_file() {
        let dir = std::env::temp_dir().join(format!("porter-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = config_file(&dir);
        let store = Store {
            browsers: vec![crate::browsers::Browser {
                id: "b1".into(),
                name: "Chrome".into(),
                kind: crate::browsers::BrowserKind::Chromium,
                executable: "/bin/true".into(),
                user_data_dir: None,
            }],
            presets: vec![Preset {
                id: "p1".into(),
                label: "GitLab".into(),
                url: "https://gitlab.com".into(),
                browser_id: "b1".into(),
                profile: None,
            }],
            default_browser: Some("b1".into()),
            default_profile: None,
        };
        save(&path, &store).unwrap();

        let mut tmp_name = path.file_name().unwrap().to_os_string();
        tmp_name.push(".tmp");
        assert!(!path.with_file_name(tmp_name).exists(), "tmp file left behind");

        let loaded = load(&path);
        assert_eq!(loaded.browsers.len(), 1);
        assert_eq!(loaded.presets.len(), 1);
        assert_eq!(loaded.default_browser.as_deref(), Some("b1"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
