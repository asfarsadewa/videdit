import type { Segment, AudioTrack } from "../types";
import { formatTime } from "../utils/format";

interface SegmentListProps {
  segments: Segment[];
  audioTracks: AudioTrack[];
  onDelete: (id: string) => void;
  onSeek: (time: number) => void;
  onAddAudio: (segmentId: string) => void;
  onDeleteAudio: (segmentId: string) => void;
}

export default function SegmentList({ 
  segments, 
  audioTracks,
  onDelete, 
  onSeek,
  onAddAudio,
  onDeleteAudio,
}: SegmentListProps) {
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
      {segments.map((seg, i) => {
        const audioTrack = audioTracks.find(t => t.segmentId === seg.id);
        
        return (
          <div
            key={seg.id}
            className="flex flex-col gap-1.5 px-2 py-1.5 bg-zinc-800 rounded text-sm hover:bg-zinc-750 group"
          >
            {/* Row 1: segment info */}
            <div className="flex items-center gap-2">
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
            
            {/* Row 2: audio assignment */}
            <div className="flex items-center gap-2 pl-7">
              {audioTrack ? (
                <>
                  <span className="text-[10px] text-amber-400 truncate max-w-[120px]" title={audioTrack.fileName}>
                    ♪ {audioTrack.fileName}
                  </span>
                  <button
                    className="text-[10px] text-zinc-500 hover:text-red-400 transition-colors"
                    onClick={() => onDeleteAudio(seg.id)}
                    title="Remove audio"
                  >
                    ✕
                  </button>
                </>
              ) : (
                <button
                  className="text-[10px] text-zinc-600 hover:text-amber-400 transition-colors"
                  onClick={() => onAddAudio(seg.id)}
                  title="Add audio to this segment"
                >
                  + Add Audio
                </button>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
