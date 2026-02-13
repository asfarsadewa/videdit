import { useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import VideoPlayer from "./components/VideoPlayer";
import Timeline from "./components/Timeline";
import SegmentList from "./components/SegmentList";
import ExportPanel from "./components/ExportPanel";
import type { VideoInfo, Segment } from "./types";
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

      setLoading(true);
      setError(null);
      setSegments([]);
      setCurrentTime(0);
      setMarkInTime(null);

      const path = typeof selected === "string" ? selected : selected;
      const info: VideoInfo = await invoke("get_video_info", { path });
      setVideoInfo(info);
      setVideoSrc(convertFileSrc(info.path));
      setDuration(info.duration);
      setLoading(false);
    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  }, []);

  const handleDrop = useCallback(
    async (e: React.DragEvent) => {
      e.preventDefault();
      const file = e.dataTransfer.files[0];
      if (!file) return;
      // Tauri drag-and-drop gives us the path via webview
      // We need to use the file's path property
      const path = (file as File & { path?: string }).path;
      if (!path) return;

      try {
        setLoading(true);
        setError(null);
        setSegments([]);
        setCurrentTime(0);
        setMarkInTime(null);

        const info: VideoInfo = await invoke("get_video_info", { path });
        setVideoInfo(info);
        setVideoSrc(convertFileSrc(info.path));
        setDuration(info.duration);
        setLoading(false);
      } catch (e) {
        setError(String(e));
        setLoading(false);
      }
    },
    [],
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
          <div className="text-center space-y-3">
            <div className="text-6xl text-zinc-700">▶</div>
            <p className="text-zinc-500">
              Drop a video file here or click <strong>Open Video</strong>
            </p>
            <p className="text-xs text-zinc-600">
              Supports MP4, MKV, MOV, AVI, WebM, and more
            </p>
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
            <ExportPanel inputPath={videoInfo!.path} segments={segments} />
          </div>
        </div>
      )}
    </div>
  );
}
