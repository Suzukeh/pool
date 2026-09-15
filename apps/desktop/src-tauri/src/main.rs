#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use pool_ffmpeg_io::MediaInfo;
use pool_plugin_sdk::Manifest;
use pool_render_core::Renderer;
use pool_timeline_model::Project;
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager};

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// 新規プロジェクトの JSON を返す。
#[tauri::command]
fn new_project(name: String) -> Result<String, String> {
    Project::new(name).to_json().map_err(|e| e.to_string())
}

/// フロントの編集結果を Rust モデルで再検証する（単一真実源）。
#[tauri::command]
fn validate_project(json: String) -> Result<(), String> {
    Project::from_json(&json).map(|_| ()).map_err(|e| e.to_string())
}

fn project_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("project.json"))
}

/// 検証通過後のみ保存する。
#[tauri::command]
fn save_project(app: tauri::AppHandle, json: String) -> Result<String, String> {
    Project::from_json(&json).map_err(|e| e.to_string())?;
    let path = project_file(&app)?;
    fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
fn load_project(app: tauri::AppHandle) -> Result<String, String> {
    let path = project_file(&app)?;
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// プレビュー用 PNG（base64）を返す。M2: 低解像度想定、Renderer は使い回す。
#[tauri::command]
fn render_preview(
    renderer: tauri::State<'_, Mutex<Option<Renderer>>>,
    json: String,
    frame: i64,
    width: u32,
    height: u32,
) -> Result<String, String> {
    let project: Project = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    let scene = project.active_scene().ok_or_else(|| "no active scene".to_string())?;
    let w = width.clamp(16, 960).max(1);
    let h = height.clamp(16, 540).max(1);
    let mut guard = renderer.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(Renderer::new().map_err(|e| e.to_string())?);
    }
    let r = guard.as_mut().ok_or_else(|| "renderer init failed".to_string())?;
    let png = r.render_png(scene, frame, w, h).map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

// ---------------------------------------------------------------------------
// M5: メディア・書出し・プラグイン・設定
// ---------------------------------------------------------------------------

#[tauri::command]
fn probe_media(path: String) -> Result<MediaInfo, String> {
    pool_ffmpeg_io::probe(std::path::Path::new(&path)).map_err(|e| e.to_string())
}

#[derive(Clone, serde::Serialize)]
struct ExportProgress {
    frame: i64,
    total: i64,
}

/// タイムライン全編を mp4 / PNG連番で書き出す（重いので blocking 実行）。
#[tauri::command]
async fn export_movie(
    app: tauri::AppHandle,
    json: String,
    out_path: String,
    width: u32,
    height: u32,
    format: String,
) -> Result<String, String> {
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        export_blocking(&json, &out_path, width, height, &format, &|frame, total| {
            let _ = app2.emit(
                "export-progress",
                ExportProgress { frame, total },
            );
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn export_blocking(
    json: &str,
    out_path: &str,
    width: u32,
    height: u32,
    format: &str,
    on_progress: &dyn Fn(i64, i64),
) -> Result<String, String> {
    let project: Project = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let scene = project.active_scene().ok_or_else(|| "no active scene".to_string())?;
    let fps = scene.fps_num as f64 / scene.fps_den.max(1) as f64;
    let total = scene.duration_frames().max(1);
    let w = width.clamp(16, 1920).max(1);
    let h = height.clamp(16, 1080).max(1);
    let mut renderer = Renderer::new().map_err(|e| e.to_string())?;
    if format == "pngseq" {
        let dir = PathBuf::from(out_path);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for f in 0..total {
            let png = renderer.render_png(scene, f, w, h).map_err(|e| e.to_string())?;
            fs::write(dir.join(format!("frame_{f:06}.png")), png).map_err(|e| e.to_string())?;
            on_progress(f + 1, total);
        }
        Ok(dir.to_string_lossy().into_owned())
    } else {
        let mut writer = pool_ffmpeg_io::Mp4Writer::new(PathBuf::from(out_path).as_path(), w, h, fps)
            .map_err(|e| e.to_string())?;
        for f in 0..total {
            let rgba = renderer.render_scene(scene, f, w, h).map_err(|e| e.to_string())?;
            writer.write_frame(&rgba).map_err(|e| e.to_string())?;
            on_progress(f + 1, total);
        }
        writer.finish().map_err(|e| e.to_string())?;
        Ok(out_path.to_string())
    }
}

#[derive(Clone, serde::Serialize)]
struct ImportedMedia {
    path: String,
    thumb: String,
    info: MediaInfo,
}

/// メディア取込：probe＋サムネイル生成（実ファイルは参照のみ）。
#[tauri::command]
fn import_media(app: tauri::AppHandle, path: String) -> Result<ImportedMedia, String> {
    let info = pool_ffmpeg_io::probe(std::path::Path::new(&path)).map_err(|e| e.to_string())?;
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("thumbs");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    let thumb_path = dir.join(format!("{:x}.png", hasher.finish()));
    if info.has_video {
        pool_ffmpeg_io::thumbnail(std::path::Path::new(&path), &thumb_path, 160)
            .map_err(|e| e.to_string())?;
    }
    let thumb = if thumb_path.is_file() {
        let bytes = fs::read(&thumb_path).map_err(|e| e.to_string())?;
        format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))
    } else {
        String::new()
    };
    Ok(ImportedMedia { path, thumb, info })
}

fn plugins_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?.join("plugins");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[tauri::command]
fn list_plugins(app: tauri::AppHandle) -> Result<Vec<Manifest>, String> {
    let dir = plugins_dir(&app)?;
    let mut out = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let manifest_path = entry.path().join("manifest.json");
        if let Ok(json) = fs::read_to_string(&manifest_path) {
            if let Ok(m) = Manifest::load(&json) {
                out.push(m);
            }
        }
    }
    Ok(out)
}

/// zip プラグインを導入する（au2pkg 相当の D&D 受け口）。
#[tauri::command]
fn install_plugin_zip(app: tauri::AppHandle, zip_path: String) -> Result<Manifest, String> {
    let file = fs::File::open(&zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let tmp = std::env::temp_dir().join(format!("pool-plugin-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    archive.extract(&tmp).map_err(|e| e.to_string())?;
    // 直下 or 単一サブフォルダの manifest.json を探す
    let root = if tmp.join("manifest.json").is_file() {
        tmp.clone()
    } else {
        let mut subs: Vec<PathBuf> = fs::read_dir(&tmp)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && p.join("manifest.json").is_file())
            .collect();
        if subs.len() != 1 {
            return Err("zip: manifest.json が見つからない（直下か単一フォルダに配置）".to_string());
        }
        subs.pop().unwrap()
    };
    let manifest = pool_plugin_host::install_from_dir(&root, &plugins_dir(&app)?)
        .map_err(|e| e.to_string())?;
    let _ = fs::remove_dir_all(&tmp);
    Ok(manifest)
}

#[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
struct Prefs {
    last_export_dir: Option<String>,
    last_import_dir: Option<String>,
}

fn prefs_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| e.to_string())?.join("ui.json"))
}

#[tauri::command]
fn get_prefs(app: tauri::AppHandle) -> Result<Prefs, String> {
    let path = prefs_path(&app)?;
    if !path.is_file() {
        return Ok(Prefs::default());
    }
    let json = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&json).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_prefs(app: tauri::AppHandle, prefs: Prefs) -> Result<(), String> {
    let path = prefs_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&prefs).map_err(|e| e.to_string())?;
    fs::write(&path, json).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(None::<Renderer>))
        .invoke_handler(tauri::generate_handler![
            get_app_version,
            new_project,
            validate_project,
            save_project,
            load_project,
            render_preview,
            probe_media,
            export_movie,
            import_media,
            list_plugins,
            install_plugin_zip,
            get_prefs,
            set_prefs
        ])
        .run(tauri::generate_context!())
        .expect("failed to run pool");
}
