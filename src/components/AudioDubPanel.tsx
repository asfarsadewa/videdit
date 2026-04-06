import type { AudioTrack } from "../types";
import { formatTime } from "../utils/format";

interface AudioDubPanelProps {
  audioTracks: AudioTrack[];
  onUpdate: (id: string, updates: Partial<AudioTrack>) => void;
  onDelete: (id: string) => void;
  onSeek: (time: number) => void;
}

export default function AudioDubPanel({
  audioTracks,
  onUpdate,
  onDelete,
  onSeek,
}: AudioDubPanelProps) {
  if (audioTracks.length === 0) {
    return (
      <div className="text-zinc-500 text-sm text-center py-4">
        No audio tracks. Press{" "}
        <kbd className="px-1.5 py-0.5 bg-zinc-700 rounded text-xs">D</kbd> to dub audio at current time.
      </div>
    );
  }

  return (
    <div className="space-y-1.5 max-h-56 overflow-y-auto">
      {audioTracks.map((track, i) => (
        <div
          key={track.id}
          className="px-2 py-2 bg-zinc-800 rounded text-sm hover:bg-zinc-750 group space-y-1.5"
        >
          {/* Row 1: track info */}
          <div className="flex items-center gap-2">
            <span className="text-amber-400 font-mono text-xs w-5">{i + 1}</span>
            <span className="text-zinc-300 text-xs truncate max-w-[140px]" title={track.fileName}>
              {track.fileName}
            </span>
            <button
              className="text-zinc-400 hover:text-white font-mono text-xs"
              onClick={() => onSeek(track.start)}
            >
              {formatTime(track.start)}
            </button>
            <span className="text-zinc-600">→</span>
            <button
              className="text-zinc-400 hover:text-white font-mono text-xs"
              onClick={() => onSeek(track.end)}
            >
              {formatTime(track.end)}
            </button>
            <span className="text-zinc-500 text-xs">
              {(track.end - track.start).toFixed(1)}s
            </span>
            <button
              className="text-zinc-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity text-xs px-1 ml-auto"
              onClick={() => onDelete(track.id)}
              title="Remove audio track"
            >
              ✕
            </button>
          </div>

          {/* Row 2: volume + radio */}
          <div className="flex items-center gap-3 pl-7">
            <div className="flex items-center gap-1.5">
              <span className="text-[10px] text-zinc-500 w-6">Vol</span>
              <input
                type="range"
                min={0}
                max={150}
                value={Math.round(track.volume * 100)}
                onChange={(e) => onUpdate(track.id, { volume: Number(e.target.value) / 100 })}
                className="w-20 accent-amber-500"
              />
              <span className="text-[10px] text-zinc-500 w-8">{Math.round(track.volume * 100)}%</span>
            </div>

            <label className="flex items-center gap-1.5 cursor-pointer">
              <input
                type="checkbox"
                checked={track.radioEffect}
                onChange={(e) => onUpdate(track.id, { radioEffect: e.target.checked })}
                className="accent-amber-500"
              />
              <span className="text-[10px] text-zinc-400">AM/SW</span>
            </label>

            {track.radioEffect && (
              <div className="flex items-center gap-1.5">
                <span className="text-[10px] text-zinc-600">AM</span>
                <input
                  type="range"
                  min={0}
                  max={100}
                  value={track.radioIntensity}
                  onChange={(e) => onUpdate(track.id, { radioIntensity: Number(e.target.value) })}
                  className="w-16 accent-amber-500"
                />
                <span className="text-[10px] text-zinc-600">SW</span>
              </div>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
