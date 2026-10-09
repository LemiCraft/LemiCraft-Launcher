use tauri::AppHandle;

// Windows 11 only: Mica is smooth from 22000, acrylic lags on window drag/resize until 22H2 (22621)
#[cfg(windows)]
const MICA_MIN_BUILD: u32 = 22000;
#[cfg(windows)]
const ACRYLIC_MIN_BUILD: u32 = 22621;
// Matches the page's colour fade, so the backdrop outlives it instead of cutting out mid-transition
#[cfg(windows)]
const FADE_OUT_MS: u64 = 350;

#[cfg(windows)]
fn windows_build() -> u32 {
    sysinfo::System::kernel_version().and_then(|v| v.trim().parse().ok()).unwrap_or(0)
}

#[cfg(windows)]
pub fn is_supported() -> bool {
    windows_build() >= MICA_MIN_BUILD
}

#[cfg(target_os = "macos")]
pub fn is_supported() -> bool {
    true
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn is_supported() -> bool {
    false
}

#[tauri::command]
pub fn get_translucency_support() -> bool {
    is_supported()
}

// Backdrop on, or off again — the window itself stays transparent so this can flip at runtime
#[cfg(windows)]
pub fn apply(app: &AppHandle, enabled: bool) {
    use std::sync::Mutex;
    use tauri::window::{Color, Effect, EffectsBuilder};
    use tauri::Manager;

    // save_settings runs on every setting change; re-applying an unchanged backdrop would flicker it
    static LAST: Mutex<Option<bool>> = Mutex::new(None);

    let Some(window) = app.get_webview_window("main") else { return };
    if !is_supported() {
        return;
    }
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if *last == Some(enabled) {
            return;
        }
        *last = Some(enabled);
    }

    if enabled {
        let effects = if windows_build() >= ACRYLIC_MIN_BUILD {
            EffectsBuilder::new().effect(Effect::Acrylic).color(Color(18, 17, 22, 160)).build()
        } else {
            EffectsBuilder::new().effect(Effect::MicaDark).build()
        };
        let _ = window.set_effects(effects);
        let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    } else {
        // Dropped only after the page has faded to opaque, and only if it wasn't switched back on meanwhile
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(FADE_OUT_MS));
            let last = LAST.lock().unwrap_or_else(|e| e.into_inner());
            if *last == Some(false) {
                let _ = window.set_effects(None);
                let _ = window.set_background_color(Some(Color(20, 19, 23, 255)));
            }
        });
    }
}

#[cfg(target_os = "macos")]
pub fn apply(app: &AppHandle, enabled: bool) {
    use std::sync::Mutex;
    use tauri::window::{Color, Effect, EffectsBuilder};
    use tauri::Manager;

    static LAST: Mutex<Option<bool>> = Mutex::new(None);

    let Some(window) = app.get_webview_window("main") else { return };
    {
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if *last == Some(enabled) {
            return;
        }
        *last = Some(enabled);
    }

    if enabled {
        let effects = EffectsBuilder::new()
            .effect(Effect::HudWindow)
            .radius(16.0)
            .state(tauri::window::EffectState::Active)
            .build();
        let _ = window.set_effects(effects);
        let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    } else {
        let _ = window.set_effects(None);
        let _ = window.set_background_color(Some(Color(20, 19, 23, 255)));
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn apply(_app: &AppHandle, _enabled: bool) {}
