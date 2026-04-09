import type { AudioTrack, Segment } from "../types";
import { formatTime } from "../utils/format";

interface AudioDubPanelProps {
  audioTracks: AudioTrack[];
  segments: Segment[];
  onUpdate: (id: string, updates: Partial<AudioTrack>) => void;
  onDelete: (id: string) => void;
  onSeek: (time: number) => void;
}

export default function AudioDubPanel({
  audioTracks,
  segments,
  onUpdate,
  onDelete,
  onSeek,
}: AudioDubPanelProps) {
  if (audioTracks.length === 0) {
    return (
      <div className="text-zinc-500 text-sm text-center py-4">
        No audio tracks. Add audio to segments from the segment list.
      </div>
    );
  }

  return (
    <div className="space-y-1.5 max-h-56 overflow-y-auto">
      {audioTracks.map((track) => {
        const segment = segments.find(s => s.id === track.segmentId);
        if (!segment) return null;
        
        const audioDuration = track.audioSourceEnd - track.audioSourceStart;
        const segmentDuration = segment.end - segment.start;
        
        return (
          <div
            key={track.id}
            className="px-2 py-2 bg-zinc-800 rounded text-sm hover:bg-zinc-750 group space-y-1.5"
          >
            {/* Row 1: segment info */}
            <div className="flex items-center gap-2">
              <span className="text-emerald-400 font-mono text-xs">
                S{segments.indexOf(segment) + 1}
              </span>
              <span className="text-zinc-300 text-xs truncate max-w-[120px]" title={track.fileName}>
                ♪ {track.fileName}
              </span>
              <button
                className="text-zinc-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity text-xs px-1 ml-auto"
                onClick={() => onDelete(track.id)}
                title="Remove audio track"
              >
                ✕
              </button>
            </div>

            {/* Row 2: audio trim controls */}
            <div className="flex items-center gap-2 pl-4">
              <span className="text-[10px] text-zinc-600 w-6">Trim</span>
              <button
                className="text-zinc-400 hover:text-white font-mono text-[10px]"
                onClick={() => onSeek(segment.start)}
              >
                {formatTime(track.audioSourceStart)}
              </button>
              <span className="text-zinc-700">→</span>
              <button
                className="text-zinc-400 hover:text-white font-mono text-[10px]"
                onClick={() => onSeek(segment.start + (track.audioSourceEnd - track.audioSourceStart))}
              >
                {formatTime(track.audioSourceEnd)}
              </button>
              <span className="text-zinc-600 text-[10px]">
                ({audioDuration.toFixed(1)}s / {segmentDuration.toFixed(1)}s)
              </span>
            </div>
            
            {/* Row 3: trim sliders */}
            <div className="flex items-center gap-2 pl-4">
              <div className="flex-1">
                <div 
                  className="relative h-2 bg-zinc-900 rounded cursor-col-resize"
                  onMouseDown={(e) => {
                    const rect = e.currentTarget.getBoundingClientRect();
                    const startX = e.clientX;
                    const startValue = track.audioSourceStart;
                    const endValue = track.audioSourceEnd;
                    const handleWidth = 6; // px width of handles for hit detection
                    
                    // Determine which handle was clicked (or none)
                    const startHandleX = (startValue / track.duration) * rect.width;
                    const endHandleX = (endValue / track.duration) * rect.width;
                    const clickX = startX - rect.left;
                    
                    const nearStartHandle = Math.abs(clickX - startHandleX) < handleWidth;
                    const nearEndHandle = Math.abs(clickX - endHandleX) < handleWidth;
                    
                    const activeHandle = nearStartHandle ? 'start' : nearEndHandle ? 'end' : null;
                    
                    if (!activeHandle) return;
                    
                    e.stopPropagation();
                    
                    const handleMouseMove = (ev: MouseEvent) => {
                      const newX = ev.clientX - rect.left;
                      const ratio = Math.max(0, Math.min(1, newX / rect.width));
                      const newValue = ratio * track.duration;
                      
                      if (activeHandle === 'start') {
                        const clampedValue = Math.max(0, Math.min(newValue, endValue - 0.1));
                        onUpdate(track.id, { audioSourceStart: clampedValue });
                      } else if (activeHandle === 'end') {
                        const clampedValue = Math.min(track.duration, Math.max(newValue, startValue + 0.1));
                        onUpdate(track.id, { audioSourceEnd: clampedValue });
                      }
                    };
                    
                    const handleMouseUp = () => {
                      window.removeEventListener('mousemove', handleMouseMove);
                      window.removeEventListener('mouseup', handleMouseUp);
                    };
                    
                    window.addEventListener('mousemove', handleMouseMove);
                    window.addEventListener('mouseup', handleMouseUp);
                  }}
                >
                  {/* Selected range highlight */}
                  <div 
                    className="absolute h-full bg-amber-500/30 rounded"
                    style={{
                      left: `${(track.audioSourceStart / track.duration) * 100}%`,
                      right: `${100 - (track.audioSourceEnd / track.duration) * 100}%`
                    }}
                  />
                  {/* Start handle */}
                  <div 
                    className="absolute top-1/2 -translate-y-1/2 w-1.5 h-4 bg-amber-500 rounded-sm hover:bg-amber-400 transition-colors"
                    style={{ left: `calc(${(track.audioSourceStart / track.duration) * 100}% - 3px)` }}
                  />
                  {/* End handle */}
                  <div 
                    className="absolute top-1/2 -translate-y-1/2 w-1.5 h-4 bg-amber-500 rounded-sm hover:bg-amber-400 transition-colors"
                    style={{ left: `calc(${(track.audioSourceEnd / track.duration) * 100}% - 3px)` }}
                  />
                </div>
              </div>
            </div>

            {/* Row 4: volume + radio */}
            <div className="flex items-center gap-3 pl-4">
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
        );
      })}
    </div>
  );
}
