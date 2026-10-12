mod browsers;
#[cfg(target_os = "macos")]
mod macos_default;
mod store;

use browsers::{infer_kind, launch, pretty_name, profiles_in, Browser, Profile};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use store::{config_file, load, new_id, save, Preset, Store};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, State, WebviewWindow,
};
#[cfg(not(target_os = "macos"))]
use tauri::path::BaseDirectory;

struct AppState {
    path: PathBuf,
    store: Mutex<Store>,
    pending_url: Mutex<Option<String>>,
}

/// Locks a mutex, recovering the inner value if a previous holder panicked.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Porter understands these URL shapes as command line arguments:
/// real http(s) links plus the `github:`/`gh:` shortcuts.
fn is_url_arg(arg: &str) -> bool {
    arg.starts_with("http://")
        || arg.starts_with("https://")
        || arg.starts_with("github:")
        || arg.starts_with("gh:")
}

fn url_arg() -> Option<String> {
    std::env::args().skip(1).find(|arg| is_url_arg(arg))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    browsers: Vec<Browser>,
    presets: Vec<Preset>,
    default_browser: Option<String>,
    default_profile: Option<String>,
}

#[tauri::command]
fn get_state(state: State<AppState>) -> Snapshot {
    let store = lock(&state.store);
    Snapshot {
        browsers: store.browsers.clone(),
        presets: store.presets.clone(),
        default_browser: store.default_browser.clone(),
        default_profile: store.default_profile.clone(),
    }
}

#[tauri::command]
fn list_profiles(state: State<AppState>, browser_id: String) -> Vec<Profile> {
    let store = lock(&state.store);
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
    let mut store = lock(&state.store);
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
    let mut store = lock(&state.store);
    store.browsers.retain(|browser| browser.id != id);
    store.presets.retain(|preset| preset.browser_id != id);
    if store.default_browser.as_deref() == Some(id.as_str()) {
        store.default_browser = None;
        store.default_profile = None;
    }
    save(&state.path, &store)
}

#[tauri::command]
fn set_default_browser(
    state: State<AppState>,
    browser_id: Option<String>,
    profile: Option<String>,
) -> Result<(), String> {
    let browser_id = browser_id.filter(|id| !id.is_empty());
    let profile = profile.filter(|value| !value.trim().is_empty());
    let mut store = lock(&state.store);
    if let Some(id) = &browser_id {
        if !store.browsers.iter().any(|browser| &browser.id == id) {
            return Err("That browser is gone".into());
        }
    }
    store.default_browser = browser_id;
    store.default_profile = if store.default_browser.is_some() {
        profile
    } else {
        None
    };
    save(&state.path, &store)
}

/// Locates the bundled install script, preferring the app's resource
/// directory (packaged builds) and falling back to repo-relative locations
/// (development runs).
#[cfg(not(target_os = "macos"))]
fn find_install_script(app: &AppHandle) -> Option<PathBuf> {
    let resource = app
        .path()
        .resolve("install-default-browser.sh", BaseDirectory::Resource)
        .ok();
    find_install_script_from(resource)
}

#[cfg(not(target_os = "macos"))]
fn bundled_icon(app: &AppHandle) -> Option<PathBuf> {
    let icon = app
        .path()
        .resolve("porter-icon.png", BaseDirectory::Resource)
        .ok()?;
    icon.is_file().then_some(icon)
}

#[cfg(any(test, not(target_os = "macos")))]
fn find_install_script_from(resource: Option<PathBuf>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = resource.into_iter().collect();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("install-default-browser.sh"));
            if let Some(root) = dir.parent() {
                candidates.push(root.join("install-default-browser.sh"));
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("install-default-browser.sh"));
        candidates.push(cwd.join("..").join("install-default-browser.sh"));
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../install-default-browser.sh"),
    );
    candidates.into_iter().find(|path| path.is_file())
}

