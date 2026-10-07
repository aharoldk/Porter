use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrowserKind {
    Chromium,
    Firefox,
    Safari,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    pub id: String,
    pub name: String,
    pub kind: BrowserKind,
    pub executable: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_data_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
}

pub struct LaunchPlan {
    pub program: String,
    pub args: Vec<String>,
}

pub fn validate_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    let normalized = if url.starts_with("github:") {
        let rest = url[7..].trim_start_matches('/');
        format!("https://github.com/{}", rest)
    } else if url.starts_with("github://") {
        let rest = url[9..].trim_start_matches('/');
        format!("https://github.com/{}", rest)
    } else if url.starts_with("gh:") {
        let rest = url[3..].trim_start_matches('/');
        format!("https://github.com/{}", rest)
    } else if url.starts_with("gh://") {
        let rest = url[5..].trim_start_matches('/');
        format!("https://github.com/{}", rest)
    } else {
        url.to_string()
    };

    if !(normalized.starts_with("https://") || normalized.starts_with("http://")) {
        return Err("URL must start with http:// or https:// or github: or gh:".into());
    }
    if normalized.chars().any(|c| c.is_control()) {
        return Err("URL has invalid characters".into());
    }
    Ok(())
}

pub fn infer_kind(executable: &str) -> BrowserKind {
    let lower = executable.to_lowercase();
    if lower.contains("firefox") || lower.contains("librewolf") || lower.contains("zen") {
        BrowserKind::Firefox
    } else if lower.contains("safari") {
        BrowserKind::Safari
    } else {
        BrowserKind::Chromium
    }
}

pub fn pretty_name(executable: &str) -> String {
    let lower = executable.to_lowercase();
    if lower.contains("helium") {
        return "Helium".into();
    }
    if lower.contains("chrome-beta") {
        return "Google Chrome Beta".into();
    }
    if lower.contains("chrome-unstable") || lower.contains("chrome-dev") {
        return "Google Chrome Dev".into();
    }
    if lower.contains("chrome") {
        return "Google Chrome".into();
    }
    if lower.contains("chromium") {
        return "Chromium".into();
    }
    if lower.contains("brave") {
        return "Brave".into();
    }
    if lower.contains("edge-dev") {
        return "Microsoft Edge Dev".into();
    }
    if lower.contains("edge-beta") {
        return "Microsoft Edge Beta".into();
    }
    if lower.contains("edge") || lower.contains("msedge") {
        return "Microsoft Edge".into();
    }
    if lower.contains("vivaldi") {
        return "Vivaldi".into();
    }
    if lower.contains("librewolf") {
        return "LibreWolf".into();
    }
    if lower.contains("zen") {
        return "Zen".into();
    }
    if lower.contains("firefox") {
        return "Firefox".into();
    }
    if lower.contains("safari") {
        return "Safari".into();
    }
    Path::new(&lower)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Browser")
        .to_string()
}

pub fn guess_user_data_dir(executable: &str, kind: BrowserKind) -> Option<PathBuf> {
    if kind == BrowserKind::Safari {
        return None;
    }
    let marker = if kind == BrowserKind::Firefox {
        "profiles.ini"
    } else {
        "Local State"
    };
    data_dir_candidates(executable, kind)
        .into_iter()
        .find(|dir| dir.join(marker).is_file())
        .or_else(|| {
            data_dir_candidates(executable, kind)
                .into_iter()
                .find(|dir| dir.is_dir())
        })
}

fn data_dir_candidates(executable: &str, kind: BrowserKind) -> Vec<PathBuf> {
    if kind == BrowserKind::Firefox {
        return firefox_dirs(executable);
    }
    let mut dirs = Vec::new();
    for rel in chromium_rels(executable) {
        for home in config_homes(executable) {
            dirs.push(home.join(&rel));
        }
    }
    dirs
}

