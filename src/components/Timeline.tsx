import { useRef, useCallback, type MouseEvent } from "react";
import type { Segment, Subtitle, AudioTrack } from "../types";
import { formatTime } from "../utils/format";

interface TimelineProps {
  duration: number;
  currentTime: number;
  segments: Segment[];
  subtitles: Subtitle[];
  audioTracks: AudioTrack[];
  onSeek: (time: number) => void;
  onSegmentUpdate: (id: string, start: number, end: number) => void;
  onAudioTrackUpdate: (id: string, start: number, end: number) => void;
}

export default function Timeline({
  duration,
  currentTime,
  segments,
  subtitles,
  audioTracks,
  onSeek,
  onSegmentUpdate,
  onAudioTrackUpdate,
}: TimelineProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const dragging = useRef<{
    id: string;
    type: "segment" | "audio";
    handle: "start" | "end" | "body";
    offsetRatio: number;
  } | null>(null);

  const getTimeFromX = useCallback(
    (clientX: number) => {
      const track = trackRef.current;
      if (!track || duration <= 0) return 0;
      const rect = track.getBoundingClientRect();
      const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      return ratio * duration;
    },
    [duration],
  );

  const handleTrackClick = useCallback(
    (e: MouseEvent) => {
      if (dragging.current) return;
      onSeek(getTimeFromX(e.clientX));
    },
    [getTimeFromX, onSeek],
  );

  const handleSegmentMouseDown = useCallback(
    (e: MouseEvent, segId: string, handle: "start" | "end" | "body") => {
      e.stopPropagation();
      const seg = segments.find((s) => s.id === segId);
      if (!seg) return;

      const time = getTimeFromX(e.clientX);
      const offsetRatio = handle === "body" ? (time - seg.start) / duration : 0;
      dragging.current = { id: segId, type: "segment", handle, offsetRatio };

      function onMouseMove(ev: globalThis.MouseEvent) {
        if (!dragging.current || !trackRef.current) return;
        const d = dragging.current;
        const t = getTimeFromX(ev.clientX);
        const s = segments.find((s) => s.id === d.id);
        if (!s) return;

        if (d.handle === "start") {
          onSegmentUpdate(d.id, Math.min(t, s.end - 0.1), s.end);
        } else if (d.handle === "end") {
          onSegmentUpdate(d.id, s.start, Math.max(t, s.start + 0.1));
        } else {
          const len = s.end - s.start;
          const newStart = Math.max(0, Math.min(duration - len, t - d.offsetRatio * duration));
          onSegmentUpdate(d.id, newStart, newStart + len);
        }
      }

      function onMouseUp() {
        dragging.current = null;
        window.removeEventListener("mousemove", onMouseMove);
        window.removeEventListener("mouseup", onMouseUp);
      }

      window.addEventListener("mousemove", onMouseMove);
      window.addEventListener("mouseup", onMouseUp);
    },
    [segments, duration, getTimeFromX, onSegmentUpdate],
  );

  const handleAudioMouseDown = useCallback(
    (e: MouseEvent, trackId: string, handle: "start" | "end" | "body") => {
      e.stopPropagation();
      const track = audioTracks.find((a) => a.id === trackId);
      if (!track) return;

      const time = getTimeFromX(e.clientX);
      const offsetRatio = handle === "body" ? (time - track.start) / duration : 0;
      dragging.current = { id: trackId, type: "audio", handle, offsetRatio };

      function onMouseMove(ev: globalThis.MouseEvent) {
        if (!dragging.current || !trackRef.current) return;
        const d = dragging.current;
        const t = getTimeFromX(ev.clientX);
        const a = audioTracks.find((a) => a.id === d.id);
        if (!a) return;

        if (d.handle === "start") {
          onAudioTrackUpdate(d.id, Math.min(t, a.end - 0.1), a.end);
        } else if (d.handle === "end") {
          onAudioTrackUpdate(d.id, a.start, Math.max(t, a.start + 0.1));
        } else {
          const len = a.end - a.start;
          const newStart = Math.max(0, Math.min(duration - len, t - d.offsetRatio * duration));
          onAudioTrackUpdate(d.id, newStart, newStart + len);
        }
      }

      function onMouseUp() {
        dragging.current = null;
        window.removeEventListener("mousemove", onMouseMove);
        window.removeEventListener("mouseup", onMouseUp);
      }

      window.addEventListener("mousemove", onMouseMove);
      window.addEventListener("mouseup", onMouseUp);
    },
    [audioTracks, duration, getTimeFromX, onAudioTrackUpdate],
  );

  if (duration <= 0) return null;

  const playheadPos = (currentTime / duration) * 100;

  // Generate time labels
  const labelCount = Math.min(10, Math.max(2, Math.floor(duration / 10)));
  const labels = Array.from({ length: labelCount + 1 }, (_, i) => {
    const t = (i / labelCount) * duration;
    return { time: t, pos: (t / duration) * 100 };
  });

  return (
    <div className="px-4 py-3 select-none">
      {/* Time labels */}
      <div className="relative h-5 text-[10px] text-zinc-500">
        {labels.map((l) => (
          <span
            key={l.time}
            className="absolute -translate-x-1/2"
            style={{ left: `${l.pos}%` }}
          >
            {formatTime(l.time)}
          </span>
        ))}
      </div>

      {/* Tracks container — playhead spans all lanes */}
      <div className="relative">
        {/* Video + Segment track */}
        <div
          ref={trackRef}
          className="relative h-10 bg-zinc-800 rounded-t cursor-pointer"
          onMouseDown={handleTrackClick}
        >
          {segments.map((seg) => {
            const left = (seg.start / duration) * 100;
            const width = ((seg.end - seg.start) / duration) * 100;
            return (
              <div
                key={seg.id}
                className="absolute top-0 h-full bg-emerald-600/50 border border-emerald-400/60 rounded-sm group"
                style={{ left: `${left}%`, width: `${width}%` }}
                onMouseDown={(e) => handleSegmentMouseDown(e, seg.id, "body")}
              >
                <div
                  className="absolute left-0 top-0 w-2 h-full cursor-col-resize bg-emerald-400/80 rounded-l-sm opacity-0 group-hover:opacity-100 transition-opacity"
                  onMouseDown={(e) => handleSegmentMouseDown(e, seg.id, "start")}
                />
                <div
                  className="absolute right-0 top-0 w-2 h-full cursor-col-resize bg-emerald-400/80 rounded-r-sm opacity-0 group-hover:opacity-100 transition-opacity"
                  onMouseDown={(e) => handleSegmentMouseDown(e, seg.id, "end")}
                />
              </div>
            );
          })}

          {subtitles.map((sub) => {
            const left = (sub.start / duration) * 100;
            const width = ((sub.end - sub.start) / duration) * 100;
            return (
              <div
                key={sub.id}
                className="absolute bottom-0 h-2 bg-cyan-600/60 border border-cyan-400/60 rounded-sm hover:bg-cyan-500/80 cursor-pointer"
                style={{ left: `${left}%`, width: `${width}%` }}
                onMouseDown={(e) => {
                  e.stopPropagation();
                  onSeek(sub.start);
                }}
                title={sub.text || "Subtitle"}
              />
            );
          })}
        </div>

        {/* Audio dub track */}
        {audioTracks.length > 0 && (
          <div
            className="relative h-6 bg-zinc-800/60 border-t border-zinc-700/50 rounded-b cursor-pointer"
            onMouseDown={handleTrackClick}
          >
            {audioTracks.map((track) => {
              const left = (track.start / duration) * 100;
              const width = ((track.end - track.start) / duration) * 100;
              return (
                <div
                  key={track.id}
                  className="absolute top-0 h-full bg-amber-600/40 border border-amber-400/50 rounded-sm group flex items-center justify-center overflow-hidden"
                  style={{ left: `${left}%`, width: `${width}%` }}
                  onMouseDown={(e) => handleAudioMouseDown(e, track.id, "body")}
                  title={track.fileName}
                >
                  <span className="text-[8px] text-amber-200/70 truncate px-1">{track.fileName}</span>
                  {track.radioEffect && (
                    <span className="text-[7px] text-amber-300/50 absolute top-0 right-0.5">AM</span>
                  )}
                  <div
                    className="absolute left-0 top-0 w-1.5 h-full cursor-col-resize bg-amber-400/70 rounded-l-sm opacity-0 group-hover:opacity-100 transition-opacity"
                    onMouseDown={(e) => handleAudioMouseDown(e, track.id, "start")}
                  />
                  <div
                    className="absolute right-0 top-0 w-1.5 h-full cursor-col-resize bg-amber-400/70 rounded-r-sm opacity-0 group-hover:opacity-100 transition-opacity"
                    onMouseDown={(e) => handleAudioMouseDown(e, track.id, "end")}
                  />
                </div>
              );
            })}
          </div>
        )}

        {/* Playhead — spans all lanes */}
        <div
          className="absolute top-0 w-0.5 h-full bg-red-500 pointer-events-none z-10"
          style={{ left: `${playheadPos}%` }}
        >
          <div className="absolute -top-1.5 left-1/2 -translate-x-1/2 w-2.5 h-2.5 bg-red-500 rounded-full" />
        </div>
      </div>
    </div>
  );
}