#[tauri::command]
#[cfg_attr(target_os = "macos", allow(unused_variables))]
fn install_default_browser(app: AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        macos_default::set_default("http")?;
        macos_default::set_default("https")?;
        return Ok(
            "Porter is now the default browser on macOS. If links still open elsewhere, \
             check System Settings > Desktop & Dock."
                .into(),
        );
    }
    #[cfg(not(target_os = "macos"))]
    {
        let script =
            find_install_script(&app).ok_or("install-default-browser.sh was not found near Porter")?;
        let exe = std::env::current_exe().map_err(|err| err.to_string())?;
        let exe = exe.canonicalize().unwrap_or(exe);
        let mut command = std::process::Command::new("bash");
        command.arg(&script).arg(&exe);
        if let Some(icon) = bundled_icon(&app) {
            command.arg(icon);
        }
        let output = command
            .output()
            .map_err(|err| format!("Could not run {}: {}", script.display(), err))?;
        let mut text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&stderr);
        }
        if !output.status.success() {
            return Err(if text.is_empty() {
                format!("Script failed with status {}", output.status)
            } else {
                text
            });
        }
        Ok(text)
    }
}

#[tauri::command]
fn default_browser_status() -> Option<bool> {
    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("xdg-settings")
            .args(["get", "default-web-browser"])
            .output()
            .ok()?;
        let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if value.is_empty() {
            return Some(false);
        }
        // Packaged builds register as `Porter.desktop`, the install script
        // writes a user-level `porter.desktop`.
        Some(value.to_lowercase().ends_with("porter.desktop"))
    }
    #[cfg(target_os = "macos")]
    {
        let current = macos_default::current_default("https")?;
        Some(current == macos_default::BUNDLE_ID)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
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
    let mut store = lock(&state.store);
    if !store.browsers.iter().any(|browser| browser.id == preset.browser_id) {
        return Err("Pick a browser first".into());
    }
    store.presets.push(preset.clone());
    save(&state.path, &store)?;
    Ok(preset)
}

#[tauri::command]
fn delete_preset(state: State<AppState>, id: String) -> Result<(), String> {
    let mut store = lock(&state.store);
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
    let store = lock(&state.store);
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
    let store = lock(&state.store);
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

/// Stores an incoming URL and asks the panel to handle it. Delivery channels
/// can overlap (command line argument, single-instance forward, macOS
/// AppleEvent), so a URL still waiting to be launched is only handled once.
fn receive_url(app: &AppHandle, url: String) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    {
        let mut pending = lock(&state.pending_url);
        if pending.as_deref() == Some(url.as_str()) {
            return;
        }
        *pending = Some(url.clone());
    }
    let _ = app.emit("open-url", url);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn take_pending_url(state: State<AppState>) -> Option<String> {
    lock(&state.pending_url).take()
}

struct LaunchChoice {
    browser: Browser,
    profile: Option<String>,
}

fn resolve_launch(
    store: &Store,
    url: &str,
    picked_browser: Option<String>,
    picked_profile: Option<String>,
) -> Option<LaunchChoice> {
    let picked_browser = picked_browser.filter(|id| !id.is_empty());
    let picked_profile = picked_profile.filter(|value| !value.trim().is_empty());
    let find = |id: &str| store.browsers.iter().find(|browser| browser.id == id);

    if let Some(route) = find_route(&store.presets, url) {
        let browser = find(&route.browser_id)?;
        return Some(LaunchChoice {
            browser: browser.clone(),
            profile: route.profile.clone().or(picked_profile),
        });
    }
    if let Some(default_id) = store
        .default_browser
        .as_deref()
        .filter(|id| !id.is_empty())
    {
        if let Some(browser) = find(default_id) {
            return Some(LaunchChoice {
                browser: browser.clone(),
                profile: store.default_profile.clone().or(picked_profile),
            });
        }
    }
    picked_browser
        .as_deref()
        .and_then(find)
        .or_else(|| store.browsers.first())
        .map(|browser| LaunchChoice {
            browser: browser.clone(),
            profile: picked_profile,
        })
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
    lock(&state.pending_url).take();
    let store = lock(&state.store);
    let choice = resolve_launch(&store, &url, browser_id, profile).ok_or("No browser selected")?;
    drop(store);
    launch(&choice.browser, choice.profile.as_deref(), &url)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(url) = argv.iter().find(|arg| is_url_arg(arg)) {
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
                pending_url: Mutex::new(None),
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
            set_default_browser,
            install_default_browser,
            default_browser_status,
            save_preset,
            delete_preset,
            launch_url,
            launch_preset,
            take_pending_url,
            auto_launch_url,
        ])
        .build(tauri::generate_context!())
        .expect("error while building porter");
    app.run(|app, event| {
        receive_opened_urls(app, event);
    });
}