fn chromium_rels(executable: &str) -> Vec<String> {
    let lower = executable.to_lowercase();
    if cfg!(windows) {
        let rel = if lower.contains("helium") {
            r"net.imput.helium\User Data"
        } else if lower.contains("chrome-beta") {
            r"Google\Chrome Beta\User Data"
        } else if lower.contains("chrome-unstable") || lower.contains("chrome-dev") {
            r"Google\Chrome Dev\User Data"
        } else if lower.contains("chrome") {
            r"Google\Chrome\User Data"
        } else if lower.contains("chromium") {
            r"Chromium\User Data"
        } else if lower.contains("brave") {
            r"BraveSoftware\Brave-Browser\User Data"
        } else if lower.contains("edge-dev") {
            r"Microsoft\Edge Dev\User Data"
        } else if lower.contains("edge-beta") {
            r"Microsoft\Edge Beta\User Data"
        } else if lower.contains("edge") || lower.contains("msedge") {
            r"Microsoft\Edge\User Data"
        } else if lower.contains("vivaldi") {
            r"Vivaldi\User Data"
        } else {
            return Vec::new();
        };
        return vec![rel.into()];
    }
    if cfg!(target_os = "macos") {
        let rel = if lower.contains("helium") {
            "net.imput.helium"
        } else if lower.contains("chrome") {
            "Google/Chrome"
        } else if lower.contains("chromium") {
            "Chromium"
        } else if lower.contains("brave") {
            "BraveSoftware/Brave-Browser"
        } else if lower.contains("edge") {
            "Microsoft Edge"
        } else if lower.contains("vivaldi") {
            "Vivaldi"
        } else {
            return Vec::new();
        };
        return vec![rel.into()];
    }
    let mut rels = Vec::new();
    if lower.contains("helium") {
        rels.push("net.imput.helium".into());
    } else if lower.contains("chrome-beta") {
        rels.push("google-chrome-beta".into());
    } else if lower.contains("chrome-unstable") {
        rels.push("google-chrome-unstable".into());
    } else if lower.contains("chrome") {
        rels.push("google-chrome".into());
    } else if lower.contains("chromium") {
        rels.push("chromium".into());
    } else if lower.contains("brave") {
        rels.push("BraveSoftware/Brave-Browser".into());
    } else if lower.contains("edge-dev") {
        rels.push("microsoft-edge-dev".into());
    } else if lower.contains("edge-beta") {
        rels.push("microsoft-edge-beta".into());
    } else if lower.contains("edge") {
        rels.push("microsoft-edge".into());
    } else if lower.contains("vivaldi") {
        rels.push("vivaldi".into());
    }
    rels
}

fn firefox_dirs(executable: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = home_dir() {
        let lower = executable.to_lowercase();
        if lower.contains("zen") {
            dirs.push(home.join(".var/app/app.zen_browser.zen/.zen"));
            dirs.push(home.join(".zen"));
            dirs.push(home.join("Library/Application Support/zen"));
        } else {
            dirs.push(home.join(".mozilla/firefox"));
            dirs.push(home.join("snap/firefox/common/.mozilla/firefox"));
            dirs.push(home.join(".var/app/org.mozilla.firefox/.mozilla/firefox"));
            dirs.push(home.join("Library/Application Support/Firefox"));
        }
    }
    if let Some(appdata) = std::env::var_os("APPDATA") {
        dirs.push(PathBuf::from(appdata).join("Mozilla/Firefox"));
    }
    dirs
}

fn config_homes(executable: &str) -> Vec<PathBuf> {
    let mut homes = Vec::new();
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            homes.push(PathBuf::from(local));
        }
        return homes;
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = home_dir() {
            homes.push(home.join("Library/Application Support"));
        }
        return homes;
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        homes.push(PathBuf::from(xdg));
    }
    if let Some(home) = home_dir() {
        homes.push(home.join(".config"));
        if executable.to_lowercase().contains("vivaldi") {
            homes.push(home.join(".var/app/com.vivaldi.Vivaldi/config"));
        }
    }
    homes
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

pub fn profiles_in(kind: BrowserKind, user_data_dir: Option<&str>) -> Vec<Profile> {
    let Some(dir) = user_data_dir.map(PathBuf::from) else {
        return Vec::new();
    };
    match kind {
        BrowserKind::Chromium => chromium_profiles(&dir),
        BrowserKind::Firefox => firefox_profiles(&dir),
        BrowserKind::Safari => Vec::new(),
    }
}

fn chromium_profiles(dir: &Path) -> Vec<Profile> {
    let local_state = dir.join("Local State");
    if let Ok(text) = std::fs::read_to_string(local_state) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(cache) = value
                .pointer("/profile/info_cache")
                .and_then(|v| v.as_object())
            {
                let mut profiles: Vec<Profile> = cache
                    .iter()
                    .filter(|(id, _)| id.as_str() != "System Profile")
                    .map(|(id, info)| Profile {
                        id: id.clone(),
                        name: info
                            .get("name")
                            .and_then(|n| n.as_str())
                            .filter(|n| !n.is_empty())
                            .unwrap_or(id)
                            .to_string(),
                    })
                    .collect();
                profiles.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                if !profiles.is_empty() {
                    return profiles;
                }
            }
        }
    }
    let mut profiles = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let looks_like = name == "Default" || name.starts_with("Profile ");
            if looks_like && entry.path().join("Preferences").is_file() {
                profiles.push(Profile {
                    id: name.clone(),
                    name,
                });
            }
        }
    }
    profiles.sort_by(|a, b| a.name.cmp(&b.name));
    profiles
}

