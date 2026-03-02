import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import type { Segment, ExportProgress, Subtitle } from "../types";
import { formatDuration } from "../utils/format";

interface ExportPanelProps {
  inputPath: string;
  segments: Segment[];
  subtitles: Subtitle[];
  isFromRecording?: boolean;
}

export default function ExportPanel({ inputPath, segments, subtitles, isFromRecording }: ExportPanelProps) {
  const [merge, setMerge] = useState(true);
  const [compress, setCompress] = useState(false);
  const [quality, setQuality] = useState(23);
  const [subtitleOption, setSubtitleOption] = useState<'none' | 'srt' | 'burn'>('none');
  const [exporting, setExporting] = useState(false);
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const unlisten = listen<ExportProgress>("export-progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (subtitles.length === 0) {
      setSubtitleOption('none');
    }
  }, [subtitles.length]);

  const totalDuration = segments.reduce((sum, s) => sum + (s.end - s.start), 0);

  const handleExport = useCallback(async () => {
    setError(null);
    try {
      const outputPath = await save({
        defaultPath: "output.mp4",
        filters: [{ name: "Video", extensions: ["mp4", "mkv", "mov", "avi"] }],
      });
      if (!outputPath) return;

      setExporting(true);
      setProgress(null);

      await invoke("export_video", {
        inputPath,
        segments: segments.map((s) => ({
          id: s.id,
          start: s.start,
          end: s.end,
        })),
        subtitles: subtitleOption !== 'none' ? subtitles.map((s) => ({
          start: s.start,
          end: s.end,
          text: s.text,
        })) : [],
        outputPath,
        merge,
        compress,
        quality,
        burnSubtitles: subtitleOption === 'burn',
      });

      // Clean up temp recording file after successful export
      if (isFromRecording) {
        await invoke("cleanup_recording_temp").catch(() => {});
      }

      setExporting(false);
    } catch (e) {
      setError(String(e));
      setExporting(false);
    }
  }, [inputPath, segments, subtitles, merge, compress, quality, subtitleOption, isFromRecording]);

  const isDisabled =
    (segments.length === 0 && !(subtitles.length > 0 && subtitleOption === 'srt'))
    || exporting;

  return (
    <div className="p-4 border-t border-zinc-800 space-y-3">
      <div className="flex items-center justify-between">
        <div className="text-sm text-zinc-400">
          <span className="text-zinc-200 font-medium">{segments.length}</span> segment
          {segments.length !== 1 ? "s" : ""} ·{" "}
          <span className="text-zinc-200 font-medium">{formatDuration(totalDuration)}</span> total
        </div>

        <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
          <input
            type="checkbox"
            checked={merge}
            onChange={(e) => setMerge(e.target.checked)}
            className="accent-emerald-500"
            disabled={segments.length <= 1}
          />
          Merge into single file
        </label>
      </div>

      <div className="flex items-center gap-4">
        <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
          <input
            type="checkbox"
            checked={compress}
            onChange={(e) => setCompress(e.target.checked)}
            className="accent-emerald-500"
          />
          Compress (smaller file)
        </label>
      </div>

      {subtitles.length > 0 && (
        <div className="space-y-2">
          <span className="text-sm text-zinc-400">Subtitles:</span>
          <div className="flex items-center gap-4">
            <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
              <input
                type="radio"
                name="subtitleOption"
                checked={subtitleOption === 'none'}
                onChange={() => setSubtitleOption('none')}
                className="accent-zinc-500"
              />
              Don't export
            </label>
            <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
              <input
                type="radio"
                name="subtitleOption"
                checked={subtitleOption === 'srt'}
                onChange={() => setSubtitleOption('srt')}
                className="accent-cyan-500"
              />
              Export as .srt file
            </label>
            <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
              <input
                type="radio"
                name="subtitleOption"
                checked={subtitleOption === 'burn'}
                onChange={() => setSubtitleOption('burn')}
                className="accent-cyan-500"
              />
              Burn into video (re-encodes)
            </label>
          </div>
        </div>
      )}

      {compress && (
        <div className="space-y-1">
          <div className="flex items-center gap-3">
            <span className="text-xs text-zinc-500 shrink-0">Higher quality</span>
            <input
              type="range"
              min={18}
              max={28}
              value={quality}
              onChange={(e) => setQuality(Number(e.target.value))}
              className="flex-1 accent-emerald-500"
            />
            <span className="text-xs text-zinc-500 shrink-0">Smaller file</span>
          </div>
          <p className="text-xs text-zinc-600 text-center">CRF {quality}</p>
        </div>
      )}

      {/* Keyframe notice */}
      <p className="text-xs text-zinc-600">
        {compress
          ? "Re-encoded export — frame-accurate cuts."
          : "Lossless export — cuts occur at nearest keyframe (±1-2s accuracy)."}
      </p>

      {/* Progress bar */}
      {exporting && progress && (
        <div className="space-y-1">
          <div className="w-full bg-zinc-800 rounded-full h-2">
            <div
              className="bg-emerald-500 h-2 rounded-full transition-all duration-300"
              style={{ width: `${progress.percent}%` }}
            />
          </div>
          <p className="text-xs text-zinc-500">{progress.message}</p>
        </div>
      )}

      {/* Done message */}
      {progress?.phase === "done" && !exporting && (
        <p className="text-sm text-emerald-400">{progress.message}</p>
      )}

      {error && <p className="text-sm text-red-400">{error}</p>}

      <button
        onClick={handleExport}
        disabled={isDisabled}
        className="w-full py-2 rounded font-medium text-sm transition-colors
          bg-emerald-600 hover:bg-emerald-500 text-white
          disabled:bg-zinc-700 disabled:text-zinc-500 disabled:cursor-not-allowed"
      >
        {exporting ? "Exporting..." : "Export"}
      </button>
    </div>
  );
}
