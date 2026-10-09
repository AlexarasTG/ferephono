use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dispatch2::MainThreadBound;
use ferephono::DictationState;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSPanel, NSScreen, NSWindow, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

const WIDTH: f64 = 360.0;
const HEIGHT: f64 = 160.0;
const BOTTOM_MARGIN: f64 = 16.0;

pub struct DictationOverlay {
    panel: MainThreadBound<Retained<NSPanel>>,
    revision: AtomicU64,
}

impl DictationOverlay {
    pub fn new(app: &tauri::AppHandle) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let mtm = MainThreadMarker::new().ok_or("Overlay must be created on the main thread")?;
        let webview = WebviewWindowBuilder::new(
            app,
            "dictation-overlay",
            WebviewUrl::App("dictation-overlay/index.html".into()),
        )
        .inner_size(WIDTH, HEIGHT)
        .visible(false)
        .focused(false)
        .focusable(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .skip_taskbar(true)
        .build()?;
        // A non-activating panel can appear over fullscreen apps without changing the insertion target.
        let panel = NSPanel::initWithContentRect_styleMask_backing_defer(
            NSPanel::alloc(mtm),
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(WIDTH, HEIGHT)),
            NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::Buffered,
            false,
        );
        panel.setOpaque(false);
        panel.setBackgroundColor(Some(&NSColor::clearColor()));
        panel.setHasShadow(false);
        panel.setIgnoresMouseEvents(true);
        panel.setHidesOnDeactivate(false);
        panel.setBecomesKeyOnlyIfNeeded(true);
        panel.setLevel(3); // AppKit floating-window level.
        panel.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        // SAFETY: The webview owns the source window, and we are on the main thread.
        let source = unsafe { &*webview.ns_window()?.cast::<NSWindow>() };
        let content = source
            .contentView()
            .ok_or("Overlay content view is missing")?;
        source.setContentView(None);
        panel.setContentView(Some(&content));
        Ok(Arc::new(Self {
            panel: MainThreadBound::new(panel, mtm),
            revision: AtomicU64::new(0),
        }))
    }

    pub fn update(self: &Arc<Self>, app: &tauri::AppHandle, state: &DictationState) {
        let revision = self.revision.fetch_add(1, Ordering::SeqCst) + 1;
        let presenter = self.clone();
        let handle = app.clone();
        let state = state.clone();
        let delay = match state {
            DictationState::Idle => Some(Duration::from_millis(350)),
            DictationState::Error(_) => Some(Duration::from_secs(6)),
            _ => None,
        };
        if let Err(error) = app.run_on_main_thread(move || {
            let mtm = MainThreadMarker::new().expect("scheduled on main thread");
            let panel = presenter.panel.get(mtm);
            if !matches!(state, DictationState::Idle) {
                if !panel.isVisible() {
                    if let Some(screen) = NSScreen::mainScreen(mtm) {
                        let frame = screen.visibleFrame();
                        panel.setFrameOrigin(NSPoint::new(
                            frame.origin.x + (frame.size.width - WIDTH) / 2.0,
                            frame.origin.y + BOTTOM_MARGIN,
                        ));
                    }
                }
                panel.orderFrontRegardless();
            }
            if let Err(error) = handle.emit_to("dictation-overlay", "overlay-state-changed", &state)
            {
                log::error!("unable to send dictation overlay state: {error}");
            }
        }) {
            log::error!("unable to update dictation overlay: {error}");
        }
        if let Some(delay) = delay {
            let presenter = self.clone();
            let app = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                let _ = app.run_on_main_thread(move || {
                    // An earlier completion/error must never hide a newer recording.
                    if presenter.revision.load(Ordering::SeqCst) == revision {
                        let mtm = MainThreadMarker::new().expect("scheduled on main thread");
                        presenter.panel.get(mtm).orderOut(None);
                    }
                });
            });
        }
    }
}

pub fn update(app: &tauri::AppHandle, state: &DictationState) {
    app.state::<Arc<DictationOverlay>>().update(app, state);
}
