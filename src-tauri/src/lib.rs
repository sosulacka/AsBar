//! AsBar — a Dynamic Island for Windows, built on Tauri 2.
//!
//! Responsibilities of this module:
//!  * own the persisted [`Config`] and apply island geometry,
//!  * poll the OS media bus and stream updates to the island webview,
//!  * cache album art under `C:/AsBar/Assets/Icons`,
//!  * expose transport / settings / autostart commands to the frontend,
//!  * wire up the system-tray icon.

mod ai;
mod autostart;
mod browser;
mod config;
mod media;
mod visualizer;

use std::collections::HashSet;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WindowEvent,
};

use config::{Anchor, Config};

/// Transparent breathing room (logical px) around the island for shadow and
/// the scale-in animation. The webview centers the pill inside this padding.
/// Shared, in-memory copy of the user config.
struct AppState {
    config: Mutex<Config>,
    /// Latest island accent color, mirrored to the other windows for the glow.
    accent: Mutex<String>,
    /// Labels of windows that *should* be on screen right now. The visibility
    /// guard restores any of these that Win+D / "Show desktop" minimized.
    visible: Mutex<HashSet<String>>,
}

/// Mark a window as intended-visible (or not) for the visibility guard.
fn mark_visible(app: &AppHandle, label: &str, vis: bool) {
    let state = app.state::<AppState>();
    let mut set = state.visible.lock();
    if vis {
        set.insert(label.to_string());
    } else {
        set.remove(label);
    }
}

