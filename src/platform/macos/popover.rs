use std::sync::atomic::{AtomicPtr, Ordering};

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{NSPopover, NSPopoverBehavior, NSView, NSViewController, NSWindow};
use objc2_foundation::NSRectEdge;
use tauri::Manager;

static POPOVER: AtomicPtr<NSPopover> = AtomicPtr::new(std::ptr::null_mut());

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

    window
        .with_webview(move |webview| {
            if let Err(error) =
                unsafe { present_on_main(webview.ns_window(), button, presentation) }
            {
                log::error!("unable to present native macOS popover: {error}");
            }
        })
        .map_err(|error| format!("Unable to access the macOS webview: {error}"))
}

unsafe fn present_on_main(
    window: *mut std::ffi::c_void,
    button: usize,
    presentation: Presentation,
) -> Result<(), String> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "Popover presentation was not scheduled on the main thread.".to_owned())?;
    let popover = POPOVER.load(Ordering::Acquire);
    let popover = if popover.is_null() {
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
        let popover = Retained::into_raw(popover);
        POPOVER.store(popover, Ordering::Release);
        popover
    } else {
        popover
    };
    let popover = unsafe { &*popover };

    if popover.isShown() {
        if matches!(presentation, Presentation::Toggle) {
            popover.close();
        }
    } else {
        let button: &NSView = unsafe { &*(button as *const NSView) };
        popover.showRelativeToRect_ofView_preferredEdge(button.bounds(), button, NSRectEdge::MinY);
    }
    Ok(())
}
