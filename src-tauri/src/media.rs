//! Windows System Media Transport Controls (SMTC) integration.
//!
//! SMTC is the OS-level bus that every well-behaved media app reports to:
//! Spotify, browsers playing YouTube / Yandex Music, native players, etc.
//! We read the *current* session from it and expose a flat snapshot plus
//! transport controls. This is far more robust than scraping each service.

use serde::Serialize;

#[cfg(windows)]
use windows::{
    Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as SessionManager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    },
    Storage::Streams::DataReader,
};

/// A flat, serializable view of whatever is playing right now.
#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Snapshot {
    /// True when a media session exists at all.
    pub has_session: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// `"playing"`, `"paused"` or `"stopped"`.
    pub status: String,
    /// Human-friendly service name (e.g. `"Spotify"`).
    pub source: String,
    /// Raw AppUserModelId reported by the OS.
    pub source_id: String,
    /// Playback position in seconds.
    pub position: f64,
    /// Total track length in seconds (0 if unknown).
    pub duration: f64,
    /// Stable id of the current track (`artist|title`), for change detection.
    pub track_id: String,
}

/// Decoded album-art bytes plus a file extension (`"png"` / `"jpg"`).
pub struct Thumbnail {
    pub bytes: Vec<u8>,
    pub ext: &'static str,
}

/// Initialise the COM/WinRT apartment for the current thread. Safe to call
/// repeatedly — subsequent calls just return `S_FALSE`.
#[cfg(windows)]
pub fn ensure_com() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

#[cfg(not(windows))]
pub fn ensure_com() {}

#[cfg(windows)]
fn request_manager() -> windows::core::Result<SessionManager> {
    ensure_com();
    SessionManager::RequestAsync()?.get()
}

/// Map a raw AppUserModelId to a friendly service label.
fn friendly_source(aumid: &str) -> String {
    let lower = aumid.to_ascii_lowercase();
    let known = [
        ("spotify", "Spotify"),
        ("yandex", "Yandex Music"),
        ("msedge", "Microsoft Edge"),
        ("chrome", "Google Chrome"),
        ("firefox", "Firefox"),
        ("zen", "Zen Browser"),
        ("opera", "Opera"),
        ("brave", "Brave"),
        ("vivaldi", "Vivaldi"),
        ("vlc", "VLC"),
        ("foobar", "foobar2000"),
        ("aimp", "AIMP"),
        ("applemusic", "Apple Music"),
        ("itunes", "iTunes"),
        ("musicbee", "MusicBee"),
        ("winamp", "Winamp"),
        ("deezer", "Deezer"),
        ("tidal", "TIDAL"),
    ];
    for (needle, label) in known {
        if lower.contains(needle) {
            return label.to_string();
        }
    }
    // Mozilla browsers register under a hex CLSID; recognise the common one.
    if lower.contains("308046b0af4a39cb") {
        return "Firefox".to_string();
    }
    // Fall back to a cleaned executable name.
    aumid
        .rsplit(['\\', '!'])
        .next()
        .unwrap_or(aumid)
        .trim_end_matches(".exe")
        .to_string()
}

/// Current time as 100-ns ticks since 1601-01-01 (the epoch WinRT `DateTime`
/// uses for `LastUpdatedTime`), so the two can be subtracted directly.
#[cfg(windows)]
fn now_ticks_1601() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    // 100-ns ticks between 1601-01-01 and the Unix epoch (1970-01-01).
    const EPOCH_DIFF: i64 = 116_444_736_000_000_000;
    let since_unix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (since_unix.as_nanos() / 100) as i64 + EPOCH_DIFF
}

/// Read the current media snapshot. Returns `has_session == false` when no app
/// is reporting to SMTC.
#[cfg(windows)]
pub fn snapshot() -> Snapshot {
    match snapshot_inner() {
        Ok(Some(s)) => s,
        _ => Snapshot::default(),
    }
}

