import { useState, useEffect, useCallback, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import type { Segment, ExportProgress, Subtitle, AudioTrack, EmbeddedSubtitleTrack } from "../types";
import { formatDuration } from "../utils/format";

interface ExportPanelProps {
  inputPath: string;
  duration: number;
  segments: Segment[];
  subtitles: Subtitle[];
  subtitleTracks: EmbeddedSubtitleTrack[];
  audioTracks: AudioTrack[];
  isFromRecording?: boolean;
}

type SubtitleOption = 'none' | 'authoredSrt' | 'authoredBurn' | 'embeddedBurn';

export default function ExportPanel({
  inputPath,
  duration,
  segments,
  subtitles,
  subtitleTracks,
  audioTracks,
  isFromRecording,
}: ExportPanelProps) {
  const [merge, setMerge] = useState(true);
  const [compress, setCompress] = useState(false);
  const [quality, setQuality] = useState(23);
  const [subtitleOption, setSubtitleOption] = useState<SubtitleOption>('none');
  const [selectedEmbeddedSubtitleIndex, setSelectedEmbeddedSubtitleIndex] = useState<number | null>(null);
  const [originalRadio, setOriginalRadio] = useState(false);
  const [originalRadioIntensity, setOriginalRadioIntensity] = useState(30);
  const [vhsEffect, setVhsEffect] = useState(false);
  const [vhsIntensity, setVhsIntensity] = useState(40);
  const [vhsScanlines, setVhsScanlines] = useState(true);
  const [vhsColorProfile, setVhsColorProfile] = useState<'faded' | 'preserved'>('faded');
  const [exporting, setExporting] = useState(false);
  const [progress, setProgress] = useState<ExportProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  const isWholeExport = segments.length === 0;
  const exportRange: 'segments' | 'whole' = isWholeExport ? 'whole' : 'segments';
  const effectiveMerge = !isWholeExport && segments.length > 1 && merge;
  const totalDuration = isWholeExport ? duration : segments.reduce((sum, s) => sum + (s.end - s.start), 0);
  const supportedEmbeddedTracks = useMemo(
    () => subtitleTracks.filter((track) => track.supportedForBurn),
    [subtitleTracks],
  );
  const segmentById = useMemo(() => new Map(segments.map((segment) => [segment.id, segment])), [segments]);
  const exportableAudioTracks = useMemo(
    () => audioTracks.filter((track) => segmentById.has(track.segmentId)),
    [audioTracks, segmentById],
  );
  const selectedEmbeddedTrack = supportedEmbeddedTracks.find(
    (track) => track.subtitleIndex === selectedEmbeddedSubtitleIndex,
  );

  useEffect(() => {
    const unlisten = listen<ExportProgress>("export-progress", (event) => {
      setProgress(event.payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (subtitles.length === 0 && (subtitleOption === 'authoredSrt' || subtitleOption === 'authoredBurn')) {
      setSubtitleOption('none');
    }
  }, [subtitles.length, subtitleOption]);

  useEffect(() => {
    if (supportedEmbeddedTracks.length === 0) {
      setSelectedEmbeddedSubtitleIndex(null);
      if (subtitleOption === 'embeddedBurn') {
        setSubtitleOption('none');
      }
      return;
    }

    const selectionStillExists = supportedEmbeddedTracks.some(
      (track) => track.subtitleIndex === selectedEmbeddedSubtitleIndex,
    );
    if (!selectionStillExists) {
      const defaultTrack = supportedEmbeddedTracks.find((track) => track.isDefault) ?? supportedEmbeddedTracks[0];
      setSelectedEmbeddedSubtitleIndex(defaultTrack.subtitleIndex);
    }
  }, [supportedEmbeddedTracks, selectedEmbeddedSubtitleIndex, subtitleOption]);

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
        subtitles: subtitleOption === 'authoredSrt' || subtitleOption === 'authoredBurn' ? subtitles.map((s) => ({
          start: s.start,
          end: s.end,
          text: s.text,
        })) : [],
        outputPath,
        exportRange,
        videoDuration: duration,
        merge: effectiveMerge,
        compress,
        quality,
        subtitleMode: subtitleOption,
        embeddedSubtitleIndex: subtitleOption === 'embeddedBurn' ? selectedEmbeddedSubtitleIndex : null,
        audioTracks: exportableAudioTracks.map((t) => {
          const segment = segmentById.get(t.segmentId)!;
          return {
            id: t.id,
            segmentId: t.segmentId,
            segmentStart: segment.start,
            segmentEnd: segment.end,
            filePath: t.filePath,
            audioSourceStart: t.audioSourceStart,
            audioSourceEnd: t.audioSourceEnd,
            volume: t.volume,
            radioEffect: t.radioEffect,
            radioIntensity: t.radioIntensity,
          };
        }),
        originalRadio,
        originalRadioIntensity,
        vhsEffect,
        vhsIntensity,
        vhsScanlines,
        vhsColorProfile,
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
  }, [
    inputPath,
    duration,
    segments,
    subtitles,
    exportRange,
    effectiveMerge,
    compress,
    quality,
    subtitleOption,
    selectedEmbeddedSubtitleIndex,
    exportableAudioTracks,
    segmentById,
    originalRadio,
    originalRadioIntensity,
    vhsEffect,
    vhsIntensity,
    vhsScanlines,
    vhsColorProfile,
    isFromRecording,
  ]);

  const isDisabled =
    exporting
    || (isWholeExport && duration <= 0)
    || (subtitleOption === 'embeddedBurn' && !selectedEmbeddedTrack);
  const hasSubtitleControls = subtitles.length > 0 || subtitleTracks.length > 0;
  const isBurningSubtitles = subtitleOption === 'authoredBurn' || subtitleOption === 'embeddedBurn';

  return (
    <div className="p-4 border-t border-zinc-800 space-y-3">
      <div className="flex items-center justify-between">
        <div className="text-sm text-zinc-400">
          {isWholeExport ? (
            <span className="text-zinc-200 font-medium">Whole video</span>
          ) : (
            <>
              <span className="text-zinc-200 font-medium">{segments.length}</span> segment
              {segments.length !== 1 ? "s" : ""}
            </>
          )}{" "}
          ·{" "}
          <span className="text-zinc-200 font-medium">{formatDuration(totalDuration)}</span> total
        </div>

        {!isWholeExport && (
          <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
            <input
              type="checkbox"
              checked={effectiveMerge}
              onChange={(e) => setMerge(e.target.checked)}
              className="accent-emerald-500"
              disabled={segments.length <= 1}
            />
            Merge into single file
          </label>
        )}
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

      {hasSubtitleControls && (
        <div className="space-y-2">
          <span className="text-sm text-zinc-400">Subtitles:</span>
          <div className="flex flex-wrap items-center gap-4">
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
            {subtitles.length > 0 && (
              <>
                <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
                  <input
                    type="radio"
                    name="subtitleOption"
                    checked={subtitleOption === 'authoredSrt'}
                    onChange={() => setSubtitleOption('authoredSrt')}
                    className="accent-cyan-500"
                  />
                  Export authored .srt
                </label>
                <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
                  <input
                    type="radio"
                    name="subtitleOption"
                    checked={subtitleOption === 'authoredBurn'}
                    onChange={() => setSubtitleOption('authoredBurn')}
                    className="accent-cyan-500"
                  />
                  Burn authored subtitles
                </label>
              </>
            )}
            {supportedEmbeddedTracks.length > 0 && (
              <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
                <input
                  type="radio"
                  name="subtitleOption"
                  checked={subtitleOption === 'embeddedBurn'}
                  onChange={() => setSubtitleOption('embeddedBurn')}
                  className="accent-cyan-500"
                />
                Burn embedded subtitle
              </label>
            )}
            {subtitleOption === 'embeddedBurn' && supportedEmbeddedTracks.length > 0 && (
              <select
                value={selectedEmbeddedSubtitleIndex ?? ""}
                onChange={(e) => setSelectedEmbeddedSubtitleIndex(Number(e.target.value))}
                className="min-w-52 max-w-full bg-zinc-800 border border-zinc-700 rounded px-2 py-1 text-sm text-zinc-200"
              >
                {supportedEmbeddedTracks.map((track) => (
                  <option key={track.subtitleIndex} value={track.subtitleIndex}>
                    {track.label}
                  </option>
                ))}
              </select>
            )}
          </div>
          {subtitleTracks.length > 0 && supportedEmbeddedTracks.length === 0 && (
            <p className="text-xs text-zinc-600">
              Embedded subtitles detected, but none are text-based tracks that can be burned in.
            </p>
          )}
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

      {/* VHS/CRT video effect */}
      <div className="flex items-center gap-4">
        <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
          <input
            type="checkbox"
            checked={vhsEffect}
            onChange={(e) => setVhsEffect(e.target.checked)}
            className="accent-fuchsia-500"
          />
          VHS/CRT effect
        </label>
        {vhsEffect && (
          <div className="flex flex-wrap items-center gap-4">
            <div className="flex items-center gap-2">
              <span className="text-xs text-zinc-600">Light</span>
              <input
                type="range"
                min={0}
                max={100}
                value={vhsIntensity}
                onChange={(e) => setVhsIntensity(Number(e.target.value))}
                className="w-28 accent-fuchsia-500"
              />
              <span className="text-xs text-zinc-600">Damaged</span>
            </div>
            <label className="flex items-center gap-1.5 text-xs text-zinc-400 cursor-pointer">
              <input
                type="checkbox"
                checked={vhsScanlines}
                onChange={(e) => setVhsScanlines(e.target.checked)}
                className="accent-fuchsia-500"
              />
              Scanlines
            </label>
            {!vhsScanlines && (
              <div className="flex items-center gap-2">
                <span className="text-xs text-zinc-600">Colour</span>
                <label className="flex items-center gap-1.5 text-xs text-zinc-400 cursor-pointer">
                  <input
                    type="radio"
                    name="vhsColorProfile"
                    checked={vhsColorProfile === 'faded'}
                    onChange={() => setVhsColorProfile('faded')}
                    className="accent-fuchsia-500"
                  />
                  Faded
                </label>
                <label className="flex items-center gap-1.5 text-xs text-zinc-400 cursor-pointer">
                  <input
                    type="radio"
                    name="vhsColorProfile"
                    checked={vhsColorProfile === 'preserved'}
                    onChange={() => setVhsColorProfile('preserved')}
                    className="accent-fuchsia-500"
                  />
                  Preserved
                </label>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Original audio radio effect */}
      <div className="flex items-center gap-4">
        <label className="flex items-center gap-2 text-sm text-zinc-400 cursor-pointer">
          <input
            type="checkbox"
            checked={originalRadio}
            onChange={(e) => setOriginalRadio(e.target.checked)}
            className="accent-amber-500"
          />
          AM/SW radio effect on original audio
        </label>
        {originalRadio && (
          <div className="flex items-center gap-2">
            <span className="text-xs text-zinc-600">AM</span>
            <input
              type="range"
              min={0}
              max={100}
              value={originalRadioIntensity}
              onChange={(e) => setOriginalRadioIntensity(Number(e.target.value))}
              className="w-24 accent-amber-500"
            />
            <span className="text-xs text-zinc-600">SW</span>
          </div>
        )}
      </div>

      {exportableAudioTracks.length > 0 && (
        <p className="text-xs text-amber-500/70">
          {exportableAudioTracks.length} audio track{exportableAudioTracks.length !== 1 ? "s" : ""} will be mixed into the export
          {exportableAudioTracks.some((t) => t.radioEffect) && " (includes AM/SW processing)"}
        </p>
      )}

      {/* Keyframe notice */}
      <p className="text-xs text-zinc-600">
        {vhsEffect
          ? vhsScanlines
            ? "VHS/CRT export re-encodes video into stretched 4:3 with analog blur, bleed, noise, and scanlines."
            : vhsColorProfile === 'preserved'
              ? "VHS/CRT export re-encodes video into stretched 4:3 with soft tube blur, bleed, preserved colour, and tracking distortion."
              : "VHS/CRT export re-encodes video into stretched 4:3 with soft tube blur, bleed, wash, and tracking distortion."
          : compress
          ? "Re-encoded export — frame-accurate cuts."
          : isBurningSubtitles
            ? "Subtitle burn-in re-encodes video."
          : exportableAudioTracks.length > 0 || originalRadio
            ? "Video copied losslessly, audio re-encoded for mixing."
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
