//! The tray webview is hosted by a real nonactivating NSPanel. A regular
//! NSWindow plus collection flags still activates the app when Tauri focuses it,
//! which can switch away from another application's full-screen Space.
use super::*;
use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSBackingStoreType, NSPanel, NSPopUpMenuWindowLevel, NSScreen,
    NSView, NSWindow, NSWindowCollectionBehavior as Behavior, NSWindowStyleMask,
};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSSize, ns_string};
use std::cell::RefCell;

define_class!(
    // SAFETY: The panel is allocated as this class, never cast or swizzled from
    // a Tauri window. AppKit access and its retained owner stay on the main thread.
    #[unsafe(super = NSPanel)]
    #[thread_kind = MainThreadOnly]
    #[name = "ThesaurusTrayPanel"]
    struct TrayPanel;

    unsafe impl NSObjectProtocol for TrayPanel {}

    impl TrayPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key(&self) -> bool { true }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main(&self) -> bool { false }

        #[unsafe(method(resignKeyWindow))]
        fn resign_key(&self) {
            // SAFETY: Call the superclass implementation of this override.
            unsafe { let _: () = msg_send![super(self), resignKeyWindow]; }
            if self.isVisible() {
                if let Ok(mut last) = LAST_BLUR.lock() {
                    *last = Some(Instant::now());
                }
                self.orderOut(None);
            }
        }
    }
);

thread_local! {
    // NSPanel is deliberately not Send/Sync. Do not put it in Tauri's shared state.
    static PANEL: RefCell<Option<Retained<TrayPanel>>> = const { RefCell::new(None) };
}

fn on_main<T: Send + 'static>(
    window: &WebviewWindow,
    action: impl FnOnce(&WebviewWindow, MainThreadMarker) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    if let Some(main) = MainThreadMarker::new() {
        return action(window, main);
    }
    let owned = window.clone();
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    window
        .run_on_main_thread(move || {
            let result = MainThreadMarker::new()
                .ok_or_else(|| "Tray controls require the main thread".to_string())
                .and_then(|main| action(&owned, main));
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    receive
        .recv()
        .map_err(|_| "Tray controls were interrupted".to_string())?
}

fn panel() -> Result<Retained<TrayPanel>, String> {
    PANEL
        .with(|slot| slot.borrow().clone())
        .ok_or_else(|| "Tray panel is unavailable".into())
}

pub(super) fn teardown(window: &WebviewWindow) -> Result<(), String> {
    on_main(window, |window, _main| {
        if let Some(panel) = PANEL.with(|slot| slot.borrow_mut().take()) {
            panel.orderOut(None);
            if let Some(content) = panel.contentView() {
                let pointer = window.ns_window().map_err(|e| e.to_string())?;
                if pointer.is_null() {
                    return Err("Tray webview owner is unavailable".into());
                }
                // SAFETY: Both owners are live and this closure is on AppKit's
                // main thread. Return the view before releasing its panel.
                let original = unsafe { &*pointer.cast::<NSWindow>() };
                panel.setContentView(None);
                original.setContentView(Some(&content));
            }
            panel.close();
        }
        Ok(())
    })
}

pub(super) fn prepare(window: &WebviewWindow) -> Result<(), String> {
    on_main(window, |window, main| {
        if PANEL.with(|slot| slot.borrow().is_some()) {
            return Ok(());
        }
        let pointer = window.ns_window().map_err(|e| e.to_string())?;
        if pointer.is_null() {
            return Err("Tray webview is unavailable".into());
        }
        // SAFETY: Tauri owns this live NSWindow; this entire operation runs on
        // its main thread. Retain the content view before detaching it.
        let original = unsafe { &*pointer.cast::<NSWindow>() };
        let content = original
            .contentView()
            .ok_or("Tray content is unavailable")?;
        let responder = original.firstResponder();
        // Set NonactivatingPanel at initialization. Toggling that bit on an
        // already-created NSWindow does not establish the correct activation state.
        let panel: Retained<TrayPanel> = unsafe {
            msg_send![TrayPanel::alloc(main),
                initWithContentRect: NSRect::new(NSPoint::new(0., 0.), NSSize::new(WIDTH, 260.)),
                styleMask: NSWindowStyleMask::NonactivatingPanel,
                backing: NSBackingStoreType::Buffered,
                defer: false]
        };
        // SAFETY: The thread-local retained owner, rather than close(), releases it.
        unsafe {
            panel.setReleasedWhenClosed(false);
        }
        panel.setTitle(ns_string!("Thesaurus quick controls"));
        panel.setFloatingPanel(true);
        panel.setHidesOnDeactivate(false);
        panel.setBecomesKeyOnlyIfNeeded(false);
        panel.setHasShadow(true);
        panel.setLevel(NSPopUpMenuWindowLevel);
        let mut behavior = Behavior::CanJoinAllSpaces
            | Behavior::FullScreenAuxiliary
            | Behavior::FullScreenDisallowsTiling;
        if objc2::available!(macos = 13.0) {
            behavior.insert(Behavior::CanJoinAllApplications);
        }
        panel.setCollectionBehavior(behavior);
        // Keep Tauri's hidden window and webview owner for IPC/lifecycle, but
        // present its content through the panel. No runtime class replacement.
        // Tao still owns the hidden NSWindow and may ask for its NSView while
        // another window is destroyed or recreated. Keep a valid placeholder
        // in that window; a nil content view violates Tao's window contract.
        // The original view remains retained by this scope until the panel
        // takes ownership, and teardown returns it before destroying the owner.
        let placeholder = NSView::initWithFrame(NSView::alloc(main), content.frame());
        original.setContentView(Some(&placeholder));
        content.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );
        panel.setContentView(Some(&content));
        panel.setContentSize(NSSize::new(WIDTH, 260.));
        if let Some(responder) = responder {
            panel.makeFirstResponder(Some(&responder));
        }
        PANEL.with(|slot| *slot.borrow_mut() = Some(panel));
        Ok(())
    })
}

