mod audio_capture;
mod ffmpeg;
mod recording;

use std::sync::{Arc, Mutex};

use audio_capture::{AudioDevice, AudioCaptureHandle};
use ffmpeg::{AudioInfo, AudioTrack, Segment, VideoInfo};
use recording::SharedRecordingState;
use tauri::Emitter;
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

// Mic recording state
pub struct MicRecordingState {
    pub handle: Option<AudioCaptureHandle>,
    pub start_time: Option<std::time::Instant>,
    pub segment_id: Option<String>,
    pub output_path: Option<std::path::PathBuf>,
    pub device_name: Option<String>,
}

impl Default for MicRecordingState {
    fn default() -> Self {
        Self {
            handle: None,
            start_time: None,
            segment_id: None,
            output_path: None,
            device_name: None,
        }
    }
}

pub type SharedMicRecordingState = Arc<Mutex<MicRecordingState>>;

#[tauri::command]
fn get_video_info(app: tauri::AppHandle, path: String) -> Result<VideoInfo, String> {
    ffmpeg::probe_video(&app, &path)
}

#[tauri::command]
fn get_audio_info(app: tauri::AppHandle, path: String) -> Result<AudioInfo, String> {
    ffmpeg::probe_audio(&app, &path)
}

#[tauri::command]
fn export_video(
    app: tauri::AppHandle,
    input_path: String,
    segments: Vec<Segment>,
    subtitles: Vec<ffmpeg::Subtitle>,
    output_path: String,
    merge: bool,
    compress: bool,
    quality: u32,
    burn_subtitles: bool,
    audio_tracks: Vec<AudioTrack>,
    original_radio: bool,
    original_radio_intensity: u32,
) -> Result<String, String> {
    for (i, sub) in subtitles.iter().enumerate() {
        if sub.start < 0.0 {
            return Err(format!("Subtitle {i}: start ({}) must be non-negative", sub.start));
        }
        if sub.end <= sub.start {
            return Err(format!(
                "Subtitle {i}: end ({}) must be greater than start ({})",
                sub.end, sub.start
            ));
        }
        if sub.text.trim().is_empty() {
            return Err(format!("Subtitle {i}: text must not be empty"));
        }
    }

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
        &audio_tracks,
        original_radio,
        original_radio_intensity,
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

// Mic recording commands
#[tauri::command]
fn enumerate_mic_devices() -> Result<Vec<AudioDevice>, String> {
    Ok(audio_capture::enumerate_capture_devices())
}

#[tauri::command]
fn start_mic_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedMicRecordingState>,
    segment_id: String,
    device_id: Option<String>,
    max_duration_secs: f64,
) -> Result<(), String> {
    let mut mic_state = state.lock().map_err(|e| format!("Lock error: {e}"))?;

    // Stop any existing mic recording
    if let Some(ref mut handle) = mic_state.handle {
        handle.stop();
    }

    // Create temp file path
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("Time error: {e}"))?
        .as_millis();
    let temp_path = std::env::temp_dir().join(format!("videdit-mic-{}.wav", timestamp));

    log::info!("Starting mic recording for segment {} to {:?}", segment_id, temp_path);

    // Start capture
    let handle = audio_capture::start_mic_capture(temp_path.clone(), device_id);

    if !handle.has_audio {
        return Err("No microphone device available".to_string());
    }

    let device_name = handle.device_name.clone();
    let output_path = handle.output_path.clone();
    let start_time = std::time::Instant::now();

    // Spawn a thread to auto-stop at max_duration
    let stop_flag = handle.stop_flag();
    let segment_id_clone = segment_id.clone();
    let app_clone = app.clone();
    let output_path_clone = output_path.clone();
    let device_name_clone = device_name.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs_f64(max_duration_secs));
        // Only auto-stop if recording is still running (flag is false)
        if !stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
            stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            // Give the capture thread time to finalize the WAV
            std::thread::sleep(std::time::Duration::from_millis(300));

            let duration_secs = get_wav_duration(&output_path_clone).unwrap_or(max_duration_secs);
            log::info!("Mic recording for segment {} auto-stopped at max duration ({:.2}s)", segment_id_clone, duration_secs);

            let result = MicRecordingResult {
                segment_id: segment_id_clone.clone(),
                file_path: output_path_clone.to_string_lossy().to_string(),
                duration: duration_secs,
                device_name: device_name_clone.unwrap_or_else(|| "Unknown".to_string()),
            };
            let _ = app_clone.emit("mic-recording-stopped", &result);
        }
    });

    mic_state.start_time = Some(start_time);
    mic_state.segment_id = Some(segment_id.clone());
    mic_state.handle = Some(handle);
    mic_state.output_path = Some(output_path);
    mic_state.device_name = device_name;

    let _ = app.emit("mic-recording-started", &segment_id);

    Ok(())
}