#[cfg(windows)]
fn snapshot_inner() -> windows::core::Result<Option<Snapshot>> {
    let manager = request_manager()?;
    let session = match manager.GetCurrentSession() {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };

    let props = session.TryGetMediaPropertiesAsync()?.get()?;
    let title = props.Title().unwrap_or_default().to_string_lossy();
    let artist = props.Artist().unwrap_or_default().to_string_lossy();
    let album = props.AlbumTitle().unwrap_or_default().to_string_lossy();

    let playback = session.GetPlaybackInfo()?;
    let status = match playback.PlaybackStatus()? {
        PlaybackStatus::Playing => "playing",
        PlaybackStatus::Paused => "paused",
        _ => "stopped",
    };

    let aumid = session.SourceAppUserModelId().unwrap_or_default().to_string_lossy();

    let (position, duration) = match session.GetTimelineProperties() {
        Ok(t) => {
            let mut pos = t.Position().map(|v| v.Duration).unwrap_or(0) as f64 / 1.0e7;
            let dur = t.EndTime().map(|v| v.Duration).unwrap_or(0) as f64 / 1.0e7;
            // Browsers (YouTube etc.) report `Position` as of `LastUpdatedTime`,
            // not "now", and refresh it only every few seconds. Add the elapsed
            // wall-clock time while playing so the seek bar advances smoothly
            // instead of freezing on the last reported value.
            if status == "playing" {
                let last = t.LastUpdatedTime().map(|v| v.UniversalTime).unwrap_or(0);
                if last > 0 {
                    let elapsed = (now_ticks_1601() - last) as f64 / 1.0e7;
                    if elapsed > 0.0 && elapsed < 86_400.0 {
                        pos += elapsed;
                    }
                }
            }
            pos = pos.max(0.0);
            if dur > 0.0 {
                pos = pos.min(dur);
            }
            (pos, dur.max(0.0))
        }
        Err(_) => (0.0, 0.0),
    };

    if title.is_empty() && artist.is_empty() {
        return Ok(None);
    }

    let track_id = format!("{artist}|{title}");
    Ok(Some(Snapshot {
        has_session: true,
        source: friendly_source(&aumid),
        source_id: aumid,
        title,
        artist,
        album,
        status: status.to_string(),
        position,
        duration,
        track_id,
    }))
}

/// Fetch the album art of the current session as raw image bytes.
#[cfg(windows)]
pub fn fetch_thumbnail() -> Option<Thumbnail> {
    fetch_thumbnail_inner().ok().flatten()
}

#[cfg(windows)]
fn fetch_thumbnail_inner() -> windows::core::Result<Option<Thumbnail>> {
    let manager = request_manager()?;
    let session = match manager.GetCurrentSession() {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    let props = session.TryGetMediaPropertiesAsync()?.get()?;
    let reference = match props.Thumbnail() {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };

    let stream = reference.OpenReadAsync()?.get()?;
    let size = stream.Size()? as u32;
    if size == 0 {
        return Ok(None);
    }

    let input = stream.GetInputStreamAt(0)?;
    let reader = DataReader::CreateDataReader(&input)?;
    reader.LoadAsync(size)?.get()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes)?;

    let ext = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else {
        "png"
    };
    Ok(Some(Thumbnail { bytes, ext }))
}

/// Transport actions we expose to the UI.
#[cfg(windows)]
pub fn toggle_play_pause() -> bool {
    with_session(|s| s.TryTogglePlayPauseAsync()?.get())
}

#[cfg(windows)]
pub fn next() -> bool {
    with_session(|s| s.TrySkipNextAsync()?.get())
}

#[cfg(windows)]
pub fn previous() -> bool {
    with_session(|s| s.TrySkipPreviousAsync()?.get())
}

/// Seek to an absolute position (seconds) within the current track.
#[cfg(windows)]
pub fn seek(seconds: f64) -> bool {
    let ticks = (seconds.max(0.0) * 1.0e7) as i64;
    with_session(|s| s.TryChangePlaybackPositionAsync(ticks)?.get())
}

#[cfg(windows)]
fn with_session<F>(action: F) -> bool
where
    F: FnOnce(
        &windows::Media::Control::GlobalSystemMediaTransportControlsSession,
    ) -> windows::core::Result<bool>,
{
    (|| -> windows::core::Result<bool> {
        let manager = request_manager()?;
        let session = manager.GetCurrentSession()?;
        action(&session)
    })()
    .unwrap_or(false)
}

// ---- Non-Windows stubs so the crate still type-checks on other targets. ----

#[cfg(not(windows))]
pub fn snapshot() -> Snapshot {
    Snapshot::default()
}
#[cfg(not(windows))]
pub fn fetch_thumbnail() -> Option<Thumbnail> {
    None
}
#[cfg(not(windows))]
pub fn toggle_play_pause() -> bool {
    false
}
#[cfg(not(windows))]
pub fn next() -> bool {
    false
}
#[cfg(not(windows))]
pub fn previous() -> bool {
    false
}
#[cfg(not(windows))]
pub fn seek(_seconds: f64) -> bool {
    false
}
