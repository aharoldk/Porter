mod browsers;
mod store;

use browsers::{infer_kind, launch, pretty_name, profiles_in, Browser, Profile};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use store::{config_file, load, new_id, save, Preset, Store};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WebviewWindow,
};

struct AppState {
    path: PathBuf,
    store: Mutex<Store>,
    pending_url: Mutex<Option<String>>,
}

fn url_arg() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|arg| arg.starts_with("http://") || arg.starts_with("https://"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    browsers: Vec<Browser>,
    presets: Vec<Preset>,
    profiles: Vec<Profile>,
}

#[tauri::command]
fn get_state(state: State<AppState>) -> Snapshot {
    let store = state.store.lock().expect("store");
    Snapshot {
        browsers: store.browsers.clone(),
        presets: store.presets.clone(),
        profiles: Vec::new(),
    }
}

#[tauri::command]
fn list_profiles(state: State<AppState>, browser_id: String) -> Vec<Profile> {
    let store = state.store.lock().expect("store");
    store
        .browsers
        .iter()
        .find(|browser| browser.id == browser_id)
        .map(|browser| profiles_in(browser.kind, browser.user_data_dir.as_deref()))
        .unwrap_or_default()
}

#[tauri::command]
fn add_browser(state: State<AppState>) -> Result<Option<Browser>, String> {
    let executable = rfd::FileDialog::new()
        .set_title("Choose the browser")
        .pick_file();
    let Some(executable) = executable else {
        return Ok(None);
    };
    let kind = infer_kind(&executable.to_string_lossy());
    let guessed = browsers::guess_user_data_dir(&executable.to_string_lossy(), kind);
    let user_data_dir = if kind == browsers::BrowserKind::Safari {
        None
    } else if guessed.as_ref().is_some_and(|dir| marker_exists(dir, kind)) {
        guessed
    } else {
        rfd::FileDialog::new()
            .set_title("Choose the profile folder")
            .pick_folder()
            .or(guessed)
    };
    let browser = Browser {
        id: new_id(),
        name: pretty_name(&executable.to_string_lossy()),
        kind,
        executable: executable.to_string_lossy().into_owned(),
        user_data_dir: user_data_dir.map(|dir| dir.to_string_lossy().into_owned()),
    };
    let mut store = state.store.lock().expect("store");
    store.browsers.push(browser.clone());
    save(&state.path, &store)?;
    Ok(Some(browser))
}

fn marker_exists(dir: &Path, kind: browsers::BrowserKind) -> bool {
    match kind {
        browsers::BrowserKind::Chromium => dir.join("Local State").is_file(),
        browsers::BrowserKind::Firefox => dir.join("profiles.ini").is_file(),
        browsers::BrowserKind::Safari => true,
    }
}

#[tauri::command]
fn remove_browser(state: State<AppState>, id: String) -> Result<(), String> {
    let mut store = state.store.lock().expect("store");
    store.browsers.retain(|browser| browser.id != id);
    store.presets.retain(|preset| preset.browser_id != id);
    save(&state.path, &store)
}

#[tauri::command]
fn save_preset(
    state: State<AppState>,
    label: String,
    url: String,
    browser_id: String,
    profile: Option<String>,
) -> Result<Preset, String> {
    let label = label.trim().to_string();
    if label.is_empty() {
        return Err("Give the shortcut a name".into());
    }
    browsers::validate_url(&url)?;
    let preset = Preset {
        id: new_id(),
        label,
        url: url.trim().to_string(),
        browser_id,
        profile: profile.filter(|value| !value.trim().is_empty()),
    };
    let mut store = state.store.lock().expect("store");
    if !store.browsers.iter().any(|browser| browser.id == preset.browser_id) {
        return Err("Pick a browser first".into());
    }
    store.presets.push(preset.clone());
    save(&state.path, &store)?;
    Ok(preset)
}

#[tauri::command]
fn delete_preset(state: State<AppState>, id: String) -> Result<(), String> {
    let mut store = state.store.lock().expect("store");
    store.presets.retain(|preset| preset.id != id);
    save(&state.path, &store)
}

#[tauri::command]
fn launch_url(
    state: State<AppState>,
    browser_id: String,
    profile: Option<String>,
    url: String,
) -> Result<(), String> {
    let store = state.store.lock().expect("store");
    if let Some(route) = find_route(&store.presets, &url) {
        let browser = store
            .browsers
            .iter()
            .find(|browser| browser.id == route.browser_id)
            .ok_or("That browser was removed")?;
        return launch(browser, route.profile.as_deref(), &url);
    }
    let browser = store
        .browsers
        .iter()
        .find(|browser| browser.id == browser_id)
        .ok_or("Pick a browser first")?;
    launch(browser, profile.as_deref(), &url)
}

#[tauri::command]
fn launch_preset(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let store = state.store.lock().expect("store");
    let preset = store
        .presets
        .iter()
        .find(|preset| preset.id == id)
        .ok_or("Shortcut is gone")?
        .clone();
    let browser = store
        .browsers
        .iter()
        .find(|browser| browser.id == preset.browser_id)
        .ok_or("That browser was removed")?
        .clone();
    drop(store);
    launch(&browser, preset.profile.as_deref(), &preset.url)?;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    Ok(())
}

fn normalize_origin(url: &str) -> String {
    let url = url.trim();
    let rest = if let Some(r) = url.strip_prefix("github://") {
        r
    } else if let Some(r) = url.strip_prefix("github:") {
        r
    } else if let Some(r) = url.strip_prefix("gh://") {
        r
    } else if let Some(r) = url.strip_prefix("gh:") {
        r
    } else {
        url.strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .unwrap_or(url)
    };
    let rest = rest.trim_start_matches('/');
    if url.starts_with("github:") || url.starts_with("gh:") {
        return "github.com".into();
    }
    let rest = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    rest.to_ascii_lowercase()
}

