use std::cell::RefCell;
use std::sync::Arc;

use dispatch2::MainThreadBound;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSPopover, NSPopoverBehavior, NSView, NSViewController, NSWindow};
use objc2_foundation::NSRectEdge;
use tauri::Manager;

pub(super) struct PopoverPresenter {
    state: MainThreadBound<RefCell<Option<PresentedPopover>>>,
}

struct PresentedPopover {
    source_window: usize,
    popover: Retained<NSPopover>,
}

impl PopoverPresenter {
    pub(super) fn new() -> Result<Self, String> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| "Popover presenter must be created on the main thread.".to_owned())?;
        Ok(Self {
            state: MainThreadBound::new(RefCell::new(None), mtm),
        })
    }
}

#[derive(Clone, Copy)]
enum Presentation {
    Show,
    Toggle,
}

pub fn show(app: &tauri::AppHandle, tray: &tauri::tray::TrayIcon) -> Result<(), String> {
    present(app, tray, Presentation::Show)
}

pub fn toggle(app: &tauri::AppHandle, tray: &tauri::tray::TrayIcon) -> Result<(), String> {
    present(app, tray, Presentation::Toggle)
}

fn present(
    app: &tauri::AppHandle,
    tray: &tauri::tray::TrayIcon,
    presentation: Presentation,
) -> Result<(), String> {
    let button = tray
        .with_inner_tray_icon(|inner| {
            let mtm = MainThreadMarker::new()?;
            let status_item = inner.ns_status_item()?;
            let button = status_item.button(mtm)?;
            Some(Retained::as_ptr(&button) as usize)
        })
        .map_err(|error| format!("Unable to access the macOS status item: {error}"))?
        .ok_or_else(|| "The macOS status-item button is unavailable.".to_owned())?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "The main webview was not created.".to_owned())?;
    let presenter = app.state::<Arc<PopoverPresenter>>().inner().clone();

    window
        .with_webview(move |webview| {
            if let Err(error) =
                unsafe { presenter.present_on_main(webview.ns_window(), button, presentation) }
            {
                log::error!("unable to present native macOS popover: {error}");
            }
        })
        .map_err(|error| format!("Unable to access the macOS webview: {error}"))
}

impl PopoverPresenter {
    unsafe fn present_on_main(
        &self,
        window: *mut std::ffi::c_void,
        button: usize,
        presentation: Presentation,
    ) -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or_else(|| {
            "Popover presentation was not scheduled on the main thread.".to_owned()
        })?;
        let source_window = window as usize;
        let mut state = self.state.get(mtm).borrow_mut();

        if state
            .as_ref()
            .is_none_or(|presented| presented.source_window != source_window)
        {
            if let Some(presented) = state.take() {
                presented.popover.close();
            }
            *state = Some(unsafe { create_popover(window, source_window, mtm)? });
        }

        let popover = state
            .as_ref()
            .expect("popover was initialized")
            .popover
            .clone();
        drop(state);

        if popover.isShown() {
            if matches!(presentation, Presentation::Toggle) {
                popover.close();
            }
        } else {
            let button: &NSView = unsafe { &*(button as *const NSView) };
            popover.showRelativeToRect_ofView_preferredEdge(
                button.bounds(),
                button,
                NSRectEdge::MinY,
            );
        }
        Ok(())
    }
}

unsafe fn create_popover(
    window: *mut std::ffi::c_void,
    source_window: usize,
    mtm: MainThreadMarker,
) -> Result<PresentedPopover, String> {
    let window: &NSWindow = unsafe { &*window.cast() };
    let content = window
        .contentView()
        .ok_or_else(|| "The Tauri window has no content view.".to_owned())?;
    let content_size = content.frame().size;
    let controller = NSViewController::new(mtm);
    window.setContentView(None);
    controller.setView(&content);

    let popover = NSPopover::new(mtm);
    popover.setBehavior(NSPopoverBehavior::Transient);
    popover.setAnimates(false);
    popover.setContentSize(content_size);
    popover.setContentViewController(Some(&controller));

    Ok(PresentedPopover {
        source_window,
        popover,
    })
}