fn firefox_profiles(dir: &Path) -> Vec<Profile> {
    let Ok(text) = std::fs::read_to_string(dir.join("profiles.ini")) else {
        return Vec::new();
    };
    let mut profiles = Vec::new();
    let mut in_profile = false;
    let mut name: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix('[') {
            if let Some(found) = name.take() {
                profiles.push(Profile {
                    id: found.clone(),
                    name: found,
                });
            }
            in_profile = rest.to_ascii_lowercase().starts_with("profile");
            continue;
        }
        if in_profile {
            if let Some(value) = line.strip_prefix("Name=") {
                let value = value.trim();
                if !value.is_empty() {
                    name = Some(value.to_string());
                }
            }
        }
    }
    if let Some(found) = name {
        profiles.push(Profile {
            id: found.clone(),
            name: found,
        });
    }
    profiles
}

pub fn plan(browser: &Browser, profile: Option<&str>, url: &str) -> Result<LaunchPlan, String> {
    validate_url(url)?;
    let profile = profile.map(str::trim).filter(|p| !p.is_empty());
    if browser.kind == BrowserKind::Safari {
        return Ok(LaunchPlan {
            program: "open".into(),
            args: vec!["-a".into(), "Safari".into(), url.trim().into()],
        });
    }
    if !Path::new(&browser.executable).exists() {
        return Err(format!("Can't find {}", browser.executable));
    }
    let mut args = Vec::new();
    match browser.kind {
        BrowserKind::Chromium => {
            if let Some(dir) = browser.user_data_dir.as_deref() {
                args.push(format!("--user-data-dir={dir}"));
            }
            if let Some(profile) = profile {
                args.push(format!("--profile-directory={profile}"));
            }
            args.push(url.trim().into());
        }
        BrowserKind::Firefox => {
            if let Some(profile) = profile {
                args.push("-P".into());
                args.push(profile.into());
            }
            args.push("-new-instance".into());
            args.push(url.trim().into());
        }
        BrowserKind::Safari => unreachable!(),
    }
    Ok(LaunchPlan {
        program: browser.executable.clone(),
        args,
    })
}

pub fn launch(browser: &Browser, profile: Option<&str>, url: &str) -> Result<(), String> {
    let plan = plan(browser, profile, url)?;
    std::process::Command::new(plan.program)
        .args(plan.args)
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

struct Seed {
    name: &'static str,
    kind: BrowserKind,
    bins: &'static [&'static str],
}

pub fn detect_installed() -> Vec<Browser> {
    let mut found = Vec::new();
    let mut seen = Vec::new();
    let mut consider = |name: String, kind: BrowserKind, executable: String| {
        if seen.iter().any(|item: &String| item == &executable) {
            return;
        }
        if !usable(Path::new(&executable)) {
            return;
        }
        seen.push(executable.clone());
        let user_data_dir =
            guess_user_data_dir(&executable, kind).map(|dir| dir.to_string_lossy().into_owned());
        found.push(Browser {
            id: String::new(),
            name,
            kind,
            executable,
            user_data_dir,
        });
    };

    for seed in seeds() {
        if let Some(path) = seed.bins.iter().copied().find(|path| usable(Path::new(path))) {
            consider(seed.name.into(), seed.kind, path.into());
        }
    }
    for (name, kind, path) in windows_bins() {
        consider(name, kind, path);
    }
    found
}

fn usable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    if path == Path::new("/usr/bin/firefox") {
        return Path::new("/snap/bin/firefox").exists()
            || Path::new("/usr/lib/firefox/firefox").exists();
    }
    true
}

fn seeds() -> Vec<Seed> {
    vec![
        Seed {
            name: "Helium",
            kind: BrowserKind::Chromium,
            bins: &[
                "/usr/bin/helium",
                "/opt/helium/helium",
                "/Applications/Helium.app/Contents/MacOS/Helium",
            ],
        },
        Seed {
            name: "Google Chrome",
            kind: BrowserKind::Chromium,
            bins: &[
                "/usr/bin/google-chrome",
                "/usr/bin/google-chrome-stable",
                "/opt/google/chrome/google-chrome",
                "/var/lib/flatpak/exports/bin/com.google.Chrome",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            ],
        },
        Seed {
            name: "Chromium",
            kind: BrowserKind::Chromium,
            bins: &[
                "/usr/bin/chromium",
                "/usr/bin/chromium-browser",
                "/snap/bin/chromium",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
            ],
        },
        Seed {
            name: "Brave",
            kind: BrowserKind::Chromium,
            bins: &[
                "/usr/bin/brave-browser",
                "/usr/bin/brave",
                "/opt/brave.com/brave/brave",
                "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
            ],
        },
        Seed {
            name: "Microsoft Edge",
            kind: BrowserKind::Chromium,
            bins: &[
                "/usr/bin/microsoft-edge",
                "/usr/bin/microsoft-edge-stable",
                "/opt/microsoft/msedge/microsoft-edge",
                "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
            ],
        },
        Seed {
            name: "Vivaldi",
            kind: BrowserKind::Chromium,
            bins: &[
                "/usr/bin/vivaldi",
                "/usr/bin/vivaldi-stable",
                "/var/lib/flatpak/exports/bin/com.vivaldi.Vivaldi",
                "/Applications/Vivaldi.app/Contents/MacOS/Vivaldi",
            ],
        },
        Seed {
            name: "Firefox",
            kind: BrowserKind::Firefox,
            bins: &[
                "/usr/bin/firefox",
                "/snap/bin/firefox",
                "/var/lib/flatpak/exports/bin/org.mozilla.firefox",
                "/Applications/Firefox.app/Contents/MacOS/firefox",
            ],
        },
        Seed {
            name: "Safari",
            kind: BrowserKind::Safari,
            bins: &["/Applications/Safari.app/Contents/MacOS/Safari"],
        },
    ]
}

