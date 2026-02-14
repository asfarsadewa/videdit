import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import VideoPlayer from "./components/VideoPlayer";
import Timeline from "./components/Timeline";
import SegmentList from "./components/SegmentList";
import ExportPanel from "./components/ExportPanel";
import RecordingIndicator from "./components/RecordingIndicator";
import type { VideoInfo, Segment, RecordingStartedPayload } from "./types";
import { generateId, clamp } from "./utils/format";

export default function App() {
  const [videoInfo, setVideoInfo] = useState<VideoInfo | null>(null);
  const [videoSrc, setVideoSrc] = useState<string | null>(null);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [segments, setSegments] = useState<Segment[]>([]);
  const [markInTime, setMarkInTime] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [recordingStartTime, setRecordingStartTime] = useState<number>(0);
  const [isFromRecording, setIsFromRecording] = useState(false);
  const [hasRecordingAudio, setHasRecordingAudio] = useState(false);

  const loadVideo = useCallback(async (path: string) => {
    setLoading(true);
    setError(null);
    setSegments([]);
    setCurrentTime(0);
    setMarkInTime(null);

    try {
      const info: VideoInfo = await invoke("get_video_info", { path });
      setVideoInfo(info);
      setVideoSrc(convertFileSrc(info.path));
      setDuration(info.duration);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  const openFile = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Video",
            extensions: ["mp4", "mkv", "mov", "avi", "webm", "ts", "flv", "wmv"],
          },
        ],
      });
      if (!selected) return;
      setIsFromRecording(false);
      await loadVideo(typeof selected === "string" ? selected : selected);
    } catch (e) {
      setError(String(e));
    }
  }, [loadVideo]);

  // Listen for recording events from Rust backend
  useEffect(() => {
    const unlistenStarted = listen<RecordingStartedPayload>("recording-started", (event) => {
      setIsRecording(true);
      setRecordingStartTime(Date.now());
      setHasRecordingAudio(event.payload.hasAudio);
    });
    const unlistenStopped = listen<string>("recording-stopped", (event) => {
      setIsRecording(false);
      setRecordingStartTime(0);
      setIsFromRecording(true);
      // Auto-load the recorded video
      loadVideo(event.payload);
    });
    return () => {
      unlistenStarted.then((fn) => fn());
      unlistenStopped.then((fn) => fn());
    };
  }, [loadVideo]);

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      const file = e.dataTransfer.files[0];
      if (!file) return;
      const path = (file as File & { path?: string }).path;
      if (!path) return;
      setIsFromRecording(false);
      await loadVideo(path);
    },
    [loadVideo],
  );

  const addSegment = useCallback(
    (start: number, end: number) => {
      const newSeg: Segment = {
        id: generateId(),
        start: clamp(start, 0, duration),
        end: clamp(end, 0, duration),
      };

      setSegments((prev) => {
        // Check for overlaps
        const overlaps = prev.some(
          (s) => newSeg.start < s.end && newSeg.end > s.start,
        );
        if (overlaps) return prev;

        return [...prev, newSeg].sort((a, b) => a.start - b.start);
      });
    },
    [duration],
  );

  const handleMarkIn = useCallback(() => {
    setMarkInTime(currentTime);
  }, [currentTime]);

  const handleMarkOut = useCallback(() => {
    if (markInTime !== null && currentTime > markInTime) {
      addSegment(markInTime, currentTime);
      setMarkInTime(null);
    }
  }, [markInTime, currentTime, addSegment]);

  const handleAddSegmentAtPlayhead = useCallback(() => {
    const start = currentTime;
    const end = Math.min(currentTime + 5, duration);
    if (end > start) addSegment(start, end);
  }, [currentTime, duration, addSegment]);

  const handleSegmentUpdate = useCallback(
    (id: string, start: number, end: number) => {
      setSegments((prev) => {
        const updated = prev.map((s) =>
          s.id === id
            ? { ...s, start: clamp(start, 0, duration), end: clamp(end, 0, duration) }
            : s,
        );

        // Validate no overlaps
        const target = updated.find((s) => s.id === id)!;
        const hasOverlap = updated.some(
          (s) => s.id !== id && target.start < s.end && target.end > s.start,
        );
        if (hasOverlap) return prev;

        return updated.sort((a, b) => a.start - b.start);
      });
    },
    [duration],
  );

  const handleDeleteSegment = useCallback((id: string) => {
    setSegments((prev) => prev.filter((s) => s.id !== id));
  }, []);

  return (
    <div
      className="flex flex-col h-screen bg-zinc-900 text-zinc-100 overflow-hidden"
      onDragOver={(e) => e.preventDefault()}
      onDrop={handleDrop}
    >
      {/* Toolbar */}
      <div className="flex items-center gap-3 px-4 py-2 border-b border-zinc-800 bg-zinc-900/80 shrink-0">
        <button
          onClick={openFile}
          className="px-3 py-1.5 text-sm bg-zinc-800 hover:bg-zinc-700 rounded transition-colors"
        >
          Open Video
        </button>
        {videoInfo && (
          <>
            <span className="text-xs text-zinc-500 truncate max-w-md">
              {videoInfo.path.split("\\").pop()}
            </span>
            <span className="text-xs text-zinc-600">
              {videoInfo.width}x{videoInfo.height} · {videoInfo.codec} · {videoInfo.fps.toFixed(1)} fps
            </span>
          </>
        )}
        {loading && <span className="text-xs text-zinc-500">Loading...</span>}
        {error && <span className="text-xs text-red-400 truncate max-w-sm">{error}</span>}

        <div className="ml-auto flex items-center gap-2">
          {isRecording && <RecordingIndicator startTime={recordingStartTime} hasAudio={hasRecordingAudio} />}
          {videoInfo && (
            <button
              onClick={handleAddSegmentAtPlayhead}
              className="px-3 py-1.5 text-sm bg-emerald-700 hover:bg-emerald-600 rounded transition-colors"
            >
              + Add Segment
            </button>
          )}
          {markInTime !== null && (
            <span className="text-xs text-amber-400">
              Mark in set at {markInTime.toFixed(1)}s — press O to mark out
            </span>
          )}
        </div>
      </div>

      {/* Main content */}
      {!videoSrc ? (
        <div className="flex-1 flex items-center justify-center">
          <div className="flex flex-col items-center gap-10 max-w-lg px-6">
            {/* Drop zone */}
            <button
              onClick={openFile}
              className="group w-full border border-dashed border-zinc-700 hover:border-zinc-500 rounded-lg
                         py-12 px-8 transition-colors cursor-pointer bg-zinc-800/20 hover:bg-zinc-800/40"
            >
              <div className="flex flex-col items-center gap-3">
                <svg className="w-8 h-8 text-zinc-600 group-hover:text-zinc-400 transition-colors" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
                  <polyline points="17 8 12 3 7 8" />
                  <line x1="12" y1="3" x2="12" y2="15" />
                </svg>
                <p className="text-sm text-zinc-400 group-hover:text-zinc-300 transition-colors">
                  Drop a video file or click to browse
                </p>
                <p className="text-xs text-zinc-600">
                  MP4, MKV, MOV, AVI, WebM, TS, FLV, WMV
                </p>
              </div>
            </button>

            {/* Divider */}
            <div className="flex items-center gap-4 w-full">
              <div className="flex-1 h-px bg-zinc-800" />
              <span className="text-[11px] text-zinc-600 uppercase tracking-widest">or</span>
              <div className="flex-1 h-px bg-zinc-800" />
            </div>

            {/* Screen recording hint */}
            <div className="w-full space-y-3">
              <p className="text-xs text-zinc-500 text-center">Record your screen directly</p>
              <div className="flex justify-center gap-3">
                <div className="flex items-center gap-2 px-3 py-2 rounded bg-zinc-800/60 border border-zinc-800">
                  <kbd className="px-1.5 py-0.5 text-[11px] font-mono bg-zinc-700 text-zinc-300 rounded">F9</kbd>
                  <span className="text-xs text-zinc-500">Start recording</span>
                </div>
                <div className="flex items-center gap-2 px-3 py-2 rounded bg-zinc-800/60 border border-zinc-800">
                  <kbd className="px-1.5 py-0.5 text-[11px] font-mono bg-zinc-700 text-zinc-300 rounded">F10</kbd>
                  <span className="text-xs text-zinc-500">Stop recording</span>
                </div>
              </div>
              <p className="text-[11px] text-zinc-600 text-center">
                Captures display + system audio (requires Stereo Mix) — works while minimized
              </p>
            </div>
          </div>
        </div>
      ) : (
        <div className="flex-1 flex flex-col min-h-0">
          {/* Video player */}
          <VideoPlayer
            src={videoSrc}
            currentTime={currentTime}
            onTimeUpdate={setCurrentTime}
            onDurationChange={setDuration}
            onMarkIn={handleMarkIn}
            onMarkOut={handleMarkOut}
          />

          {/* Timeline */}
          <Timeline
            duration={duration}
            currentTime={currentTime}
            segments={segments}
            onSeek={setCurrentTime}
            onSegmentUpdate={handleSegmentUpdate}
          />

          {/* Bottom panel */}
          <div className="shrink-0 border-t border-zinc-800">
            <div className="px-4 py-2">
              <SegmentList
                segments={segments}
                onDelete={handleDeleteSegment}
                onSeek={setCurrentTime}
              />
            </div>
            <ExportPanel inputPath={videoInfo!.path} segments={segments} isFromRecording={isFromRecording} />
          </div>
        </div>
      )}
    </div>
  );
}
