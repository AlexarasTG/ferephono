use dispatch2::MainThreadBound;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSMenu, NSMenuDelegate, NSModalResponseOK, NSOpenPanel, NSWindow};
use objc2_foundation::{NSObject, NSObjectProtocol, NSString};
use tauri::{Emitter, Manager};

use super::PermissionManager;

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = tauri::AppHandle]
    struct StatusMenuDelegate;

    unsafe impl NSObjectProtocol for StatusMenuDelegate {}

    unsafe impl NSMenuDelegate for StatusMenuDelegate {
        #[unsafe(method(menuWillOpen:))]
        fn menu_will_open(&self, _menu: &NSMenu) {
            let permissions = PermissionManager::new().snapshot();
            let _ = self.ivars().emit("permissions-changed", permissions);
        }
    }
);

pub fn install(app: &tauri::AppHandle, tray: &tauri::tray::TrayIcon) -> Result<(), String> {
    let webview = app
        .get_webview_window("main")
        .ok_or("The status webview was not created.")?;
    let source_window = webview.ns_window().map_err(|error| error.to_string())? as usize;

    let app = app.clone();
    tray.with_inner_tray_icon(move |inner| -> Result<(), String> {
        let mtm =
            MainThreadMarker::new().ok_or("Status menu must be created on the main thread.")?;
        let status_item = inner
            .ns_status_item()
            .ok_or("The macOS status item is unavailable.")?;
        // Use Tauri's registered menu so its tray click handler tracks this same menu.
        let menu = status_item.menu(mtm).ok_or("The status menu is missing.")?;
        let menu_item = menu
            .itemAtIndex(0)
            .ok_or("The status menu item is missing.")?;
        // SAFETY: Tauri owns the source window; this closure runs on the main thread.
        let source = unsafe { &*(source_window as *const NSWindow) };
        let content = source
            .contentView()
            .ok_or("The status content view is missing.")?;
        source.setContentView(None);

        menu_item.setView(Some(&content));
        menu.setAutoenablesItems(false);
        // NSMenu keeps its delegate weakly; the app owns it for the tray's lifetime.
        let delegate = StatusMenuDelegate::alloc(mtm).set_ivars(app.clone());
        let delegate: Retained<StatusMenuDelegate> = unsafe { msg_send![super(delegate), init] };
        menu.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        app.manage(MainThreadBound::new(delegate, mtm));
        Ok(())
    })
    .map_err(|error| format!("Unable to access the macOS status item: {error}"))?
}

pub fn show(tray: &tauri::tray::TrayIcon) -> Result<(), String> {
    tray.with_inner_tray_icon(|inner| -> Result<(), String> {
        let mtm = MainThreadMarker::new().ok_or("Status menu must be shown on the main thread.")?;
        let status_item = inner
            .ns_status_item()
            .ok_or("The macOS status item is unavailable.")?;
        let button = status_item
            .button(mtm)
            .ok_or("The status button is unavailable.")?;
        unsafe { button.performClick(None) };
        Ok(())
    })
    .map_err(|error| format!("Unable to access the macOS status item: {error}"))?
}

pub fn choose_folder(
    app: &tauri::AppHandle,
    callback: impl FnOnce(Option<std::path::PathBuf>) + Send + 'static,
) -> Result<(), String> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let mtm = MainThreadMarker::new().expect("folder picker runs on the main thread");
        if let Some(tray) = handle.tray_by_id("main") {
            let _ = tray.with_inner_tray_icon(move |inner| {
                let mtm = MainThreadMarker::new().expect("menu access runs on the main thread");
                if let Some(menu) = inner.ns_status_item().and_then(|item| item.menu(mtm)) {
                    menu.cancelTracking();
                }
            });
        }
        // The status window has no content; a sheet would expose that empty window.
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseDirectories(true);
        panel.setCanChooseFiles(false);
        panel.setAllowsMultipleSelection(false);
        panel.setMessage(Some(&NSString::from_str(
            "Choose folder for filename matching",
        )));
        log::debug!("opening standalone filename matching folder picker");
        let folder = if panel.runModal() == NSModalResponseOK {
            panel
                .URL()
                .and_then(|url| url.path())
                .map(|path| std::path::PathBuf::from(path.to_string()))
        } else {
            None
        };
        panel.orderOut(None);
        log::debug!(
            "filename matching folder picker closed: selected={}",
            folder.is_some()
        );
        callback(folder);
    })
    .map_err(|e| format!("Unable to open folder picker: {e}"))
}
