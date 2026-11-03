use std::io::Cursor;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::bridge::{Bridge, BridgeStatus, ConnectionInfo, JobResult};
use crate::error::LibError;
use crate::images::{hdr_preview_data_url, hdr_size, image_preview_data_url};
use crate::library::{AssetEntry, Library, PlannedSave};
use crate::model::AssetDraft;
use crate::parse::{parse_model as parse_model_file, ParsedModel};
use crate::pipeline;
use crate::render::{Film, RenderOptions};
use crate::settings::{settings_path, Settings};
use crate::AppState;

fn lock<'a, T>(m: &'a Mutex<T>) -> Result<MutexGuard<'a, T>, LibError> {
    m.lock()
        .map_err(|_| LibError::Io(std::io::Error::other("state lock poisoned")))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HdrPreview {
    pub width: u32,
    pub height: u32,
    pub data_url: String,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, LibError> {
    Ok(lock(&state.settings)?.clone())
}

#[tauri::command]
pub async fn set_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), LibError> {
    settings.save(&settings_path())?;
    let root = settings.library_root.clone();
    std::fs::create_dir_all(&root)?;
    app.asset_protocol_scope()
        .allow_directory(&root, true)
        .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))?;
    let old_port = lock(&state.settings)?.port;
    let new_port = settings.port;
    *lock(&state.settings)? = settings;
    *lock(&state.library)? = Library { root };
    if new_port != old_port {
        if let Some(old_bridge) = lock(&state.bridge)?.take() {
            old_bridge.shutdown();
        }
        let handle = app.clone();
        let on_change: std::sync::Arc<dyn Fn(Vec<ConnectionInfo>) + Send + Sync> =
            std::sync::Arc::new(move |conns: Vec<ConnectionInfo>| {
                let _ = handle.emit("bridge://connections", conns);
            });
        let status = match Bridge::start(new_port, on_change).await {
            Ok(bridge) => {
                let bound = bridge.port();
                *lock(&state.bridge)? = Some(bridge);
                BridgeStatus::Online { port: bound }
            }
            Err(error) => {
                let reason = match &error {
                    LibError::Bridge(message) => message.clone(),
                    other => other.to_string(),
                };
                BridgeStatus::Offline { port: new_port, reason }
            }
        };
        *lock(&state.bridge_status)? = status.clone();
        let _ = app.emit("bridge://status", status);
    }
    Ok(())
}

#[tauri::command]
pub async fn list_assets(state: State<'_, AppState>) -> Result<Vec<AssetEntry>, LibError> {
    let root = lock(&state.library)?.root.clone();
    tauri::async_runtime::spawn_blocking(move || Library { root }.scan())
        .await
        .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))
}

#[tauri::command]
pub async fn parse_model(path: PathBuf) -> Result<ParsedModel, LibError> {
    tauri::async_runtime::spawn_blocking(move || parse_model_file(&path))
        .await
        .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))?
}

#[tauri::command]
pub async fn hdr_preview(
    path: PathBuf,
    rotation: f32,
    intensity: f32,
) -> Result<HdrPreview, LibError> {
    tauri::async_runtime::spawn_blocking(move || {
        let (width, height) = hdr_size(&path)?;
        let data_url = hdr_preview_data_url(&path, rotation, intensity)?;
        Ok(HdrPreview {
            width,
            height,
            data_url,
        })
    })
    .await
    .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))?
}

#[tauri::command]
pub async fn preview_source(
    source: crate::mesh::PreviewSource,
) -> Result<tauri::ipc::Response, LibError> {
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        crate::mesh::load_source(&source).map(|m| m.to_preview_bytes())
    })
    .await
    .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))??;
    Ok(tauri::ipc::Response::new(bytes))
}

#[tauri::command]
pub async fn image_preview(path: PathBuf) -> Result<String, LibError> {
    tauri::async_runtime::spawn_blocking(move || image_preview_data_url(&path))
        .await
        .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))?
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderProgressEvent {
    pub samples: u32,
    pub elapsed: f32,
    pub seconds: f32,
    pub preview_data_url: String,
    pub denoised: bool,
}

