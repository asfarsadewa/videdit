use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

use crate::ffmpeg::resolve_sidecar;
#[cfg(windows)]
use crate::ffmpeg::hide_console_window;

pub type SharedRecordingState = Arc<Mutex<RecordingState>>;

pub struct RecordingState {
    pub is_recording: bool,
    pub output_path: Option<PathBuf>,
    pub ffmpeg_child: Option<Child>,
}

impl Default for RecordingState {
    fn default() -> Self {
        Self {
            is_recording: false,
            output_path: None,
            ffmpeg_child: None,
        }
    }
}

/// Parse FFmpeg's device listing to extract audio device names.
fn list_audio_devices(app: &AppHandle) -> Vec<String> {
    let ffmpeg = match resolve_sidecar(app, "ffmpeg") {
        Ok(p) => p,
        Err(_) => return vec![],
    };

    let mut cmd = Command::new(&ffmpeg);
    cmd.args([
        "-list_devices",
        "true",
        "-f",
        "dshow",
        "-i",
        "dummy",
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    #[cfg(windows)]
    hide_console_window(&mut cmd);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };

    // FFmpeg prints device list to stderr
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut devices = Vec::new();
    let mut in_audio_section = false;

    for line in stderr.lines() {
        if line.contains("DirectShow audio devices") {
            in_audio_section = true;
            continue;
        }
        if line.contains("DirectShow video devices") {
            in_audio_section = false;
            continue;
        }
        if in_audio_section {
            // Lines look like: [dshow @ ...] "Device Name"
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    let name = &line[start + 1..start + 1 + end];
                    // Skip "Alternative name" entries
                    if !name.starts_with('@') {
                        devices.push(name.to_string());
                    }
                }
            }
        }
    }

    devices
}

pub fn start_recording(app: &AppHandle, state: &SharedRecordingState) -> Result<(), String> {
    let mut state = state.lock().map_err(|e| format!("Lock error: {e}"))?;

    if state.is_recording {
        return Err("Already recording".to_string());
    }

    let ffmpeg = resolve_sidecar(app, "ffmpeg")?;
    let audio_devices = list_audio_devices(app);

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let temp_dir = std::env::temp_dir();
    let output_path = temp_dir.join(format!("videdit-recording-{timestamp}.mp4"));

    let mut cmd = Command::new(&ffmpeg);
    cmd.args(["-y", "-f", "gdigrab", "-framerate", "30", "-i", "desktop"]);

    // Add audio capture if a device is found
    if let Some(audio_device) = audio_devices.first() {
        cmd.args([
            "-f",
            "dshow",
            "-i",
            &format!("audio={audio_device}"),
        ]);
    }

    cmd.args([
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-crf",
        "23",
    ]);

    // Only add audio encoding args if we have audio
    if !audio_devices.is_empty() {
        cmd.args(["-c:a", "aac", "-b:a", "192k"]);
    }

    cmd.arg(output_path.to_str().unwrap());
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    hide_console_window(&mut cmd);

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start FFmpeg recording: {e}"))?;

    state.is_recording = true;
    state.output_path = Some(output_path);
    state.ffmpeg_child = Some(child);

    let _ = app.emit("recording-started", ());

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

    let output_path = state
        .output_path
        .take()
        .ok_or("No output path")?;

    // Send 'q' to FFmpeg's stdin for graceful stop
    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        let _ = stdin.write_all(b"q\n");
        let _ = stdin.flush();
    }
    // Drop stdin so FFmpeg sees EOF
    drop(child.stdin.take());

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

    state.is_recording = false;

    let path_str = output_path.to_string_lossy().to_string();
    let _ = app.emit("recording-stopped", &path_str);

    Ok(path_str)
}
