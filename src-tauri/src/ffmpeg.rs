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
pub struct AudioTrack {
    pub id: String,
    pub file_path: String,
    pub start: f64,
    pub end: f64,
    pub volume: f64,
    pub radio_effect: bool,
    pub radio_intensity: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AudioInfo {
    pub path: String,
    pub duration: f64,
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

pub fn probe_audio(app: &AppHandle, path: &str) -> Result<AudioInfo, String> {
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
        "a:0",
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

    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);

    if duration <= 0.0 {
        return Err("Could not determine audio duration".to_string());
    }

    Ok(AudioInfo {
        path: path.to_string(),
        duration,
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
    audio_tracks: &[AudioTrack],
    original_radio: bool,
    original_radio_intensity: u32,
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
        let output_srt = PathBuf::from(output_path).with_extension("srt");
        let progress = ExportProgress {
            segment_index: 0,
            total_segments: 1,
            percent: 100.0,
            phase: "done".to_string(),
            message: "SRT file exported".to_string(),
        };
        let _ = app.emit("export-progress", &progress);
        return Ok(output_srt.to_string_lossy().into_owned());
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

        // Find audio tracks overlapping this segment
        let overlapping_audio: Vec<&AudioTrack> = audio_tracks
            .iter()
            .filter(|a| a.start < seg.end && a.end > seg.start)
            .collect();

        let has_audio_processing = !overlapping_audio.is_empty() || original_radio;
        let seg_duration = seg.end - seg.start;

        let mut cmd = Command::new(&ffmpeg);
        cmd.args(["-y", "-ss", &format!("{:.3}", seg.start), "-i", input_path]);

        // Additional audio file inputs (one per overlapping track)
        for audio in &overlapping_audio {
            cmd.args(["-i", &audio.file_path]);
        }

        if has_audio_processing {
            build_audio_filter_cmd(
                &mut cmd,
                &overlapping_audio,
                original_radio,
                original_radio_intensity,
                seg.start,
                seg_duration,
                seg_srt.as_deref(),
                compress,
                quality,
            );
        } else if compress {
            cmd.args([
                "-t", &format!("{seg_duration:.3}"),
                "-c:v", "libx264", "-preset", "medium",
                "-crf", &quality.to_string(),
                "-c:a", "aac", "-b:a", "192k",
                "-avoid_negative_ts", "make_zero", "-map", "0",
            ]);
            if let Some(ref srt) = seg_srt {
                let srt_escaped = escape_path_for_filter(srt);
                cmd.args(["-vf", &format!("subtitles='{}'", srt_escaped)]);
            }
        } else if burn_subtitles {
            cmd.args([
                "-t", &format!("{seg_duration:.3}"),
                "-c:v", "libx264", "-preset", "medium", "-crf", "18",
                "-c:a", "copy", "-avoid_negative_ts", "make_zero",
            ]);
            if let Some(ref srt) = seg_srt {
                let srt_escaped = escape_path_for_filter(srt);
                cmd.args(["-vf", &format!("subtitles='{}'", srt_escaped)]);
            }
        } else {
            cmd.args([
                "-t", &format!("{seg_duration:.3}"),
                "-c", "copy", "-avoid_negative_ts", "make_zero", "-map", "0",
            ]);
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
///
/// Order of operations:
///   1. Normalize backslash separators to forward slashes.
///   2. Escape filter-graph special characters (`\`, `'`, `[`, `]`, `;`)
///      by prefixing each with a backslash.
///   3. Escape a Windows drive-letter colon (`C:` → `C\:`) — done last so
///      the backslash added here is not re-escaped by step 2.
fn escape_path_for_filter(path: &std::path::Path) -> String {
    // Step 1: normalize path separators (removes all native backslashes)
    let s = path.to_string_lossy().replace('\\', "/");

    // Step 2: escape filter-graph metacharacters
    // Backslash is escaped first; after step 1 there are none, but included for correctness.
    let s = s
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace(';', "\\;");

    // Step 3: escape Windows drive-letter colon (e.g. "C:/" → "C\:/")
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        format!("{}\\:{}", &s[..1], &s[2..])
    } else {
        s
    }
}

/// Radio effect parameters interpolated by intensity (0–100).
/// Returns (highpass_freq, lowpass_freq, noise_amplitude, compressor_ratio).
fn radio_params(intensity: u32) -> (f64, f64, f64, f64) {
    let t = (intensity.min(100) as f64) / 100.0;
    let low_cut = 200.0 + t * 300.0;
    let high_cut = 4000.0 - t * 1200.0;
    let noise_amp = 0.005 + t * 0.030;
    let ratio = 2.0 + t * 6.0;
    (low_cut, high_cut, noise_amp, ratio)
}

/// Build the FFmpeg filter_complex args for audio mixing + optional radio effect.
/// Handles both subtitle burning and audio overlay in a single filter graph.
fn build_audio_filter_cmd(
    cmd: &mut Command,
    overlapping_audio: &[&AudioTrack],
    original_radio: bool,
    original_radio_intensity: u32,
    seg_start: f64,
    seg_duration: f64,
    seg_srt: Option<&Path>,
    compress: bool,
    quality: u32,
) {
    let mut filters: Vec<String> = Vec::new();
    let mut audio_labels: Vec<String> = Vec::new();
    let seg_end = seg_start + seg_duration;

    if original_radio {
        let (low_cut, high_cut, noise_amp, ratio) = radio_params(original_radio_intensity);
        filters.push(format!(
            "[0:a]highpass=f={low_cut:.0},lowpass=f={high_cut:.0},\
             acompressor=threshold=0.1:ratio={ratio:.1}:attack=5:release=50[orig_eq]"
        ));
        filters.push(format!(
            "anoisesrc=d={seg_duration:.3}:c=pink:r=44100:a={noise_amp:.4}[orig_noise]"
        ));
        filters.push("[orig_eq][orig_noise]amix=inputs=2:duration=first[orig_out]".into());
        audio_labels.push("[orig_out]".into());
    } else {
        audio_labels.push("[0:a]".into());
    }

    for (j, audio) in overlapping_audio.iter().enumerate() {
        let input_idx = j + 1;
        let overlap_start = audio.start.max(seg_start);
        let overlap_end = audio.end.min(seg_end);
        let audio_file_offset = overlap_start - audio.start;
        let audio_file_end = audio_file_offset + (overlap_end - overlap_start);
        let delay_ms = ((overlap_start - seg_start) * 1000.0).round() as i64;
        let vol = audio.volume.clamp(0.0, 2.0);

        let base = format!(
            "[{input_idx}:a]atrim=start={audio_file_offset:.3}:end={audio_file_end:.3},\
             asetpts=PTS-STARTPTS,volume={vol:.2}"
        );

        if audio.radio_effect {
            let (low_cut, high_cut, noise_amp, ratio) = radio_params(audio.radio_intensity);
            let eq_label = format!("dub{j}_eq");
            filters.push(format!(
                "{base},highpass=f={low_cut:.0},lowpass=f={high_cut:.0},\
                 acompressor=threshold=0.1:ratio={ratio:.1}:attack=5:release=50,\
                 adelay={delay_ms}|{delay_ms},apad[{eq_label}]"
            ));
            let noise_label = format!("dub{j}_n");
            filters.push(format!(
                "anoisesrc=d={seg_duration:.3}:c=pink:r=44100:a={noise_amp:.4}[{noise_label}]"
            ));
            let out_label = format!("dub{j}");
            filters.push(format!(
                "[{eq_label}][{noise_label}]amix=inputs=2:duration=first[{out_label}]"
            ));
            audio_labels.push(format!("[{out_label}]"));
        } else {
            let out_label = format!("dub{j}");
            filters.push(format!(
                "{base},adelay={delay_ms}|{delay_ms},apad[{out_label}]"
            ));
            audio_labels.push(format!("[{out_label}]"));
        }
    }

    if audio_labels.len() > 1 {
        let mix_in: String = audio_labels.join("");
        let n = audio_labels.len();
        filters.push(format!(
            "{mix_in}amix=inputs={n}:duration=first:dropout_transition=0[aout]"
        ));
    } else if audio_labels[0] != "[0:a]" {
        let last = filters.last_mut().unwrap();
        let label = audio_labels[0].trim_matches(|c| c == '[' || c == ']');
        *last = last.replace(&format!("[{label}]"), "[aout]");
    }

    let has_aout = audio_labels.len() > 1 || audio_labels[0] != "[0:a]";

    let has_vfilter = seg_srt.is_some();
    if let Some(srt) = seg_srt {
        let srt_escaped = escape_path_for_filter(srt);
        filters.push(format!("[0:v]subtitles='{srt_escaped}'[vout]"));
    }

    let filter_complex = filters.join(";");
    log::info!("Audio filter_complex: {}", filter_complex);
    cmd.args(["-filter_complex", &filter_complex]);

    if has_vfilter {
        cmd.args(["-map", "[vout]"]);
    } else {
        cmd.args(["-map", "0:v"]);
    }

    if has_aout {
        cmd.args(["-map", "[aout]"]);
    } else {
        cmd.args(["-map", "0:a?"]);
    }

    cmd.args(["-t", &format!("{seg_duration:.3}")]);

    if compress || has_vfilter {
        let crf = if compress { quality.to_string() } else { "18".into() };
        cmd.args(["-c:v", "libx264", "-preset", "medium", "-crf", &crf]);
    } else {
        cmd.args(["-c:v", "copy"]);
    }
    cmd.args(["-c:a", "aac", "-b:a", "192k", "-avoid_negative_ts", "make_zero"]);
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
        .filter_map(|sub| {
            let clipped_start = sub.start.max(seg_start).min(seg_end);
            let clipped_end = sub.end.min(seg_end).max(seg_start);
            if clipped_end <= clipped_start {
                return None;
            }
            Some(Subtitle {
                start: (clipped_start - seg_start).max(0.0),
                end: (clipped_end - seg_start).max(0.0),
                text: sub.text.clone(),
            })
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
                let clipped_start = sub.start.max(seg.start).min(seg.end);
                let clipped_end = sub.end.min(seg.end).max(seg.start);
                if clipped_end > clipped_start {
                    remapped.push(Subtitle {
                        start: (clipped_start - seg.start + cumulative_offset).max(0.0),
                        end: (clipped_end - seg.start + cumulative_offset).max(0.0),
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
        let stem = out
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| format!("Cannot derive file stem from output path: {output_path}"))?;
        let parent = out
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        for (i, seg) in segments.iter().enumerate() {
            let srt_path = parent.join(format!("{}_{:03}.srt", stem, i + 1));
            let filtered: Vec<Subtitle> = subtitles
                .iter()
                .filter_map(|sub| {
                    let clipped_start = sub.start.max(seg.start).min(seg.end);
                    let clipped_end = sub.end.min(seg.end).max(seg.start);
                    if clipped_end <= clipped_start {
                        return None;
                    }
                    Some(Subtitle {
                        start: (clipped_start - seg.start).max(0.0),
                        end: (clipped_end - seg.start).max(0.0),
                        text: sub.text.clone(),
                    })
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
