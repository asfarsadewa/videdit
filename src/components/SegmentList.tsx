import type { Segment } from "../types";
import { formatTime } from "../utils/format";

interface SegmentListProps {
  segments: Segment[];
  onDelete: (id: string) => void;
  onSeek: (time: number) => void;
}

export default function SegmentList({ segments, onDelete, onSeek }: SegmentListProps) {
  if (segments.length === 0) {
    return (
      <div className="text-zinc-500 text-sm text-center py-4">
        No segments. Press <kbd className="px-1.5 py-0.5 bg-zinc-700 rounded text-xs">I</kbd> to mark in and{" "}
        <kbd className="px-1.5 py-0.5 bg-zinc-700 rounded text-xs">O</kbd> to mark out.
      </div>
    );
  }

  return (
    <div className="space-y-1 max-h-40 overflow-y-auto">
      {segments.map((seg, i) => (
        <div
          key={seg.id}
          className="flex items-center gap-2 px-2 py-1.5 bg-zinc-800 rounded text-sm hover:bg-zinc-750 group"
        >
          <span className="text-emerald-400 font-mono text-xs w-5">{i + 1}</span>
          <button
            className="text-zinc-300 hover:text-white font-mono text-xs"
            onClick={() => onSeek(seg.start)}
          >
            {formatTime(seg.start)}
          </button>
          <span className="text-zinc-600">→</span>
          <button
            className="text-zinc-300 hover:text-white font-mono text-xs"
            onClick={() => onSeek(seg.end)}
          >
            {formatTime(seg.end)}
          </button>
          <span className="text-zinc-500 text-xs ml-auto">
            {(seg.end - seg.start).toFixed(1)}s
          </span>
          <button
            className="text-zinc-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity text-xs px-1"
            onClick={() => onDelete(seg.id)}
            title="Remove segment"
          >
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}
