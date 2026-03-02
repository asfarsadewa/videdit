mod audio_capture;
mod ffmpeg;
mod recording;

use std::sync::{Arc, Mutex};

use ffmpeg::{Segment, VideoInfo};
use recording::SharedRecordingState;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

#[tauri::command]
fn get_video_info(app: tauri::AppHandle, path: String) -> Result<VideoInfo, String> {
    ffmpeg::probe_video(&app, &path)
}

#[derive(Debug, serde::Deserialize)]
pub struct SubtitleInput {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[tauri::command]
fn export_video(
    app: tauri::AppHandle,
    input_path: String,
    segments: Vec<Segment>,
    subtitles: Vec<SubtitleInput>,
    output_path: String,
    merge: bool,
    compress: bool,
    quality: u32,
    burn_subtitles: bool,
) -> Result<String, String> {
    let subtitles: Vec<ffmpeg::Subtitle> = subtitles
        .into_iter()
        .map(|s| ffmpeg::Subtitle {
            start: s.start,
            end: s.end,
            text: s.text,
        })
        .collect();
    
    ffmpeg::export_segments(
        &app,
        &input_path,
        &segments,
        &subtitles,
        &output_path,
        merge,
        compress,
        quality,
        burn_subtitles,
    )
}

#[tauri::command]
fn start_screen_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedRecordingState>,
) -> Result<(), String> {
    recording::start_recording(&app, &state)
}

#[tauri::command]
fn stop_screen_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedRecordingState>,
) -> Result<String, String> {
    recording::stop_recording(&app, &state)
}

#[tauri::command]
fn cleanup_recording_temp(state: tauri::State<'_, SharedRecordingState>) {
    recording::cleanup_temp_file(&state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Arc::new(Mutex::new(recording::RecordingState::default())) as SharedRecordingState)
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Register global shortcuts
            let f9: Shortcut = "F9".parse().unwrap();
            let f10: Shortcut = "F10".parse().unwrap();

            let handle = app.handle().clone();
            app.global_shortcut().on_shortcuts([f9, f10], move |_app, shortcut, event| {
                if event.state != ShortcutState::Pressed {
                    return;
                }
                let state = handle.state::<SharedRecordingState>();
                if shortcut == &f9 {
                    if let Err(e) = recording::start_recording(&handle, &state) {
                        log::error!("Failed to start recording: {e}");
                    }
                } else if shortcut == &f10 {
                    if let Err(e) = recording::stop_recording(&handle, &state) {
                        log::error!("Failed to stop recording: {e}");
                    }
                }
            })?;

            // Handle window close — stop recording gracefully
            let handle = app.handle().clone();
            let main_window = app.get_webview_window("main").unwrap();
            main_window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { .. } = event {
                    let state = handle.state::<SharedRecordingState>();
                    let _ = recording::stop_recording(&handle, &state);
                    recording::cleanup_temp_file(&state);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_video_info,
            export_video,
            start_screen_recording,
            stop_screen_recording,
            cleanup_recording_temp,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