/// 256 px PNG data URL of the current film, for the dialog's
/// progressive preview. Best effort: empty on encode failure.
fn preview_data_url(film: &Film) -> String {
    let img = film.to_rgba8(0.0);
    let small = image::imageops::resize(&img, 256, 256, image::imageops::FilterType::Triangle);
    let mut buf = Cursor::new(Vec::new());
    if image::DynamicImage::ImageRgba8(small)
        .write_to(&mut buf, image::ImageFormat::Png)
        .is_err()
    {
        return String::new();
    }
    let encoded = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
    format!("data:image/png;base64,{encoded}")
}

#[tauri::command]
pub async fn save_asset(
    app: AppHandle,
    state: State<'_, AppState>,
    draft: AssetDraft,
) -> Result<AssetEntry, LibError> {
    let root = lock(&state.library)?.root.clone();
    let cancel = state.cancel.clone();
    cancel.store(false, Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = Library { root };
        let opts = RenderOptions {
            size: 512,
            seconds: 10.0,
            max_samples: None,
            seed: 0,
            denoise: true,
        };
        let mut last_emit = Instant::now() - Duration::from_millis(500);
        pipeline::add_asset(&lib, &draft, &opts, &cancel, |progress, film| {
            // The final denoised event is never throttled away.
            if progress.denoised || last_emit.elapsed() >= Duration::from_millis(500) {
                last_emit = Instant::now();
                let _ = app.emit(
                    "render://progress",
                    RenderProgressEvent {
                        samples: progress.samples,
                        elapsed: progress.elapsed,
                        seconds: opts.seconds,
                        preview_data_url: preview_data_url(film),
                        denoised: progress.denoised,
                    },
                );
            }
        })
    })
    .await
    .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))?
}

#[tauri::command]
pub fn cancel_render(state: State<'_, AppState>) {
    state.cancel.store(true, Ordering::Relaxed);
}

/// Where the bundled `uhub` CLI lives: next to the app executable.
#[tauri::command]
pub fn cli_path() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent()
                .map(|dir| dir.join(if cfg!(windows) { "uhub.exe" } else { "uhub" }))
        })
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "uhub".to_string())
}

#[tauri::command]
pub async fn plan_save(
    state: State<'_, AppState>,
    draft: AssetDraft,
) -> Result<PlannedSave, LibError> {
    let root = lock(&state.library)?.root.clone();
    let planned = tauri::async_runtime::spawn_blocking(move || {
        // plan() never fails, so the Saves-to card always has a
        // target (with <Category>/<Name> placeholders while the
        // fields are empty); validation errors surface separately.
        let library = Library { root };
        let mut planned = library.plan(draft.asset_type, &draft.category, &draft.name);
        planned.files = crate::library::planned_files(&draft, &planned.dir_name);
        planned
    })
    .await
    .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))?;
    Ok(planned)
}

#[tauri::command]
pub async fn delete_asset(state: State<'_, AppState>, dir: PathBuf) -> Result<(), LibError> {
    let root = lock(&state.library)?.root.clone();
    tauri::async_runtime::spawn_blocking(move || Library { root }.delete(&dir))
        .await
        .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))?
}

#[tauri::command]
pub async fn reveal_asset(app: AppHandle, dir: PathBuf) -> Result<(), LibError> {
    app.opener()
        .reveal_item_in_dir(dir)
        .map_err(|e| LibError::Io(std::io::Error::other(e.to_string())))
}

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<ConnectionInfo>, LibError> {
    let bridge = lock(&state.bridge)?.clone();
    Ok(bridge.map(|b| b.connections()).unwrap_or_default())
}

#[tauri::command]
pub async fn get_bridge_status(state: State<'_, AppState>) -> Result<BridgeStatus, LibError> {
    Ok(lock(&state.bridge_status)?.clone())
}

#[tauri::command]
pub async fn export_asset(
    state: State<'_, AppState>,
    conn_id: u64,
    renderer: String,
    asset_dir: String,
) -> Result<JobResult, LibError> {
    let bridge = lock(&state.bridge)?
        .clone()
        .ok_or_else(|| LibError::Bridge("Bridge offline".to_string()))?;
    let root = lock(&state.library)?.root.clone();
    bridge
        .send_import(
            conn_id,
            &renderer,
            &root,
            std::path::Path::new(&asset_dir),
            Duration::from_secs(120),
        )
        .await
}