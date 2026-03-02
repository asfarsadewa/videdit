use std::io::BufRead;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::audio_capture::{self, AudioCaptureHandle};
use crate::ffmpeg::resolve_sidecar;
#[cfg(windows)]
use crate::ffmpeg::hide_console_window;

#[derive(Debug, Serialize, Clone)]
pub struct RecordingStartedPayload {
    pub has_audio: bool,
    pub audio_device: Option<String>,
}

pub type SharedRecordingState = Arc<Mutex<RecordingState>>;

pub struct RecordingState {
    pub is_recording: bool,
    pub output_path: Option<PathBuf>,
    pub ffmpeg_child: Option<Child>,
    pub audio_capture: Option<AudioCaptureHandle>,
    /// Persists the temp file path after stop_recording so it can be cleaned up later
    pub last_temp_path: Option<PathBuf>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            is_recording: false,
            output_path: None,
            ffmpeg_child: None,
            audio_capture: None,
            last_temp_path: None,
        }
    }
}

pub fn start_recording(app: &AppHandle, state: &SharedRecordingState) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| format!("Lock error: {e}"))?;

    if state.is_recording {
        return Err("Already recording".to_string());
    }

    // Clean up previous temp file before starting a new recording
    if let Some(prev) = state.last_temp_path.take() {
        let _ = std::fs::remove_file(&prev);
    }

    let ffmpeg = resolve_sidecar(app, "ffmpeg")?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let temp_dir = std::env::temp_dir();
    let video_temp_path = temp_dir.join(format!("videdit-rec-video-{timestamp}.mp4"));
    let audio_temp_path = temp_dir.join(format!("videdit-rec-audio-{timestamp}.wav"));
    let final_output_path = temp_dir.join(format!("videdit-recording-{timestamp}.mp4"));

    // Start FFmpeg for video-only capture (gdigrab)
    let mut cmd = Command::new(&ffmpeg);
    cmd.args([
        "-y",
        "-f", "gdigrab",
        "-framerate", "30",
        "-i", "desktop",
        "-c:v", "libx264",
        "-preset", "ultrafast",
        "-crf", "23",
        "-g", "30",  // Keyframe every 1 second (30fps). Limits lossless trim snap to ≤1s.
    ]);
    cmd.arg(video_temp_path.to_str().unwrap());
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    hide_console_window(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start FFmpeg recording: {e}"))?;

    // Log FFmpeg stderr in a background thread
    if let Some(stderr) = child.stderr.take() {
        std::thread::spawn(move || {
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                log::info!("FFmpeg recording: {}", line);
                if line.contains("error") || line.contains("Error") || line.contains("Failed") {
                    log::error!("FFmpeg error: {}", line);
                }
            }
        });
    }

    // Start WASAPI loopback audio capture
    let audio_handle = audio_capture::start_audio_capture(audio_temp_path);

    let payload = RecordingStartedPayload {
        has_audio: audio_handle.has_audio,
        audio_device: audio_handle.device_name.clone(),
    };

    state.is_recording = true;
    state.output_path = Some(final_output_path);
    state.ffmpeg_child = Some(child);
    state.audio_capture = Some(audio_handle);

    // Store the video temp path so stop_recording can find it
    // We'll use the output_path field for the final path, and derive video temp from it
    let _ = app.emit("recording-started", payload);

    Ok(())
}

