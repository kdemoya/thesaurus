//! A transient tray surface. The pinned companion keeps its own window and position.
use super::*;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{WebviewUrl, WebviewWindow, WebviewWindowBuilder};

static LAST_BLUR: Mutex<Option<Instant>> = Mutex::new(None);
static LAST_ANCHOR: Mutex<Option<tauri::Rect>> = Mutex::new(None);
const WIDTH: f64 = 352.;

#[cfg(target_os = "macos")]
mod macos;

static HEIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(260);

fn hide(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    return macos::hide(window);
    #[cfg(not(target_os = "macos"))]
    window.hide().map_err(|e| e.to_string())
}
fn is_visible(window: &WebviewWindow) -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    return macos::is_visible(window);
    #[cfg(not(target_os = "macos"))]
    window.is_visible().map_err(|e| e.to_string())
}
/// Provider-owned diagnostics; no raw native handle or other app data escapes.
pub fn status(app: &tauri::AppHandle) -> Result<Value, String> {
    let Some(window) = app.get_webview_window("meeting-popover") else {
        return Ok(json!({"visible":false,"on_active_space":false}));
    };
    #[cfg(target_os = "macos")]
    return macos::status(&window);
    #[cfg(not(target_os = "macos"))]
    Ok(json!({"visible":window.is_visible().map_err(|e|e.to_string())?,"on_active_space":null}))
}

fn position(app: &tauri::AppHandle, window: &WebviewWindow) -> Result<(), String> {
    let anchor = app
        .tray_by_id("meeting-tray")
        .and_then(|tray| tray.rect().ok().flatten())
        .or_else(|| LAST_ANCHOR.lock().ok().and_then(|r| *r));
    let primary = app.primary_monitor().map_err(|e| e.to_string())?;
    let guess = primary.as_ref().map(|m| m.scale_factor()).unwrap_or(1.);
    let point = anchor.map(|r| r.position.to_physical::<f64>(guess));
    let monitor = point
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or(primary);
    let Some(monitor) = monitor else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    // Measure in logical points before moving between monitors with different scales.
    let height = HEIGHT.load(std::sync::atomic::Ordering::Relaxed) as f64;
    let size = tauri::PhysicalSize::new(
        (WIDTH * scale).round() as u32,
        (height * scale).round() as u32,
    );
    let margin = (6. * scale).round() as i32;
    let min_x = work.position.x + margin;
    let min_y = work.position.y + margin;
    let max_x = (work.position.x + work.size.width as i32 - size.width as i32 - margin).max(min_x);
    let max_y =
        (work.position.y + work.size.height as i32 - size.height as i32 - margin).max(min_y);
    let (x, y) = if let Some(rect) = anchor {
        let p = rect.position.to_physical::<f64>(scale);
        let s = rect.size.to_physical::<f64>(scale);
        let x = (p.x + s.width / 2. - size.width as f64 / 2.).round() as i32;
        // Put the dropdown below a menu bar, or above a bottom-mounted taskbar.
        let bottom = (p.y + s.height + 4. * scale).round() as i32;
        let y = if bottom + size.height as i32 <= work.position.y + work.size.height as i32 {
            bottom
        } else {
            (p.y - 4. * scale).round() as i32 - size.height as i32
        };
        (x, y)
    } else {
        (max_x, min_y)
    };
    let x = x.clamp(min_x, max_x);
    let y = y.clamp(min_y, max_y);
    #[cfg(target_os = "macos")]
    return macos::place(window, x, y, scale, height);
    #[cfg(not(target_os = "macos"))]
    {
        window
            .set_size(tauri::LogicalSize::new(WIDTH, height))
            .map_err(|e| e.to_string())?;
        window
            .set_position(tauri::PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())
    }
}

pub fn prepare(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window("meeting-popover") {
        #[cfg(target_os = "macos")]
        macos::prepare(&w)?;
        return Ok(w);
    }
    let window = WebviewWindowBuilder::new(
        app,
        "meeting-popover",
        WebviewUrl::App("index.html?tray=1".into()),
    )
    .title("Thesaurus quick controls")
    .inner_size(WIDTH, 260.)
    .decorations(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .always_on_top(true)
    .visible_on_all_workspaces(true)
    .skip_taskbar(true)
    .shadow(true)
    .visible(false)
    .focused(false)
    .build()
    .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    macos::prepare(&window)?;
    let handle = window.clone();
    window.on_window_event(move |event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = hide(&handle);
        }
        #[cfg(not(target_os = "macos"))]
        tauri::WindowEvent::Focused(false) => {
            if handle.is_visible().unwrap_or(false) {
                if let Ok(mut last) = LAST_BLUR.lock() {
                    *last = Some(Instant::now());
                }
                let _ = hide(&handle);
            }
        }
        _ => {}
    });
    Ok(window)
}
pub fn show(app: &tauri::AppHandle) -> Result<(), String> {
    let w = prepare(app)?;
    position(app, &w)?;
    #[cfg(target_os = "macos")]
    macos::show(&w)?;
    #[cfg(not(target_os = "macos"))]
    {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    }
    let _ = w.emit("tray-opened", json!({}));
    Ok(())
}
pub(super) fn remember_anchor(anchor: tauri::Rect) {
    if let Ok(mut saved) = LAST_ANCHOR.lock() {
        *saved = Some(anchor);
    }
}
pub(super) fn toggle_current(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("meeting-popover") {
        if is_visible(&w)? {
            return hide(&w);
        }
    }
    // Clicking the tray can blur the webview before the click reaches Rust.
    // Treat that same click as dismissal instead of immediately reopening it.
    if LAST_BLUR
        .lock()
        .ok()
        .and_then(|t| *t)
        .is_some_and(|t| t.elapsed() < Duration::from_millis(180))
    {
        return Ok(());
    }
    show(app)
}
pub fn dismiss(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("meeting-popover") {
        let _ = hide(&w);
    }
}
pub(super) fn teardown(app: &tauri::AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("meeting-popover") {
        #[cfg(target_os = "macos")]
        macos::teardown(&w)?;
        w.destroy().map_err(|e| e.to_string())?;
    }
    if let Ok(mut anchor) = LAST_ANCHOR.lock() {
        *anchor = None;
    }
    if let Ok(mut blur) = LAST_BLUR.lock() {
        *blur = None;
    }
    Ok(())
}
pub(super) fn set_height(app: &tauri::AppHandle, height: f64) -> Result<(), String> {
    if !height.is_finite() {
        return Err("Invalid popover size".into());
    }
    let window = app
        .get_webview_window("meeting-popover")
        .ok_or("Popover is unavailable")?;
    let height = height.ceil().clamp(180., 460.);
    let height = height as u32;
    if HEIGHT.swap(height, std::sync::atomic::Ordering::Relaxed) != height {
        position(app, &window)?;
    }
    Ok(())
}
