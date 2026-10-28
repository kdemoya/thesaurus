use crate::capture::{command, read_bounded};
use crate::{private_dir, private_write};
use anyhow::{Context, Result, bail};
use std::path::Path;
use thesaurus_contracts::CaptureWindow;
use tokio::process::Command;
#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::ffi::{c_char, c_void};
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGWindowListCopyWindowInfo(options: u32, relative: u32) -> *const c_void;
        fn CGRequestScreenCaptureAccess() -> bool;
        fn CGPreflightScreenCaptureAccess() -> bool;
        static kCGWindowNumber: *const c_void;
        static kCGWindowOwnerPID: *const c_void;
        static kCGWindowOwnerName: *const c_void;
        static kCGWindowName: *const c_void;
        static kCGWindowLayer: *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
        fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
        fn CFNumberGetValue(number: *const c_void, kind: isize, value: *mut c_void) -> bool;
        fn CFStringGetCString(
            string: *const c_void,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> bool;
        fn CFRelease(value: *const c_void);
    }
    unsafe fn number(dict: *const c_void, key: *const c_void) -> i64 {
        let v = unsafe { CFDictionaryGetValue(dict, key) };
        if v.is_null() {
            return -1;
        }
        let mut n = 0i64;
        if unsafe { CFNumberGetValue(v, 4, (&mut n as *mut i64).cast()) } {
            n
        } else {
            -1
        }
    }
    unsafe fn string(dict: *const c_void, key: *const c_void) -> String {
        let v = unsafe { CFDictionaryGetValue(dict, key) };
        if v.is_null() {
            return String::new();
        }
        let mut bytes = [0u8; 4096];
        if unsafe {
            CFStringGetCString(
                v,
                bytes.as_mut_ptr().cast(),
                bytes.len() as isize,
                0x08000100,
            )
        } {
            let n = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            String::from_utf8_lossy(&bytes[..n]).into_owned()
        } else {
            String::new()
        }
    }
    pub fn permission() -> bool {
        unsafe { CGRequestScreenCaptureAccess() }
    }
    pub fn permission_status() -> bool {
        unsafe { CGPreflightScreenCaptureAccess() }
    }
    pub fn windows() -> Result<Vec<CaptureWindow>> {
        unsafe {
            let array = CGWindowListCopyWindowInfo(1 | 16, 0);
            if array.is_null() {
                bail!(
                    "invalid: cannot enumerate capture windows; check macOS screen recording permission"
                );
            }
            let mut windows = vec![];
            for n in 0..CFArrayGetCount(array).min(1000) {
                let d = CFArrayGetValueAtIndex(array, n);
                let id = number(d, kCGWindowNumber);
                let pid = number(d, kCGWindowOwnerPID);
                if number(d, kCGWindowLayer) != 0 || id <= 0 || id > u32::MAX as i64 || pid <= 0 {
                    continue;
                }
                let app = string(d, kCGWindowOwnerName);
                let title = string(d, kCGWindowName);
                if !app.is_empty() && !title.is_empty() {
                    windows.push(CaptureWindow {
                        window_id: id as u32,
                        process_id: pid,
                        app,
                        title,
                    });
                }
            }
            CFRelease(array);
            Ok(windows)
        }
    }
}
pub fn capture_permission_status() -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        Ok(mac::permission_status())
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("unsupported: macOS capture permission is unavailable on this platform")
    }
}
pub fn request_capture_permission() -> Result<bool> {
    #[cfg(target_os = "macos")]
    {
        Ok(mac::permission())
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("invalid: macOS capture permission is unavailable on this platform")
    }
}
pub fn capture_windows() -> Result<Vec<CaptureWindow>> {
    #[cfg(target_os = "macos")]
    {
        mac::windows()
    }
    #[cfg(not(target_os = "macos"))]
    {
        bail!("invalid: selected-window activity capture currently requires macOS")
    }
}
pub(crate) async fn capture_png(window: &CaptureWindow, path: &Path) -> Result<Vec<u8>> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (window, path);
        bail!("invalid: activity capture requires macOS");
    }
    #[cfg(target_os = "macos")]
    {
        let selected=capture_windows()?.into_iter().find(|w|w.window_id==window.window_id && w.process_id==window.process_id && w.app==window.app && w.title==window.title).context("invalid: selected window is unavailable or changed title/app; reselect before capture")?;
        let mut c = Command::new("/usr/sbin/screencapture");
        c.args(["-x", "-o", "-t", "png", "-l"])
            .arg(selected.window_id.to_string())
            .arg(path);
        command(&mut c, 10, 1000).await?;
        // Window IDs can disappear or be reused while the system capture
        // command runs. Discard the temporary image before OCR or retention if
        // the approved identity no longer matches after capture.
        if !capture_windows()?.iter().any(|w| {
            w.window_id == window.window_id
                && w.process_id == window.process_id
                && w.app == window.app
                && w.title == window.title
        }) {
            bail!(
                "invalid: selected window changed during capture; frame discarded, reselect before capture"
            );
        }
        let bytes = read_bounded(path, 4_000_000).await?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            bail!("invalid: macOS did not return a PNG frame");
        }
        Ok(bytes)
    }
}
pub(crate) async fn ocr(path: &Path, dir: &Path) -> Result<String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (path, dir);
        bail!("invalid: local Vision OCR requires macOS");
    }
    #[cfg(target_os = "macos")]
    {
        let helper = dir.join("activity-tmp/recognize.swift");
        private_dir(helper.parent().context("invalid helper folder")?)?;
        private_write(
            &helper,
            include_str!("../../capture/recognize.swift").as_bytes(),
        )?;
        let cache = dir.join("activity-tmp/modules");
        private_dir(&cache)?;
        let processor = || {
            let mut c = Command::new("/usr/bin/swift");
            c.arg("-module-cache-path")
                .arg(&cache)
                .arg(&helper)
                .arg(path);
            c
        };
        // A failed Vision compute operation can poison later requests in that
        // process. Retry the supported older CPU revision in a fresh helper.
        // First use may be slow; both helpers share one overall deadline and
        // command() kills each child on timeout/cancellation.
        let bytes = tokio::time::timeout(std::time::Duration::from_secs(180), async {
            match command(&mut processor(), 120, 200000).await {
                Ok(bytes) => Ok(bytes),
                Err(_) => {
                    let mut compatibility = processor();
                    compatibility.arg("--cpu-compatibility");
                    command(&mut compatibility, 180, 200000).await.context(
                        "invalid: local OCR failed after a separate CPU compatibility attempt",
                    )
                }
            }
        })
        .await
        .context("invalid: local OCR processor timed out")??;
        let text = String::from_utf8(bytes)?;
        if text.len() > 100000 {
            bail!("invalid: retained OCR text exceeds100KB");
        }
        Ok(text)
    }
}
