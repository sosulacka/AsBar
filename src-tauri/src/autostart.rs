//! Launch-on-startup via a Startup-folder shortcut.
//!
//! Deliberately *not* a registry `Run` key: a `.lnk` dropped into the user's
//! Startup folder shows up in Task Manager → Startup apps, where the user can
//! enable/disable it directly — which is exactly the requested behaviour.

use std::path::PathBuf;

const SHORTCUT_NAME: &str = "AsBar.lnk";

/// `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`
fn startup_dir() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup"),
    )
}

fn shortcut_path() -> Option<PathBuf> {
    Some(startup_dir()?.join(SHORTCUT_NAME))
}

/// Whether the startup shortcut currently exists.
pub fn is_enabled() -> bool {
    shortcut_path().map(|p| p.exists()).unwrap_or(false)
}

/// Install or remove the startup shortcut.
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    let path = shortcut_path().ok_or("could not locate the Startup folder")?;
    if enabled {
        create_shortcut(&path)
    } else {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[cfg(windows)]
fn create_shortcut(lnk_path: &std::path::Path) -> Result<(), String> {
    use windows::core::{HSTRING, Interface};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let work_dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("C:/"));

    unsafe {
        // S_FALSE (already initialised) is fine; only hard failures matter.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW =
            CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;

        link.SetPath(&HSTRING::from(exe.as_os_str()))
            .map_err(|e| e.to_string())?;
        link.SetWorkingDirectory(&HSTRING::from(work_dir.as_os_str()))
            .map_err(|e| e.to_string())?;
        link.SetDescription(&HSTRING::from("AsBar — Dynamic Island"))
            .map_err(|e| e.to_string())?;

        let persist: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        persist
            .Save(&HSTRING::from(lnk_path.as_os_str()), true)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn create_shortcut(_lnk_path: &std::path::Path) -> Result<(), String> {
    Err("autostart is only supported on Windows".into())
}
