use crate::platform;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

pub(super) fn install(app: &tauri::App) -> tauri::Result<()> {
    let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../../icons/tray.png"))?;
    let panel_item =
        MenuItem::with_id(app, "status-menu", "Floating Whisprer", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&panel_item])?;

    let tray = TrayIconBuilder::with_id("main")
        .icon(tray_icon)
        .icon_as_template(true)
        .tooltip("Floating Whisprer")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .build(app)?;
    platform::install_status_menu(app.handle(), &tray).map_err(std::io::Error::other)?;
    Ok(())
}
