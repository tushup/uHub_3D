pub mod commands;
pub mod error;
pub mod images;
pub mod library;
pub mod model;
pub mod naming;
pub mod objfix;
pub mod parse;
pub mod pipeline;
pub mod mesh;
pub mod cli;
pub mod render;
pub mod settings;
pub mod view;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
#[cfg(target_os = "macos")]
use tauri::Emitter;
use tauri::Manager;

use crate::library::Library;
use crate::settings::{settings_path, Settings};

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub library: Mutex<Library>,
    /// Set by `cancel_render`; read by the render loop.
    pub cancel: Arc<AtomicBool>,
}

// Native menu exists on macOS only (D-18). Windows/Linux use the
// in-window chrome, the New popover and shortcuts instead.
#[cfg(target_os = "macos")]
fn build_menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let preferences = MenuItem::with_id(app, "preferences", "Preferences…", true, Some("Cmd+,"))?;
    let app_menu = Submenu::with_items(
        app,
        "uHub 3D",
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &preferences,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;
    let new_3d = MenuItem::with_id(app, "new-asset-model", "3D Asset", true, None::<&str>)?;
    let new_material = MenuItem::with_id(app, "new-asset-material", "Material", true, None::<&str>)?;
    let new_hdri = MenuItem::with_id(app, "new-asset-hdri", "HDRI", true, None::<&str>)?;
    let new_asset = Submenu::with_items(app, "New Asset", true, &[&new_3d, &new_material, &new_hdri])?;
    let file = Submenu::with_items(app, "File", true, &[&new_asset])?;
    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let window = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, None)?,
            &PredefinedMenuItem::close(app, None)?,
        ],
    )?;
    let help = Submenu::with_items(
        app,
        "Help",
        true,
        &[&PredefinedMenuItem::about(app, Some("About uHub 3D"), None)?],
    )?;
    Menu::with_items(app, &[&app_menu, &file, &edit, &window, &help])
}

#[cfg(target_os = "macos")]
fn handle_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "new-asset-model" => {
            let _ = app.emit("menu://new-asset", "model");
        }
        "new-asset-material" => {
            let _ = app.emit("menu://new-asset", "material");
        }
        "new-asset-hdri" => {
            let _ = app.emit("menu://new-asset", "hdri");
        }
        "preferences" => {
            let _ = app.emit("menu://preferences", ());
        }
        _ => {}
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let settings = Settings::load(&settings_path());
    let library = Library {
        root: settings.library_root.clone(),
    };
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            settings: Mutex::new(settings),
            library: Mutex::new(library),
            cancel: Arc::new(AtomicBool::new(false)),
        });
    #[cfg(target_os = "macos")]
    let builder = builder.menu(build_menu).on_menu_event(handle_menu_event);
    builder
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::set_settings,
            commands::list_assets,
            commands::parse_model,
            commands::hdr_preview,
            commands::image_preview,
            commands::preview_source,
            commands::save_asset,
            commands::plan_save,
            commands::cancel_render,
            commands::cli_path,
            commands::delete_asset,
            commands::reveal_asset,
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            let root = state
                .library
                .lock()
                .map(|l| l.root.clone())
                .unwrap_or_default();
            std::fs::create_dir_all(&root)?;
            app.asset_protocol_scope().allow_directory(&root, true)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
