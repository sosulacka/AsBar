//! Reads the URL of the active browser tab via Windows UI Automation.
//!
//! SMTC never tells us the page a browser is playing, so to recognise YouTube
//! (and grab a crisp `i.ytimg.com` thumbnail instead of the tiny one the browser
//! hands to SMTC) we walk the open browser windows, read each one's address bar
//! through UI Automation and look for a `youtube.com/watch?v=…` URL.
//!
//! Caveat: the address bar only reflects each window's *active* tab, so a video
//! playing in a background tab can't be detected this way.

/// True when a friendly source label looks like a web browser.
pub fn is_browser(source: &str) -> bool {
    let s = source.to_ascii_lowercase();
    // NB: not "yandex" — the Yandex Music desktop app shares that label but is a
    // native player whose art comes from SMTC, not a YouTube tab.
    ["chrome", "edge", "firefox", "zen", "opera", "brave", "vivaldi"]
        .iter()
        .any(|b| s.contains(b))
}

/// Pull an 11-char YouTube video id out of a watch / shorts / youtu.be URL.
pub fn extract_video_id(url: &str) -> Option<String> {
    if !(url.contains("youtube.com") || url.contains("youtu.be")) {
        return None;
    }
    for marker in ["v=", "/shorts/", "youtu.be/", "/embed/", "/live/"] {
        if let Some(idx) = url.find(marker) {
            let rest = &url[idx + marker.len()..];
            let id: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .collect();
            if id.len() >= 11 {
                return Some(id[..11].to_string());
            }
        }
    }
    None
}

/// Best high-resolution thumbnail URL for a video id (caller falls back if 404).
pub fn thumbnail_urls(id: &str) -> [String; 3] {
    [
        format!("https://i.ytimg.com/vi/{id}/maxresdefault.jpg"),
        format!("https://i.ytimg.com/vi/{id}/sddefault.jpg"),
        format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
    ]
}

// ---------------------------------------------------------------------------
// Windows UI Automation
// ---------------------------------------------------------------------------

#[cfg(windows)]
pub fn active_youtube_video_id() -> Option<String> {
    inner::active_youtube_video_id()
}

#[cfg(not(windows))]
pub fn active_youtube_video_id() -> Option<String> {
    None
}

#[cfg(windows)]
mod inner {
    use super::extract_video_id;
    use windows::core::{Interface, VARIANT};
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationValuePattern, TreeScope_Descendants,
        UIA_ControlTypePropertyId, UIA_EditControlTypeId, UIA_ValuePatternId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, IsWindowVisible,
    };

    /// Window classes used by the browsers we care about.
    fn is_browser_window(hwnd: HWND) -> bool {
        let mut buf = [0u16; 64];
        let len = unsafe { GetClassNameW(hwnd, &mut buf) };
        if len <= 0 {
            return false;
        }
        let class = String::from_utf16_lossy(&buf[..len as usize]);
        // Chromium family (Chrome/Edge/Brave/Opera/Vivaldi/Yandex) + Firefox/Zen.
        class == "Chrome_WidgetWin_1" || class == "MozillaWindowClass"
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let windows = &mut *(lparam.0 as *mut Vec<HWND>);
        if IsWindowVisible(hwnd).as_bool() && is_browser_window(hwnd) {
            windows.push(hwnd);
        }
        BOOL(1) // keep going
    }

    pub fn active_youtube_video_id() -> Option<String> {
        crate::media::ensure_com();

        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) }.ok()?;

        // Match the address bar by control type (Edit). FindFirst walks the tree
        // in pre-order, and the toolbar (with the omnibox) sits before the web
        // content, so it short-circuits without descending the whole page DOM.
        let edit_value = VARIANT::from(UIA_EditControlTypeId.0);
        let condition = unsafe {
            automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &edit_value)
        }
        .ok()?;

        let mut hwnds: Vec<HWND> = Vec::new();
        unsafe {
            let _ = EnumWindows(
                Some(collect),
                LPARAM(&mut hwnds as *mut Vec<HWND> as isize),
            );
        }

        for hwnd in hwnds {
            let Ok(element) = (unsafe { automation.ElementFromHandle(hwnd) }) else {
                continue;
            };
            let Ok(edit) = (unsafe { element.FindFirst(TreeScope_Descendants, &condition) })
            else {
                continue;
            };
            // FindFirst yields a null interface when nothing matches.
            if edit.as_raw().is_null() {
                continue;
            }
            let Ok(pattern) = (unsafe { edit.GetCurrentPattern(UIA_ValuePatternId) }) else {
                continue;
            };
            let Ok(value_pattern) = pattern.cast::<IUIAutomationValuePattern>() else {
                continue;
            };
            let Ok(bstr) = (unsafe { value_pattern.CurrentValue() }) else {
                continue;
            };
            let url = bstr.to_string();
            if let Some(id) = extract_video_id(&url) {
                return Some(id);
            }
        }
        None
    }
}