/// Restore any "should be visible" window that Win+D minimized or hid. These
/// are borderless tool windows with no minimize affordance, so any minimized /
/// hidden state comes from "Show desktop" and must be undone.
fn spawn_visibility_guard(app: AppHandle) {
    std::thread::spawn(move || loop {
        let labels: Vec<String> = {
            let state = app.state::<AppState>();
            let set = state.visible.lock();
            set.iter().cloned().collect()
        };
        for label in labels {
            if let Some(w) = app.get_webview_window(&label) {
                if w.is_minimized().unwrap_or(false) {
                    let _ = w.unminimize();
                }
                if !w.is_visible().unwrap_or(true) {
                    let _ = w.show();
                }
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    });
}

/// Payload pushed to the island on every poll tick.
#[derive(Serialize, Clone)]
struct MediaEvent {
    media: media::Snapshot,
    /// Data-URI of the album art. `None` = unchanged, `Some("")` = cleared.
    thumb: Option<String>,
    #[serde(rename = "thumbChanged")]
    thumb_changed: bool,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read the Windows system accent color as `#RRGGBB`.
#[cfg(windows)]
fn system_accent() -> Option<String> {
    use windows::UI::ViewManagement::{UIColorType, UISettings};
    media::ensure_com();
    let settings = UISettings::new().ok()?;
    let c = settings.GetColorValue(UIColorType::Accent).ok()?;
    Some(format!("#{:02X}{:02X}{:02X}", c.R, c.G, c.B))
}

#[cfg(not(windows))]
fn system_accent() -> Option<String> {
    None
}

/// Make a string safe to use as a filename and keep it short.
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if "<>:\"/\\|?*".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().to_string();
    let truncated: String = cleaned.chars().take(80).collect();
    if truncated.is_empty() {
        "track".to_string()
    } else {
        truncated
    }
}

/// Cache key for a track's artwork. Scoped by source + artist + title so that
/// the same title in two apps (e.g. Spotify vs Yandex Music) — or two different
/// songs that happen to share a name — never reuse each other's cached art.
fn art_key(snap: &media::Snapshot) -> String {
    sanitize_filename(&format!("{} - {} - {}", snap.source, snap.artist, snap.title))
}

fn to_data_uri(bytes: &[u8], ext: &str) -> String {
    let mime = if ext == "jpg" { "jpeg" } else { ext };
    format!("data:image/{};base64,{}", mime, STANDARD.encode(bytes))
}

/// Return the album art for the current track as a data-URI.
///
/// For browser sources we first try to read the open YouTube tab's video id and
/// fetch a crisp `i.ytimg.com` thumbnail (the browser only hands SMTC a tiny
/// one). Otherwise we use the on-disk cache, falling back to SMTC which also
/// seeds the cache.
fn cache_and_encode(snap: &media::Snapshot) -> Option<String> {
    config::ensure_dirs();
    let key = art_key(snap);

    // Browser → try the real YouTube artwork. Cached on disk under the title so
    // repeat plays are instant; the HD copy always wins over the SMTC stub.
    if browser::is_browser(&snap.source) {
        if let Some(uri) = youtube_artwork(&key) {
            return Some(uri);
        }
    }

    for ext in ["png", "jpg", "jpeg", "ico"] {
        let path = config::icons_dir().join(format!("{key}.{ext}"));
        if let Ok(bytes) = std::fs::read(&path) {
            if !bytes.is_empty() {
                return Some(to_data_uri(&bytes, ext));
            }
        }
    }

    let thumb = media::fetch_thumbnail()?;
    let path = config::icons_dir().join(format!("{key}.{}", thumb.ext));
    let _ = std::fs::write(&path, &thumb.bytes);
    Some(to_data_uri(&thumb.bytes, thumb.ext))
}

/// Resolve the active browser tab's YouTube video, download the best available
/// thumbnail, cache it under `key.jpg` and return it as a data-URI.
fn youtube_artwork(key: &str) -> Option<String> {
    let id = browser::active_youtube_video_id()?;

    // Reuse a previously downloaded HD thumbnail for this track (large jpg) so
    // repeat plays skip the network. Small files are SMTC stubs — re-fetch HD.
    let cached = config::icons_dir().join(format!("{key}.jpg"));
    if let Ok(bytes) = std::fs::read(&cached) {
        if bytes.len() >= 20_000 {
            return Some(to_data_uri(&bytes, "jpg"));
        }
    }

    let (bytes, ext) = browser::thumbnail_urls(&id).iter().find_map(|u| fetch_image(u))?;
    let path = config::icons_dir().join(format!("{key}.{ext}"));
    let _ = std::fs::write(&path, &bytes);
    Some(to_data_uri(&bytes, ext))
}

/// Synchronously download an image; `None` on any error or an empty/placeholder
/// body (ytimg serves a tiny stub image for missing resolutions).
fn fetch_image(url: &str) -> Option<(Vec<u8>, &'static str)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .ok()?;
    let resp = client.get(url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let bytes = resp.bytes().ok()?.to_vec();
    if bytes.len() < 2000 {
        return None;
    }
    let ext = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else {
        "png"
    };
    Some((bytes, ext))
}

/// Reposition the island from the config using its *current* size. The window
/// size itself is driven by `resize_island` (JS measures the real pill), but
/// position depends only on anchor/offset/margin, so changing those must move
/// the window live without waiting for a re-measure.
fn position_island(app: &AppHandle, cfg: &Config) {
    let Some(win) = app.get_webview_window("island") else {
        return;
    };
    let monitor = match win.primary_monitor() {
        Ok(Some(m)) => m,
        _ => return,
    };
    let scale = monitor.scale_factor();
    let msize = monitor.size();
    let mpos = monitor.position();
    let win_w = win.outer_size().map(|s| s.width as i32).unwrap_or(0);
    let off = (cfg.offset_x as f64 * scale).round() as i32;

    let x = match cfg.anchor {
        Anchor::Left => mpos.x + off,
        Anchor::Right => mpos.x + msize.width as i32 - win_w - off,
        Anchor::Center => mpos.x + (msize.width as i32 - win_w) / 2 + off,
    };
    let y = mpos.y + (cfg.margin_top as f64 * scale).round() as i32;
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// Apply window-level geometry/preferences from the config.
fn apply_geometry(app: &AppHandle, cfg: &Config) {
    if let Some(win) = app.get_webview_window("island") {
        let _ = win.set_always_on_top(cfg.always_on_top);
    }
    // The settings and AI panels both follow the "always on top" preference.
    for label in ["settings", "assistant"] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.set_always_on_top(cfg.always_on_top);
        }
    }
    // Move the island live when anchor / offset / margin change.
    position_island(app, cfg);
}

/// Size the island window tightly to the pill (measured in the webview) so the
/// transparent margins don't swallow desktop clicks. `w`/`h` are logical px.
#[tauri::command]
fn resize_island(app: AppHandle, state: tauri::State<AppState>, w: f64, h: f64) {
    let Some(win) = app.get_webview_window("island") else {
        return;
    };
    let monitor = match win.primary_monitor() {
        Ok(Some(m)) => m,
        _ => return,
    };
    let scale = monitor.scale_factor();
    let msize = monitor.size();
    let mpos = monitor.position();

    let cfg = state.config.lock().clone();
    // No side margin: WebView2 captures clicks over the ENTIRE window, including
    // transparent pixels, so any padding around the pill becomes a dead zone
    // that blocks the desktop. Size the window to exactly the measured content.
    let win_w = (w * scale).round() as i32;
    let win_h = (h * scale).round() as i32;
    let off = (cfg.offset_x as f64 * scale).round() as i32;

    let x = match cfg.anchor {
        Anchor::Left => mpos.x + off,
        Anchor::Right => mpos.x + msize.width as i32 - win_w - off,
        Anchor::Center => mpos.x + (msize.width as i32 - win_w) / 2 + off,
    };
    let y = mpos.y + (cfg.margin_top as f64 * scale).round() as i32;

    let _ = win.set_size(PhysicalSize::new(win_w.max(1) as u32, win_h.max(1) as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// Resolve the effective accent (system override when requested) and ship the
/// full theme to the island webview as a JSON event.
fn push_theme(app: &AppHandle, cfg: &Config) {
    let mut payload = serde_json::to_value(cfg).unwrap_or_default();
    if cfg.follow_system_accent {
        if let Some(accent) = system_accent() {
            payload["accent_color"] = serde_json::Value::String(accent);
        }
    }
    let _ = app.emit("config:update", payload);
}

/// Position a window just beneath the island, horizontally centered on it and
/// clamped to the monitor, so it reads as unfolding out of the pill.
fn anchor_under_island(app: &AppHandle, win: &tauri::WebviewWindow) {
    if let Some(island) = app.get_webview_window("island") {
        if let (Ok(ipos), Ok(isize), Ok(wsize)) =
            (island.outer_position(), island.outer_size(), win.outer_size())
        {
            let island_center_x = ipos.x + isize.width as i32 / 2;
            let mut x = island_center_x - wsize.width as i32 / 2;
            // Overlap the transparent island padding so the panel grows from it.
            let mut y = ipos.y + isize.height as i32 - 18;

            if let Ok(Some(monitor)) = win.primary_monitor() {
                let mp = monitor.position();
                let ms = monitor.size();
                let min_x = mp.x + 8;
                let max_x = mp.x + ms.width as i32 - wsize.width as i32 - 8;
                x = x.clamp(min_x, max_x.max(min_x));
                let max_y = mp.y + ms.height as i32 - wsize.height as i32 - 8;
                y = y.clamp(mp.y + 8, max_y.max(mp.y + 8));
            }
            let _ = win.set_position(PhysicalPosition::new(x, y));
        }
    }
}

/// Open (or reveal) the settings window, anchored beneath the island.
fn open_settings(app: &AppHandle) {
    let Some(win) = app.get_webview_window("settings") else {
        return;
    };
    anchor_under_island(app, &win);
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.emit("window:shown", ());
    mark_visible(app, "settings", true);
}

// ---------------------------------------------------------------------------
// Background poller
// ---------------------------------------------------------------------------

fn spawn_poller(app: AppHandle) {
    std::thread::spawn(move || {
        media::ensure_com();
        let mut last_track = String::new();
        // Some players (Yandex Music, browsers) publish their album art a beat
        // after the track changes, so we keep retrying until it shows up.
        let mut have_art = false;
        let mut art_tries = 0u32;
        const MAX_ART_TRIES: u32 = 12; // ~6s at the 500ms poll interval
        loop {
            let snap = media::snapshot();
            let track_changed = snap.track_id != last_track;
            if track_changed {
                have_art = false;
                art_tries = 0;
            }

            let thumb = if !snap.has_session {
                if track_changed { Some(String::new()) } else { None }
            } else if have_art {
                None // already showing art for this track
            } else if art_tries < MAX_ART_TRIES {
                art_tries += 1;
                let uri = cache_and_encode(&snap).unwrap_or_default();
                if !uri.is_empty() {
                    have_art = true;
                    Some(uri)
                } else if track_changed {
                    Some(String::new()) // clear the previous track's art at once
                } else {
                    None // nothing yet — try again next tick
                }
            } else {
                None // gave up retrying for this track
            };
            last_track = snap.track_id.clone();

            let _ = app.emit(
                "media:update",
                MediaEvent {
                    media: snap,
                    thumb,
                    thumb_changed: track_changed,
                },
            );
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_now_playing() -> media::Snapshot {
    media::snapshot()
}

#[tauri::command]
fn media_toggle() -> bool {
    media::toggle_play_pause()
}

#[tauri::command]
fn media_next() -> bool {
    media::next()
}

#[tauri::command]
fn media_previous() -> bool {
    media::previous()
}

#[tauri::command]
fn media_seek(position: f64) -> bool {
    media::seek(position)
}

/// The island reports its current accent color; mirror it to the other windows
/// so their "flashlight" glow matches the music.
#[tauri::command]
fn report_accent(app: AppHandle, state: tauri::State<AppState>, color: String) {
    *state.accent.lock() = color.clone();
    let _ = app.emit("accent:update", color);
}

#[tauri::command]
fn get_config(state: tauri::State<AppState>) -> Config {
    state.config.lock().clone()
}

#[tauri::command]
fn get_system_accent() -> Option<String> {
    system_accent()
}

/// Current island accent — used by settings/AI to seed their glow on open.
#[tauri::command]
fn get_accent(state: tauri::State<AppState>) -> String {
    state.accent.lock().clone()
}

/// Persist a new config, then re-apply geometry, theme and autostart.
#[tauri::command]
fn save_config(app: AppHandle, state: tauri::State<AppState>, config: Config) -> Result<(), String> {
    // Reconcile autostart with the requested flag before storing.
    let (prev_autostart, prev_lang) = {
        let c = state.config.lock();
        (c.autostart, c.language.clone())
    };
    if config.autostart != prev_autostart {
        autostart::set_enabled(config.autostart)?;
    }
    let effective_autostart = autostart::is_enabled();

    let mut stored = config;
    stored.autostart = effective_autostart;
    stored.save().map_err(|e| e.to_string())?;

    let lang_changed = stored.language != prev_lang;
    *state.config.lock() = stored.clone();
    apply_geometry(&app, &stored);
    push_theme(&app, &stored);
    if lang_changed {
        refresh_tray(&app, &stored.language);
    }
    Ok(())
}

#[tauri::command]
fn open_settings_window(app: AppHandle) {
    open_settings(&app);
}

#[tauri::command]
fn close_settings_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.hide();
    }
    mark_visible(&app, "settings", false);
}

/// Push the current theme on demand (called by the island once it has loaded).
#[tauri::command]
fn request_theme(app: AppHandle, state: tauri::State<AppState>) {
    let cfg = state.config.lock().clone();
    push_theme(&app, &cfg);
}

/// Reveal the AI assistant window, anchored beneath the island like settings.
fn open_assistant(app: &AppHandle) {
    let Some(win) = app.get_webview_window("assistant") else {
        return;
    };
    anchor_under_island(app, &win);
    // Respect the "always on top" preference, like the island and settings.
    let on_top = app.state::<AppState>().config.lock().always_on_top;
    let _ = win.set_always_on_top(on_top);
    let _ = win.show();
    let _ = win.set_focus();
    let _ = win.emit("window:shown", ());
    mark_visible(app, "assistant", true);
}

#[tauri::command]
fn open_assistant_window(app: AppHandle) {
    open_assistant(&app);
}

#[tauri::command]
fn close_assistant_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("assistant") {
        let _ = win.hide();
    }
    mark_visible(&app, "assistant", false);
}

// ---------------------------------------------------------------------------
// Tray
// ---------------------------------------------------------------------------

/// Localized tray menu labels: (assistant, settings, toggle, quit).
fn tray_labels(lang: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    if lang == "en" {
        ("AI assistant", "Settings", "Show / hide island", "Quit")
    } else {
        ("AI-ассистент", "Настройки", "Показать / скрыть остров", "Выход")
    }
}

/// Build (or rebuild) the tray menu in the given language and attach it.
fn build_tray_menu(app: &AppHandle, lang: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let (assistant, settings, toggle, quit) = tray_labels(lang);
    let assistant_i = MenuItem::with_id(app, "assistant", assistant, true, Some("Ctrl+Space"))?;
    let settings_i = MenuItem::with_id(app, "settings", settings, true, None::<&str>)?;
    let toggle_i = MenuItem::with_id(app, "toggle", toggle, true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", quit, true, None::<&str>)?;
    Menu::with_items(app, &[&assistant_i, &settings_i, &toggle_i, &sep, &quit_i])
}

/// Swap the tray menu to match a language change.
fn refresh_tray(app: &AppHandle, lang: &str) {
    if let Some(tray) = app.tray_by_id("asbar-tray") {
        if let Ok(menu) = build_tray_menu(app, lang) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let lang = app.state::<AppState>().config.lock().language.clone();
    let menu = build_tray_menu(app, &lang)?;

    TrayIconBuilder::with_id("asbar-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("AsBar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "assistant" => open_assistant(app),
            "settings" => open_settings(app),
            "toggle" => {
                if let Some(win) = app.get_webview_window("island") {
                    match win.is_visible() {
                        Ok(true) => {
                            let _ = win.hide();
                            mark_visible(app, "island", false);
                        }
                        _ => {
                            let _ = win.show();
                            mark_visible(app, "island", true);
                        }
                    }
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
                open_settings(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    config::ensure_dirs();
    let mut cfg = Config::load();
    cfg.autostart = autostart::is_enabled();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state() == ShortcutState::Pressed
                        && shortcut.matches(
                            tauri_plugin_global_shortcut::Modifiers::CONTROL,
                            tauri_plugin_global_shortcut::Code::Space,
                        )
                    {
                        // Toggle the assistant on Ctrl+Space.
                        if let Some(win) = app.get_webview_window("assistant") {
                            if win.is_visible().unwrap_or(false) {
                                let _ = win.hide();
                                mark_visible(app, "assistant", false);
                            } else {
                                open_assistant(app);
                            }
                        }
                    }
                })
                .build(),
        )
        .manage(AppState {
            config: Mutex::new(cfg),
            accent: Mutex::new("#E0E0EC".to_string()),
            // The island starts visible; settings/assistant are added when opened.
            visible: Mutex::new(HashSet::from(["island".to_string()])),
        })
        .invoke_handler(tauri::generate_handler![
            get_now_playing,
            media_toggle,
            media_next,
            media_previous,
            media_seek,
            report_accent,
            resize_island,
            get_config,
            get_system_accent,
            get_accent,
            save_config,
            open_settings_window,
            close_settings_window,
            request_theme,
            open_assistant_window,
            close_assistant_window,
            ai::ai_chat,
            ai::ai_models,
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // The settings window hides instead of tearing down; closing the
                // island quits the whole app.
                if window.label() == "settings" || window.label() == "assistant" {
                    api.prevent_close();
                    let _ = window.hide();
                    mark_visible(&window.app_handle(), window.label(), false);
                } else if window.label() == "island" {
                    window.app_handle().exit(0);
                }
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            let cfg = app.state::<AppState>().config.lock().clone();

            apply_geometry(&handle, &cfg);
            build_tray(&handle)?;
            spawn_poller(handle.clone());
            visualizer::spawn(handle.clone());
            spawn_visibility_guard(handle.clone());

            // Global hotkey: Ctrl+Space toggles the AI assistant.
            {
                use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
                let shortcut = Shortcut::new(Some(Modifiers::CONTROL), Code::Space);
                let _ = app.global_shortcut().register(shortcut);
            }

            // Push the initial theme shortly after launch so the webview is ready.
            let theme_handle = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(400));
                let cfg = theme_handle.state::<AppState>().config.lock().clone();
                push_theme(&theme_handle, &cfg);
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running AsBar");
}