pub fn stop_recording(app: &AppHandle, state: &SharedRecordingState) -> Result<String, String> {
    let mut state = state.lock().map_err(|e| format!("Lock error: {e}"))?;

    if !state.is_recording {
        return Err("Not recording".to_string());
    }

    let mut child = state
        .ffmpeg_child
        .take()
        .ok_or("No FFmpeg process found")?;

    let final_output_path = state
        .output_path
        .take()
        .ok_or("No output path")?;

    let mut audio_handle = state.audio_capture.take();

    // Send 'q' to FFmpeg's stdin for graceful stop
    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        let _ = stdin.write_all(b"q\n");
        let _ = stdin.flush();
    }
    // Drop stdin so FFmpeg sees EOF
    drop(child.stdin.take());

    // Stop audio capture
    let has_audio = audio_handle.as_ref().map_or(false, |h| h.has_audio);
    let audio_path = audio_handle.as_ref().map(|h| h.output_path.clone());
    if let Some(ref mut handle) = audio_handle {
        handle.stop();
    }

    // Wait for FFmpeg to exit, force kill after 5s timeout
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(_) => break,
        }
    }

    // Derive the video temp path from the final output path
    // final: videdit-recording-{ts}.mp4 → video: videdit-rec-video-{ts}.mp4
    let video_temp_path = {
        let fname = final_output_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace("videdit-recording-", "videdit-rec-video-");
        final_output_path.with_file_name(fname)
    };

    log::info!("Video temp path: {:?}", video_temp_path);
    log::info!("Final output path: {:?}", final_output_path);

    // Check if video temp file exists and has content
    let video_valid = video_temp_path.exists()
        && std::fs::metadata(&video_temp_path)
            .map(|m| m.len() > 1024)
            .unwrap_or(false);

    if !video_valid {
        log::warn!("Video temp file is missing or too small, recording may have failed");
    }

    // Mux video + audio if audio was captured
    let result_path = if has_audio {
        if let Some(audio_path) = &audio_path {
            if video_valid {
                match mux_video_audio(app, &video_temp_path, audio_path, &final_output_path) {
                    Ok(()) => {
                        // Clean up temp files
                        let _ = std::fs::remove_file(&video_temp_path);
                        let _ = std::fs::remove_file(audio_path);
                        final_output_path.clone()
                    }
                    Err(e) => {
                        log::error!("Mux failed: {e}, falling back to video-only");
                        // Fall back to video-only
                        let _ = std::fs::rename(&video_temp_path, &final_output_path);
                        let _ = std::fs::remove_file(audio_path);
                        final_output_path.clone()
                    }
                }
            } else {
                log::warn!("No valid video, using audio-only recording");
                let audio_only_path = final_output_path.with_extension("wav");
                std::fs::rename(audio_path, &audio_only_path)
                    .map_err(|e| format!("Failed to save audio-only recording: {e}"))?;
                audio_only_path
            }
        } else {
            if video_valid {
                std::fs::rename(&video_temp_path, &final_output_path)
                    .map_err(|e| format!("Failed to save recording: {e}"))?;
            }
            final_output_path.clone()
        }
    } else {
        // No audio — just rename video to final path
        if video_valid {
            std::fs::rename(&video_temp_path, &final_output_path)
                .map_err(|e| format!("Failed to save recording: {e}"))?;
        }
        if let Some(ap) = &audio_path {
            let _ = std::fs::remove_file(ap);
        }
        final_output_path.clone()
    };

    state.is_recording = false;
    state.last_temp_path = Some(result_path.clone());

    if std::fs::metadata(&result_path).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(format!(
            "Recording output is missing or empty: {}",
            result_path.display()
        ));
    }

    let path_str = result_path.to_string_lossy().to_string();
    let _ = app.emit("recording-stopped", &path_str);

    Ok(path_str)
}

/// Mux a video file and an audio file into a single output using FFmpeg.
fn mux_video_audio(
    app: &AppHandle,
    video_path: &PathBuf,
    audio_path: &PathBuf,
    output_path: &PathBuf,
) -> Result<(), String> {
    let ffmpeg = resolve_sidecar(app, "ffmpeg")?;

    let mut cmd = Command::new(&ffmpeg);
    cmd.args([
        "-y",
        "-i", video_path.to_str().unwrap(),
        "-i", audio_path.to_str().unwrap(),
        "-c:v", "copy",
        "-c:a", "aac",
        "-b:a", "192k",
        "-shortest",
    ]);
    cmd.arg(output_path.to_str().unwrap());
    cmd.stdout(Stdio::null())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    hide_console_window(&mut cmd);

    log::info!("Muxing video + audio → {:?}", output_path);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to start FFmpeg mux: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg mux failed: {stderr}"));
    }

    log::info!("Mux complete");
    Ok(())
}

/// Delete the last temp recording file and any leftover videdit temp files.
pub fn cleanup_temp_file(state: &SharedRecordingState) {
    if let Ok(mut state) = state.lock() {
        if let Some(path) = state.last_temp_path.take() {
            let _ = std::fs::remove_file(&path);
        }
    }
    cleanup_all_temp_files();
}

/// Remove all videdit-* temp files from the system temp directory.
fn cleanup_all_temp_files() {
    let temp_dir = std::env::temp_dir();
    if let Ok(entries) = std::fs::read_dir(&temp_dir) {
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("videdit-") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}