#[tauri::command]
fn stop_mic_recording(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedMicRecordingState>,
) -> Result<MicRecordingResult, String> {
    let mut mic_state = state.lock().map_err(|e| format!("Lock error: {e}"))?;

    let segment_id = mic_state.segment_id.take()
        .ok_or("No segment ID for recording")?;

    let duration_secs = mic_state.start_time
        .map(|t| t.elapsed().as_secs_f64())
        .unwrap_or(0.0);

    // If handle exists, stop it and get info from it
    if let Some(mut handle) = mic_state.handle.take() {
        let output_path = handle.output_path.clone();
        let device_name = handle.device_name.clone();
        handle.stop();
        mic_state.start_time = None;

        // Use actual WAV file duration instead of wall-clock time
        let actual_duration = get_wav_duration(&output_path).unwrap_or(duration_secs);

        let result = MicRecordingResult {
            segment_id: segment_id.clone(),
            file_path: output_path.to_string_lossy().to_string(),
            duration: actual_duration,
            device_name: device_name.unwrap_or_else(|| "Unknown".to_string()),
        };
        let _ = app.emit("mic-recording-stopped", &result);

        Ok(result)
    } else {
        // Handle already stopped (e.g., by auto-stop), return stored data
        let output_path = mic_state.output_path.take()
            .ok_or("No output path stored")?;
        let device_name = mic_state.device_name.take();
        mic_state.start_time = None;

        let actual_duration = get_wav_duration(&output_path).unwrap_or(duration_secs);

        let result = MicRecordingResult {
            segment_id: segment_id.clone(),
            file_path: output_path.to_string_lossy().to_string(),
            duration: actual_duration,
            device_name: device_name.unwrap_or_else(|| "Unknown".to_string()),
        };
        let _ = app.emit("mic-recording-stopped", &result);

        Ok(result)
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MicRecordingResult {
    pub segment_id: String,
    pub file_path: String,
    pub duration: f64,
    pub device_name: String,
}

fn get_wav_duration(path: &std::path::Path) -> Option<f64> {
    let reader = hound::WavReader::open(path).ok()?;
    let spec = reader.spec();
    let total_samples = reader.duration(); // frames (samples per channel)
    if spec.sample_rate == 0 {
        return None;
    }
    Some(total_samples as f64 / spec.sample_rate as f64)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(Arc::new(Mutex::new(recording::RecordingState::default())) as SharedRecordingState)
        .manage(Arc::new(Mutex::new(MicRecordingState::default())) as SharedMicRecordingState)
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
            let f11: Shortcut = "F11".parse().unwrap();

            let handle = app.handle().clone();
            app.global_shortcut().on_shortcuts([f9, f10, f11], move |_app, shortcut, event| {
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
                } else if shortcut == &f11 {
                    // Stop mic recording via global shortcut
                    let mic_state = handle.state::<SharedMicRecordingState>();
                    let mut mic_guard = match mic_state.lock() {
                        Ok(g) => g,
                        Err(e) => {
                            log::error!("Failed to lock mic state: {e}");
                            return;
                        }
                    };

                    let segment_id = mic_guard.segment_id.take();
                    let duration_secs = mic_guard.start_time
                        .map(|t| t.elapsed().as_secs_f64())
                        .unwrap_or(0.0);

                    // If handle exists, stop it
                    if let Some(mut handle_inner) = mic_guard.handle.take() {
                        handle_inner.stop();
                    }
                    mic_guard.start_time = None;
                    let output_path = mic_guard.output_path.take();
                    let device_name = mic_guard.device_name.take();
                    drop(mic_guard);

                    if let Some(sid) = segment_id {
                        let file_path_str = output_path.as_ref().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
                        let actual_duration = output_path.as_ref().and_then(|p| get_wav_duration(p)).unwrap_or(duration_secs);
                        let result = MicRecordingResult {
                            segment_id: sid.clone(),
                            file_path: file_path_str,
                            duration: actual_duration,
                            device_name: device_name.unwrap_or_else(|| "Unknown".to_string()),
                        };
                        let _ = handle.emit("mic-recording-stopped", &result);
                        log::info!("Mic recording stopped via F11 for segment {}, duration: {:.2}s", sid, actual_duration);
                    } else {
                        log::info!("No active mic recording to stop");
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
            get_audio_info,
            export_video,
            start_screen_recording,
            stop_screen_recording,
            cleanup_recording_temp,
            enumerate_mic_devices,
            start_mic_recording,
            stop_mic_recording,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