pub(super) fn place(
    window: &WebviewWindow,
    x: i32,
    y: i32,
    scale: f64,
    height: f64,
) -> Result<(), String> {
    on_main(window, move |_, main| {
        let panel = panel()?;
        // Tauri's global positions start at the primary display's top-left;
        // AppKit's start at its bottom-left. Match Tao's screen conversion.
        let screens = NSScreen::screens(main);
        let primary = screens.firstObject().ok_or("No display available")?.frame();
        panel.setContentSize(NSSize::new(WIDTH, height));
        panel.setFrameTopLeftPoint(NSPoint::new(
            x as f64 / scale,
            primary.origin.y + primary.size.height - y as f64 / scale,
        ));
        Ok(())
    })
}

pub(super) fn show(window: &WebviewWindow) -> Result<(), String> {
    on_main(window, |_, _| {
        let panel = panel()?;
        // Do not call Tauri show()/set_focus(): Tao activates NSApplication.
        // A nonactivating key panel accepts typing while the full-screen app
        // remains active, and returns keyboard focus when it is dismissed.
        panel.orderFrontRegardless();
        panel.makeKeyWindow();
        Ok(())
    })
}

pub(super) fn hide(window: &WebviewWindow) -> Result<(), String> {
    on_main(window, |_, _| {
        panel()?.orderOut(None);
        Ok(())
    })
}

pub(super) fn is_visible(window: &WebviewWindow) -> Result<bool, String> {
    on_main(window, |_, _| Ok(panel()?.isVisible()))
}
pub(super) fn status(window: &WebviewWindow) -> Result<Value, String> {
    on_main(window, |_, _| {
        let panel = panel()?;
        Ok(
            json!({"visible":panel.isVisible(),"on_active_space":panel.isOnActiveSpace(),"full_screen_auxiliary":panel.collectionBehavior().contains(Behavior::FullScreenAuxiliary),"nonactivating":panel.styleMask().contains(NSWindowStyleMask::NonactivatingPanel)}),
        )
    })
}
