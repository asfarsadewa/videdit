use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VideoInfo {
    pub path: String,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub codec: String,
    pub fps: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Segment {
    pub id: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Subtitle {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExportProgress {
    pub segment_index: usize,
    pub total_segments: usize,
    pub percent: f64,
    pub phase: String,
    pub message: String,
}

/// Resolve sidecar binary path relative to the current executable.
/// In dev mode (tauri dev), sidecars are copied to target/debug/ by tauri-build.
/// In production, they sit next to the main exe.
/// `name` should match the externalBin config entry, e.g. "binaries/ffmpeg".
pub fn resolve_sidecar(_app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {e}"))?;
    let exe_dir = exe_path
        .parent()
        .ok_or("Current exe has no parent directory")?;

    let mut sidecar_path = exe_dir.join(name);

    #[cfg(windows)]
    {
        let needs_exe = sidecar_path
            .extension()
            .is_none_or(|ext| ext != "exe");
        if needs_exe {
            sidecar_path.as_mut_os_string().push(".exe");
        }
    }

    if sidecar_path.exists() {
        Ok(sidecar_path)
    } else {
        Err(format!(
            "Sidecar binary not found at: {}. Make sure to run via 'npm run tauri dev'.",
            sidecar_path.display()
        ))
    }
}

#[cfg(windows)]
pub fn hide_console_window(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000) // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
pub fn hide_console_window(cmd: &mut Command) -> &mut Command {
    cmd
}

pub fn probe_video(app: &AppHandle, path: &str) -> Result<VideoInfo, String> {
    let ffprobe = resolve_sidecar(app, "ffprobe")?;

    let mut cmd = Command::new(&ffprobe);
    cmd.args([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        "-select_streams",
        "v:0",
        path,
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    hide_console_window(&mut cmd);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {stderr}"));
    }

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| format!("Failed to parse ffprobe output: {e}"))?;

    let stream = json["streams"]
        .as_array()
        .and_then(|s| s.first())
        .ok_or("No video stream found")?;

    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);

    let width = stream["width"].as_u64().unwrap_or(0) as u32;
    let height = stream["height"].as_u64().unwrap_or(0) as u32;
    let codec = stream["codec_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let fps = parse_fps(stream["r_frame_rate"].as_str().unwrap_or("0/1"));

    Ok(VideoInfo {
        path: path.to_string(),
        duration,
        width,
        height,
        codec,
        fps,
    })
}

fn parse_fps(rate: &str) -> f64 {
    let parts: Vec<&str> = rate.split('/').collect();
    if parts.len() == 2 {
        let num: f64 = parts[0].parse().unwrap_or(0.0);
        let den: f64 = parts[1].parse().unwrap_or(1.0);
        if den > 0.0 {
            return num / den;
        }
    }
    0.0
}

pub fn export_segments(
    app: &AppHandle,
    input_path: &str,
    segments: &[Segment],
    subtitles: &[Subtitle],
    output_path: &str,
    merge: bool,
    compress: bool,
    quality: u32,
    burn_subtitles: bool,
) -> Result<String, String> {
    let ffmpeg = resolve_sidecar(app, "ffmpeg")?;
    let temp_dir = tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {e}"))?;
    let total = segments.len();

    let mut temp_files: Vec<PathBuf> = Vec::new();

    // Export SRT alongside video if not burning (handles subtitle-only export too)
    if !subtitles.is_empty() && !burn_subtitles {
        create_srt_for_export(subtitles, segments, output_path, merge)?;
    }

    // If only exporting SRT (no segments), we're done
    if segments.is_empty() && !subtitles.is_empty() && !burn_subtitles {
        let progress = ExportProgress {
            segment_index: 0,
            total_segments: 1,
            percent: 100.0,
            phase: "done".to_string(),
            message: "SRT file exported".to_string(),
        };
        let _ = app.emit("export-progress", &progress);
        return Ok(output_path.to_string());
    }

    for (i, seg) in segments.iter().enumerate() {
        let progress = ExportProgress {
            segment_index: i,
            total_segments: total,
            percent: (i as f64 / total as f64) * 100.0,
            phase: "cutting".to_string(),
            message: format!("Cutting segment {} of {}", i + 1, total),
        };
        let _ = app.emit("export-progress", &progress);

        let out_file = if merge || total > 1 {
            temp_dir.path().join(format!("segment_{:04}.mp4", i))
        } else {
            PathBuf::from(output_path)
        };

        // Create per-segment SRT with timestamps offset to segment start = 0
        let seg_srt = if burn_subtitles && !subtitles.is_empty() {
            create_srt_for_segment(subtitles, seg.start, seg.end, &temp_dir, &format!("sub_{i}.srt"))?
        } else {
            None
        };

        let mut cmd = Command::new(&ffmpeg);
        if compress {
            let seg_duration = seg.end - seg.start;
            cmd.args([
                "-y",
                "-ss",
                &format!("{:.3}", seg.start),
                "-i",
                input_path,
                "-t",
                &format!("{:.3}", seg_duration),
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                &quality.to_string(),
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-avoid_negative_ts",
                "make_zero",
                "-map",
                "0",
            ]);
            
            // Add subtitle filter if burning
            if let Some(ref srt) = seg_srt {
                let srt_escaped = escape_path_for_filter(srt);
                log::info!("Adding subtitle filter with SRT: {}", srt_escaped);
                cmd.args(["-vf", &format!("subtitles='{}'", srt_escaped)]);
            }
        } else {
            let seg_duration = seg.end - seg.start;
            // Burning subtitles requires re-encoding
            if let Some(ref srt) = seg_srt {
                let srt_escaped = escape_path_for_filter(srt);
                log::info!("Adding subtitle filter with SRT: {}", srt_escaped);
                cmd.args([
                    "-y",
                    "-ss",
                    &format!("{:.3}", seg.start),
                    "-i",
                    input_path,
                    "-t",
                    &format!("{:.3}", seg_duration),
                    "-c:v",
                    "libx264",
                    "-preset",
                    "medium",
                    "-crf",
                    "18",
                    "-c:a",
                    "copy",
                    "-avoid_negative_ts",
                    "make_zero",
                    "-vf",
                    &format!("subtitles='{}'", srt_escaped),
                ]);
            } else {
                // True lossless copy
                cmd.args([
                    "-y",
                    "-ss",
                    &format!("{:.3}", seg.start),
                    "-i",
                    input_path,
                    "-t",
                    &format!("{:.3}", seg_duration),
                    "-c",
                    "copy",
                    "-avoid_negative_ts",
                    "make_zero",
                    "-map",
                    "0",
                ]);
            }
        }
        cmd.arg(out_file.to_str().unwrap())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_console_window(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start ffmpeg: {e}"))?;

        // Capture stderr for error reporting
        let mut stderr_output = String::new();
        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            let duration = seg.end - seg.start;
            for line in reader.lines().map_while(Result::ok) {
                stderr_output.push_str(&line);
                stderr_output.push('\n');
                if let Some(time) = parse_ffmpeg_time(&line) {
                    // Both paths now use input-level seeking, so time= is relative (starts near 0).
                    let elapsed = time;
                    let seg_percent = (elapsed / duration).min(1.0) * 100.0;
                    let overall = ((i as f64 + seg_percent / 100.0) / total as f64) * 100.0;
                    let progress = ExportProgress {
                        segment_index: i,
                        total_segments: total,
                        percent: overall,
                        phase: "cutting".to_string(),
                        message: format!("Cutting segment {} of {} ({:.0}%)", i + 1, total, seg_percent),
                    };
                    let _ = app.emit("export-progress", &progress);
                }
            }
        }

        let status = child.wait().map_err(|e| format!("FFmpeg process error: {e}"))?;
        if !status.success() {
            log::error!("FFmpeg stderr: {}", stderr_output);
            // Extract error lines (lines containing "Error" or at the end)
            let error_lines: Vec<&str> = stderr_output
                .lines()
                .filter(|l| l.contains("Error") || l.contains("error") || l.contains("Invalid"))
                .take(3)
                .collect();
            let error_msg = if error_lines.is_empty() {
                stderr_output.lines().take(10).collect::<Vec<_>>().join(" | ")
            } else {
                error_lines.join(" | ")
            };
            return Err(format!("FFmpeg failed on segment {}: {}", i + 1, error_msg));
        }

        temp_files.push(out_file);
    }

    if total == 1 && !merge {
        let progress = ExportProgress {
            segment_index: 0,
            total_segments: 1,
            percent: 100.0,
            phase: "done".to_string(),
            message: "Export complete".to_string(),
        };
        let _ = app.emit("export-progress", &progress);
        return Ok(output_path.to_string());
    }

    if !merge {
        let out = Path::new(output_path);
        let stem = out.file_stem().unwrap().to_str().unwrap();
        let ext = out.extension().unwrap_or_default().to_str().unwrap();
        let parent = out.parent().unwrap();

        for (i, temp) in temp_files.iter().enumerate() {
            let dest = parent.join(format!("{}_{:03}.{}", stem, i + 1, ext));
            std::fs::copy(temp, &dest)
                .map_err(|e| format!("Failed to copy segment file: {e}"))?;
        }

        let progress = ExportProgress {
            segment_index: total,
            total_segments: total,
            percent: 100.0,
            phase: "done".to_string(),
            message: format!("Exported {} separate files", total),
        };
        let _ = app.emit("export-progress", &progress);
        return Ok(output_path.to_string());
    }

    // Merge segments using concat
    let concat_list = temp_dir.path().join("concat.txt");
    let list_content: String = temp_files
        .iter()
        .map(|f| format!("file '{}'", f.to_str().unwrap().replace('\\', "/")))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&concat_list, &list_content)
        .map_err(|e| format!("Failed to write concat list: {e}"))?;

    let merge_progress = ExportProgress {
        segment_index: total,
        total_segments: total,
        percent: 95.0,
        phase: "merging".to_string(),
        message: "Merging segments...".to_string(),
    };
    let _ = app.emit("export-progress", &merge_progress);

    let mut cmd = Command::new(&ffmpeg);
    cmd.args([
        "-y",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        concat_list.to_str().unwrap(),
        "-c",
        "copy",
        output_path,
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    hide_console_window(&mut cmd);

    let status = cmd
        .spawn()
        .map_err(|e| format!("Failed to start ffmpeg merge: {e}"))?
        .wait()
        .map_err(|e| format!("FFmpeg merge error: {e}"))?;

    if !status.success() {
        return Err("FFmpeg merge failed".to_string());
    }

    let progress = ExportProgress {
        segment_index: total,
        total_segments: total,
        percent: 100.0,
        phase: "done".to_string(),
        message: "Export complete".to_string(),
    };
    let _ = app.emit("export-progress", &progress);

    Ok(output_path.to_string())
}

/// Escape a file path for use inside an FFmpeg filter graph string.
/// FFmpeg's filter graph parser uses `:` as its option delimiter, so Windows
/// drive letters (e.g. `C:`) must have their colon escaped as `\:`.
fn escape_path_for_filter(path: &std::path::Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    // If second char is ':', it's a Windows drive letter — escape the colon.
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        format!("{}\\:{}", &s[..1], &s[2..])
    } else {
        s
    }
}

fn parse_ffmpeg_time(line: &str) -> Option<f64> {
    let time_idx = line.find("time=")?;
    let time_str = &line[time_idx + 5..];
    let end = time_str.find(' ').unwrap_or(time_str.len());
    let time_str = &time_str[..end];

    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() == 3 {
        let h: f64 = parts[0].parse().ok()?;
        let m: f64 = parts[1].parse().ok()?;
        let s: f64 = parts[2].parse().ok()?;
        Some(h * 3600.0 + m * 60.0 + s)
    } else {
        None
    }
}

/// Write subtitles to an SRT file at the given path.
fn write_srt_to_path(subtitles: &[Subtitle], path: &Path) -> Result<(), String> {
    let mut content = String::new();
    for (i, sub) in subtitles.iter().enumerate() {
        let start = format_time_srt(sub.start);
        let end = format_time_srt(sub.end);
        content.push_str(&format!("{}\n{} --> {}\n{}\n\n", i + 1, start, end, sub.text));
    }
    std::fs::write(path, &content).map_err(|e| format!("Failed to write SRT file: {e}"))
}

/// Create a temp SRT file for a single segment with timestamps offset so the segment starts at 0.
/// Returns None if no subtitles overlap the segment (caller should skip the subtitle filter).
fn create_srt_for_segment(
    subtitles: &[Subtitle],
    seg_start: f64,
    seg_end: f64,
    temp_dir: &tempfile::TempDir,
    name: &str,
) -> Result<Option<PathBuf>, String> {
    let filtered: Vec<Subtitle> = subtitles
        .iter()
        .filter(|sub| sub.end > seg_start && sub.start < seg_end)
        .map(|sub| Subtitle {
            start: (sub.start - seg_start).max(0.0),
            end: (sub.end - seg_start).max(0.0),
            text: sub.text.clone(),
        })
        .collect();

    if filtered.is_empty() {
        return Ok(None);
    }

    let srt_path = temp_dir.path().join(name);
    write_srt_to_path(&filtered, &srt_path)?;
    Ok(Some(srt_path))
}

/// Export SRT file(s) with correctly remapped timestamps alongside the video output.
///
/// - No segments: original timestamps → `output.srt`
/// - Merge or single segment: merged timeline timestamps → `output.srt`
/// - Multiple separate segments: per-segment offset timestamps → `output_001.srt`, etc.
fn create_srt_for_export(
    subtitles: &[Subtitle],
    segments: &[Segment],
    output_path: &str,
    merge: bool,
) -> Result<(), String> {
    let out = Path::new(output_path);

    if segments.is_empty() {
        let srt_path = out.with_extension("srt");
        write_srt_to_path(subtitles, &srt_path)?;
        log::info!("Saved SRT to: {:?}", srt_path);
        return Ok(());
    }

    if merge || segments.len() == 1 {
        // Remap each subtitle to its position in the merged output timeline
        let mut remapped: Vec<Subtitle> = Vec::new();
        let mut cumulative_offset = 0.0_f64;
        for seg in segments {
            for sub in subtitles {
                if sub.end > seg.start && sub.start < seg.end {
                    remapped.push(Subtitle {
                        start: (sub.start - seg.start + cumulative_offset).max(0.0),
                        end: (sub.end - seg.start + cumulative_offset).max(0.0),
                        text: sub.text.clone(),
                    });
                }
            }
            cumulative_offset += seg.end - seg.start;
        }
        remapped.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(std::cmp::Ordering::Equal));
        let srt_path = out.with_extension("srt");
        write_srt_to_path(&remapped, &srt_path)?;
        log::info!("Saved SRT to: {:?}", srt_path);
    } else {
        // One SRT per segment file with timestamps offset to segment start = 0
        let stem = out.file_stem().unwrap().to_str().unwrap();
        let parent = out.parent().unwrap();
        for (i, seg) in segments.iter().enumerate() {
            let srt_path = parent.join(format!("{}_{:03}.srt", stem, i + 1));
            let filtered: Vec<Subtitle> = subtitles
                .iter()
                .filter(|sub| sub.end > seg.start && sub.start < seg.end)
                .map(|sub| Subtitle {
                    start: (sub.start - seg.start).max(0.0),
                    end: (sub.end - seg.start).max(0.0),
                    text: sub.text.clone(),
                })
                .collect();
            write_srt_to_path(&filtered, &srt_path)?;
            log::info!("Saved SRT to: {:?}", srt_path);
        }
    }

    Ok(())
}

/// Format time in seconds to SRT format (HH:MM:SS,mmm).
fn format_time_srt(seconds: f64) -> String {
    let hours = (seconds / 3600.0) as u32;
    let minutes = ((seconds % 3600.0) / 60.0) as u32;
    let secs = (seconds % 60.0) as u32;
    let millis = ((seconds * 1000.0) % 1000.0) as u32;
    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, millis)
}