fn find_route<'a>(presets: &'a [Preset], url: &str) -> Option<&'a Preset> {
    let target = normalize_origin(url);
    if target.is_empty() {
        return None;
    }
    presets
        .iter()
        .filter(|preset| !normalize_origin(&preset.url).is_empty())
        .filter(|preset| {
            let base = normalize_origin(&preset.url);
            target == base || target.ends_with(&format!(".{}", base))
        })
        .max_by_key(|preset| normalize_origin(&preset.url).len())
}

fn seed_if_empty(store: &mut Store) {
    if !store.browsers.is_empty() {
        return;
    }
    for mut browser in browsers::detect_installed() {
        browser.id = new_id();
        store.browsers.push(browser);
    }
}

fn toggle_panel(window: &WebviewWindow) {
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    let _ = window.show();
    let _ = window.set_focus();
}

fn receive_url(app: &AppHandle, url: String) {
    app.state::<AppState>()
        .pending_url
        .lock()
        .expect("pending_url")
        .replace(url.clone());
    let _ = app.emit("open-url", url);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn take_pending_url(state: State<AppState>) -> Option<String> {
    state.pending_url.lock().expect("pending_url").take()
}

#[tauri::command]
fn auto_launch_url(
    state: State<AppState>,
    url: String,
    browser_id: Option<String>,
    profile: Option<String>,
) -> Result<(), String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("No pending URL".into());
    }
    state.pending_url.lock().expect("pending_url").take();
    let store = state.store.lock().expect("store");

    let route = find_route(&store.presets, &url);
    let picked_browser = browser_id.filter(|id| !id.is_empty());
    let picked_profile = profile.filter(|p| !p.trim().is_empty());

    let chosen_browser_id = route
        .map(|p| p.browser_id.clone())
        .or(picked_browser)
        .or_else(|| store.browsers.first().map(|b| b.id.clone()));
    let chosen_profile = route
        .and_then(|p| p.profile.clone())
        .or(picked_profile);

    let browser = chosen_browser_id
        .and_then(|id| store.browsers.iter().find(|b| b.id == id).cloned());
    let route_label = route.map(|r| r.label.clone());
    drop(store);

    let browser = browser.ok_or("No browser selected")?;
    eprintln!(
        "[auto_launch] url={} route={} browser={} profile={:?}",
        url,
        route_label.as_deref().unwrap_or("-"),
        browser.name,
        chosen_profile
    );
    let result = launch(&browser, chosen_profile.as_deref(), &url);
    eprintln!("[auto_launch] result={:?}", result);
    result
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(url) = argv
                .iter()
                .find(|arg| {
                    arg.starts_with("http://")
                        || arg.starts_with("https://")
                        || arg.starts_with("github:")
                        || arg.starts_with("github://")
                        || arg.starts_with("gh:")
                        || arg.starts_with("gh://")
                })
            {
                receive_url(app, url.clone());
            }
        }))
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            fs::create_dir_all(&dir).ok();
            let path = config_file(&dir);
            let mut store = load(&path);
            seed_if_empty(&mut store);
            save(&path, &store).ok();

            let pending_url = url_arg();
            app.manage(AppState {
                path,
                store: Mutex::new(store),
                pending_url: Mutex::new(pending_url.clone()),
            });

            let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;
            let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
            TrayIconBuilder::with_id("porter")
                .icon(icon)
                .tooltip("Porter")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => {
                        if let Some(window) = app.get_webview_window("main") {
                            toggle_panel(&window);
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            toggle_panel(&window);
                        }
                    }
                })
                .build(app)?;

            if let Some(url) = pending_url {
                receive_url(app.handle(), url);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            list_profiles,
            add_browser,
            remove_browser,
            save_preset,
            delete_preset,
            launch_url,
            launch_preset,
            take_pending_url,
            auto_launch_url,     // <-- FIXED
        ])
        .run(tauri::generate_context!())
        .expect("error while running porter");
}
#[cfg(test)]
mod route_tests {
    use super::*;

    fn preset(url: &str) -> Preset {
        Preset {
            id: "1".into(),
            label: "test".into(),
            url: url.into(),
            browser_id: "b".into(),
            profile: None,
        }
    }

    #[test]
    fn matches_same_origin_path() {
        let presets = vec![preset("https://gitlab.com")];
        let url = "https://gitlab.com/dk-digital-bank/services/notification-adaptor/-/merge_requests/68";
        assert!(find_route(&presets, url).is_some());
    }

    #[test]
    fn matches_subdomain() {
        let presets = vec![preset("https://github.com")];
        assert!(find_route(&presets, "https://gist.github.com/foo").is_some());
    }

    #[test]
    fn no_cross_domain_match() {
        let presets = vec![preset("https://gitlab.com")];
        assert!(find_route(&presets, "https://evil-gitlab.com/x").is_none());
        assert!(find_route(&presets, "https://gitlab.com.evil.com/x").is_none());
    }

    #[test]
    fn longest_base_wins() {
        let presets = vec![preset("https://gitlab.com"), preset("https://gitlab.com/foo")];
        let url = "https://gitlab.com/foo/bar";
        assert_eq!(
            find_route(&presets, url).map(|p| p.url.as_str()),
            Some("https://gitlab.com/foo")
        );
    }

    #[test]
    fn exact_match() {
        let presets = vec![preset("https://github.com")];
        assert!(find_route(&presets, "https://github.com/aharoldk/keel").is_some());
        assert!(find_route(&presets, "https://gitlab.com").is_none());
    }
}