fn windows_bins() -> Vec<(String, BrowserKind, String)> {
    if !cfg!(windows) {
        return Vec::new();
    }
    let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(|key| std::env::var_os(key).map(PathBuf::from))
        .collect();
    let specs = [
        ("Google Chrome", BrowserKind::Chromium, r"Google\Chrome\Application\chrome.exe"),
        ("Helium", BrowserKind::Chromium, r"Helium\Application\helium.exe"),
        ("Chromium", BrowserKind::Chromium, r"Chromium\Application\chrome.exe"),
        ("Brave", BrowserKind::Chromium, r"BraveSoftware\Brave-Browser\Application\brave.exe"),
        ("Microsoft Edge", BrowserKind::Chromium, r"Microsoft\Edge\Application\msedge.exe"),
        ("Vivaldi", BrowserKind::Chromium, r"Vivaldi\Application\vivaldi.exe"),
        ("Firefox", BrowserKind::Firefox, r"Mozilla Firefox\firefox.exe"),
    ];
    let mut found = Vec::new();
    for root in roots {
        for (name, kind, rel) in specs {
            let path = root.join(rel);
            if path.is_file() {
                found.push((name.into(), kind, path.to_string_lossy().into_owned()));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_chromium_profile_names() {
        let dir = std::env::temp_dir().join(format!("porter-chrome-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("Local State"),
            r#"{"profile":{"info_cache":{"Profile 2":{"name":"Work"},"Default":{"name":"Personal"},"System Profile":{"name":"System"}}}}"#,
        )
        .unwrap();
        let profiles = chromium_profiles(&dir);
        assert_eq!(profiles.len(), 2);
        assert!(profiles.iter().any(|p| p.id == "Profile 2" && p.name == "Work"));
        assert!(profiles.iter().all(|p| p.id != "System Profile"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reads_firefox_profile_names() {
        let dir = std::env::temp_dir().join(format!("porter-ff-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("profiles.ini"),
            "[Profile0]\nName=Personal\nIsRelative=1\nPath=abc\n\n[Profile1]\nName=Work\nIsRelative=1\nPath=def\n",
        )
        .unwrap();
        let profiles = firefox_profiles(&dir);
        assert_eq!(
            profiles,
            vec![
                Profile {
                    id: "Personal".into(),
                    name: "Personal".into()
                },
                Profile {
                    id: "Work".into(),
                    name: "Work".into()
                }
            ]
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn chromium_launch_args() {
        let browser = Browser {
            id: "1".into(),
            name: "Helium".into(),
            kind: BrowserKind::Chromium,
            executable: "/bin/true".into(),
            user_data_dir: Some("/tmp/helium".into()),
        };
        let plan = plan(&browser, Some("Profile 1"), "https://example.com").unwrap();
        assert_eq!(
            plan.args,
            vec![
                "--user-data-dir=/tmp/helium",
                "--profile-directory=Profile 1",
                "https://example.com",
            ]
        );
    }

    #[test]
    fn firefox_launch_args() {
        let browser = Browser {
            id: "1".into(),
            name: "Firefox".into(),
            kind: BrowserKind::Firefox,
            executable: "/bin/true".into(),
            user_data_dir: Some("/tmp/ff".into()),
        };
        let plan = plan(&browser, Some("Work"), "https://example.com").unwrap();
        assert_eq!(
            plan.args,
            vec!["-P", "Work", "-new-instance", "https://example.com"]
        );
    }

    #[test]
    fn rejects_non_http_urls() {
        assert!(validate_url("file:///etc/passwd").is_err());
        assert!(validate_url("https://example.com").is_ok());
    }
}
