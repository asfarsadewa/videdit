mod ffmpeg;

use ffmpeg::{Segment, VideoInfo};

#[tauri::command]
fn get_video_info(app: tauri::AppHandle, path: String) -> Result<VideoInfo, String> {
    ffmpeg::probe_video(&app, &path)
}

#[tauri::command]
fn export_video(
    app: tauri::AppHandle,
    input_path: String,
    segments: Vec<Segment>,
    output_path: String,
    merge: bool,
) -> Result<String, String> {
    ffmpeg::export_segments(&app, &input_path, &segments, &output_path, merge)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_video_info, export_video])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
