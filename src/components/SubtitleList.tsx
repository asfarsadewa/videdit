import type { Subtitle } from "../types";
import { formatTime } from "../utils/format";

interface SubtitleListProps {
  subtitles: Subtitle[];
  onDelete: (id: string) => void;
  onSeek: (time: number) => void;
  onEdit: (subtitle: Subtitle) => void;
}

export default function SubtitleList({
  subtitles,
  onDelete,
  onSeek,
  onEdit,
}: SubtitleListProps) {
  if (subtitles.length === 0) {
    return (
      <div className="text-zinc-500 text-sm text-center py-4">
        No subtitles. Press{" "}
        <kbd className="px-1.5 py-0.5 bg-zinc-700 rounded text-xs">S</kbd> to add a subtitle at current time.
      </div>
    );
  }

  return (
    <div className="space-y-1 max-h-48 overflow-y-auto">
      {subtitles.map((sub, i) => (
        <div
          key={sub.id}
          className="flex items-center gap-2 px-2 py-2 bg-zinc-800 rounded text-sm hover:bg-zinc-700 group"
        >
          <span className="text-cyan-400 font-mono text-xs w-5">{i + 1}</span>
          <button
            className="text-zinc-300 hover:text-white font-mono text-xs"
            onClick={() => onSeek(sub.start)}
            title="Jump to start"
          >
            {formatTime(sub.start)}
          </button>
          <span className="text-zinc-600">→</span>
          <button
            className="text-zinc-300 hover:text-white font-mono text-xs"
            onClick={() => onSeek(sub.end)}
            title="Jump to end"
          >
            {formatTime(sub.end)}
          </button>
          <span className="text-zinc-500 text-xs ml-1">
            {((sub.end - sub.start)).toFixed(1)}s
          </span>
          <button
            className="flex-1 text-left text-zinc-400 hover:text-zinc-200 truncate text-xs px-2 py-1 bg-zinc-900/50 rounded"
            onClick={() => onEdit(sub)}
            title="Click to edit"
          >
            {sub.text || <span className="text-zinc-600 italic">Empty...</span>}
          </button>
          <button
            className="text-zinc-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity text-xs px-1"
            onClick={() => onDelete(sub.id)}
            title="Delete subtitle"
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}
