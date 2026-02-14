use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode};

pub struct AudioCaptureHandle {
    stop_flag: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub output_path: PathBuf,
    pub has_audio: bool,
    pub device_name: Option<String>,
}

impl AudioCaptureHandle {
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Start capturing system audio via WASAPI loopback into a WAV file.
/// Returns a handle to stop the capture. If no audio device is available,
/// returns a handle with `has_audio: false` instead of an error.
pub fn start_audio_capture(output_path: PathBuf) -> AudioCaptureHandle {
    // Initialize COM and get device info on this thread first
    let _ = wasapi::initialize_mta().ok();

    let device_result = DeviceEnumerator::new().and_then(|enumerator| {
        enumerator.get_default_device(&Direction::Render)
    });

    let device = match device_result {
        Ok(d) => d,
        Err(e) => {
            log::warn!("No audio render device found: {e}");
            return AudioCaptureHandle {
                stop_flag: Arc::new(AtomicBool::new(true)),
                thread: None,
                output_path,
                has_audio: false,
                device_name: None,
            };
        }
    };

    let device_name = device.get_friendlyname().ok();
    log::info!(
        "WASAPI loopback: using device {:?}",
        device_name.as_deref().unwrap_or("unknown")
    );

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_clone = stop_flag.clone();
    let out_path = output_path.clone();
    let dev_name = device_name.clone();

    let thread = std::thread::spawn(move || {
        if let Err(e) = capture_loop(stop_flag_clone, out_path) {
            log::error!("Audio capture error: {e}");
        }
    });

    AudioCaptureHandle {
        stop_flag,
        thread: Some(thread),
        output_path,
        has_audio: true,
        device_name: dev_name,
    }
}

fn capture_loop(stop_flag: Arc<AtomicBool>, output_path: PathBuf) -> Result<(), String> {
    let _ = wasapi::initialize_mta().ok();

    let enumerator =
        DeviceEnumerator::new().map_err(|e| format!("DeviceEnumerator failed: {e}"))?;
    let device = enumerator
        .get_default_device(&Direction::Render)
        .map_err(|e| format!("No render device: {e}"))?;

    let mut audio_client = device
        .get_iaudioclient()
        .map_err(|e| format!("IAudioClient failed: {e}"))?;

    let mix_format = audio_client
        .get_mixformat()
        .map_err(|e| format!("GetMixFormat failed: {e}"))?;

    let sample_type = mix_format
        .get_subformat()
        .map_err(|e| format!("get_subformat failed: {e}"))?;

    log::info!(
        "WASAPI format: {} Hz, {} ch, {:?}",
        mix_format.get_samplespersec(),
        mix_format.get_nchannels(),
        sample_type,
    );

    // Initialize as loopback: Render device + Capture direction = AUDCLNT_STREAMFLAGS_LOOPBACK
    // Use EventsShared mode with autoconvert enabled
    let stream_mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: 2_000_000, // 200ms buffer
    };

    audio_client
        .initialize_client(&mix_format, &Direction::Capture, &stream_mode)
        .map_err(|e| format!("Initialize loopback failed: {e}"))?;

    let block_align = mix_format.get_blockalign() as usize;
    let channels = mix_format.get_nchannels();
    let sample_rate = mix_format.get_samplespersec();

    // Determine WAV spec based on the WASAPI format
    let (bits_per_sample, wav_sample_format) = match sample_type {
        SampleType::Float => (32u16, hound::SampleFormat::Float),
        SampleType::Int => (16u16, hound::SampleFormat::Int),
    };

    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample,
        sample_format: wav_sample_format,
    };

    let mut writer = hound::WavWriter::create(&output_path, spec)
        .map_err(|e| format!("WavWriter create failed: {e}"))?;

    let event_handle = audio_client
        .set_get_eventhandle()
        .map_err(|e| format!("SetEventHandle failed: {e}"))?;

    let buffer_size = audio_client
        .get_buffer_size()
        .map_err(|e| format!("GetBufferSize failed: {e}"))?;

    let capture_client = audio_client
        .get_audiocaptureclient()
        .map_err(|e| format!("GetCaptureClient failed: {e}"))?;

    audio_client
        .start_stream()
        .map_err(|e| format!("StartStream failed: {e}"))?;

    log::info!("WASAPI loopback capture started");

    // Allocate a buffer large enough for the max frames
    let mut data_buf = vec![0u8; buffer_size as usize * block_align];

    while !stop_flag.load(Ordering::SeqCst) {
        // Wait up to 200ms for audio data
        if event_handle.wait_for_event(200).is_err() {
            continue;
        }

        // Read all available packets
        loop {
            match capture_client.get_next_packet_size() {
                Ok(Some(0)) | Ok(None) => break,
                Err(_) => break,
                Ok(Some(_)) => {}
            }

            match capture_client.read_from_device(&mut data_buf) {
                Ok((n_frames, _info)) => {
                    if n_frames > 0 {
                        let byte_count = n_frames as usize * block_align;
                        write_samples(&mut writer, &data_buf[..byte_count], &sample_type);
                    }
                }
                Err(e) => {
                    log::warn!("Read from device error: {e}");
                    break;
                }
            }
        }
    }

    audio_client.stop_stream().ok();
    writer.finalize().map_err(|e| format!("WAV finalize failed: {e}"))?;

    log::info!("WASAPI loopback capture stopped, WAV written to {:?}", output_path);
    Ok(())
}

fn write_samples(
    writer: &mut hound::WavWriter<std::io::BufWriter<std::fs::File>>,
    raw_bytes: &[u8],
    sample_type: &SampleType,
) {
    match sample_type {
        SampleType::Float => {
            for chunk in raw_bytes.chunks_exact(4) {
                let sample = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                writer.write_sample(sample).ok();
            }
        }
        SampleType::Int => {
            for chunk in raw_bytes.chunks_exact(2) {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                writer.write_sample(sample).ok();
            }
        }
    }
}
