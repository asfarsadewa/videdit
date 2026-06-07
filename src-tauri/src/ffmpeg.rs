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
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    pub id: String,
    pub segment_id: String,
    #[serde(default)]
    pub segment_start: f64,
    #[serde(default)]
    pub segment_end: f64,
    pub file_path: String,
    pub audio_source_start: f64,
    pub audio_source_end: f64,
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

    let has_audio = json["streams"]
        .as_array()
        .map(|arr| {
            !arr.is_empty()
                && arr[0]["codec_type"].as_str().unwrap_or("") == "audio"
        })
        .unwrap_or(false);

    if !has_audio {
        return Err("No audio stream found".to_string());
    }

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
    vhs_effect: bool,
    vhs_intensity: u32,
    vhs_scanlines: bool,
    vhs_color_profile: &str,
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

        // Find audio tracks that belong to this segment
        let overlapping_audio: Vec<&AudioTrack> = audio_tracks
            .iter()
            .filter(|a| a.segment_id == seg.id)
            .collect();

        let seg_duration = seg.end - seg.start;

        // Probe the input to know whether it actually has an audio stream.
        // This avoids referencing [0:a] in filters when the input has no audio.
        let input_has_audio = has_audio_stream(app, input_path);
        let effective_original_radio = original_radio && input_has_audio;
        let effective_has_audio_processing = !overlapping_audio.is_empty() || effective_original_radio;
        let effective_has_video_processing = seg_srt.is_some() || vhs_effect;

        let mut cmd = Command::new(&ffmpeg);
        cmd.args(["-y", "-ss", &format!("{:.3}", seg.start), "-i", input_path]);

        // Additional audio file inputs with seeking to avoid loading entire files
        for audio in &overlapping_audio {
            // Seek to audio source start and limit duration
            let audio_duration = audio.audio_source_end - audio.audio_source_start;
            cmd.args(["-ss", &format!("{:.3}", audio.audio_source_start)]);
            cmd.args(["-t", &format!("{:.3}", audio_duration)]);
            cmd.args(["-i", &audio.file_path]);
        }

        if effective_has_audio_processing || effective_has_video_processing {
            build_export_filter_cmd(
                &mut cmd,
                &overlapping_audio,
                effective_original_radio,
                original_radio_intensity,
                seg.start,
                seg_duration,
                seg_srt.as_deref(),
                compress,
                quality,
                merge,
                input_has_audio,
                vhs_effect,
                vhs_intensity,
                vhs_scanlines,
                vhs_color_profile,
            );
        } else if merge {
            // When merging segments, always re-encode to a consistent profile
            // so the concat demuxer sees identical codec params across all segments.
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
                "-c:a", "aac", "-b:a", "192k",
                "-avoid_negative_ts", "make_zero",
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct RadioParams {
    low_cut: f64,
    high_cut: f64,
    tilt_db: f64,
    drive: f64,
    fade_depth: f64,
    noise_amp: f64,
    crackle_amp: f64,
    crackle_chance: f64,
}

/// Radio effect parameters interpolated by intensity (0–100).
fn radio_params(intensity: u32) -> RadioParams {
    let t = (intensity.min(100) as f64) / 100.0;
    RadioParams {
        low_cut: 200.0 + t * 150.0,
        high_cut: 4500.0 - t * 1700.0,
        tilt_db: -3.0 - t * 2.0,
        drive: 2.2 + t * 0.8,
        fade_depth: 0.108 + t * 0.207,
        noise_amp: 0.015 + t * 0.020,
        crackle_amp: 0.600 + t * 0.220,
        crackle_chance: 0.000041 + t * 0.000118,
    }
}

fn push_radio_effect_filters(
    filters: &mut Vec<String>,
    input_chain: &str,
    duration: f64,
    intensity: u32,
    eq_label: &str,
    noise_label: &str,
    crackle_label: &str,
    out_label: &str,
) {
    let p = radio_params(intensity);
    let mix_label = format!("{out_label}_mix");
    filters.push(format!(
        "{input_chain}aformat=channel_layouts=mono,aresample=22050,\
         highshelf=f=1500:g={tilt:.1},highpass=f={low_cut:.0},lowpass=f={high_cut:.0},\
         volume={drive:.2},asoftclip=type=tanh:threshold=0.95:output=0.90,\
         volume='1-{fade:.3}*(0.5-0.5*(0.7*sin(2*PI*0.17*t)+0.3*sin(2*PI*0.73*t)))':eval=frame[{eq_label}]",
        low_cut = p.low_cut,
        high_cut = p.high_cut,
        tilt = p.tilt_db,
        drive = p.drive,
        fade = p.fade_depth,
    ));
    filters.push(format!(
        "anoisesrc=d={duration:.3}:c=white:r=22050:a={noise_amp:.4},\
         highpass=f={low_cut:.0},lowpass=f={high_cut:.0}[{noise_label}]",
        noise_amp = p.noise_amp,
        low_cut = p.low_cut,
        high_cut = p.high_cut,
    ));

    if p.crackle_amp > 0.0001 && p.crackle_chance > 0.000001 {
        filters.push(format!(
            "aevalsrc='if(lt(random(0),{chance:.6}),{amp:.4}*(2*random(1)-1),0)':s=22050:d={duration:.3},\
             highpass=f={low_cut:.0},lowpass=f={high_cut:.0}[{crackle_label}]",
            chance = p.crackle_chance,
            amp = p.crackle_amp,
            low_cut = p.low_cut,
            high_cut = p.high_cut,
        ));
        filters.push(format!(
            "[{eq_label}][{noise_label}][{crackle_label}]amix=inputs=3:duration=first:normalize=0[{mix_label}]"
        ));
    } else {
        filters.push(format!(
            "[{eq_label}][{noise_label}]amix=inputs=2:duration=first:normalize=0[{mix_label}]"
        ));
    }
    filters.push(format!(
        "[{mix_label}]acompressor=threshold=0.5:ratio=3.0:attack=5:release=80:makeup=1.26,\
         alimiter=limit=0.95,aformat=channel_layouts=mono[{out_label}]"
    ));
}

/// Quick probe to check whether the input file has at least one audio stream.
fn has_audio_stream(app: &AppHandle, path: &str) -> bool {
    let ffprobe = match resolve_sidecar(app, "ffprobe") {
        Ok(p) => p,
        Err(_) => return false,
    };

    let mut cmd = Command::new(&ffprobe);
    cmd.args([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_streams",
        "-select_streams",
        "a",
        path,
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    hide_console_window(&mut cmd);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return false,
    };

    if !output.status.success() {
        return false;
    }

    let json: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return false,
    };

    json["streams"]
        .as_array()
        .map(|arr| !arr.is_empty())
        .unwrap_or(false)
}

/// Build the FFmpeg filter_complex args for video effects, subtitle burning,
/// audio mixing, and optional radio effects.
fn build_export_filter_cmd(
    cmd: &mut Command,
    overlapping_audio: &[&AudioTrack],
    original_radio: bool,
    original_radio_intensity: u32,
    _seg_start: f64,
    seg_duration: f64,
    seg_srt: Option<&Path>,
    compress: bool,
    quality: u32,
    merge: bool,
    input_has_audio: bool,
    vhs_effect: bool,
    vhs_intensity: u32,
    vhs_scanlines: bool,
    vhs_color_profile: &str,
) {
    let mut filters: Vec<String> = Vec::new();
    let mut audio_labels: Vec<String> = Vec::new();
    let has_radio_processing = (original_radio && input_has_audio)
        || overlapping_audio.iter().any(|audio| audio.radio_effect);

    if original_radio && input_has_audio {
        push_radio_effect_filters(
            &mut filters,
            "[0:a]",
            seg_duration,
            original_radio_intensity,
            "orig_eq",
            "orig_noise",
            "orig_crackle",
            "orig_out",
        );
        audio_labels.push("[orig_out]".into());
    } else if input_has_audio {
        audio_labels.push("[0:a]".into());
    }

    for (j, audio) in overlapping_audio.iter().enumerate() {
        let input_idx = j + 1;
        let vol = audio.volume.clamp(0.0, 2.0);
        let audio_duration = audio.audio_source_end - audio.audio_source_start;
        let effective_duration = audio_duration.min(seg_duration);

        // Audio is already trimmed at input level, just apply volume and effects
        let base = format!(
            "[{input_idx}:a]asetpts=PTS-STARTPTS,volume={vol:.2}"
        );

        if audio.radio_effect {
            let eq_label = format!("dub{j}_eq");
            let noise_label = format!("dub{j}_n");
            let crackle_label = format!("dub{j}_c");
            let out_label = format!("dub{j}");
            push_radio_effect_filters(
                &mut filters,
                &format!("{base},"),
                effective_duration,
                audio.radio_intensity,
                &eq_label,
                &noise_label,
                &crackle_label,
                &out_label,
            );
            audio_labels.push(format!("[{out_label}]"));
        } else {
            let out_label = format!("dub{j}");
            filters.push(format!("{base}[{out_label}]"));
            audio_labels.push(format!("[{out_label}]"));
        }
    }

    if audio_labels.len() > 1 {
        let mix_in: String = audio_labels.join("");
        let n = audio_labels.len();
        if has_radio_processing {
            filters.push(format!(
                "{mix_in}amix=inputs={n}:duration=first:dropout_transition=0:normalize=0[aout_mix]"
            ));
            filters.push("[aout_mix]aformat=channel_layouts=mono[aout]".into());
        } else {
            filters.push(format!(
                "{mix_in}amix=inputs={n}:duration=first:dropout_transition=0[aout]"
            ));
        }
    } else if audio_labels.len() == 1 && audio_labels[0] != "[0:a]" {
        let last = filters.last_mut().unwrap();
        let label = audio_labels[0].trim_matches(|c| c == '[' || c == ']');
        *last = last.replace(&format!("[{label}]"), "[aout]");
    }

    let has_aout = !audio_labels.is_empty()
        && (audio_labels.len() > 1 || audio_labels[0] != "[0:a]");

    let has_vfilter = seg_srt.is_some() || vhs_effect;
    let color_profile = parse_vhs_color_profile(vhs_color_profile);
    if let Some(srt) = seg_srt {
        let srt_escaped = escape_path_for_filter(srt);
        if vhs_effect {
            filters.push(format!("[0:v]subtitles='{srt_escaped}'[vsub]"));
            filters.push(build_vhs_filter_chain("[vsub]", "[vout]", vhs_intensity, vhs_scanlines, color_profile));
        } else {
            filters.push(format!("[0:v]subtitles='{srt_escaped}'[vout]"));
        }
    } else if vhs_effect {
        filters.push(build_vhs_filter_chain("[0:v]", "[vout]", vhs_intensity, vhs_scanlines, color_profile));
    }

    let filter_complex = filters.join(";");
    log::info!("Export filter_complex: {}", filter_complex);
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

    if merge || compress || has_vfilter {
        let crf = if vhs_effect {
            if compress {
                quality.to_string()
            } else {
                "18".into()
            }
        } else if compress || merge {
            quality.to_string()
        } else {
            "18".into()
        };
        cmd.args(["-c:v", "libx264", "-preset", "medium", "-crf", &crf]);
    } else {
        cmd.args(["-c:v", "copy"]);
    }
    cmd.args(["-c:a", "aac", "-b:a", "192k"]);
    if has_radio_processing {
        cmd.args(["-ac", "1"]);
    }
    cmd.args(["-avoid_negative_ts", "make_zero"]);
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct VhsParams {
    blur_radius: f64,
    chroma_shift: i32,
    rgb_shift: i32,
    ghost_opacity: f64,
    noise_strength: u32,
    contrast: f64,
    brightness: f64,
    saturation: f64,
    gamma: f64,
    scanline_alpha: f64,
    tracking_alpha: f64,
    tracking_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VhsColorProfile {
    Faded,
    Preserved,
}

fn parse_vhs_color_profile(value: &str) -> VhsColorProfile {
    match value {
        "preserved" => VhsColorProfile::Preserved,
        _ => VhsColorProfile::Faded,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct VhsColorParams {
    pre_contrast: f64,
    pre_brightness: f64,
    pre_saturation: f64,
    pre_gamma: f64,
    hue_saturation: f64,
    post_contrast: f64,
    post_brightness: f64,
    post_saturation: f64,
    post_gamma: f64,
}

fn vhs_color_params(p: VhsParams, profile: VhsColorProfile) -> VhsColorParams {
    match profile {
        VhsColorProfile::Faded => VhsColorParams {
            pre_contrast: 0.94 - p.ghost_opacity * 0.20,
            pre_brightness: 0.018 + p.tracking_alpha * 0.04,
            pre_saturation: 0.72 - p.ghost_opacity * 0.35,
            pre_gamma: 1.10 + p.ghost_opacity * 0.35,
            hue_saturation: 0.82 - p.ghost_opacity * 0.30,
            post_contrast: 0.88 - p.ghost_opacity * 0.18,
            post_brightness: 0.010 + p.tracking_alpha * 0.03,
            post_saturation: 0.68 - p.ghost_opacity * 0.25,
            post_gamma: 1.08 + p.ghost_opacity * 0.24,
        },
        VhsColorProfile::Preserved => VhsColorParams {
            pre_contrast: 0.99 - p.ghost_opacity * 0.12,
            pre_brightness: 0.012 + p.tracking_alpha * 0.025,
            pre_saturation: 0.86 - p.ghost_opacity * 0.42,
            pre_gamma: 1.04 + p.ghost_opacity * 0.16,
            hue_saturation: 1.00 - p.ghost_opacity * 0.42,
            post_contrast: 0.96 - p.ghost_opacity * 0.10,
            post_brightness: 0.006 + p.tracking_alpha * 0.015,
            post_saturation: 0.90 - p.ghost_opacity * 0.43,
            post_gamma: 1.03 + p.ghost_opacity * 0.12,
        },
    }
}

fn vhs_params(intensity: u32) -> VhsParams {
    let t = intensity.min(100) as f64 / 100.0;
    VhsParams {
        blur_radius: 0.35 + t * 1.10,
        chroma_shift: (1.0 + t * 5.0).round() as i32,
        rgb_shift: (1.0 + t * 3.0).round() as i32,
        ghost_opacity: 0.10 + t * 0.22,
        noise_strength: (5.0 + t * 20.0).round() as u32,
        contrast: 1.08 - t * 0.20,
        brightness: -0.010 - t * 0.025,
        saturation: 0.88 - t * 0.28,
        gamma: 1.02 + t * 0.08,
        scanline_alpha: 0.08 + t * 0.16,
        tracking_alpha: 0.05 + t * 0.13,
        tracking_height: (2.0 + t * 8.0).round() as u32,
    }
}

fn build_tracking_distortion(p: VhsParams) -> String {
    let skew = 4 + p.rgb_shift * 2;
    let half_skew = (skew / 2).max(1);
    let neg_skew = -skew;
    let neg_half_skew = -half_skew;
    let secondary_tracking = p.tracking_alpha * 0.70;
    let secondary_height = (p.tracking_height / 2).max(1);

    // Two staggered gates give the tracking hits a less regular cadence while staying reproducible.
    format!(
        "drawbox=x=0:y='trunc(mod(t*61,ih))':w=iw:h={track_height}:c=white@{tracking:.2}:t=fill:enable='lt(mod(t,3.1),0.16)',\
         drawbox=x=0:y='trunc(mod(t*83+ih/3,ih))':w=iw:h={secondary_height}:c=black@{secondary_tracking:.2}:t=fill:enable='lt(mod(t+1.7,4.4),0.12)',\
         perspective=x0={skew}:y0=0:x1=W+{skew}:y1=0:x2={neg_half_skew}:y2=H:x3=W{neg_half_skew}:y3=H:interpolation=linear:eval=init:enable='lt(mod(t,3.1),0.16)',\
         perspective=x0={neg_skew}:y0=0:x1=W{neg_skew}:y1=0:x2={half_skew}:y2=H:x3=W+{half_skew}:y3=H:interpolation=linear:eval=init:enable='lt(mod(t+1.7,4.4),0.12)'",
        track_height = p.tracking_height,
        tracking = p.tracking_alpha,
    )
}

fn build_vhs_filter_chain(
    input_label: &str,
    output_label: &str,
    intensity: u32,
    scanlines: bool,
    color_profile: VhsColorProfile,
) -> String {
    let p = vhs_params(intensity);
    let chroma_shift = p.chroma_shift;
    let rgb_shift = p.rgb_shift;
    let tracking_distortion = build_tracking_distortion(p);

    if scanlines {
        format!(
            "{input_label}\
             scale=trunc(ih*4/3/2)*2:trunc(ih/2)*2,setsar=1,\
             boxblur=luma_radius={blur:.2}:luma_power=1:chroma_radius={chroma_blur:.2}:chroma_power=1,\
             chromashift=cbh={chroma_shift}:crh={neg_chroma_shift}:edge=smear,\
             rgbashift=rh={rgb_shift}:bh={neg_rgb_shift}:edge=smear,\
             tblend=all_mode=average:all_opacity={ghost:.2},\
             noise=alls={noise}:allf=t+u,\
             eq=contrast={contrast:.2}:brightness={brightness:.3}:saturation={saturation:.2}:gamma={gamma:.2},\
             drawgrid=w=iw:h=2:t=1:c=black@{scanlines:.2},\
             {tracking_distortion},\
             vignette=angle=PI/5,format=yuv420p{output_label}",
            blur = p.blur_radius,
            chroma_blur = p.blur_radius + 0.40,
            neg_chroma_shift = -chroma_shift,
            neg_rgb_shift = -rgb_shift,
            ghost = p.ghost_opacity,
            noise = p.noise_strength,
            contrast = p.contrast,
            brightness = p.brightness,
            saturation = p.saturation,
            gamma = p.gamma,
            scanlines = p.scanline_alpha,
        )
    } else {
        let color = vhs_color_params(p, color_profile);
        format!(
            "{input_label}\
             scale=trunc(ih*4/3/2)*2:trunc(ih/2)*2,setsar=1,\
             eq=contrast={pre_contrast:.2}:brightness={pre_brightness:.3}:saturation={pre_saturation:.2}:gamma={pre_gamma:.2},\
             hue=h=-8:s={hue_saturation:.2},\
             boxblur=luma_radius={soft_blur:.2}:luma_power=1:chroma_radius={soft_chroma_blur:.2}:chroma_power=1,\
             chromashift=cbh={chroma_shift}:crh={neg_chroma_shift}:cbv=1:crv=-1:edge=smear,\
             lagfun=decay={lag_decay:.2},\
             noise=alls={soft_noise}:allf=t:all_seed=37,\
             {tracking_distortion},\
             format=rgba,rgbashift=rh={rgb_shift}:bh={neg_rgb_shift}:rv=1:bv=-1:edge=smear,\
             boxblur=luma_radius=0.80:luma_power=1:chroma_radius=0.80:chroma_power=1,\
             eq=contrast={post_contrast:.2}:brightness={post_brightness:.3}:saturation={post_saturation:.2}:gamma={post_gamma:.2},\
             vignette=angle=PI/4,format=yuv420p{output_label}",
            pre_contrast = color.pre_contrast,
            pre_brightness = color.pre_brightness,
            pre_saturation = color.pre_saturation,
            pre_gamma = color.pre_gamma,
            hue_saturation = color.hue_saturation,
            soft_blur = p.blur_radius + 1.00,
            soft_chroma_blur = p.blur_radius + 1.45,
            neg_chroma_shift = -chroma_shift,
            lag_decay = 0.88 + p.ghost_opacity * 0.20,
            soft_noise = (2 + p.noise_strength / 4).min(8),
            neg_rgb_shift = -rgb_shift,
            post_contrast = color.post_contrast,
            post_brightness = color.post_brightness,
            post_saturation = color.post_saturation,
            post_gamma = color.post_gamma,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_export_filter_cmd, build_vhs_filter_chain, parse_vhs_color_profile,
        push_radio_effect_filters, radio_params, vhs_params, AudioTrack, VhsColorProfile,
    };
    use std::process::Command;

    #[test]
    fn vhs_params_clamp_intensity_to_100() {
        assert_eq!(vhs_params(100), vhs_params(250));
    }

    #[test]
    fn vhs_params_increase_damage_with_intensity() {
        let light = vhs_params(0);
        let damaged = vhs_params(100);

        assert!(damaged.blur_radius > light.blur_radius);
        assert!(damaged.chroma_shift > light.chroma_shift);
        assert!(damaged.rgb_shift > light.rgb_shift);
        assert!(damaged.ghost_opacity > light.ghost_opacity);
        assert!(damaged.noise_strength > light.noise_strength);
        assert!(damaged.scanline_alpha > light.scanline_alpha);
        assert!(damaged.tracking_height > light.tracking_height);
        assert!(damaged.saturation < light.saturation);
    }

    #[test]
    fn radio_params_make_sw_noisier_and_cracklier_than_am() {
        let am = radio_params(0);
        let sw = radio_params(100);

        assert_eq!(am.low_cut, 200.0);
        assert_eq!(am.high_cut, 4500.0);
        assert_eq!(am.tilt_db, -3.0);
        assert_eq!(am.drive, 2.2);
        assert_eq!(am.fade_depth, 0.108);
        assert_eq!(am.noise_amp, 0.015);
        assert_eq!(am.crackle_amp, 0.600);
        assert_eq!(am.crackle_chance, 0.000041);

        assert_eq!(sw.low_cut, 350.0);
        assert_eq!(sw.high_cut, 2800.0);
        assert_eq!(sw.tilt_db, -5.0);
        assert_eq!(sw.drive, 3.0);
        assert_eq!(sw.fade_depth, 0.315);
        assert_eq!(sw.noise_amp, 0.035);
        assert_eq!(sw.crackle_amp, 0.820);
        assert_eq!(sw.crackle_chance, 0.000159);
    }

    #[test]
    fn radio_filter_chain_uses_make_radio_sound_am_style() {
        let mut filters = Vec::new();
        push_radio_effect_filters(
            &mut filters,
            "[0:a]",
            2.0,
            0,
            "eq",
            "noise",
            "crackle",
            "out",
        );
        let chain = filters.join(";");

        assert!(chain.contains("aformat=channel_layouts=mono,aresample=22050"));
        assert!(chain.contains("highshelf=f=1500:g=-3.0"));
        assert!(chain.contains("highpass=f=200,lowpass=f=4500"));
        assert!(chain.contains("volume=2.20,asoftclip=type=tanh:threshold=0.95:output=0.90"));
        assert!(chain.contains("anoisesrc=d=2.000:c=white:r=22050:a=0.0150"));
        assert!(chain.contains("aevalsrc='if(lt(random(0),0.000041),0.6000*(2*random(1)-1),0)'"));
        assert!(chain.contains("[eq][noise][crackle]amix=inputs=3:duration=first:normalize=0[out_mix]"));
        assert!(chain.contains("[out_mix]acompressor=threshold=0.5:ratio=3.0:attack=5:release=80:makeup=1.26"));
        assert!(chain.contains("alimiter=limit=0.95,aformat=channel_layouts=mono[out]"));
    }

    #[test]
    fn radio_filter_chain_uses_make_radio_sound_sw_style() {
        let mut filters = Vec::new();
        push_radio_effect_filters(
            &mut filters,
            "[0:a]",
            2.0,
            100,
            "eq",
            "noise",
            "crackle",
            "out",
        );
        let chain = filters.join(";");

        assert!(chain.contains("aformat=channel_layouts=mono,aresample=22050"));
        assert!(chain.contains("highshelf=f=1500:g=-5.0"));
        assert!(chain.contains("highpass=f=350,lowpass=f=2800"));
        assert!(chain.contains("volume=3.00,asoftclip=type=tanh:threshold=0.95:output=0.90"));
        assert!(chain.contains("volume='1-0.315*(0.5-0.5*(0.7*sin(2*PI*0.17*t)+0.3*sin(2*PI*0.73*t)))':eval=frame[eq]"));
        assert!(chain.contains("anoisesrc=d=2.000:c=white:r=22050:a=0.0350"));
        assert!(chain.contains("aevalsrc='if(lt(random(0),0.000159),0.8200*(2*random(1)-1),0)'"));
        assert!(chain.contains("[eq][noise][crackle]amix=inputs=3:duration=first:normalize=0[out_mix]"));
        assert!(chain.contains("[out_mix]acompressor=threshold=0.5:ratio=3.0:attack=5:release=80:makeup=1.26"));
        assert!(chain.contains("alimiter=limit=0.95,aformat=channel_layouts=mono[out]"));
    }

    #[test]
    fn final_audio_mix_for_radio_exports_is_forced_to_mono() {
        let audio = AudioTrack {
            id: "dub-1".into(),
            segment_id: "seg-1".into(),
            segment_start: 0.0,
            segment_end: 2.0,
            file_path: "dub.wav".into(),
            audio_source_start: 0.0,
            audio_source_end: 2.0,
            volume: 1.0,
            radio_effect: true,
            radio_intensity: 100,
        };
        let overlapping_audio = vec![&audio];
        let mut cmd = Command::new("ffmpeg");

        build_export_filter_cmd(
            &mut cmd,
            &overlapping_audio,
            false,
            0,
            0.0,
            2.0,
            None,
            false,
            23,
            false,
            true,
            false,
            40,
            true,
            "faded",
        );

        let args: Vec<String> = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let filter_complex = args
            .windows(2)
            .find_map(|pair| (pair[0] == "-filter_complex").then(|| pair[1].clone()))
            .expect("filter_complex arg should be present");

        assert!(filter_complex.contains("[dub0_mix]acompressor=threshold=0.5:ratio=3.0:attack=5:release=80:makeup=1.26"));
        assert!(filter_complex.contains("alimiter=limit=0.95,aformat=channel_layouts=mono[dub0]"));
        assert!(filter_complex.contains("[0:a][dub0]amix=inputs=2:duration=first:dropout_transition=0:normalize=0[aout_mix]"));
        assert!(filter_complex.contains("[aout_mix]aformat=channel_layouts=mono[aout]"));
        assert!(args.windows(2).any(|pair| pair[0] == "-ac" && pair[1] == "1"));
    }

    #[test]
    fn vhs_filter_chain_contains_crt_export_steps() {
        let chain = build_vhs_filter_chain("[0:v]", "[vout]", 40, true, VhsColorProfile::Faded);

        assert!(chain.starts_with("[0:v]scale=trunc(ih*4/3/2)*2:trunc(ih/2)*2,setsar=1"));
        assert!(chain.contains("boxblur="));
        assert!(chain.contains("chromashift="));
        assert!(chain.contains("rgbashift="));
        assert!(chain.contains("tblend=all_mode=average"));
        assert!(chain.contains("noise=alls="));
        assert!(chain.contains("eq=contrast="));
        assert!(chain.contains("drawgrid=w=iw:h=2:t=1"));
        assert!(chain.contains("drawbox=x=0:y='trunc(mod(t*61,ih))'"));
        assert!(chain.contains("perspective=x0="));
        assert!(chain.ends_with("format=yuv420p[vout]"));
    }

    #[test]
    fn vhs_filter_chain_can_omit_scanlines_but_keep_distortion() {
        let chain = build_vhs_filter_chain("[0:v]", "[vout]", 40, false, VhsColorProfile::Faded);

        assert!(chain.starts_with("[0:v]scale=trunc(ih*4/3/2)*2:trunc(ih/2)*2,setsar=1"));
        assert!(!chain.contains("drawgrid="));
        assert!(chain.contains("drawbox=x=0:y='trunc(mod(t*61,ih))'"));
        assert!(chain.contains("drawbox=x=0:y='trunc(mod(t*83+ih/3,ih))'"));
        assert!(chain.contains("perspective=x0="));
        assert!(chain.contains("lagfun=decay="));
        assert!(chain.contains("chromashift="));
        assert!(chain.contains("rgbashift="));
        assert!(chain.contains("boxblur="));
        assert!(chain.contains("noise=alls="));
        assert!(chain.ends_with("format=yuv420p[vout]"));
    }

    #[test]
    fn vhs_tracking_distortion_is_more_frequent_and_skews_frame() {
        let chain = build_vhs_filter_chain("[0:v]", "[vout]", 40, true, VhsColorProfile::Faded);

        assert!(!chain.contains("mod(t,5.7)"));
        assert!(chain.contains("enable='lt(mod(t,3.1),0.16)'"));
        assert!(chain.contains("enable='lt(mod(t+1.7,4.4),0.12)'"));
        assert!(chain.contains("perspective=x0=8:y0=0:x1=W+8:y1=0:x2=-4:y2=H:x3=W-4:y3=H"));
        assert!(chain.contains("perspective=x0=-8:y0=0:x1=W-8:y1=0:x2=4:y2=H:x3=W+4:y3=H"));
    }

    #[test]
    fn vhs_color_profile_defaults_to_faded() {
        assert_eq!(parse_vhs_color_profile("faded"), VhsColorProfile::Faded);
        assert_eq!(parse_vhs_color_profile("preserved"), VhsColorProfile::Preserved);
        assert_eq!(parse_vhs_color_profile("unknown"), VhsColorProfile::Faded);
    }

    #[test]
    fn vhs_preserved_color_profile_is_less_washed_than_faded() {
        let faded = build_vhs_filter_chain("[0:v]", "[vout]", 40, false, VhsColorProfile::Faded);
        let preserved = build_vhs_filter_chain("[0:v]", "[vout]", 40, false, VhsColorProfile::Preserved);

        assert!(faded.contains("eq=contrast=0.90:brightness=0.022:saturation=0.65:gamma=1.17"));
        assert!(faded.contains("hue=h=-8:s=0.76"));
        assert!(faded.contains("eq=contrast=0.85:brightness=0.013:saturation=0.63:gamma=1.13"));

        assert!(preserved.contains("eq=contrast=0.97:brightness=0.015:saturation=0.78:gamma=1.07"));
        assert!(preserved.contains("hue=h=-8:s=0.92"));
        assert!(preserved.contains("eq=contrast=0.94:brightness=0.008:saturation=0.82:gamma=1.05"));

        assert!(!preserved.contains("drawgrid="));
        assert!(preserved.contains("drawbox=x=0:y='trunc(mod(t*61,ih))'"));
        assert!(preserved.contains("perspective=x0="));
        assert!(preserved.contains("lagfun=decay="));
        assert!(preserved.contains("chromashift="));
        assert!(preserved.contains("rgbashift="));
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