/// macOS delivers default-browser URLs through an AppleEvent instead of
/// command line arguments, surfacing them as `RunEvent::Opened`.
#[cfg(target_os = "macos")]
fn receive_opened_urls(app: &AppHandle, event: tauri::RunEvent) {
    if let tauri::RunEvent::Opened { urls } = event {
        for url in urls {
            receive_url(app, url.to_string());
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn receive_opened_urls(_app: &AppHandle, _event: tauri::RunEvent) {}
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

    fn browser(id: &str, name: &str) -> Browser {
        Browser {
            id: id.into(),
            name: name.into(),
            kind: browsers::BrowserKind::Chromium,
            executable: "/bin/true".into(),
            user_data_dir: None,
        }
    }

    fn store(browsers: Vec<Browser>, presets: Vec<Preset>) -> Store {
        Store {
            browsers,
            presets,
            default_browser: None,
            default_profile: None,
        }
    }

    #[test]
    fn default_browser_used_when_no_route() {
        let mut store = store(
            vec![browser("b1", "Chrome"), browser("b2", "Firefox")],
            vec![],
        );
        store.default_browser = Some("b2".into());
        store.default_profile = Some("work".into());
        let choice = resolve_launch(&store, "https://example.com", Some("b1".into()), None)
            .expect("a browser");
        assert_eq!(choice.browser.id, "b2");
        assert_eq!(choice.profile.as_deref(), Some("work"));
    }

    #[test]
    fn route_wins_over_default_browser() {
        let mut store = store(
            vec![browser("b1", "Chrome"), browser("b2", "Firefox")],
            vec![Preset {
                id: "p".into(),
                label: "GitLab".into(),
                url: "https://gitlab.com".into(),
                browser_id: "b1".into(),
                profile: None,
            }],
        );
        store.default_browser = Some("b2".into());
        let choice = resolve_launch(
            &store,
            "https://gitlab.com/group/repo",
            Some("b2".into()),
            None,
        )
        .expect("a browser");
        assert_eq!(choice.browser.id, "b1");
    }

    #[test]
    fn picked_browser_used_when_no_default() {
        let store = store(
            vec![browser("b1", "Chrome"), browser("b2", "Firefox")],
            vec![],
        );
        let choice = resolve_launch(&store, "https://example.com", Some("b2".into()), None)
            .expect("a browser");
        assert_eq!(choice.browser.id, "b2");
    }

    #[test]
    fn first_browser_is_last_resort() {
        let store = store(vec![browser("b1", "Chrome")], vec![]);
        let choice = resolve_launch(&store, "https://example.com", None, None).expect("a browser");
        assert_eq!(choice.browser.id, "b1");
    }

    #[test]
    fn no_browser_means_no_choice() {
        let store = store(vec![], vec![]);
        assert!(resolve_launch(&store, "https://example.com", None, None).is_none());
    }

    #[test]
    fn finds_install_script() {
        let script =
            find_install_script_from(None).expect("install-default-browser.sh should be found");
        assert!(script.ends_with("install-default-browser.sh"));
    }

    #[test]
    fn recognizes_url_arguments() {
        assert!(is_url_arg("https://example.com"));
        assert!(is_url_arg("http://example.com"));
        assert!(is_url_arg("github:user/repo"));
        assert!(is_url_arg("gh://user/repo"));
        assert!(!is_url_arg("--verbose"));
        assert!(!is_url_arg("file:///etc/passwd"));
    }
}
